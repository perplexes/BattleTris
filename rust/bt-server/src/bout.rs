//! Server-authoritative online match (a "bout").
//!
//! This is the server side of the client-server migration. The server owns the
//! authoritative simulation for a matched pair — a [`bt_core::Versus`] holding
//! both boards — and is the single source of truth. Clients send INPUTS; the
//! server applies them to the authoritative match, ticks the deterministic
//! engine on its own clock, and ships authoritative [`Snapshot`]s back. Clients
//! predict locally and reconcile against those snapshots.
//!
//! Two properties fall out for free, which is exactly why the user chose this
//! over the (faithful) P2P relay:
//!   * **Anti-cheat** — a client can only send legal *inputs*
//!     ([`is_legal_client_input`]); it can't inject weapons/funds/board state.
//!     The server resolves every cross-player effect (Mirror, Swap, taxes).
//!   * **A totally-ordered event log** — the server sees every input in order,
//!     so an online match can be recorded as a [`bt_replay::Replay`] (closing
//!     the long-standing "online games aren't replayable" gap, D5).
//!
//! Transport wiring (the `/ws` handoff from matchmaking, snapshot broadcast
//! cadence, client prediction/reconciliation) layers on top of this core.
//!
use bt_core::versus::Side;
use bt_core::weapons::{weapon_table, WeaponToken};
use bt_core::Versus;
use bt_replay::{Input, VersusFrame, VersusReplay, REPLAY_VERSION};
use serde::Serialize;

/// The authoritative tick interval (ms). Matches the engine's fixed timestep
/// (`bt_wasm::FIXED_DT_MS`), so one real interval = one deterministic step.
pub const TICK_MS: i32 = 16;

/// Map a [`Side`] to a 0/1 index (A = 0, B = 1) for per-side arrays.
fn side_idx(side: Side) -> usize {
    match side {
        Side::A => 0,
        Side::B => 1,
    }
}

/// Whether an [`Input`] is a legal action a CLIENT may submit.
///
/// The relay-internal variants (`ReceiveWeapon`, `ReceiveOpScore`, `AddFunds`,
/// `AiDrop`) must NEVER be accepted from a client — those are how the *server*
/// applies cross-player effects, and letting a client send them would let it
/// grant itself weapons or funds. Rejecting them is the heart of the
/// authoritative model's anti-cheat.
///
/// `SetPaused` is also rejected: a client-controlled pause would freeze only
/// that side's authoritative board while the opponent keeps ticking (a grief /
/// stall exploit). A faithful *synchronized* match-pause (the original's
/// `BT_PAUSE`) is server-owned and a later feature, not a client input.
pub fn is_legal_client_input(input: &Input) -> bool {
    matches!(
        input,
        Input::MoveLeft
            | Input::MoveRight
            | Input::Rotate
            | Input::SoftDrop
            | Input::BeginDrop
            | Input::LaunchWeapon(_)
            | Input::BuyWeapon(_)
            | Input::SellWeapon(_)
            | Input::LeaveBazaar
    )
}

/// Inputs allowed while a side is in the weapons bazaar. The match is frozen for
/// the synchronized bazaar (neither board ticks), so only shopping actions are
/// permitted — movement/rotate/drop/launch are inert until the player leaves.
/// `Game` already blocks drops in the bazaar, but not movement/rotate/launch, so
/// the server gates them here (otherwise a direct client could nudge its frozen
/// piece or fire weapons mid-shop).
fn is_bazaar_input(input: &Input) -> bool {
    matches!(
        input,
        Input::BuyWeapon(_) | Input::SellWeapon(_) | Input::LeaveBazaar
    )
}

/// The slim per-frame view of a player's OWN status that local prediction can't
/// derive between keyframes: `funds` (changed by an opponent's Mondale/Keating)
/// and the bazaar barrier (entry depends on COMBINED lines, so a client can't
/// foresee it). The board/score/lines come from local prediction + the periodic
/// keyframe; these three keep the HUD and the bazaar overlay prompt.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SelfStatus {
    pub funds: i64,
    pub in_bazaar: bool,
    pub lines_til_bazaar: i32,
}

/// What a player is allowed to see about their OPPONENT by default — only
/// score/lines for the opponent panel. NOT the board and NOT funds: in the
/// original those are revealed only by a spy (Ames "displays your opponent's
/// screen and your opponent's funds"). The spy will be a server-authorized
/// extension to this view — which is what makes the old unauthenticated
/// spyRequest (D4) moot.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct OppView {
    pub score: i64,
    pub lines: i64,
    pub game_over: bool,
}

/// One authoritative frame sent to a client. Per-frame frames are LIGHT — the
/// client renders its own board from local prediction; `ack` (the last input
/// seq the server applied from this client) lets it discard acked inputs. The
/// full authoritative state rides `keyframe` (the byte form of `Game::snapshot`)
/// on a throttle: the client restores it and re-applies its unacked inputs.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub tick: u64,
    pub ack: u64,
    /// 0 = ongoing, 1 = this client won, 2 = this client lost.
    pub result: i32,
    /// Prompt own-state the client can't predict between keyframes.
    pub you: SelfStatus,
    pub opp: OppView,
    /// Whether a spy of THIS client is currently active (drives showing/hiding
    /// the opponent-board panel), sent every frame.
    pub spying: bool,
    /// The opponent's board as revealed by this client's active spy — already
    /// DEGRADED to the spy's accuracy server-side (so a client can't read cells
    /// the spy didn't earn). Rides the throttled keyframe frames, like `keyframe`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spy_board: Option<Vec<i32>>,
    /// Full-state reconciliation keyframe (bytes, op_funds redacted), present
    /// only on the throttled keyframe frames; omitted from the JSON otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyframe: Option<Vec<u8>>,
}

/// % of a spy's revealed cells the server HIDES — `1 - report_prob` from
/// BTRecon.C: Ames shows 50%, Ace 85%, Condor (satellite) is perfect.
fn spy_hide_pct(token: WeaponToken) -> u32 {
    match token {
        WeaponToken::Ames => 50,
        WeaponToken::Ace => 15,
        _ => 0, // Condor
    }
}

/// Degrade a render-id grid (`Game::render_ids`, empty = -2) to a spy's accuracy
/// by HIDING a deterministic ~hide% of non-empty cells (server-side, so a
/// modified client can't read the cells the spy didn't reveal — this is what
/// makes the old unauthenticated spy request, D4, moot). Stable per position.
fn degrade_board(mut grid: Vec<i32>, token: WeaponToken) -> Vec<i32> {
    let hide = spy_hide_pct(token);
    if hide == 0 {
        return grid;
    }
    for (i, cell) in grid.iter_mut().enumerate() {
        if *cell != -2 {
            let h = (i.wrapping_mul(2_654_435_761) >> 8) % 100;
            if (h as u32) < hide {
                *cell = -2; // hide -> empty
            }
        }
    }
    grid
}

/// A server-hosted authoritative match between two clients.
pub struct Bout {
    versus: Versus,
    tick: u64,
    /// The two seeds, kept so the match can be exported as a deterministic
    /// [`VersusReplay`] (the totally-ordered input stream + seeds, closing D5).
    seed_a: u64,
    seed_b: u64,
    /// Every applied client input, stamped with the tick — the replay's frames.
    frames: Vec<VersusFrame>,
    /// Last applied input sequence number per side (A = [0], B = [1]).
    ack: [u64; 2],
    /// Active spy per side: `(token, lines remaining)`. A spy reveals the
    /// opponent's board to this side until the OPPONENT clears `duration` lines
    /// (BTRecon's `spy_on_`). A = [0], B = [1].
    spy: [Option<(WeaponToken, i32)>; 2],
    /// The opponent's line count last seen per side, to measure the spy's
    /// line-clear decrement.
    opp_lines_seen: [i64; 2],
}

impl Bout {
    /// Start a bout. The two sides get distinct seeds (so their piece streams
    /// differ); the server picks them and tells each client its seed at handoff.
    pub fn new(seed_a: u64, seed_b: u64) -> Bout {
        Bout {
            versus: Versus::new(seed_a, seed_b),
            tick: 0,
            seed_a,
            seed_b,
            frames: Vec::new(),
            ack: [0, 0],
            spy: [None, None],
            opp_lines_seen: [0, 0],
        }
    }

    /// Export the match so far as a deterministic, replayable [`VersusReplay`]
    /// (the seeds + the totally-ordered client-input stream). Replaying re-runs a
    /// `Versus`, so the whole relay reproduces — no need to record its effects.
    ///
    /// `tick_count` is `self.tick`, the number of ticks the bout actually ran.
    /// The server's match loop applies a batch of inputs and then ALWAYS ticks
    /// (`apply_input … ; bout.tick()`), so every recorded frame is stamped at a
    /// tick strictly less than `self.tick` and a `VersusReplayPlayer` (which
    /// applies a frame on its stamped tick, then ticks, stopping at `executed >=
    /// tick_count`) replays all of them. (Stamping a frame AT `self.tick` would
    /// require an input with no following tick, which the loop never produces.)
    pub fn to_replay(&self, dt_ms: i32, engine_sha: &str) -> VersusReplay {
        VersusReplay {
            version: REPLAY_VERSION,
            seed_a: self.seed_a as u32,
            seed_b: self.seed_b as u32,
            dt_ms,
            engine_sha: engine_sha.to_string(),
            tick_count: self.tick as u32,
            frames: self.frames.clone(),
            title: None,
        }
    }

    /// Apply a client's input to its side of the authoritative match. Returns
    /// false (and does nothing) if the input is rejected:
    ///   * not a legal client action ([`is_legal_client_input`] — anti-cheat),
    ///   * stale / replayed (`seq` not strictly greater than the last applied —
    ///     so a malicious/buggy client can't re-apply old inputs or rewind `ack`),
    ///   * a non-shopping action while this side is in the bazaar (the match is
    ///     frozen — only [`is_bazaar_input`] is allowed).
    /// On success `seq` becomes this side's `ack` for client reconciliation.
    pub fn apply_input(&mut self, side: Side, input: &Input, seq: u64) -> bool {
        let idx = side_idx(side);
        if !is_legal_client_input(input) || seq <= self.ack[idx] {
            return false;
        }
        let g = self.versus.game_mut(side);
        if g.is_in_bazaar() && !is_bazaar_input(input) {
            return false;
        }
        input.apply_to_game(g);
        self.ack[idx] = seq;
        // Record it (stamped with the current tick — inputs for tick N are drained
        // before the Nth tick advances, so a replay applies them at the same tick).
        self.frames.push(VersusFrame { tick: self.tick as u32, side: idx as u8, input: input.clone() });
        true
    }

    /// Advance the authoritative simulation by `dt_ms` (the server's clock), and
    /// run the spy bookkeeping (BTRecon): a launched spy reveals the opponent for
    /// `duration` of the OPPONENT's line-clears; relaunch accumulates + switches
    /// the accuracy to the newest spy.
    pub fn tick(&mut self, dt_ms: i32) {
        self.versus.tick(dt_ms);
        self.tick += 1;
        for (i, side) in [Side::A, Side::B].into_iter().enumerate() {
            let opp_lines = self.versus.game(side.other()).score().lines;
            // 1. Charge the ACTIVE spy first, for the opponent's clears since last
            //    seen (before any relaunch resets the baseline).
            if let Some((tok, rem)) = self.spy[i] {
                let delta = (opp_lines - self.opp_lines_seen[i]).max(0) as i32;
                let left = rem - delta;
                self.spy[i] = if left > 0 { Some((tok, left)) } else { None };
            }
            self.opp_lines_seen[i] = opp_lines;
            // 2. Then apply any new launches (each accumulates the budget; the
            //    newest token sets the accuracy), counting from `opp_lines`.
            for tok in self.versus.take_spy_launches(side) {
                let add = weapon_table()[tok.index()].duration as i32;
                let cur = self.spy[i].map_or(0, |(_, r)| r);
                self.spy[i] = Some((tok, cur + add));
            }
        }
    }

    /// Take (and clear) the "a client can't have predicted this" flag from the
    /// last tick (a delivered weapon / funds tax / bazaar entry) — the server
    /// pushes a prompt keyframe when it's set.
    pub fn take_dirty(&mut self) -> bool {
        self.versus.take_dirty()
    }

    /// 0 = ongoing, 1 = A won, 2 = B won.
    pub fn result(&self) -> i32 {
        self.versus.result()
    }

    pub fn is_over(&self) -> bool {
        self.versus.is_over()
    }

    /// This side's cleared-line count — for settling the match outcome (TrueSkill).
    pub fn lines(&self, side: Side) -> u32 {
        self.versus.game(side).score().lines.max(0) as u32
    }

    /// This side's final score — for the per-player `high_score` stat at settlement.
    pub fn score(&self, side: Side) -> i64 {
        self.versus.game(side).score().score
    }

    /// This side's final funds — for the per-player `high_funds` stat at settlement.
    pub fn funds(&self, side: Side) -> i64 {
        self.versus.game(side).score().funds
    }

    /// How many ticks the match has run — the unit for the per-player time stats
    /// (`longest_game`, `fastest_kill`, `quickest_death`).
    pub fn tick_count(&self) -> u64 {
        self.tick
    }

    /// The authoritative snapshot for `side` as a ready-to-send ws message: the
    /// [`Snapshot`] fields plus a `{"type":"snapshot"}` tag.
    pub fn snapshot_message(&self, side: Side, include_keyframe: bool) -> String {
        let mut v = serde_json::to_value(self.snapshot_for(side, include_keyframe))
            .unwrap_or(serde_json::Value::Null);
        if let Some(obj) = v.as_object_mut() {
            obj.insert("type".into(), serde_json::Value::String("snapshot".into()));
        }
        v.to_string()
    }

    /// Build the authoritative snapshot for `side`. `include_keyframe` attaches
    /// the full-state keyframe (the caller throttles it); otherwise the frame is
    /// just tick/ack/result/opp and the client renders from its local prediction.
    pub fn snapshot_for(&self, side: Side, include_keyframe: bool) -> Snapshot {
        let me = self.versus.game(side);
        let them = self.versus.game(side.other());
        let spy = self.spy[side_idx(side)];
        // The (degraded) opponent board rides the keyframe frames while spying.
        let spy_board = match (spy, include_keyframe) {
            (Some((tok, _)), true) => Some(degrade_board(them.render_ids(), tok)),
            _ => None,
        };

        // The match result is latched as A/B; translate to this client's POV
        // (1 = you won, 2 = you lost).
        let result = match (self.versus.result(), side) {
            (0, _) => 0,
            (1, Side::A) | (2, Side::B) => 1, // this side won
            _ => 2,                            // this side lost
        };

        Snapshot {
            tick: self.tick,
            ack: self.ack[side_idx(side)],
            result,
            you: SelfStatus {
                funds: me.score().funds,
                in_bazaar: me.is_in_bazaar(),
                lines_til_bazaar: me.lines_til_bazaar(),
            },
            opp: OppView {
                score: them.score().score,
                lines: them.score().lines,
                game_over: them.is_game_over(),
            },
            spying: spy.is_some(),
            spy_board,
            // op_funds-redacted: a client must not learn the opponent's funds.
            keyframe: include_keyframe.then(|| me.client_keyframe_bytes()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // -----------------------------------------------------------------------
    // Proptest strategies
    // -----------------------------------------------------------------------

    /// Strategy: one of the five relay-internal inputs the server must NEVER
    /// accept from a client.
    fn relay_internal_input() -> impl Strategy<Value = Input> {
        prop_oneof![
            any::<i32>().prop_map(Input::ReceiveWeapon),
            (any::<i64>(), any::<i64>(), any::<i64>())
                .prop_map(|(score, lines, funds)| Input::ReceiveOpScore { score, lines, funds }),
            any::<i64>().prop_map(Input::AddFunds),
            Just(Input::AiDrop),
            any::<bool>().prop_map(Input::SetPaused),
        ]
    }

    /// Strategy: EVERY legal client input variant (the exact set
    /// `is_legal_client_input` admits). Used to prove each one is ACCEPTED — so
    /// dropping any single arm from the allow-list (e.g. removing `Rotate`) is
    /// caught.
    fn legal_client_input() -> impl Strategy<Value = Input> {
        prop_oneof![
            Just(Input::MoveLeft),
            Just(Input::MoveRight),
            Just(Input::Rotate),
            Just(Input::SoftDrop),
            Just(Input::BeginDrop),
            (0u32..10u32).prop_map(Input::LaunchWeapon),
            (0i32..34i32).prop_map(Input::BuyWeapon),
            (0i32..34i32).prop_map(Input::SellWeapon),
            Just(Input::LeaveBazaar),
        ]
    }

    /// Strategy: a non-shopping legal client input — legal in general, but illegal
    /// WHILE in the bazaar (the match is frozen; only buy/sell/leave shop actions
    /// are allowed there). The bazaar gate must reject every one of these.
    fn non_bazaar_legal_input() -> impl Strategy<Value = Input> {
        prop_oneof![
            Just(Input::MoveLeft),
            Just(Input::MoveRight),
            Just(Input::Rotate),
            Just(Input::SoftDrop),
            Just(Input::BeginDrop),
            (0u32..10u32).prop_map(Input::LaunchWeapon),
        ]
    }

    /// Force `side` into the bazaar deterministically by crossing a 20-line bazaar
    /// boundary via the score mirror (combined lines 19 -> lines_til_baz = 1, then
    /// 20 -> update_bazaar sees new_til(20) > 1 and fires `in_bazaar`). Uses only
    /// the engine's own bazaar logic, so it's faithful to a real entry.
    fn force_into_bazaar(b: &mut Bout, side: Side) {
        b.versus.game_mut(side).receive_op_score(0, 19, 0);
        b.versus.game_mut(side).receive_op_score(0, 20, 0);
    }

    /// Strategy: legal client inputs that must NEVER change a player's funds via
    /// the `apply_input` call itself (funds may only change later, in the engine
    /// tick, from line clears). Excludes SoftDrop (can trigger a lock+clear in
    /// the call) and Buy/Sell (which legitimately change funds inside the
    /// bazaar) so the injection oracle below has a clean "funds must not move".
    fn noninjecting_input() -> impl Strategy<Value = Input> {
        prop_oneof![
            Just(Input::MoveLeft),
            Just(Input::MoveRight),
            Just(Input::Rotate),
            Just(Input::BeginDrop),
            (0u32..10u32).prop_map(Input::LaunchWeapon),
        ]
    }

    // -----------------------------------------------------------------------
    // Property (a): apply_input REJECTS every relay-internal input.
    //   - Returns false.
    //   - Never mutates funds/score (no state injected).
    // -----------------------------------------------------------------------
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn relay_internal_inputs_always_rejected(
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
            side_idx in 0usize..2,
            seq in 1u64..=u64::MAX,
            input in relay_internal_input(),
        ) {
            let mut b = Bout::new(seed_a, seed_b);
            let side = if side_idx == 0 { Side::A } else { Side::B };

            // Snapshot the FULL latent state before the attempt — BOTH the per-side
            // game serialization (board + pending-weapon queue + arsenal + funds +
            // remaining-effect counters; see Game::snapshot_bytes) AND every
            // Bout-only field (both acks, the frame log's CONTENTS, both spy slots,
            // opp_lines_seen, and the tick counter). A rejected input must touch NONE
            // of it. The earlier version checked only funds + ack + game snapshots,
            // so a mutant that scribbled on Bout-only state (e.g. `self.spy[idx] =
            // Some((Condor, 20))`) before returning false survived.
            let snap_a = b.versus.game(Side::A).snapshot_bytes();
            let snap_b = b.versus.game(Side::B).snapshot_bytes();
            let frames_before = b.frames.clone();
            let ack_before = b.ack;
            let spy_before = b.spy;
            let opp_lines_before = b.opp_lines_seen;
            let tick_before = b.tick;

            let accepted = b.apply_input(side, &input, seq);

            // Must be rejected.
            prop_assert!(!accepted, "relay-internal input {:?} was accepted (should be rejected)", input);

            // Nothing — game OR Bout-only — moved.
            prop_assert_eq!(&b.versus.game(Side::A).snapshot_bytes(), &snap_a,
                "Side A game state changed after rejected relay-internal input {:?}", input);
            prop_assert_eq!(&b.versus.game(Side::B).snapshot_bytes(), &snap_b,
                "Side B game state changed after rejected relay-internal input {:?}", input);
            prop_assert_eq!(&b.frames, &frames_before,
                "the frame log changed after a rejected relay-internal input {:?}", input);
            prop_assert_eq!(b.ack, ack_before,
                "an ack advanced after a rejected relay-internal input {:?}", input);
            prop_assert_eq!(b.spy, spy_before,
                "spy state changed after a rejected relay-internal input {:?}", input);
            prop_assert_eq!(b.opp_lines_seen, opp_lines_before,
                "opp_lines_seen changed after a rejected relay-internal input {:?}", input);
            prop_assert_eq!(b.tick, tick_before,
                "the tick counter moved after a rejected relay-internal input {:?}", input);

            // And NO DELAYED effect: a clean bout that never saw the input must stay
            // bit-identical through enough ticks to FORCE several natural locks
            // (BT_DROP_TIME=512ms => ~900 ticks per drop from the top; 1500 guarantees
            // multiple locks), so a queued weapon surfacing at a lock/flush diverges.
            let mut control = Bout::new(seed_a, seed_b);
            for _ in 0..1500 {
                if b.is_over() && control.is_over() {
                    break;
                }
                b.tick(16);
                control.tick(16);
            }
            prop_assert_eq!(
                &b.versus.game(Side::A).snapshot_bytes(),
                &control.versus.game(Side::A).snapshot_bytes(),
                "Side A diverged from a clean bout after a rejected input {:?} (latent injection surfaced on a lock)", input
            );
            prop_assert_eq!(
                &b.versus.game(Side::B).snapshot_bytes(),
                &control.versus.game(Side::B).snapshot_bytes(),
                "Side B diverged from a clean bout after a rejected input {:?} (latent injection surfaced on a lock)", input
            );
        }
    }

    // -----------------------------------------------------------------------
    // Property (b): apply_input enforces strictly-increasing seq.
    //   - seq <= last_ack => rejected, ack does not move backward.
    //   - seq >  last_ack => accepted (for a legal input), ack advances.
    // -----------------------------------------------------------------------
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn seq_monotonicity_enforced(
            seed in any::<u64>(),
            // Random (often out-of-order) seqs to exercise both stale-reject and
            // fresh-accept. We never tick, so MoveLeft never locks -> the game
            // never enters the bazaar -> a fresh legal seq MUST be accepted.
            seqs in prop::collection::vec(1u64..=1000u64, 1..256),
        ) {
            let mut b = Bout::new(seed, seed.wrapping_add(1));

            for seq in seqs {
                let ack_before = b.snapshot_for(Side::A, false).ack;
                let accepted = b.apply_input(Side::A, &Input::MoveLeft, seq);

                // BICONDITIONAL: a legal non-bazaar input is accepted IFF its seq
                // is fresh (strictly greater than the last ack). This requires
                // the server to ACCEPT fresh inputs — a server that rejected
                // everything (or accepted stale seqs) now fails.
                prop_assert_eq!(accepted, seq > ack_before,
                    "MoveLeft seq {} (ack {}): accepted={} (expected {})",
                    seq, ack_before, accepted, seq > ack_before);

                let ack_after = b.snapshot_for(Side::A, false).ack;
                if accepted {
                    prop_assert_eq!(ack_after, seq, "ack did not advance to applied seq {}", seq);
                } else {
                    prop_assert_eq!(ack_after, ack_before, "ack moved after a rejected seq {}", seq);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Property (b'): EVERY legal client input variant is ACCEPTED outside the
    //   bazaar and advances `ack`. The existing acceptance test only ever feeds
    //   `MoveLeft`, so removing any OTHER arm from `is_legal_client_input` (e.g.
    //   `Rotate`, `SoftDrop`, `LaunchWeapon`, `LeaveBazaar`) still passed — the
    //   accepted path for those variants was unproven. A fresh Bout (no ticks ->
    //   never in the bazaar) with a fresh seq must accept each variant and move
    //   that side's ack to the applied seq.
    // -----------------------------------------------------------------------
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn every_legal_client_input_is_accepted_outside_bazaar(
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
            side_idx in 0usize..2,
            seq in 1u64..=u64::MAX,
            input in legal_client_input(),
        ) {
            let mut b = Bout::new(seed_a, seed_b);
            let side = if side_idx == 0 { Side::A } else { Side::B };

            // Cross-check: the gate function itself must admit this variant.
            prop_assert!(is_legal_client_input(&input),
                "legal_client_input() produced a variant the gate rejects: {:?}", input);
            // Fresh Bout is never in the bazaar, so a fresh-seq legal input is accepted.
            prop_assert!(!b.versus.game(side).is_in_bazaar(), "fresh bout must not be in the bazaar");

            let ack_before = b.snapshot_for(side, false).ack;
            let accepted = b.apply_input(side, &input, seq);
            prop_assert!(accepted,
                "legal client input {:?} (seq {}, ack {}) was rejected outside the bazaar",
                input, seq, ack_before);

            // ack advanced to this seq for THIS side (and only this side).
            prop_assert_eq!(b.snapshot_for(side, false).ack, seq,
                "ack must advance to the applied seq for {:?}", input);
            prop_assert_eq!(b.snapshot_for(side.other(), false).ack, 0,
                "the other side's ack must be untouched by {:?}", input);
        }
    }

    // -----------------------------------------------------------------------
    // Property (b''): the BAZAAR INPUT GATE. While a side is shopping the match is
    //   frozen; only buy/sell/leave are legal. A non-shopping input (move / rotate
    //   / drop / launch) must be REJECTED with NO state change — no ack advance, no
    //   recorded frame, no game movement. This gate had zero coverage, so a mutant
    //   `if false && g.is_in_bazaar() && !is_bazaar_input(input) { return false; }`
    //   (letting a client nudge its frozen piece / fire weapons mid-shop) survived.
    // -----------------------------------------------------------------------
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn bazaar_gate_rejects_non_shopping_inputs(
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
            side_idx in 0usize..2,
            // seq 2.. leaves seq 1 for the baseline accepted shopping input below.
            seq in 2u64..=u64::MAX,
            input in non_bazaar_legal_input(),
        ) {
            let mut b = Bout::new(seed_a, seed_b);
            let side = if side_idx == 0 { Side::A } else { Side::B };

            force_into_bazaar(&mut b, side);
            prop_assert!(b.versus.game(side).is_in_bazaar(),
                "precondition: the side must actually be in the bazaar");

            // Baseline: a SHOPPING input (SellWeapon of an empty slot) is bazaar-legal
            // and accepted, advancing ack to 1 — so the rejection below is a true
            // no-advance, not just "ack was already 0" (a gate that rejected
            // EVERYTHING in the bazaar would still pass a 0==0 check).
            prop_assert!(b.apply_input(side, &Input::SellWeapon(0), 1),
                "a shopping input must be accepted while in the bazaar");
            prop_assert_eq!(b.snapshot_for(side, false).ack, 1, "shopping input advanced ack to 1");

            let ack_before = b.snapshot_for(side, false).ack;
            let frames_before = b.frames.clone();
            let game_before = b.versus.game(side).snapshot_bytes();

            // The non-shopping input must be REJECTED while in the bazaar.
            let accepted = b.apply_input(side, &input, seq);
            prop_assert!(!accepted,
                "non-shopping input {:?} must be rejected while in the bazaar", input);
            prop_assert_eq!(b.snapshot_for(side, false).ack, ack_before,
                "ack must NOT advance on a bazaar-rejected input {:?}", input);
            prop_assert_eq!(&b.frames, &frames_before,
                "NO frame must be recorded for a bazaar-rejected input {:?}", input);
            prop_assert_eq!(b.versus.game(side).snapshot_bytes(), game_before,
                "the game must NOT move on a bazaar-rejected input {:?}", input);
        }
    }

    // -----------------------------------------------------------------------
    // Property (c): INJECTION ORACLE. A non-economic legal input (move / rotate /
    //   drop / launch) must NEVER change a player's funds when applied — funds
    //   may only move later, inside the engine tick, from real line clears. So a
    //   bug where e.g. `MoveLeft` granted +999 funds is caught directly (the
    //   apply call would move funds). We also keep the funds >= 0 invariant after
    //   the tick (legitimate clears never make funds negative).
    // -----------------------------------------------------------------------
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn legal_inputs_never_inject_funds(
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
            ops in prop::collection::vec((0usize..2, noninjecting_input()), 0..256),
        ) {
            let mut b = Bout::new(seed_a, seed_b);
            // Stock both arsenals with BENIGN (non-funds) weapons so LaunchWeapon
            // actually fires rather than being a no-op (the launch path was
            // otherwise untested). None of these credit/debit the launcher's funds
            // via the apply, and none make a side's funds go negative on delivery.
            for tok in [
                bt_core::WeaponToken::RiseUp,
                bt_core::WeaponToken::FlipOut,
                bt_core::WeaponToken::Bottle,
                bt_core::WeaponToken::Susan,
                bt_core::WeaponToken::NoSlide,
                bt_core::WeaponToken::Speedy,
            ] {
                b.versus.game_mut(Side::A).grant_weapon(tok);
                b.versus.game_mut(Side::B).grant_weapon(tok);
            }
            let mut next_seq = [1u64, 1u64];

            for (side_idx, input) in ops {
                let side = if side_idx == 0 { Side::A } else { Side::B };
                let seq = next_seq[side_idx];
                next_seq[side_idx] += 1;

                let fa0 = b.versus.game(Side::A).score().funds;
                let fb0 = b.versus.game(Side::B).score().funds;

                let _ = b.apply_input(side, &input, seq);

                // The apply itself must inject NOTHING into either side's funds.
                prop_assert_eq!(b.versus.game(Side::A).score().funds, fa0,
                    "Side A funds changed by applying {:?} (injection!)", input);
                prop_assert_eq!(b.versus.game(Side::B).score().funds, fb0,
                    "Side B funds changed by applying {:?} (injection!)", input);

                // Legitimate gameplay (line clears) happens in the tick; funds may
                // rise but never go negative from client inputs.
                b.tick(16);
                prop_assert!(b.versus.game(Side::A).score().funds >= 0, "Side A funds negative");
                prop_assert!(b.versus.game(Side::B).score().funds >= 0, "Side B funds negative");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Property (c'): the TICK credits funds ONLY when a line actually clears.
    // The ">= 0" check above is too weak — a per-tick `add_funds(1)` at the top
    // of Bout::tick keeps funds non-negative and slips through, before OR after
    // the first lock. Here, in a fresh bout with NO inputs (so no launched
    // weapons -> no garbage-line insertions to confound the board count), the
    // ONLY legitimate funds source is this side clearing its own lines, and a
    // clear is the ONLY thing that DECREASES the locked-cell count (a lock adds
    // <=8 cells; a clear removes a multiple of 10 -> a lock+clear still nets a
    // decrease). So on any tick where a side's board fill did NOT strictly
    // decrease, that side's funds MUST be unchanged — pre- and post-lock alike.
    // -----------------------------------------------------------------------
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn tick_credits_funds_only_on_a_line_clear(
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
        ) {
            let mut b = Bout::new(seed_a, seed_b);
            for _ in 0..1500 {
                if b.is_over() {
                    break;
                }
                let pre = [
                    (board_filled(b.versus.game(Side::A)), b.versus.game(Side::A).score().funds),
                    (board_filled(b.versus.game(Side::B)), b.versus.game(Side::B).score().funds),
                ];
                b.tick(16);
                for (i, side) in [Side::A, Side::B].into_iter().enumerate() {
                    let (fill0, funds0) = pre[i];
                    let fill1 = board_filled(b.versus.game(side));
                    // No strict decrease in locked cells => no line cleared this
                    // tick => funds cannot have legitimately changed.
                    if fill1 >= fill0 {
                        prop_assert_eq!(
                            b.versus.game(side).score().funds, funds0,
                            "side {:?} funds changed on a tick with no line clear \
                             (fill {} -> {}) — tick-path funds injection",
                            side, fill0, fill1
                        );
                    }
                }
            }
        }
    }

    /// FULL per-side fingerprint: the locked board, the falling piece pose, AND
    /// the score triple (score / lines / funds) + op-mirror. `export_board()` alone
    /// misses the current piece + all scoring, so a replay that diverges on a
    /// trailing input (which moves the piece/score but not the locked board) or on
    /// score/funds slips through a board-only comparison.
    fn side_fingerprint(g: &bt_core::Game) -> (Vec<i32>, i32, i32, i32, i64, i64, i64, i64, i64, i64) {
        let (px, py, po) = g.current_piece().map(|p| (p.x, p.y, p.orientation)).unwrap_or((-99, -99, -99));
        let s = g.score();
        (g.export_board(), px, py, po, s.score, s.lines, s.funds, s.op_score, s.op_lines, s.op_funds)
    }

    // -----------------------------------------------------------------------
    // Property (d): `Bout::to_replay` REPLAYS BIT-EXACT. The server records every
    //   accepted client input (stamped with the tick) and exports the match as a
    //   VersusReplay; a VersusReplayPlayer must reconstruct BOTH boards, the
    //   falling pieces, the FULL scores, AND the result exactly. We compare FULL
    //   side fingerprints (board + falling piece + score/lines/funds/op_*), NOT
    //   just export_board — so a recorded-input substitution that keeps the locked
    //   board identical but changes the piece pose or the score (e.g. recording
    //   `AiDrop` in place of `BeginDrop`: the flat AI score vs the human hard-drop
    //   bonus) is caught. We mirror the server's match loop exactly — apply a
    //   batch of inputs, THEN always tick — so a frame is never stamped at a tick
    //   the replay won't reach (the real loop's `apply_input … ; bout.tick()`).
    // -----------------------------------------------------------------------
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(96))]

        #[test]
        fn to_replay_reconstructs_the_match_bit_exact(
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
            // Per loop-iteration: a batch of (side, input) applied this tick, then
            // exactly one tick — faithful to the server's `apply…; bout.tick()`.
            iters in prop::collection::vec(
                prop::collection::vec((0usize..2, legal_client_input()), 0..3),
                0..400,
            ),
        ) {
            use bt_replay::VersusReplayPlayer;

            // NOTE: no out-of-band arsenal grants — the replay reconstructs the
            // match from the SEEDS + the recorded INPUT stream alone, so any state
            // change that didn't come through a recorded input (a direct
            // grant_weapon) would legitimately diverge. LaunchWeapon is therefore a
            // no-op here on both sides (its acceptance is covered separately); what
            // this property pins is that the recorded inputs + ticks replay exactly.
            let mut b = Bout::new(seed_a, seed_b);
            let mut next_seq = [1u64, 1u64];
            // INDEPENDENT oracle of the expected frames: each time `apply_input`
            // ACCEPTS an input we record exactly what we submitted, stamped with the
            // tick the bout was at. This is built in the TEST (not via the bout's
            // own frame-push), so comparing the export against it catches a
            // recording mutant that mangles a frame's payload (e.g. stamping every
            // `LaunchWeapon(slot)` as `LaunchWeapon(0)`) — `to_replay().frames ==
            // b.frames` would NOT, since both sides share the mutated push.
            let mut expected_frames: Vec<bt_replay::VersusFrame> = Vec::new();

            for batch in iters {
                if b.is_over() { break; }
                for (side_idx, input) in batch {
                    let side = if side_idx == 0 { Side::A } else { Side::B };
                    let seq = next_seq[side_idx];
                    next_seq[side_idx] += 1;
                    let tick_now = b.tick_count() as u32;
                    if b.apply_input(side, &input, seq) {
                        expected_frames.push(bt_replay::VersusFrame {
                            tick: tick_now, side: side_idx as u8, input: input.clone(),
                        });
                    }
                }
                // The server ALWAYS ticks after draining a batch of inputs.
                b.tick(16);
            }

            let live_a = side_fingerprint(b.versus.game(Side::A));
            let live_b = side_fingerprint(b.versus.game(Side::B));
            let live_result = b.result();

            // Export. The exported frames must EXACTLY equal what the test observed
            // being ACCEPTED — same tick, side, AND input payload (slot/token).
            // The bout ticks at TICK_MS internally, so to_replay must be given the
            // same dt for a faithful replay; we pass a DISTINCTIVE engine_sha so a
            // blank/dropped one is caught.
            let engine_sha = "pbt-sha-7f3a9c";
            let exported = b.to_replay(TICK_MS, engine_sha);
            prop_assert_eq!(&exported.frames, &expected_frames,
                "to_replay must export every accepted input verbatim (tick/side/input)");
            // The HEADER metadata must reflect the export args + match state — a
            // `to_replay` with `version: 0`, blank `engine_sha`, a wrong `dt_ms`, or
            // a stale `tick_count` is caught here.
            prop_assert_eq!(exported.version, REPLAY_VERSION, "to_replay must stamp REPLAY_VERSION");
            prop_assert_eq!(exported.dt_ms, TICK_MS, "to_replay must record the given dt_ms");
            prop_assert_eq!(&exported.engine_sha, engine_sha, "to_replay must record the given engine_sha");
            prop_assert_eq!(exported.tick_count, b.tick_count() as u32, "to_replay tick_count must equal the bout's tick count");
            prop_assert_eq!(exported.seed_a, seed_a as u32, "to_replay must record seed_a");
            prop_assert_eq!(exported.seed_b, seed_b as u32, "to_replay must record seed_b");

            // Replay (through JSON too — the on-disk form).
            let replay = bt_replay::VersusReplay::from_json(&exported.to_json())
                .expect("to_replay JSON must parse");
            let mut player = VersusReplayPlayer::new(replay);
            player.run_to_end();

            prop_assert_eq!(side_fingerprint(player.game(true)), live_a,
                "replayed Side A (board+piece+score) must match the live bout");
            prop_assert_eq!(side_fingerprint(player.game(false)), live_b,
                "replayed Side B (board+piece+score) must match the live bout");
            prop_assert_eq!(player.result(), live_result,
                "replayed match result must match the live bout");
        }
    }

    /// Count of occupied board cells (the locked stack — the falling piece is not
    /// part of the board until it locks).
    fn board_filled(g: &bt_core::Game) -> i64 {
        let b = g.board();
        (0..b.height)
            .flat_map(|y| (0..b.width).map(move |x| (x, y)))
            .filter(|&(x, y)| b.get(x, y).is_some())
            .count() as i64
    }

    /// Force `side` to clear roughly `target` lines by repeatedly prefilling its
    /// bottom two rows and locking. Returns the side's final cleared-line count.
    fn force_side_clears(b: &mut Bout, side: bt_core::versus::Side, target: i64) -> i64 {
        let mut guard = 0;
        while b.versus.game(side).score().lines < target && guard < 60 {
            guard += 1;
            {
                let bd = b.versus.game_mut(side).board_mut();
                let (w, h) = (bd.width, bd.height);
                for y in [h - 1, h - 2] {
                    for x in 0..w { bd.set(x, y, Some(bt_core::Cell::die(6))); }
                }
            }
            let before = b.versus.game(side).score().lines;
            for _ in 0..600 {
                b.versus.game_mut(side).begin_drop();
                b.tick(16);
                if b.is_over() || b.versus.game(side).score().lines > before { break; }
            }
            if b.is_over() { break; }
        }
        b.versus.game(side).score().lines
    }

    // -----------------------------------------------------------------------
    // Property (g): `Bout::take_dirty` reports cross-player events. A delivered
    //   weapon (and funds steal / bazaar entry) is something a client can't predict
    //   from its own inputs, so the server sets a dirty flag to push a prompt
    //   keyframe. Nothing pinned `Bout::take_dirty`, so `take_dirty() -> false`
    //   survived. Here: A launches a weapon at B, a tick DELIVERS it -> take_dirty
    //   must be true exactly once, then clear.
    // -----------------------------------------------------------------------
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn take_dirty_fires_on_a_delivered_weapon(
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
        ) {
            let mut b = Bout::new(seed_a, seed_b);
            // Drain the start-of-match dirty (if any).
            let _ = b.take_dirty();
            // A acquires + launches a cross-player weapon at B.
            b.versus.game_mut(Side::A).grant_weapon(bt_core::WeaponToken::RiseUp);
            prop_assert!(b.apply_input(Side::A, &Input::LaunchWeapon(0), 1),
                "the launch input is accepted");
            prop_assert!(!b.take_dirty(),
                "apply_input alone must not mark dirty (delivery happens in the tick)");

            // The tick relays the launch -> weapon delivered -> dirty set.
            b.tick(16);
            prop_assert!(b.take_dirty(),
                "a delivered cross-player weapon must mark the bout dirty");
            // And it CLEARS after being taken.
            prop_assert!(!b.take_dirty(), "take_dirty must clear after being read");
        }
    }

    // -----------------------------------------------------------------------
    // Property (h): `Bout::lines(side)` reflects the side's real cleared-line
    //   count (used to settle TrueSkill). Only ever checked at the fresh zero
    //   state, so `lines(_) -> 0` survived. Force real clears on a side and assert
    //   `lines(side)` equals the underlying game's `score().lines`.
    // -----------------------------------------------------------------------
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(32))]

        #[test]
        fn lines_accessor_tracks_real_clears(
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
            side_idx in 0usize..2,
        ) {
            let mut b = Bout::new(seed_a, seed_b);
            let side = if side_idx == 0 { Side::A } else { Side::B };
            let cleared = force_side_clears(&mut b, side, 4);
            prop_assume!(cleared > 0); // the side really cleared lines
            // The accessor must mirror the engine's count exactly (not a constant).
            prop_assert_eq!(b.lines(side) as i64, b.versus.game(side).score().lines,
                "Bout::lines(side) must equal the side's real cleared-line count");
            prop_assert_eq!(b.lines(side), cleared as u32,
                "Bout::lines(side) must be the {} lines we forced", cleared);
        }
    }

    // -----------------------------------------------------------------------
    // Property (i): a SPY EXPIRES after the opponent clears its `duration` lines.
    //   The spy bookkeeping decrements the remaining budget by the opponent's
    //   line-clears each tick; once it hits 0 the spy is dropped (`spying` false,
    //   no spy_board). Only ever checked immediately after launch, so `let delta =
    //   0` (a spy that never expires) survived. Here we launch a spy, force the
    //   opponent through MORE than its duration of clears, and assert it expired.
    // -----------------------------------------------------------------------
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(24))]

        #[test]
        fn a_spy_expires_after_the_opponents_duration_of_clears(
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
            // 0=Ames(20), 1=Ace(30) — pick the shorter-duration spies to keep the
            // forced-clear loop bounded.
            spy_idx in 0usize..2,
        ) {
            let spy = [bt_core::WeaponToken::Ames, bt_core::WeaponToken::Ace][spy_idx];
            let duration = weapon_table()[spy.index()].duration as i64;

            let mut b = Bout::new(seed_a, seed_b);
            // A launches the spy at B.
            b.versus.game_mut(Side::A).grant_weapon(spy);
            prop_assert!(b.apply_input(Side::A, &Input::LaunchWeapon(0), 1));
            b.tick(16); // relay records the spy launch; the bout activates it
            prop_assert!(b.snapshot_for(Side::A, false).spying,
                "A must be spying immediately after launching {:?}", spy);

            // Force B (the opponent) to clear MORE than `duration` lines; the spy's
            // budget is charged by B's clears each tick.
            let got = force_side_clears(&mut b, Side::B, duration + 2);
            prop_assume!(got >= duration); // B cleared enough to exhaust the spy

            // The spy must have EXPIRED — A is no longer spying, and a keyframe
            // frame carries no spy board.
            prop_assert!(!b.snapshot_for(Side::A, false).spying,
                "the spy must expire after the opponent clears its {}-line duration", duration);
            prop_assert!(b.snapshot_for(Side::A, true).spy_board.is_none(),
                "an expired spy must not still reveal the opponent board");
        }
    }

    #[test]
    fn rejects_relay_internal_inputs_from_clients() {
        // The anti-cheat core: a client must not be able to grant itself a
        // weapon, op-score, or funds.
        assert!(!is_legal_client_input(&Input::ReceiveWeapon(7)));
        assert!(!is_legal_client_input(&Input::AddFunds(9999)));
        assert!(!is_legal_client_input(&Input::AiDrop));
        assert!(!is_legal_client_input(&Input::ReceiveOpScore { score: 1, lines: 1, funds: 1 }));
        // A client-controlled pause is rejected (it would freeze only one board).
        assert!(!is_legal_client_input(&Input::SetPaused(true)));
        // Legal player actions pass.
        assert!(is_legal_client_input(&Input::MoveLeft));
        assert!(is_legal_client_input(&Input::LaunchWeapon(3)));
        assert!(is_legal_client_input(&Input::BuyWeapon(7)));
        assert!(is_legal_client_input(&Input::LeaveBazaar));
        // Only shopping actions are bazaar-legal.
        assert!(is_bazaar_input(&Input::BuyWeapon(0)));
        assert!(is_bazaar_input(&Input::LeaveBazaar));
        assert!(!is_bazaar_input(&Input::MoveLeft));
        assert!(!is_bazaar_input(&Input::LaunchWeapon(0)));
    }

    #[test]
    fn apply_input_rejects_illegal_and_records_ack_for_legal() {
        let mut b = Bout::new(1, 2);
        assert!(!b.apply_input(Side::A, &Input::AddFunds(500), 1), "funds injection rejected");
        assert_eq!(b.versus.game(Side::A).score().funds, 0, "no funds granted");

        assert!(b.apply_input(Side::A, &Input::MoveLeft, 5), "legal move accepted");
        assert_eq!(b.snapshot_for(Side::A, false).ack, 5, "ack advanced to the applied seq");
        assert_eq!(b.snapshot_for(Side::B, false).ack, 0, "the other side's ack is independent");
    }

    #[test]
    fn apply_input_rejects_stale_and_out_of_order_seqs() {
        let mut b = Bout::new(1, 2);
        assert!(b.apply_input(Side::A, &Input::MoveLeft, 5));
        // A replay of the same seq, or any seq <= ack, is rejected and ack holds.
        assert!(!b.apply_input(Side::A, &Input::MoveRight, 5), "duplicate seq rejected");
        assert!(!b.apply_input(Side::A, &Input::MoveRight, 3), "older seq rejected");
        assert_eq!(b.snapshot_for(Side::A, false).ack, 5, "ack never moves backward");
        assert!(b.apply_input(Side::A, &Input::MoveRight, 6), "the next seq advances");
        assert_eq!(b.snapshot_for(Side::A, false).ack, 6);
    }

    #[test]
    fn snapshot_is_light_by_default_and_carries_a_keyframe_on_request() {
        let b = Bout::new(1, 2);
        let light = b.snapshot_for(Side::A, false);
        assert!(light.keyframe.is_none(), "the default frame is light (no keyframe)");
        assert_eq!(light.result, 0, "ongoing");
        assert_eq!(light.opp.score, 0, "opponent starts at 0");
        assert_eq!(light.you.funds, 0, "own status present every frame");
        assert!(!light.you.in_bazaar);

        let full = b.snapshot_for(Side::A, true);
        let kf = full.keyframe.expect("keyframe present on request");
        assert_eq!(kf.len() % 8, 0, "keyframe is a buffer of i64s");
        // It's a real full-state keyframe: it restores into a fresh engine.
        let mut g = bt_core::Game::new(999);
        assert!(g.restore_bytes(&kf), "the keyframe restores a full game");
    }

    // -----------------------------------------------------------------------
    // Property (j): `snapshot_for` reports the REAL client-visible state, not
    //   fresh-zero defaults. The existing snapshot test only checks a brand-new
    //   bout, so a `snapshot_for` that hardcoded `opp.score: 0`, `opp.lines: 0`,
    //   `in_bazaar: false`, `lines_til_bazaar: 0`, or `you.funds: 0` survived. We
    //   drive non-trivial state (a side clears lines + banks funds; the other may
    //   enter the bazaar) and assert each snapshot field matches the engine from
    //   BOTH points of view. Also pins the settlement accessors score()/funds()/
    //   lines() against the same engine state.
    // -----------------------------------------------------------------------
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(24))]

        #[test]
        fn snapshot_and_settlement_reflect_real_engine_state(
            seed_a in any::<u64>(),
            seed_b in any::<u64>(),
            clear_side_idx in 0usize..2,
            extra_funds in 1i64..100_000,
        ) {
            let mut b = Bout::new(seed_a, seed_b);
            let clear_side = if clear_side_idx == 0 { Side::A } else { Side::B };
            // Make `clear_side` clear lines (banks score+lines+funds), and credit
            // some extra funds so funds is non-zero on that side.
            let cleared = force_side_clears(&mut b, clear_side, 3);
            prop_assume!(cleared > 0 && !b.is_over());
            b.versus.game_mut(clear_side).add_funds(extra_funds);
            // Force the OTHER side into the bazaar so `in_bazaar` / `lines_til_bazaar`
            // are NON-trivial (else those assertions are vacuous — both default
            // to false/20 and a hardcoded mutant would slip through).
            force_into_bazaar(&mut b, clear_side.other());
            prop_assert!(b.versus.game(clear_side.other()).is_in_bazaar(),
                "the other side must be in the bazaar (non-vacuity for in_bazaar)");

            // For BOTH sides, the snapshot must mirror the engine exactly.
            for side in [Side::A, Side::B] {
                let snap = b.snapshot_for(side, false);
                let me = b.versus.game(side);
                let them = b.versus.game(side.other());
                prop_assert_eq!(snap.you.funds, me.score().funds,
                    "you.funds must mirror the engine ({:?})", side);
                prop_assert_eq!(snap.you.in_bazaar, me.is_in_bazaar(),
                    "you.in_bazaar must mirror the engine ({:?})", side);
                prop_assert_eq!(snap.you.lines_til_bazaar, me.lines_til_bazaar(),
                    "you.lines_til_bazaar must mirror the engine ({:?})", side);
                prop_assert_eq!(snap.opp.score, them.score().score,
                    "opp.score must mirror the OPPONENT's engine score ({:?})", side);
                prop_assert_eq!(snap.opp.lines, them.score().lines,
                    "opp.lines must mirror the OPPONENT's engine lines ({:?})", side);
                prop_assert_eq!(snap.opp.game_over, them.is_game_over(),
                    "opp.game_over must mirror the opponent ({:?})", side);
            }

            // Non-vacuity: the clearing side genuinely has non-zero lines/score and
            // funds, so a hardcoded-zero accessor really diverges.
            let cs = b.versus.game(clear_side).score();
            prop_assert!(cs.lines > 0 && cs.funds > 0, "the clearing side must have banked lines + funds");

            // Settlement accessors mirror the engine.
            for side in [Side::A, Side::B] {
                let g = b.versus.game(side);
                prop_assert_eq!(b.score(side), g.score().score, "Bout::score(side) must mirror the engine");
                prop_assert_eq!(b.funds(side), g.score().funds, "Bout::funds(side) must mirror the engine");
                prop_assert_eq!(b.lines(side) as i64, g.score().lines, "Bout::lines(side) must mirror the engine");
            }
        }
    }

    #[test]
    fn client_keyframe_redacts_opponent_funds_but_keeps_op_lines() {
        let mut b = Bout::new(1, 2);
        // The server forwards B's score into A's mirror (op_score/op_lines/op_funds).
        b.versus.game_mut(Side::A).receive_op_score(50, 3, 777);
        assert_eq!(b.versus.game(Side::A).score().op_funds, 777, "mirrored internally");

        let kf = b.snapshot_for(Side::A, true).keyframe.unwrap();
        let mut g = bt_core::Game::new(0);
        assert!(g.restore_bytes(&kf));
        assert_eq!(g.score().op_funds, 0, "the client keyframe must NOT leak opponent funds");
        assert_eq!(g.score().op_lines, 3, "but op_lines (drives the bazaar) is preserved");
    }

    #[test]
    fn a_launched_weapon_is_resolved_authoritatively_across_the_bout() {
        let mut b = Bout::new(1, 2);
        // A buys + launches RiseUp at B (legal client inputs only).
        b.versus.game_mut(Side::A).grant_weapon(bt_core::WeaponToken::RiseUp);
        assert!(b.apply_input(Side::A, &Input::LaunchWeapon(0), 1));
        // Tick the authoritative match; then drive B down to flush the weapon.
        // Each input needs a strictly-increasing seq (the monotonicity gate).
        b.tick(16);
        let mut seq = 0u64;
        for _ in 0..400 {
            seq += 1;
            b.apply_input(Side::B, &Input::BeginDrop, seq);
            b.tick(16);
            let board = b.versus.game(Side::B).export_board();
            // Count non-empty cells (tag != 0 in each quad).
            let filled = board.chunks(4).filter(|q| q[0] != 0).count();
            if filled >= 9 {
                return; // B received A's RiseUp row — resolved server-side
            }
        }
        panic!("RiseUp was not delivered to B by the authoritative bout");
    }

    // -----------------------------------------------------------------------
    // Property (f): SPY DEGRADATION privacy. Each spy reveals a DIFFERENT fraction
    //   of the opponent board, and the degradation is what stops a modified client
    //   from reading cells the spy didn't earn. The old test only asserted "some
    //   cells visible", so `Ames => 0` (reveal EVERYTHING — a full info leak)
    //   survived. Here, over a FULLY-filled board, we pin each token's reveal:
    //     * Ames  must HIDE some AND REVEAL some (a partial, ~50% view).
    //     * Ace   must hide FEWER than Ames (it's the more accurate spy).
    //     * Condor must reveal ALL (perfect satellite — hides nothing).
    //   `degrade_board` hides by turning a cell to -2 (empty); a revealed cell
    //   keeps its id. Empty cells (-2) are never "revealed", so we fill the board.
    // -----------------------------------------------------------------------
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        #[test]
        fn spy_degradation_hides_the_right_fraction_per_token(
            // A fully-filled board of arbitrary non-empty render ids (>= 0).
            ids in prop::collection::vec(0i32..30, 280),
        ) {
            // Sanity: the source grid has NO empty cells, so every -2 in the output
            // is a HIDE the spy imposed (not a pre-existing empty).
            prop_assert!(ids.iter().all(|&v| v != -2), "source grid must be fully filled");
            let total = ids.len();
            let hidden = |grid: &[i32]| grid.iter().filter(|&&v| v == -2).count();

            let ames = degrade_board(ids.clone(), WeaponToken::Ames);
            let ace = degrade_board(ids.clone(), WeaponToken::Ace);
            let condor = degrade_board(ids.clone(), WeaponToken::Condor);

            let (h_ames, h_ace, h_condor) = (hidden(&ames), hidden(&ace), hidden(&condor));
            let pct = |h: usize| (h as f64) / (total as f64) * 100.0;

            // Ames hides ~50% (`spy_hide_pct(Ames)`). The hide is a DETERMINISTIC
            // hash, so over a full board the fraction is stable; pin it to a BAND
            // around the spec so a mutant that drifts the rate (e.g. `Ames => 2`,
            // a near-full info leak, or `Ames => 95`, a near-blackout) FAILS — the
            // old "hides some / reveals some" check let a 2% hider pass.
            prop_assert!((35.0..=65.0).contains(&pct(h_ames)),
                "Ames must hide ~50% of cells (in [35,65]); hid {:.1}% ({}/{})",
                pct(h_ames), h_ames, total);

            // Ace hides ~15% — a band around its spec, and strictly fewer than Ames.
            prop_assert!((5.0..=30.0).contains(&pct(h_ace)),
                "Ace must hide ~15% of cells (in [5,30]); hid {:.1}% ({}/{})",
                pct(h_ace), h_ace, total);
            prop_assert!(h_ace < h_ames,
                "Ace must hide FEWER cells than Ames (the more accurate spy): ace={} ames={}",
                h_ace, h_ames);

            // Condor is perfect: it hides NOTHING (reveals the whole board).
            prop_assert_eq!(h_condor, 0,
                "Condor (satellite) must reveal the ENTIRE board (hide nothing); hid {}", h_condor);
            prop_assert_eq!(&condor, &ids, "Condor's output must equal the source grid exactly");
        }
    }

    #[test]
    fn a_spy_reveals_a_degraded_opponent_board_only_to_the_launcher() {
        let mut b = Bout::new(1, 2);
        // Give B some board so the reveal is non-empty.
        for x in 0..6 {
            b.versus.game_mut(Side::B).board_mut().set(x, 20, Some(bt_core::Cell::die(3)));
        }
        b.versus.game_mut(Side::A).grant_weapon(WeaponToken::Ames);
        assert!(b.apply_input(Side::A, &Input::LaunchWeapon(0), 1));
        b.tick(16); // relay records the spy; the bout activates it

        let sa = b.snapshot_for(Side::A, true);
        assert!(sa.spying, "A is spying after launching Ames");
        let board = sa.spy_board.expect("A gets the opponent board on a keyframe frame");
        let (w, h) = (b.versus.game(Side::B).board().width, b.versus.game(Side::B).board().height);
        assert_eq!(board.len() as i32, w * h, "a full (degraded) render-id grid (not quads)");
        assert!(board.iter().any(|&id| id != -2), "and it shows some of the opponent's cells");

        // B is not spying and gets nothing; and a light frame carries no spy board.
        let sb = b.snapshot_for(Side::B, true);
        assert!(!sb.spying && sb.spy_board.is_none(), "the spied player learns nothing");
        assert!(b.snapshot_for(Side::A, false).spy_board.is_none(), "spy board rides keyframes only");
    }

    #[test]
    fn settlement_accessors_report_per_side_state_and_tick_count() {
        let mut b = Bout::new(1, 2);
        assert_eq!(b.tick_count(), 0, "no ticks yet");
        assert_eq!(b.score(Side::A), 0);
        assert_eq!(b.funds(Side::B), 0);
        b.tick(16);
        b.tick(16);
        assert_eq!(b.tick_count(), 2, "tick_count advances with the sim");
    }

    #[test]
    fn result_is_translated_to_each_clients_point_of_view() {
        let mut b = Bout::new(7, 8);
        // Bury B (fill every column but col 0 -> no clears, spawn fails).
        let (w, h) = {
            let g = b.versus.game(Side::B);
            (g.board().width, g.board().height)
        };
        for y in 0..h {
            for x in 1..w {
                b.versus
                    .game_mut(Side::B)
                    .board_mut()
                    .set(x, y, Some(bt_core::Cell::die(1)));
            }
        }
        for _ in 0..500 {
            b.tick(16);
            if b.is_over() {
                break;
            }
        }
        assert_eq!(b.result(), 1, "A won (B topped out)");
        assert_eq!(b.snapshot_for(Side::A, false).result, 1, "A's POV: you won");
        assert_eq!(b.snapshot_for(Side::B, false).result, 2, "B's POV: you lost");
    }
}

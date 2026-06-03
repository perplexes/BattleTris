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
use bt_core::Versus;
use bt_replay::Input;
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

/// The authoritative RENDER view of a player's own board: enough to draw their
/// side and show a divergence, using the engine's flat i32 codec
/// (`export_board`/`export_arsenal`, the same one the old P2P Swap/Susan relay
/// used). NOTE: this is NOT a full reconciliation keyframe — `export_board`
/// omits the falling piece, phase/timers, RNG + piece-manager state, and the
/// active-weapon flags/durations + pending queue. Phase 4 (client prediction)
/// needs a complete `Game` keyframe (a serialize/restore of the whole engine
/// state) to re-sim unacked inputs on top of; this struct is the display view.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SelfView {
    pub board: Vec<i32>,
    pub arsenal: Vec<i32>,
    pub score: i64,
    pub lines: i64,
    pub funds: i64,
    pub in_bazaar: bool,
    pub lines_til_bazaar: i32,
    pub game_over: bool,
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

/// One authoritative frame sent to a client. `ack` is the last input sequence
/// the server has applied from THIS client, so the client can discard
/// acknowledged inputs and re-apply only the unacked ones on top of `you`.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub tick: u64,
    pub ack: u64,
    /// 0 = ongoing, 1 = this client won, 2 = this client lost.
    pub result: i32,
    pub you: SelfView,
    pub opp: OppView,
}

/// A server-hosted authoritative match between two clients.
pub struct Bout {
    versus: Versus,
    tick: u64,
    /// Last applied input sequence number per side (A = [0], B = [1]).
    ack: [u64; 2],
}

impl Bout {
    /// Start a bout. The two sides get distinct seeds (so their piece streams
    /// differ); the server picks them and tells each client its seed at handoff.
    pub fn new(seed_a: u64, seed_b: u64) -> Bout {
        Bout {
            versus: Versus::new(seed_a, seed_b),
            tick: 0,
            ack: [0, 0],
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
        true
    }

    /// Advance the authoritative simulation by `dt_ms` (the server's clock).
    pub fn tick(&mut self, dt_ms: i32) {
        self.versus.tick(dt_ms);
        self.tick += 1;
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

    /// The authoritative snapshot for `side` as a ready-to-send ws message: the
    /// [`Snapshot`] fields plus a `{"type":"snapshot"}` tag.
    pub fn snapshot_message(&self, side: Side) -> String {
        let mut v =
            serde_json::to_value(self.snapshot_for(side)).unwrap_or(serde_json::Value::Null);
        if let Some(obj) = v.as_object_mut() {
            obj.insert("type".into(), serde_json::Value::String("snapshot".into()));
        }
        v.to_string()
    }

    /// Build the authoritative snapshot to send to `side`.
    pub fn snapshot_for(&self, side: Side) -> Snapshot {
        let me = self.versus.game(side);
        let them = self.versus.game(side.other());
        let s = me.score();

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
            you: SelfView {
                board: me.export_board(),
                arsenal: me.export_arsenal(),
                score: s.score,
                lines: s.lines,
                funds: s.funds,
                in_bazaar: me.is_in_bazaar(),
                lines_til_bazaar: me.lines_til_bazaar(),
                game_over: me.is_game_over(),
            },
            opp: OppView {
                score: them.score().score,
                lines: them.score().lines,
                game_over: them.is_game_over(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(b.snapshot_for(Side::A).you.funds, 0, "no funds granted");

        assert!(b.apply_input(Side::A, &Input::MoveLeft, 5), "legal move accepted");
        assert_eq!(b.snapshot_for(Side::A).ack, 5, "ack advanced to the applied seq");
        assert_eq!(b.snapshot_for(Side::B).ack, 0, "the other side's ack is independent");
    }

    #[test]
    fn apply_input_rejects_stale_and_out_of_order_seqs() {
        let mut b = Bout::new(1, 2);
        assert!(b.apply_input(Side::A, &Input::MoveLeft, 5));
        // A replay of the same seq, or any seq <= ack, is rejected and ack holds.
        assert!(!b.apply_input(Side::A, &Input::MoveRight, 5), "duplicate seq rejected");
        assert!(!b.apply_input(Side::A, &Input::MoveRight, 3), "older seq rejected");
        assert_eq!(b.snapshot_for(Side::A).ack, 5, "ack never moves backward");
        assert!(b.apply_input(Side::A, &Input::MoveRight, 6), "the next seq advances");
        assert_eq!(b.snapshot_for(Side::A).ack, 6);
    }

    #[test]
    fn snapshot_reflects_authoritative_state_and_is_per_side() {
        let mut b = Bout::new(1, 2);
        let snap_a = b.snapshot_for(Side::A);
        // A full board export is width*height*4 ints (flat [tag,a,b,hidden] cells).
        assert_eq!(snap_a.you.board.len() % 4, 0);
        assert!(!snap_a.you.board.is_empty());
        assert_eq!(snap_a.you.arsenal.len(), 20, "10 slots * [token,qty]");
        assert_eq!(snap_a.result, 0, "ongoing");
        // The two sides see mirrored opp/you score views (both 0 at start here).
        let snap_b = b.snapshot_for(Side::B);
        assert_eq!(snap_a.opp.score, snap_b.you.score);
        let _ = &mut b;
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
            let board = b.snapshot_for(Side::B).you.board;
            // Count non-empty cells (tag != 0 in each quad).
            let filled = board.chunks(4).filter(|q| q[0] != 0).count();
            if filled >= 9 {
                return; // B received A's RiseUp row — resolved server-side
            }
        }
        panic!("RiseUp was not delivered to B by the authoritative bout");
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
        assert_eq!(b.snapshot_for(Side::A).result, 1, "A's POV: you won");
        assert_eq!(b.snapshot_for(Side::B).result, 2, "B's POV: you lost");
    }
}

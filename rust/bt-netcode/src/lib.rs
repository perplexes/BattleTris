//! Shared client-side prediction + reconciliation for a server-authoritative
//! BattleTris match. Both the browser (via `bt-wasm`'s `WasmClient`) and the
//! bot (`bt-bot`) run this crate.
//!
//! # Why this crate exists
//!
//! The server runs the only authoritative sim; each client keeps a local
//! [`bt_core::Game`] seeded from the same piece stream, predicts its own inputs
//! immediately (so play feels instant), and reconciles to the server's keyframes.
//! That prediction/reconciliation logic is delicate (replaying unacked inputs
//! on top of a keyframe, gating inputs at the bazaar barrier), so it lives in
//! one place, [`Predictor`], driven identically by both clients. Its invariants
//! are pinned by the proptests here; sharing the single implementation is what
//! keeps the browser and the bot consistent.
//!
//! # The model
//!
//! - [`Predictor::predict`] applies an input to the local sim, queues it as
//!   *unacked*, and hands back the `(seq, Input)` to send. Inputs carry a monotonic
//!   per-bout `seq`.
//! - [`Predictor::on_snapshot`] reconciles against an authoritative frame: it drops
//!   inputs the server has now acked (`seq <= ack`), and on a keyframe overwrites
//!   the local state with the authoritative one and *replays the still-unacked tail*
//!   on top. Replaying the unacked tail is exactly what stops a not-yet-acked input
//!   from being lost (the snap-back): the predicted-but-unconfirmed move survives the
//!   restore. See the property tests in `tests/`.
//!
//! The bot layers its own pure policy (`bt-bot`'s `sync::decide`) on top to decide
//! *when* to predict; this crate owns only the mechanics.

use bt_core::Game;
use bt_core::WeaponToken;
use bt_replay::Input;
use serde::Serialize;

/// A client's local predicted game plus the bookkeeping needed to reconcile it to
/// the server's authoritative keyframes.
pub struct Predictor {
    /// The local predicted sim. Seeded from the same value as the server's side, so
    /// it stays in lockstep until a cross-player event makes a keyframe necessary.
    game: Game,
    /// Monotonic input counter for THIS bout. Starts at 0 with each `Predictor`, so
    /// it lines up with the server's per-bout `ack` baseline (a fresh bout is never
    /// stuck waiting for an ack that can't arrive).
    input_seq: u64,
    /// Inputs sent to the server but not yet acked, oldest first, as `(seq, input)`.
    /// Re-applied on top of a keyframe during reconciliation.
    unacked: Vec<(u64, Input)>,
    /// Authoritative "you are shopping" from the latest snapshot.
    you_bazaar: bool,
    /// Authoritative "your opponent is shopping" from the latest snapshot. The bazaar
    /// is a barrier: while either side shops the whole match is frozen server-side.
    opp_bazaar: bool,
}

impl Predictor {
    /// A fresh predictor for a bout, its local sim seeded to match the server's side.
    pub fn new(seed: u64) -> Predictor {
        Predictor {
            game: Game::new(seed),
            input_seq: 0,
            unacked: Vec::new(),
            you_bazaar: false,
            opp_bazaar: false,
        }
    }

    /// The local predicted game, for rendering and HUD reads.
    pub fn game(&self) -> &Game {
        &self.game
    }

    /// Mutable access to the local game, for draining queued events into the host's
    /// event encoding (e.g. the browser's sound triggers). Not for applying inputs;
    /// always go through [`predict`](Self::predict) so they're queued for reconciliation.
    pub fn game_mut(&mut self) -> &mut Game {
        &mut self.game
    }

    /// Advance the local prediction one fixed step. Callers must NOT tick while a
    /// bazaar barrier is up ([`barrier`](Self::barrier) is true), because the server
    /// freezes then and ticking the local sim would drift it ahead.
    pub fn tick(&mut self, dt_ms: i32) {
        self.game.tick(dt_ms);
    }

    /// The seq of the most recently sent input this bout (0 if none sent yet). The
    /// bot gates on `ack < input_seq()` ⇒ "inputs still in flight, don't act".
    pub fn input_seq(&self) -> u64 {
        self.input_seq
    }

    /// How many sent inputs are still unacked (drives the debug overlay).
    pub fn unacked_len(&self) -> usize {
        self.unacked.len()
    }

    /// Is a bazaar barrier up? True while either side is shopping. Gameplay inputs
    /// are frozen and only shopping/leave actions are valid. (The authoritative read;
    /// matches the browser's `inBazaar()`.)
    pub fn barrier(&self) -> bool {
        self.you_bazaar || self.opp_bazaar
    }

    /// Predict a local input and, when appropriate, return the `(seq, Input)` to send
    /// to the server. Returns `None` when the input is suppressed:
    ///
    /// - a gameplay input (anything but Buy/Sell/LeaveBazaar) while a bazaar barrier
    ///   is up (the central gate that keeps a frozen match from being driven), or
    /// - a `BuyWeapon`/`SellWeapon` the local engine rejected (insufficient funds /
    ///   not shopping): only an *accepted* buy/sell is forwarded, so the prediction and
    ///   the wire stay in lockstep instead of sending a shop action the server can't honor.
    ///
    /// `LeaveBazaar` is forwarded but NOT applied locally: the bazaar is a server-side
    /// barrier that clears (via the next keyframe) only once BOTH sides are done;
    /// leaving the local sim early would tick it out of a state the server still holds.
    pub fn predict(&mut self, input: Input) -> Option<(u64, Input)> {
        // The bazaar barrier gate. Mirrors `main.js`'s central `predict` guard so no
        // call site can sneak a gameplay input into a frozen match.
        if self.barrier() && !is_shopping(&input) && !matches!(input, Input::LeaveBazaar) {
            return None;
        }
        match &input {
            // Server-confirmed release only; never left locally (see the doc above).
            Input::LeaveBazaar => {}
            // Forward only an accepted buy/sell.
            Input::BuyWeapon(_) | Input::SellWeapon(_) => {
                if !apply_shop(&mut self.game, &input) {
                    return None;
                }
            }
            // Everything else predicts locally. A no-op (e.g. a move into the wall)
            // still sends: the server applies the same no-op, so both stay in step.
            other => other.apply_to_game(&mut self.game),
        }
        self.input_seq += 1;
        self.unacked.push((self.input_seq, input.clone()));
        Some((self.input_seq, input))
    }

    /// Reconcile against an authoritative snapshot.
    ///
    /// `ack` is the last input seq the server has applied for us; `you_bazaar` /
    /// `opp_bazaar` are the authoritative bazaar flags (they drive [`barrier`]).
    /// `keyframe`, when present, is the full authoritative state ([`Game::snapshot_bytes`]).
    ///
    /// Always drops acked inputs (`seq <= ack`). On a keyframe it then overwrites the
    /// local state and replays the still-unacked tail on top, so a predicted input
    /// the server hasn't confirmed yet is preserved rather than snapping back.
    ///
    /// When no keyframe is present, an authoritative bazaar-exit edge (`you_bazaar`
    /// going true to false) releases the local sim instead: see the bazaar-exit note
    /// below.
    ///
    /// [`barrier`]: Self::barrier
    pub fn on_snapshot(
        &mut self,
        ack: u64,
        you_bazaar: bool,
        opp_bazaar: bool,
        keyframe: Option<&[u8]>,
    ) {
        // Detect the authoritative bazaar-exit edge before overwriting the stored flag.
        // The local sim entered the bazaar on its own when it reached the line
        // threshold, but it never applies `LeaveBazaar` locally: the barrier stays up
        // until BOTH sides leave, which only the server knows. Under model B the
        // periodic keyframe that used to restore the sim out of the bazaar is gone, so
        // a plain snapshot is now what carries the release. Without this edge the local
        // sim would sit `in_bazaar` forever once the server reopened play.
        let leaving_bazaar = self.you_bazaar && !you_bazaar;
        self.you_bazaar = you_bazaar;
        self.opp_bazaar = opp_bazaar;
        // Discard inputs the server has now applied.
        self.unacked.retain(|(seq, _)| *seq > ack);
        // On a keyframe: snap to the authoritative state, then replay the unacked tail.
        if let Some(bytes) = keyframe {
            self.game.restore_bytes(bytes);
            // Clone-free borrow: replay reads `input`, mutates only `game`.
            for (_, input) in &self.unacked {
                replay_input(&mut self.game, input);
            }
        } else if leaving_bazaar && self.game.is_in_bazaar() {
            // No keyframe to restore us out: release the local sim to match the server.
            // The shop's Buy/Sell were already applied locally during prediction, so
            // there is nothing to replay here, only the frozen flag to clear.
            self.game.leave_bazaar();
        }
    }

    /// Apply a server-sent cross-player event (a weapon arriving, the opponent's score
    /// mirror, a funds credit) to the local sim. Unlike [`predict`](Self::predict) this
    /// is NOT a local prediction: it is an authoritative effect the server already
    /// applied to its copy, so it does not touch `input_seq` or `unacked` and is never
    /// replayed after a keyframe (the keyframe already contains its effect). This is
    /// the model-B path that keeps the opponent's weapons landing in the local sim
    /// without a keyframe snap.
    pub fn apply_event(&mut self, input: &Input) {
        input.apply_to_game(&mut self.game);
    }

    /// Parse a server `event` frame's `input` field (the serde form of an `Input`) and
    /// [`apply_event`](Self::apply_event) it. A malformed value is ignored: the
    /// reconciliation keyframe still carries the authoritative state, so a dropped event
    /// cannot desync the client past the next keyframe. Returns whether it parsed.
    pub fn apply_event_json(&mut self, input_json: &str) -> bool {
        match serde_json::from_str::<Input>(input_json) {
            Ok(input) => {
                self.apply_event(&input);
                true
            }
            Err(_) => false,
        }
    }
}

/// The shopping inputs (Buy/Sell): the gameplay-affecting actions allowed while
/// the bazaar barrier is up. `LeaveBazaar` is also valid under the barrier but is
/// handled separately (forwarded, never applied locally), so it is not "shopping".
fn is_shopping(input: &Input) -> bool {
    matches!(input, Input::BuyWeapon(_) | Input::SellWeapon(_))
}

/// Apply a buy/sell to the local sim, returning whether the engine accepted it.
/// (`Input::apply_to_game` discards the accept bool, which we need to decide whether
/// to forward the input.)
fn apply_shop(game: &mut Game, input: &Input) -> bool {
    match input {
        Input::BuyWeapon(t) => WeaponToken::from_index(*t).is_some_and(|tok| game.buy_weapon(tok)),
        Input::SellWeapon(t) => WeaponToken::from_index(*t).is_some_and(|tok| game.sell_weapon(tok)),
        _ => false,
    }
}

/// Re-apply one unacked input on top of a just-restored keyframe, WITHOUT re-sending.
///
/// While the restored state is in the bazaar, only Buy/Sell replay; re-applying a
/// movement/drop/launch here would drift from the frozen server. `LeaveBazaar` is
/// never replayed locally (it's server-confirmed). Mirrors `main.js`'s `applyReprToGame`.
fn replay_input(game: &mut Game, input: &Input) {
    if game.is_in_bazaar() && !is_shopping(input) {
        return;
    }
    match input {
        Input::LeaveBazaar => {}
        other => other.apply_to_game(game),
    }
}

/// The exact wire frame for an input: `{"type":"input","seq":N,"input":<repr>}`,
/// where `<repr>` is `Input`'s serde form (`"MoveLeft"`, `{"LaunchWeapon":3}`, …).
///
/// Built in one place, reusing `Input`'s own serde, so the browser and the
/// bot can never disagree on the wire: both clients call this rather than each
/// hand-rolling a JSON shape that has to track serde by hand.
pub fn input_frame(seq: u64, input: &Input) -> String {
    #[derive(Serialize)]
    struct InputFrame<'a> {
        #[serde(rename = "type")]
        ty: &'static str,
        seq: u64,
        input: &'a Input,
    }
    serde_json::to_string(&InputFrame { ty: "input", seq, input }).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    //! Smoke tests for the mechanics. The full property suite (snap-back invariant,
    //! bazaar replay-gating, ack-convergence, browser-vs-bot cross-consistency) lives
    //! in `tests/predictor_pbt.rs`.
    use super::*;

    #[test]
    fn predict_bumps_seq_and_queues() {
        let mut p = Predictor::new(0xC0FFEE);
        assert_eq!(p.input_seq(), 0);
        let (seq, input) = p.predict(Input::MoveLeft).expect("move sends");
        assert_eq!(seq, 1);
        assert_eq!(input, Input::MoveLeft);
        assert_eq!(p.unacked_len(), 1);
        p.predict(Input::Rotate).expect("rotate sends");
        assert_eq!(p.input_seq(), 2);
        assert_eq!(p.unacked_len(), 2);
    }

    #[test]
    fn snapshot_without_keyframe_just_prunes_acked() {
        let mut p = Predictor::new(1);
        p.predict(Input::MoveLeft);
        p.predict(Input::MoveRight);
        p.predict(Input::Rotate); // seqs 1,2,3
        p.on_snapshot(2, false, false, None);
        assert_eq!(p.unacked_len(), 1, "seqs <= 2 pruned, seq 3 still in flight");
    }

    #[test]
    fn keyframe_reconciles_to_authoritative_plus_unacked_tail() {
        // The snap-back guard in miniature: predict a few inputs, then receive a
        // keyframe acking only the first. The local state must equal a reference game
        // with all the inputs applied (authoritative prefix + replayed unacked tail);
        // the unacked inputs are not lost.
        let seed = 0xABCD_1234u64;
        let inputs = [Input::MoveLeft, Input::Rotate, Input::MoveRight];

        // Reference: a server-side sim that applied the first input, snapshotted, then
        // the client predicted all three locally.
        let mut server = Game::new(seed);
        inputs[0].apply_to_game(&mut server);
        let keyframe = server.snapshot_bytes();

        let mut p = Predictor::new(seed);
        for inp in &inputs {
            p.predict(inp.clone());
        }
        // Keyframe acks seq 1 only; seqs 2,3 are still unacked → replayed on top.
        p.on_snapshot(1, false, false, Some(&keyframe));

        let mut expected = Game::new(seed);
        for inp in &inputs {
            inp.apply_to_game(&mut expected);
        }
        assert_eq!(
            p.game().snapshot_bytes(),
            expected.snapshot_bytes(),
            "reconciled state must keep the unacked inputs (no snap-back)"
        );
    }

    #[test]
    fn input_frame_matches_the_wire() {
        assert_eq!(
            input_frame(7, &Input::MoveLeft),
            r#"{"type":"input","seq":7,"input":"MoveLeft"}"#
        );
        assert_eq!(
            input_frame(2, &Input::LaunchWeapon(3)),
            r#"{"type":"input","seq":2,"input":{"LaunchWeapon":3}}"#
        );
    }

    #[test]
    fn leave_bazaar_is_sent_but_not_applied_locally() {
        // Predict a leave while authoritatively in the bazaar (barrier up): it must be
        // forwarded (Some) but must NOT leave the local sim.
        let mut p = Predictor::new(1);
        p.on_snapshot(0, true, false, None); // server says we're shopping
        assert!(p.barrier());
        // Force the local sim into the bazaar so we can observe it staying there.
        // (We can't easily reach the bazaar here without playing; instead assert the
        // send happens and the sim's bazaar flag is unchanged by the leave.)
        let before = p.game().is_in_bazaar();
        let sent = p.predict(Input::LeaveBazaar);
        assert!(sent.is_some(), "LeaveBazaar must be forwarded");
        assert_eq!(p.game().is_in_bazaar(), before, "LeaveBazaar must not be applied locally");
    }

    #[test]
    fn gameplay_input_suppressed_under_barrier() {
        let mut p = Predictor::new(1);
        p.on_snapshot(0, false, true, None); // opponent shopping → barrier up
        assert!(p.predict(Input::MoveLeft).is_none(), "movement blocked under barrier");
        assert_eq!(p.input_seq(), 0, "nothing sent");
    }

    /// Drive a fresh game across the bazaar line threshold using the opponent-lines
    /// path (`combined = op_lines + lines`, fires when it crosses a multiple of
    /// `BT_LINES_TIL_BAZ` = 20). Two `receive_op_score` calls (19 then 20) set the
    /// countdown to 1 and then trip it, which is the same `update_bazaar` crossing a
    /// real bout takes. This is how the local sim enters the bazaar on its own.
    fn drive_local_into_bazaar(p: &mut Predictor) {
        p.game_mut().receive_op_score(0, 19, 0);
        p.game_mut().receive_op_score(0, 20, 0);
        assert!(p.game().is_in_bazaar(), "setup: local sim should be shopping");
    }

    #[test]
    fn bazaar_exit_without_keyframe_releases_the_local_sim() {
        // The model-B regression guard. The local sim entered the bazaar on its own,
        // never applies LeaveBazaar locally, and (with periodic keyframes retired) the
        // server's release now arrives as a plain snapshot. The `you_bazaar` true→false
        // edge must release the local sim, or it would stay frozen forever.
        let mut p = Predictor::new(0xBA2AA2);
        drive_local_into_bazaar(&mut p);

        // Server confirms we're shopping (no keyframe). Barrier up; still in the bazaar.
        p.on_snapshot(0, true, false, None);
        assert!(p.barrier(), "barrier up while shopping");
        assert!(p.game().is_in_bazaar(), "local sim still shopping under the barrier");

        // Server reopens play with a plain snapshot (no keyframe to restore us out).
        p.on_snapshot(0, false, false, None);
        assert!(!p.barrier(), "barrier cleared");
        assert!(
            !p.game().is_in_bazaar(),
            "the bazaar-exit edge must release the local sim without a keyframe"
        );
    }

    #[test]
    fn bazaar_exit_via_keyframe_still_releases() {
        // The pre-existing path: when a keyframe IS present on the exit, the restore
        // itself carries the not-shopping state, so the manual leave must not be needed
        // (and the else-branch is correctly skipped). Guards against the new edge
        // double-handling or fighting the restore.
        let seed = 0x5EED_F00D;
        let mut p = Predictor::new(seed);
        drive_local_into_bazaar(&mut p);
        p.on_snapshot(0, true, false, None);
        assert!(p.game().is_in_bazaar());

        // A keyframe of a fresh (not-shopping) game releases the sim via restore.
        let authoritative = Game::new(seed).snapshot_bytes();
        p.on_snapshot(0, false, false, Some(&authoritative));
        assert!(!p.game().is_in_bazaar(), "keyframe restore leaves the bazaar");
    }

    #[test]
    fn no_spurious_leave_when_never_in_bazaar() {
        // A you_bazaar true→false edge while the local sim was never shopping must be a
        // no-op: `leave_bazaar` is gated on `game.is_in_bazaar()`, so a stray edge can't
        // corrupt a normally-playing sim.
        let mut p = Predictor::new(7);
        assert!(!p.game().is_in_bazaar());
        p.on_snapshot(0, true, false, None); // server claims shopping; local sim is not
        p.on_snapshot(0, false, false, None); // exit edge
        assert!(!p.game().is_in_bazaar(), "still playing; nothing to leave");
    }
}

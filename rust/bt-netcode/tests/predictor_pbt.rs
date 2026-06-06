//! Property tests for the shared [`Predictor`] — the prediction/reconciliation
//! core both the browser (`WasmClient`) and the bot (`bt-bot`) run.
//!
//! The headline is the **snap-back invariant** (`unacked_inputs_survive_keyframe_*`):
//! the bug that started this whole exercise was a predicted-but-unacked input being
//! lost on reconciliation, so the dropping piece "snapped back". These properties
//! pin that it can't happen: a keyframe acking only a prefix must still leave the
//! local state equal to "all my inputs applied" — the unacked tail is replayed, not
//! dropped.

use bt_core::Game;
use bt_netcode::{input_frame, Predictor};
use bt_replay::Input;
use proptest::prelude::*;

/// The gameplay inputs a client predicts every tick (no bazaar shopping). All are
/// always-forwarded (a no-op move at the wall still sends), so the predictor's sent
/// stream equals the generated stream — which keeps the model below exact.
fn any_movement() -> impl Strategy<Value = Input> {
    prop_oneof![
        Just(Input::MoveLeft),
        Just(Input::MoveRight),
        Just(Input::Rotate),
        Just(Input::SoftDrop),
        Just(Input::BeginDrop),
        (0u32..3).prop_map(Input::LaunchWeapon),
    ]
}

/// Every `Input` variant, for the wire-format round-trip.
fn any_input() -> impl Strategy<Value = Input> {
    prop_oneof![
        Just(Input::MoveLeft),
        Just(Input::MoveRight),
        Just(Input::Rotate),
        Just(Input::BeginDrop),
        Just(Input::AiDrop),
        Just(Input::SoftDrop),
        (0u32..10).prop_map(Input::LaunchWeapon),
        (0i32..34).prop_map(Input::BuyWeapon),
        (0i32..34).prop_map(Input::SellWeapon),
        Just(Input::LeaveBazaar),
        any::<bool>().prop_map(Input::SetPaused),
        (0i32..34).prop_map(Input::ReceiveWeapon),
        any::<i64>().prop_map(Input::AddFunds),
    ]
}

/// Apply a run of inputs to a fresh game seeded with `seed` (the authoritative path).
fn game_after(seed: u64, inputs: &[Input]) -> Game {
    let mut g = Game::new(seed);
    for i in inputs {
        i.apply_to_game(&mut g);
    }
    g
}

proptest! {
    /// THE snap-back invariant. Predict N inputs, then receive ONE keyframe that acks
    /// only the first `k` of them (the authoritative state after applying that prefix).
    /// The reconciled local state must equal "all N inputs applied" — the unacked tail
    /// (k+1..N) is replayed on top of the keyframe, never dropped.
    #[test]
    fn unacked_inputs_survive_a_keyframe(
        seed in any::<u32>(),
        inputs in prop::collection::vec(any_movement(), 0..40),
        ack in 0u64..50,
    ) {
        let seed = seed as u64;
        let mut p = Predictor::new(seed);
        // No bazaar barrier here, so every movement input is forwarded: sent == inputs.
        let sent: Vec<Input> = inputs.iter()
            .filter_map(|i| p.predict(i.clone()).map(|(_, s)| s))
            .collect();
        prop_assert_eq!(sent.len(), inputs.len(), "movement is always forwarded");

        let n = sent.len() as u64;
        let k = ack.min(n);
        // The server has applied the first k sent inputs; the keyframe is that state.
        let keyframe = game_after(seed, &sent[..k as usize]).snapshot_bytes();
        p.on_snapshot(k, false, false, Some(&keyframe));

        // Local state must match all N applied — nothing lost in reconciliation.
        let expected = game_after(seed, &sent);
        prop_assert_eq!(
            p.game().snapshot_bytes(),
            expected.snapshot_bytes(),
            "reconciled state dropped an unacked input (snap-back) at k={}/{}", k, n
        );
    }

    /// The same invariant under a STREAM of keyframes with monotonically rising acks,
    /// interleaved with fresh predictions — the real frame-by-frame loop. After each
    /// reconciliation the local state still equals "every input I've predicted so far".
    #[test]
    fn incremental_keyframes_keep_converging(
        seed in any::<u32>(),
        // Each step: a batch of new inputs, then an ack advancing by this much.
        steps in prop::collection::vec(
            (prop::collection::vec(any_movement(), 0..6), 0u64..5),
            0..15,
        ),
    ) {
        let seed = seed as u64;
        let mut p = Predictor::new(seed);
        let mut sent: Vec<Input> = Vec::new();
        let mut ack: u64 = 0;

        for (batch, ack_step) in steps {
            for i in &batch {
                if let Some((_, s)) = p.predict(i.clone()) {
                    sent.push(s);
                }
            }
            // The server can only have applied inputs we've actually sent.
            ack = (ack + ack_step).min(sent.len() as u64);
            let keyframe = game_after(seed, &sent[..ack as usize]).snapshot_bytes();
            p.on_snapshot(ack, false, false, Some(&keyframe));

            let expected = game_after(seed, &sent);
            prop_assert_eq!(
                p.game().snapshot_bytes(),
                expected.snapshot_bytes(),
                "diverged after a mid-stream keyframe (ack={}, sent={})", ack, sent.len()
            );
        }
    }

    /// Ack accounting: after any series of snapshots, the unacked count is exactly the
    /// inputs whose seq exceeds the highest ack seen — a duplicate or lower ack never
    /// resurrects an already-pruned input. (No keyframes: pure pruning.)
    #[test]
    fn acks_prune_and_never_resurrect(
        seed in any::<u32>(),
        inputs in prop::collection::vec(any_movement(), 0..30),
        acks in prop::collection::vec(0u64..40, 0..12),
    ) {
        let mut p = Predictor::new(seed as u64);
        let sent = inputs.iter().filter(|i| p.predict((*i).clone()).is_some()).count() as u64;

        let mut prev_unacked = sent as usize; // unacked never grows
        let mut max_ack = 0u64;
        for a in acks {
            p.on_snapshot(a, false, false, None);
            max_ack = max_ack.max(a);
            let unacked = p.unacked_len();
            prop_assert!(unacked <= prev_unacked, "unacked grew: {} -> {}", prev_unacked, unacked);
            prop_assert_eq!(
                unacked as u64,
                sent.saturating_sub(max_ack.min(sent)),
                "unacked != inputs above the high-water ack {}", max_ack
            );
            prev_unacked = unacked;
        }
    }

    /// The bazaar barrier gate at predict time: while EITHER side is shopping, a
    /// gameplay input is suppressed (nothing applied, nothing sent, seq unchanged);
    /// with no barrier it's forwarded. (P3-style: never drive a frozen match.)
    #[test]
    fn movement_is_gated_under_the_barrier(
        seed in any::<u32>(),
        you in any::<bool>(),
        opp in any::<bool>(),
    ) {
        let mut p = Predictor::new(seed as u64);
        p.on_snapshot(0, you, opp, None);
        prop_assert_eq!(p.barrier(), you || opp);

        let before = p.input_seq();
        let sent = p.predict(Input::MoveLeft);
        if you || opp {
            prop_assert!(sent.is_none(), "movement leaked past the barrier");
            prop_assert_eq!(p.input_seq(), before, "a gated input must not bump the seq");
        } else {
            prop_assert!(sent.is_some(), "movement must be forwarded with no barrier");
            prop_assert_eq!(p.input_seq(), before + 1);
        }
    }

    /// Determinism — the structural basis for browser/bot consistency. Two predictors
    /// fed the IDENTICAL stream of predict()/on_snapshot() calls end in identical
    /// state. Because the browser (WasmClient) and the bot (bt-bot) both drive THIS
    /// one Predictor, identical inputs ⇒ identical local state on both: there is no
    /// second reconciliation implementation that could drift.
    #[test]
    fn predictor_is_deterministic(
        seed in any::<u32>(),
        inputs in prop::collection::vec(any_movement(), 0..40),
        ack in 0u64..50,
    ) {
        let seed = seed as u64;
        let drive = |p: &mut Predictor| -> Vec<Input> {
            let sent: Vec<Input> = inputs.iter()
                .filter_map(|i| p.predict(i.clone()).map(|(_, s)| s))
                .collect();
            let k = ack.min(sent.len() as u64);
            let kf = game_after(seed, &sent[..k as usize]).snapshot_bytes();
            p.on_snapshot(k, false, false, Some(&kf));
            sent
        };
        let mut a = Predictor::new(seed);
        let mut b = Predictor::new(seed);
        let sa = drive(&mut a);
        let sb = drive(&mut b);
        prop_assert_eq!(sa, sb, "the sent stream differed across identical runs");
        prop_assert_eq!(a.game().snapshot_bytes(), b.game().snapshot_bytes());
        prop_assert_eq!(a.input_seq(), b.input_seq());
        prop_assert_eq!(a.unacked_len(), b.unacked_len());
    }

    /// The wire frame is exactly `{"type":"input","seq":N,"input":<Input serde>}` and
    /// round-trips: the embedded input parses back to the same `Input`. One builder
    /// (shared by browser + bot) means the two can't disagree on the wire.
    #[test]
    fn input_frame_round_trips(seq in 0u64..1_000_000, input in any_input()) {
        let frame = input_frame(seq, &input);
        let v: serde_json::Value = serde_json::from_str(&frame).expect("frame is valid JSON");
        prop_assert_eq!(v.get("type").and_then(|t| t.as_str()), Some("input"));
        prop_assert_eq!(v.get("seq").and_then(|s| s.as_u64()), Some(seq));
        let parsed: Input = serde_json::from_value(v.get("input").cloned().unwrap())
            .expect("input field parses back to an Input");
        prop_assert_eq!(parsed, input);
    }
}

//! Convergence property for the model-B event channel: a client whose local sim is
//! fed the server's cross-player events (and no keyframe) stays bit-identical to the
//! server's copy of that side, measured by the per-lock state hash.
//!
//! Setup: a `Versus` is the server (it owns both boards and runs the relay). A
//! `Predictor` is the client for side A; its local game starts from A's seed. Each
//! tick the player's own inputs go to BOTH the server's game A and the client, the
//! opponent plays and launches weapons at A on the server only, and the relay's
//! outbox for A is forwarded to the client as `apply_event` calls (the same `Input`s
//! the host would send). No keyframe is ever applied. The property: A's `lock_seq` and
//! `lock_hash` on the client equal the server's at every lock.

use bt_core::{Side, Versus};
use bt_netcode::Predictor;
use bt_replay::Input;
use proptest::prelude::*;

// Weapons the opponent throws at A. A mix that touches A's board (garbage lines, weird
// pieces, removed cells), its pending queue (Keating), and its funds (Mondale, an
// AddFunds event). Spies and Swap/Susan are excluded: spies do not hit the victim, and
// Swap/Susan ride keyframes (not events), which this no-keyframe test does not cover.
const OPP_WEAPONS: &[i32] = &[
    0,  // FearedWeird
    4,  // FallOut
    6,  // Lawyers
    7,  // RiseUp
    10, // Missing
    11, // PieceIt
    13, // Mondale (funds tax -> AddFunds event)
    14, // Keating (queued on victim + funds)
    28, // Mirror (curses A; A's later launches would backfire)
];

#[derive(Debug, Clone)]
enum Op {
    PLeft,
    PRight,
    PRotate,
    PDrop,
    PLaunch(usize), // A launches arsenal slot 0 (exercises Mirror backfire onto A)
    OppDrop,
    OppLaunch(usize), // index into OPP_WEAPONS
    Tick,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        10 => Just(Op::Tick),
        2 => Just(Op::PLeft),
        2 => Just(Op::PRight),
        2 => Just(Op::PRotate),
        3 => Just(Op::PDrop),
        1 => (0usize..3).prop_map(Op::PLaunch),
        3 => Just(Op::OppDrop),
        4 => (0..OPP_WEAPONS.len()).prop_map(Op::OppLaunch),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1500))]

    #[test]
    fn event_channel_keeps_a_client_in_lockstep_without_keyframes(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
        ops in prop::collection::vec(op(), 0..300),
    ) {
        let mut server = Versus::new(seed_a, seed_b);
        // The client for side A: its local game starts from A's seed, like the server.
        let mut client = Predictor::new(seed_a);

        for o in &ops {
            // The bazaar barrier freezes the whole match. Under model B the client
            // mirrors it from the snapshot's `you_bazaar`/`opp_bazaar` flags (no
            // keyframe), and the local sim, which entered the bazaar on its own when its
            // combined lines crossed the threshold, is released by the `you_bazaar`
            // true->false edge rather than a keyframe restore. Drive one full cycle here:
            // tell the client it is shopping, resolve the barrier (both players finish
            // and leave), forward the release, and assert the client left the bazaar AND
            // stayed in lock-sync across the whole cycle without ever seeing a keyframe.
            if server.game(Side::A).is_in_bazaar() || server.game(Side::B).is_in_bazaar() {
                client.on_snapshot(
                    client.input_seq(),
                    server.game(Side::A).is_in_bazaar(),
                    server.game(Side::B).is_in_bazaar(),
                    None,
                );
                // Both sides finish shopping and leave; the server reopens play.
                server.game_mut(Side::A).leave_bazaar();
                server.game_mut(Side::B).leave_bazaar();
                client.on_snapshot(client.input_seq(), false, false, None);
                prop_assert!(
                    !client.game().is_in_bazaar(),
                    "client local sim stuck in the bazaar after the server reopened play"
                );
                prop_assert_eq!(
                    client.game().lock_seq(), server.game(Side::A).lock_seq(),
                    "lock_seq diverged across the bazaar"
                );
                prop_assert_eq!(
                    client.game().lock_hash(), server.game(Side::A).lock_hash(),
                    "lock_hash diverged across the bazaar"
                );
                continue;
            }
            match o {
                // Player inputs: apply to BOTH the server's game A and the client.
                Op::PLeft => { server.game_mut(Side::A).move_left(); client.predict(Input::MoveLeft); }
                Op::PRight => { server.game_mut(Side::A).move_right(); client.predict(Input::MoveRight); }
                Op::PRotate => { server.game_mut(Side::A).rotate(); client.predict(Input::Rotate); }
                Op::PDrop => { server.game_mut(Side::A).begin_drop(); client.predict(Input::BeginDrop); }
                Op::PLaunch(slot) => {
                    // Grant the same weapon on both so the launch is symmetric, then fire
                    // it. Its cross-player effect (delivery to B, or a Mirror backfire onto
                    // A) is resolved by the server relay and reaches the client as an event.
                    let tok = 16; // Reagan: a queued board/funds weapon, fine on either side
                    server.game_mut(Side::A).grant_weapon(bt_core::WeaponToken::from_index(tok).unwrap());
                    client.game_mut().grant_weapon(bt_core::WeaponToken::from_index(tok).unwrap());
                    server.game_mut(Side::A).launch_weapon(*slot);
                    client.predict(Input::LaunchWeapon(*slot as u32));
                }
                // Opponent (server only): play and launch weapons at A.
                Op::OppDrop => { server.game_mut(Side::B).begin_drop(); }
                Op::OppLaunch(i) => {
                    let tok = bt_core::WeaponToken::from_index(OPP_WEAPONS[*i]).unwrap();
                    server.game_mut(Side::B).grant_weapon(tok);
                    server.game_mut(Side::B).launch_weapon(0);
                }
                Op::Tick => {
                    server.tick(16);
                    client.tick(16);
                    // Forward the relay's events for A to the client, in order.
                    for e in server.take_outbox(Side::A) {
                        client.apply_event(&Input::from(e));
                    }
                }
            }

            prop_assert_eq!(
                client.game().lock_seq(), server.game(Side::A).lock_seq(),
                "lock_seq diverged"
            );
            prop_assert_eq!(
                client.game().lock_hash(), server.game(Side::A).lock_hash(),
                "lock_hash diverged: the event channel did not keep the client in lockstep"
            );
        }
    }
}

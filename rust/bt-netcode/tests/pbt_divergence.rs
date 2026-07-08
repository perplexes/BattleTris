//! Property tests for `Predictor::on_lock_hash`, the model-B divergence detector.
//!
//! Reuses the `pbt_convergence.rs` harness (a `Versus` server plus a `Predictor`
//! client for side A) and adds the authoritative-pair judgement on top of it. Two
//! properties:
//!
//! - no false positives: when every cross-player event is forwarded promptly (the
//!   exact convergence scenario), `on_lock_hash` must never report a divergence.
//! - fault injection: when one opponent weapon event is deliberately held back and
//!   delivered to the client a lock late, the resulting real state divergence must be
//!   caught within a small, bounded number of subsequent locks.
//!
//! The fault-injection property is guarded against passing vacuously by
//! `fault_injection_path_is_reachable`, a plain (non-random) test that pins down one
//! concrete case which must both produce a genuine hash difference and get caught.

use bt_core::{Side, Versus, WeaponToken};
use bt_netcode::Predictor;
use bt_replay::Input;
use proptest::prelude::*;

// Weapons the opponent throws at A. Same set as `pbt_convergence.rs`'s OPP_WEAPONS
// (see its comment for why spies and Swap/Susan are excluded).
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

/// The deliberately delayed event always uses one of these. Both mutate the victim's
/// board directly when they activate, so a late delivery is virtually guaranteed to
/// produce a real `lock_hash` difference at the victim's next lock, unlike, say, a pure
/// `AddFunds`, which the hash excludes entirely (see `Game::lock_hash_of`'s doc).
const SKEW_WEAPONS: [WeaponToken; 2] = [WeaponToken::FallOut, WeaponToken::RiseUp];

#[derive(Debug, Clone)]
enum Op {
    PLeft,
    PRight,
    PRotate,
    PDrop,
    PLaunch(usize),
    OppDrop,
    OppLaunch(usize),
    Tick,
}

/// The full op mix, identical in shape to `pbt_convergence.rs`'s `op()`. Used for the
/// no-false-positives property and for the pre-skew phase of the fault-injection
/// property (getting the two sims into a varied, weapon-touched mid-game state before
/// the deliberate fault).
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

/// Drops and ticks only, no further weapon launches. Used for the observation window
/// after the deliberate skew, so exactly one held-back event is ever in flight and the
/// divergence it causes isn't confounded by additional cross-player traffic.
fn post_op() -> impl Strategy<Value = Op> {
    prop_oneof![
        10 => Just(Op::Tick),
        3 => Just(Op::PDrop),
        3 => Just(Op::OppDrop),
    ]
}

/// Apply one op to the server/client pair, mirroring `pbt_convergence.rs`'s per-op
/// dispatch (including its bazaar-cycle handling). `held`, when `Some`, holds cross-
/// player events for A that a `Tick` has already taken from the server's outbox but
/// deliberately not forwarded yet; they are released once the client has locked at
/// least once since they started being held (`held_since_lock`), and freshly-arriving
/// outbox events keep being forwarded immediately regardless, so only the one
/// deliberately-delayed batch is ever late.
fn apply_op(
    server: &mut Versus,
    client: &mut Predictor,
    op: &Op,
    held: &mut Option<Vec<Input>>,
    held_since_lock: &mut u64,
) {
    if server.game(Side::A).is_in_bazaar() || server.game(Side::B).is_in_bazaar() {
        client.on_snapshot(
            client.input_seq(),
            server.game(Side::A).is_in_bazaar(),
            server.game(Side::B).is_in_bazaar(),
            None,
        );
        server.game_mut(Side::A).leave_bazaar();
        server.game_mut(Side::B).leave_bazaar();
        client.on_snapshot(client.input_seq(), false, false, None);
        return;
    }
    match op {
        Op::PLeft => {
            server.game_mut(Side::A).move_left();
            client.predict(Input::MoveLeft);
        }
        Op::PRight => {
            server.game_mut(Side::A).move_right();
            client.predict(Input::MoveRight);
        }
        Op::PRotate => {
            server.game_mut(Side::A).rotate();
            client.predict(Input::Rotate);
        }
        Op::PDrop => {
            server.game_mut(Side::A).begin_drop();
            client.predict(Input::BeginDrop);
        }
        Op::PLaunch(slot) => {
            let tok = 16; // Reagan: a queued board/funds weapon, fine on either side
            server.game_mut(Side::A).grant_weapon(WeaponToken::from_index(tok).unwrap());
            client.game_mut().grant_weapon(WeaponToken::from_index(tok).unwrap());
            server.game_mut(Side::A).launch_weapon(*slot);
            client.predict(Input::LaunchWeapon(*slot as u32));
        }
        Op::OppDrop => {
            server.game_mut(Side::B).begin_drop();
        }
        Op::OppLaunch(i) => {
            let tok = WeaponToken::from_index(OPP_WEAPONS[*i]).unwrap();
            server.game_mut(Side::B).grant_weapon(tok);
            server.game_mut(Side::B).launch_weapon(0);
        }
        Op::Tick => {
            server.tick(16);
            client.tick(16);
            for e in server.take_outbox(Side::A) {
                client.apply_event(&Input::from(e));
            }
            if held.as_ref().is_some_and(|h| !h.is_empty())
                && client.game().lock_seq() > *held_since_lock
            {
                for e in held.take().unwrap() {
                    client.apply_event(&e);
                }
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(600))]

    #[test]
    fn no_false_positives_when_events_are_forwarded_promptly(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
        ops in prop::collection::vec(op(), 0..300),
    ) {
        let mut server = Versus::new(seed_a, seed_b);
        let mut client = Predictor::new(seed_a);
        let mut held: Option<Vec<Input>> = None;
        let mut held_since_lock = 0u64;

        for o in &ops {
            apply_op(&mut server, &mut client, o, &mut held, &mut held_since_lock);
            prop_assert!(
                !client.on_lock_hash(server.game(Side::A).lock_seq(), server.game(Side::A).lock_hash()),
                "on_lock_hash must never fire when every event is forwarded promptly"
            );
        }
    }
}

/// One fault-injection run's outcome, checked by both the property test and the
/// deterministic coverage guard below it.
struct FaultRun {
    /// The client's and server's hashes were directly observed to differ at two
    /// consecutive shared lock_seqs: a real, sustained divergence, not a single
    /// straddled-event blip that self-heals (which `on_lock_hash` correctly does not
    /// report; see its doc). This is the ground truth the property expects
    /// `on_lock_hash` to have caught.
    mismatch_seen: bool,
    /// `on_lock_hash` returned true at least once after the skew was injected.
    detected: bool,
}

/// Mutable state threaded through the post-skew observation loop, tracking ground
/// truth independently of `on_lock_hash`'s own rate limiting so the property can tell
/// a real, sustained divergence apart from a one-lock blip that self-heals (which
/// `on_lock_hash` is intentionally designed to swallow: see its doc).
#[derive(Default)]
struct Tracker {
    detected: bool,
    /// The client's `lock_seq` last time a new-lock check ran, to notice exactly when
    /// a lock has just happened rather than re-checking the same lock repeatedly.
    prev_client_lock: u64,
    /// Consecutive *distinct* shared lock_seqs at which the client's own hash differed
    /// from the server's, mirroring `on_lock_hash`'s internal two-strike counter. Reset
    /// to 0 by a matching shared lock (a real reconvergence), exactly like the
    /// production counter.
    mismatch_streak: u32,
    /// Set once `mismatch_streak` first reaches 2: a real, sustained divergence, not a
    /// one-off. Counts down the 6-lock detection budget from that point.
    budget: Option<u32>,
}

/// Judge the server's current authoritative pair against the client and update the
/// ground-truth streak. Panics (failing the test at the exact op) if a sustained
/// divergence outlives its 6-lock detection budget.
fn judge_and_track(server: &Versus, client: &mut Predictor, t: &mut Tracker) {
    let seq = server.game(Side::A).lock_seq();
    let hash = server.game(Side::A).lock_hash();
    if client.on_lock_hash(seq, hash) {
        t.detected = true;
    }

    let cur_client_lock = client.game().lock_seq();
    if cur_client_lock != t.prev_client_lock {
        t.prev_client_lock = cur_client_lock;
        if cur_client_lock == seq {
            // A newly-reached, shared lock_seq: this is ground truth, independent of
            // on_lock_hash's own rate limiting.
            if client.game().lock_hash() != hash {
                t.mismatch_streak += 1;
            } else {
                t.mismatch_streak = 0;
            }
        }
        if t.mismatch_streak >= 2 && t.budget.is_none() {
            t.budget = Some(6);
        }
        if let Some(b) = t.budget.as_mut() {
            assert!(
                t.detected || *b > 0,
                "a sustained divergence was not caught within 6 subsequent locks"
            );
            if !t.detected {
                *b -= 1;
            }
        }
    }
}

/// Run `pre_ops`, then deliberately delay one opponent-weapon event (`skew`, launched
/// from B at A) to the client until at least one client lock has happened since it was
/// held, then play `post_ops` (drops and ticks only) while judging the server's
/// authoritative pair against the client after every op.
///
/// Asserts inline (so the failure points at the exact op) that once the client's own
/// hash has genuinely, and repeatedly, disagreed with the server's at a shared lock
/// (two in a row: a real divergence rather than a single straddled-event blip that
/// self-heals), `on_lock_hash` reports it within 6 subsequent client locks.
fn run_fault_injection(
    seed_a: u64,
    seed_b: u64,
    pre_ops: &[Op],
    skew: WeaponToken,
    post_ops: &[Op],
) -> FaultRun {
    let mut server = Versus::new(seed_a, seed_b);
    let mut client = Predictor::new(seed_a);
    let mut held: Option<Vec<Input>> = None;
    let mut held_since_lock = 0u64;

    for o in pre_ops {
        apply_op(&mut server, &mut client, o, &mut held, &mut held_since_lock);
        assert!(
            !client.on_lock_hash(server.game(Side::A).lock_seq(), server.game(Side::A).lock_hash()),
            "pre-fault phase must not diverge (it is the no-false-positives scenario)"
        );
    }

    // Inject the skew: B attacks A with a board-affecting weapon. The relay applies it
    // to the server's copy of A's pending queue immediately (inside this tick), but the
    // matching event is held back from the client rather than forwarded.
    server.game_mut(Side::B).grant_weapon(skew);
    server.game_mut(Side::B).launch_weapon(0);
    server.tick(16);
    client.tick(16);
    let skew_events: Vec<Input> = server.take_outbox(Side::A).into_iter().map(Input::from).collect();
    if !skew_events.is_empty() {
        held_since_lock = client.game().lock_seq();
        held = Some(skew_events);
    }

    let mut t = Tracker { prev_client_lock: client.game().lock_seq(), ..Tracker::default() };
    judge_and_track(&server, &mut client, &mut t);

    for o in post_ops {
        apply_op(&mut server, &mut client, o, &mut held, &mut held_since_lock);
        judge_and_track(&server, &mut client, &mut t);
    }

    FaultRun { mismatch_seen: t.mismatch_streak >= 2 || t.budget.is_some(), detected: t.detected }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn fault_injection_is_caught_when_it_actually_diverges(
        seed_a in any::<u64>(),
        seed_b in any::<u64>(),
        pre_ops in prop::collection::vec(op(), 0..120),
        skew_idx in 0usize..SKEW_WEAPONS.len(),
        post_ops in prop::collection::vec(post_op(), 40..200),
    ) {
        // The bounded-detection assertion lives inside `run_fault_injection` so a
        // failure points at the exact op that missed the 6-lock window. When the skew
        // never produces a genuine difference (mismatch_seen stays false) there is
        // nothing more to check for this case; the coverage guard below pins down that
        // the productive branch is reachable at all.
        let _ = run_fault_injection(seed_a, seed_b, &pre_ops, SKEW_WEAPONS[skew_idx], &post_ops);
    }
}

#[test]
fn fault_injection_path_is_reachable() {
    // A concrete, deterministic case (no proptest shrinking or case selection involved)
    // that must both produce a genuine hash difference and get caught. Guards against
    // `fault_injection_is_caught_when_it_actually_diverges` vacuously passing 400 cases
    // in which `mismatch_seen` never actually turns true (e.g. a harness bug that made
    // the skew inert would show up here as `mismatch_seen == false`, not as a silent,
    // uninteresting pass).
    let mut post_ops = Vec::new();
    for _ in 0..8 {
        post_ops.push(Op::PDrop);
        for _ in 0..40 {
            post_ops.push(Op::Tick);
        }
    }
    let outcome = run_fault_injection(1, 2, &[], WeaponToken::RiseUp, &post_ops);
    assert!(outcome.mismatch_seen, "the skew must produce a genuine hash difference in this case");
    assert!(outcome.detected, "on_lock_hash must have caught it");
}

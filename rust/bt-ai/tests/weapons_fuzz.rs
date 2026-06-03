//! Weapons layer 3 — fuzz harness.
//!
//! There's no oracle to diff weapon storms against, so this is property-fuzzing:
//! drive Ernie while injecting random weapons at random times, and after every
//! single frame assert a set of GLOBAL invariants that must hold no matter what
//! combination of effects is stacked on the board. The point is the
//! combinatorial space no hand-written test reaches — overlapping durations,
//! Upbyside-while-FallOut, Twilight-over-Gimp, expiry mid-cascade — netting
//! panics, out-of-bounds, and state corruption.
//!
//! Everything is seeded, so any failure reproduces exactly (and could be
//! re-emitted as a watchable replay — see layer 4).

use bt_ai::Computer;
use bt_core::game::GameEvent;
use bt_core::rng::Rng;
use bt_core::weapons::WeaponToken;
use bt_core::{Board, Game};

fn cell_count(b: &Board) -> usize {
    (0..b.height)
        .flat_map(|y| (0..b.width).map(move |x| (x, y)))
        .filter(|&(x, y)| b.get(x, y).is_some())
        .count()
}

fn board_hash(b: &Board) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for y in 0..b.height {
        for x in 0..b.width {
            let v = b.get(x, y).map(|c| (c.id() as i64 as u64).wrapping_add(7)).unwrap_or(0);
            for byte in [x as u64, y as u64, v] {
                h ^= byte;
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
    h
}

/// One fuzz run: Ernie plays while random weapons rain in. Returns a final
/// fingerprint so callers can assert determinism. Panics (failing the test) if
/// any invariant is violated.
fn fuzz_run(seed: u64, steps: usize) -> (i64, i64, u64) {
    let mut g = Game::new(seed);
    let mut ernie = Computer::new();
    let mut rng = Rng::new(seed ^ 0xF00D_BABE);
    let mut committed = false;

    let mut last_score = 0i64;
    let mut last_lines = 0i64;
    let cap = (g.board().width * g.board().height) as usize;

    for _ in 0..steps {
        // ~8% of frames, deliver a random weapon to the victim.
        if rng.rand_below(100) < 8 {
            let tok = WeaponToken::ALL[rng.rand_below(WeaponToken::ALL.len() as i32) as usize];
            g.receive_weapon(tok);
        }

        if g.is_in_bazaar() {
            g.leave_bazaar();
        }
        if !committed && g.current_piece().is_some() {
            ernie.take_turn(&mut g);
            committed = true;
        }
        g.tick(16);
        if g.take_events().iter().any(|e| matches!(e, GameEvent::Locked { .. })) {
            committed = false;
        }

        // ---- invariants (must hold under any stack of weapons) ----
        let b = g.board();
        let count = cell_count(b);
        assert!(count <= cap, "seed {seed}: {count} cells exceeds capacity {cap}");
        assert_eq!((b.width, b.height), (10, 28), "seed {seed}: board geometry mutated");

        let s = g.score();
        // The hard-drop score and line count only ever go up — no weapon
        // subtracts from them (funds, by contrast, can drop: Keating/Reagan).
        assert!(s.score >= last_score, "seed {seed}: score regressed {last_score}->{}", s.score);
        assert!(s.lines >= last_lines, "seed {seed}: lines regressed {last_lines}->{}", s.lines);
        last_score = s.score;
        last_lines = s.lines;

        if g.is_game_over() {
            break;
        }
    }

    (last_score, last_lines, board_hash(g.board()))
}

/// The storm itself: many seeds, each a few thousand frames of random weapons.
/// No invariant may break across the whole space.
#[test]
fn weapon_storms_preserve_invariants() {
    for seed in 0..40u64 {
        fuzz_run(seed.wrapping_mul(2_654_435_761), 3_000);
    }
}

/// A fuzz run is fully deterministic — same seed, same trajectory. This is what
/// makes any failure reproducible (and re-playable).
#[test]
fn fuzz_runs_are_deterministic() {
    for seed in [1u64, 42, 777, 1_000_003] {
        assert_eq!(
            fuzz_run(seed, 2_000),
            fuzz_run(seed, 2_000),
            "seed {seed}: fuzz run diverged across identical replays"
        );
    }
}

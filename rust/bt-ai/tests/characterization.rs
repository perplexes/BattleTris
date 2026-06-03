//! Characterization (snapshot) tests.
//!
//! These don't assert a *spec* — they pin the engine's current deterministic
//! behavior so that ANY unintended change to piece order, the AI heuristic,
//! scoring, timing, or line-clearing shows up as a failed fingerprint in code
//! review. A surprising number in the diff ("comatose Ernie scored 500 by
//! tick 9000?") is exactly the signal that catches a regression the property
//! tests didn't think to forbid.
//!
//! The fingerprint is `(ai_score, ai_lines, ai_funds, board_hash)` after a
//! fixed number of fixed-dt ticks with the human paused (so we characterize
//! Ernie in isolation). All inputs are deterministic, so these are stable.
//!
//! ## Regenerating
//! If a change is *intentional*, regenerate the baked values:
//!   cargo test -p bt-ai --test characterization -- --ignored --nocapture
//! then paste the printed `EXPECTED` block below. Review the diff like any
//! other — a faithful change moves these in an explainable way.

use bt_ai::VsComputer;
use bt_core::Board;

const DT: i32 = 16;

/// (label, seed, level, ticks)
const CONFIGS: &[(&str, u64, usize, usize)] = &[
    ("comatose", 12_345, 0, 9_000),
    ("willing", 2_024, 5, 9_000),
    ("pepped_up", 12_345, 9, 9_000),
    ("bionic", 777, 14, 6_000),
];

/// Baked fingerprints: (ai_score, ai_lines, ai_funds, board_hash), index-aligned
/// with CONFIGS. Regenerate with the --ignored generator below.
const EXPECTED: &[(i64, i64, i64, u64)] = &[
    (462, 7, 86, 12057399988636814063),    // comatose
    (1442, 18, 99, 8806918482345380397),   // willing
    (1302, 20, 14, 10445627093318506597),  // pepped_up
    (1050, 20, 6, 14926149424688899277),   // bionic
];

fn fnv1a_board(b: &Board) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mix = |x: u64, h: &mut u64| {
        *h ^= x;
        *h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for y in 0..b.height {
        for x in 0..b.width {
            let v = match b.get(x, y) {
                Some(c) => (c.id() as i64 as u64)
                    .wrapping_mul(31)
                    .wrapping_add(c.value() as u64)
                    .wrapping_add(1),
                None => 0,
            };
            mix(x as u64, &mut h);
            mix(y as u64, &mut h);
            mix(v, &mut h);
        }
    }
    h
}

fn fingerprint(seed: u64, level: usize, ticks: usize) -> (i64, i64, i64, u64) {
    let mut vs = VsComputer::new(seed, level);
    vs.player_mut().set_paused(true); // characterize Ernie alone
    for _ in 0..ticks {
        vs.tick(DT);
        if vs.result() != 0 {
            break; // topped out — a stable terminal state
        }
    }
    let s = vs.ai().score();
    (s.score, s.lines, s.funds, fnv1a_board(vs.ai().board()))
}

#[test]
fn ai_trajectories_match_baked_snapshots() {
    let actual: Vec<_> = CONFIGS
        .iter()
        .map(|&(_, seed, level, ticks)| fingerprint(seed, level, ticks))
        .collect();

    for (i, (&(label, ..), (&got, &want))) in CONFIGS
        .iter()
        .zip(actual.iter().zip(EXPECTED.iter()))
        .enumerate()
    {
        assert_eq!(
            got, want,
            "snapshot drift at config {i} ({label}): got {got:?}, expected {want:?}. \
             If this change is intentional, regenerate (see module docs)."
        );
    }
}

/// Generator: prints the EXPECTED block. Run with
///   cargo test -p bt-ai --test characterization -- --ignored --nocapture
#[test]
#[ignore]
fn generate_snapshots() {
    println!("const EXPECTED: &[(i64, i64, i64, u64)] = &[");
    for &(label, seed, level, ticks) in CONFIGS {
        let (s, l, f, h) = fingerprint(seed, level, ticks);
        println!("    ({s}, {l}, {f}, {h}), // {label}");
    }
    println!("];");
}

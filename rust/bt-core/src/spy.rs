//! Spy reveal math (Ames / Ace / Condor), shared by the authoritative server and
//! the local mock match.
//!
//! A spy reveals the opponent's board and funds to its holder, degraded to the
//! spy's accuracy. The board degradation itself is a per-frame flicker applied on
//! the client (it blanks [`hide_pct`] of the cells each frame); what lives here is
//! the per-spy hide percentage, the funds perturbation, and the deterministic noise
//! the funds perturbation draws from. Keeping it in `bt-core` lets the server
//! ([`bt_server::bout`]) and the wasm mock match compute identical reveals from one
//! implementation rather than two that can drift.

use crate::weapons::WeaponToken;

/// How many ticks after the opponent's tetris the Ace spy keeps perturbing the
/// revealed funds. The original perturbs on the single render following a tetris
/// (`tet_`, BTRecon.C:107-110); the reveal here rides throttled keyframe frames, so
/// it holds the perturbation for a short window to stay visible at that cadence.
pub const ACE_TETRIS_WINDOW: u64 = 60;

/// Percentage of a spy's cells the holder hides each frame (`1 - report_prob` from
/// BTRecon.C): Ames shows 50%, Ace 85%, Condor (satellite) is perfect (hides none).
pub fn hide_pct(token: WeaponToken) -> u32 {
    match token {
        WeaponToken::Ames => 50,
        WeaponToken::Ace => 15,
        _ => 0, // Condor
    }
}

/// A deterministic per-tick, per-side noise for the funds perturbation. The tick is
/// run through a splitmix64 finalizer (full avalanche) so consecutive frames give
/// uncorrelated noise, where `tick * constant` alone would leave a linear walk an
/// observer could read off. No RNG state, so a caller stays a pure function of the
/// tick.
pub fn noise_for(tick: u64, side: u64) -> u64 {
    let mut z = tick.wrapping_add(side.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The funds a spy reveals to its holder, mirroring `BTRecon::adjustFunds`
/// (BTRecon.C:94-118). Condor reveals exact funds; Ames perturbs by
/// `+/- (noise % (|funds|+1))`; Ace reveals exact funds except a `+/- (noise % 100)`
/// perturbation while the opponent's tetris is recent. Computing it server-side (and
/// in the mock the same way) keeps a modified client from reading a value the spy
/// did not grant: Ames never yields the exact figure, and Ace/Condor yield exact
/// only because that is the weapon's paid effect.
pub fn adjust_funds(funds: i64, token: WeaponToken, noise: u64, ace_recent_tetris: bool) -> i64 {
    // A pseudo-random sign drawn from a high bit of the noise (the original draws
    // `mult = (rand() % 2) ? -1 : 1`, BTRecon.C:97-99).
    let sign = if noise & (1 << 33) != 0 { -1 } else { 1 };
    match token {
        WeaponToken::Condor => funds,
        WeaponToken::Ace => {
            if ace_recent_tetris {
                funds + sign * (noise % 100) as i64
            } else {
                funds
            }
        }
        // Ames (the only remaining spy). The original remaps `funds == -1` to -2
        // because `rand() % (funds + 1)` would divide by zero there
        // (BTRecon.C:103-104). For any funds the span is `|funds| + 1`, so the
        // perturbation magnitude lies in `0..=|funds|`.
        _ => {
            let base = if funds == -1 { -2 } else { funds };
            let span = base.unsigned_abs() + 1; // |funds| + 1, always >= 1
            base + sign * (noise % span) as i64
        }
    }
}

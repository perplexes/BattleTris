//! Piece selection — the faithful analogue of `BTPieceManager`
//! (`usr/src/game/BTPieceManager.{H,C}`).
//!
//! Holds the per-piece "keep probabilities" and selects the next piece by
//! rejection sampling, honoring the weapons that change the piece stream
//! (Feared Weird, Four-by-Four, No Dice, So Long, Have a Nice Day, Broken).
//!
//! ## Contract (do not change these public signatures; fill in the bodies):
//!   * [`PieceManager::new`], [`PieceManager::reset`], [`PieceManager::create`],
//!     [`PieceManager::weapon_on`], [`PieceManager::weapon_off`].

use crate::piece::{Piece, PieceKind};
use crate::rng::Rng;
use crate::weapons::WeaponToken;
use crate::constants::*;

/// `BTPieceManager`. `keep_prob[i]` is indexed by the `BT_*_PIECE` id (1..=18);
/// index 0 is unused.
#[derive(Clone, Debug)]
pub struct PieceManager {
    /// `keep_prob_[BT_MAX_PIECES+1]`.
    keep_prob: [f64; 19],
    /// `hap_on_` — pending forced happy pieces (Have a Nice Day).
    hap_on: i32,
    /// `broken_` — Broken Record active.
    broken: bool,
    /// `old_piece_` — last piece id produced (for Broken Record repeats).
    old_piece: i32,
}

impl PieceManager {
    /// Raw internal state — for full-game keyframe serialization (client-server
    /// reconciliation). `(keep_prob, hap_on, broken, old_piece)`. Pair with
    /// [`PieceManager::set_raw`].
    pub fn raw(&self) -> ([f64; 19], i32, bool, i32) {
        (self.keep_prob, self.hap_on, self.broken, self.old_piece)
    }

    /// Restore the raw internal state captured by [`PieceManager::raw`].
    pub fn set_raw(&mut self, keep_prob: [f64; 19], hap_on: i32, broken: bool, old_piece: i32) {
        self.keep_prob = keep_prob;
        self.hap_on = hap_on;
        self.broken = broken;
        self.old_piece = old_piece;
    }

    /// `BTPieceManager::BTPieceManager` — install the default keep
    /// probabilities (standard pieces 0.21, die 1.0, happy & long-dong 0.02,
    /// weird/4x4 0.0).
    pub fn new() -> PieceManager {
        let mut keep_prob = [0.0; 19];

        // Standard pieces (1..=7): BT_DEFAULT_KEEP_PROB
        for i in BT_EL_PIECE..=BT_BOX_PIECE {
            keep_prob[i as usize] = BT_DEFAULT_KEEP_PROB;
        }

        // Die piece: 1.0
        keep_prob[BT_DIE_PIECE as usize] = BT_DIE_KEEP_PROB;

        // Happy piece: BT_EXOTIC_KEEP_PROB
        keep_prob[BT_HAP_PIECE as usize] = BT_EXOTIC_KEEP_PROB;

        // Weird pieces (10..=16): 0.0 (already initialized to 0)

        // Four-by-four piece (17): 0.0 (already initialized to 0)

        // Long-dong piece (18): BT_EXOTIC_KEEP_PROB
        keep_prob[BT_LONG_DONG_PIECE as usize] = BT_EXOTIC_KEEP_PROB;

        PieceManager {
            keep_prob,
            hap_on: 0,
            broken: false,
            old_piece: 0,
        }
    }

    /// `BT_START` handling in `BTPieceManager::receive` — reset to defaults.
    pub fn reset(&mut self) {
        // Standard pieces (1..=7): BT_DEFAULT_KEEP_PROB
        for i in BT_EL_PIECE..=BT_BOX_PIECE {
            self.keep_prob[i as usize] = BT_DEFAULT_KEEP_PROB;
        }

        // Die piece: 1.0
        self.keep_prob[BT_DIE_PIECE as usize] = BT_DIE_KEEP_PROB;

        // Happy piece: BT_EXOTIC_KEEP_PROB
        self.keep_prob[BT_HAP_PIECE as usize] = BT_EXOTIC_KEEP_PROB;

        // Weird pieces (10..=18): 0.0
        for i in (BT_WEIRD_OFFS + 1)..=BT_MAX_PIECES {
            self.keep_prob[i as usize] = 0.0;
        }

        // Long-dong piece (18): BT_EXOTIC_KEEP_PROB (override the 0.0 from above)
        self.keep_prob[BT_LONG_DONG_PIECE as usize] = BT_EXOTIC_KEEP_PROB;

        self.broken = false;
        self.hap_on = 0;
    }

    /// `BTPieceManager::create` — select and construct the next piece at
    /// `(x, y)`.
    ///
    /// Selection (faithful, incl. RNG consumption order):
    ///   * if `!hap_on && (!broken || (broken && rng.lrand48() % BT_BROKEN_PROB == 0))`:
    ///     loop { `i = rng.rand_below(BT_MAX_PIECES) + 1`; if `rng.drand48() < keep_prob[i]` break }
    ///   * else if `!hap_on && broken`: `i = old_piece`
    ///   * else: `hap_on -= 1`; `i = BT_HAP_PIECE`
    ///   then `old_piece = i`. The die's pip value is `rng.rand_below(6) + 1`,
    ///   drawn ONLY when `i == BT_DIE_PIECE` (matches `BTDiePiece::construct`).
    pub fn create(&mut self, rng: &mut Rng, x: i32, y: i32) -> Piece {
        let i = if self.hap_on == 0 && (!self.broken || (self.broken && rng.lrand48() % BT_BROKEN_PROB == 0)) {
            // Standard piece selection via rejection sampling
            loop {
                let candidate = rng.rand_below(BT_MAX_PIECES) + 1;
                if rng.drand48() < self.keep_prob[candidate as usize] {
                    break candidate;
                }
            }
        } else if self.hap_on == 0 && self.broken {
            // Broken Record: repeat the old piece. Guard the degenerate case
            // where Broken activated before any piece spawned (old_piece still
            // 0, an invalid id) — fall back to a valid piece instead of
            // panicking in `from_id`. Unreachable in real play (a Game always
            // spawns first), so this never changes RNG order or a live game.
            if self.old_piece != 0 {
                self.old_piece
            } else {
                BT_EL_PIECE
            }
        } else {
            // Happy piece forced
            self.hap_on -= 1;
            BT_HAP_PIECE
        };

        self.old_piece = i;

        // Convert id to PieceKind
        let kind = PieceKind::from_id(i).expect("Invalid piece id");

        // Compute die value ONLY if the kind is Die
        let die_value = if kind == PieceKind::Die {
            (rng.rand_below(6) + 1) as u8
        } else {
            1
        };

        Piece::construct(kind, x, y, die_value)
    }

    /// Weapon activation effects on the piece stream
    /// (`BTPieceManager::receive`, `BT_WPN_ON`).
    pub fn weapon_on(&mut self, w: WeaponToken) {
        match w {
            WeaponToken::FearedWeird => {
                // Standard pieces (1..=7): 0.0
                for i in BT_EL_PIECE..=BT_BOX_PIECE {
                    self.keep_prob[i as usize] = 0.0;
                }
                // Weird pieces (10..=16): BT_DEFAULT_KEEP_PROB
                for i in (BT_WEIRD_OFFS + 1)..=BT_WLONG_PIECE {
                    self.keep_prob[i as usize] = BT_DEFAULT_KEEP_PROB;
                }
            }
            WeaponToken::FourByFour => {
                self.keep_prob[BT_BOX_PIECE as usize] = 0.0;
                self.keep_prob[BT_4X4_PIECE as usize] = BT_DEFAULT_KEEP_PROB;
            }
            WeaponToken::Broken => {
                self.broken = true;
            }
            WeaponToken::NoDice => {
                self.keep_prob[BT_DIE_PIECE as usize] = 0.0;
            }
            WeaponToken::SoLong => {
                self.keep_prob[BT_LONG_PIECE as usize] = 0.0;
            }
            WeaponToken::NiceDay => {
                self.hap_on += 1;
            }
            _ => {
                // Other weapons have no effect on piece stream
            }
        }
    }

    /// Weapon deactivation effects (`BTPieceManager::receive`, `BT_WPN_OFF`).
    pub fn weapon_off(&mut self, w: WeaponToken) {
        match w {
            WeaponToken::FearedWeird => {
                // Standard pieces (1..=7): BT_DEFAULT_KEEP_PROB
                for i in BT_EL_PIECE..=BT_BOX_PIECE {
                    self.keep_prob[i as usize] = BT_DEFAULT_KEEP_PROB;
                }
                // Weird pieces (10..=16): 0.0
                for i in (BT_WEIRD_OFFS + 1)..=BT_WLONG_PIECE {
                    self.keep_prob[i as usize] = 0.0;
                }
            }
            WeaponToken::FourByFour => {
                self.keep_prob[BT_BOX_PIECE as usize] = BT_DEFAULT_KEEP_PROB;
                self.keep_prob[BT_4X4_PIECE as usize] = 0.0;
            }
            WeaponToken::NoDice => {
                self.keep_prob[BT_DIE_PIECE as usize] = BT_DIE_KEEP_PROB;
            }
            WeaponToken::SoLong => {
                self.keep_prob[BT_LONG_PIECE as usize] = BT_DEFAULT_KEEP_PROB;
            }
            WeaponToken::Broken => {
                self.broken = false;
            }
            _ => {
                // Other weapons have no effect on piece stream
            }
        }
    }
}

impl Default for PieceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::CellKind;

    #[test]
    fn test_determinism() {
        // Two managers + two Rng::new(99) produce the same sequence of 200 piece kinds
        let mut manager1 = PieceManager::new();
        let mut manager2 = PieceManager::new();
        let mut rng1 = Rng::new(99);
        let mut rng2 = Rng::new(99);

        let mut kinds1 = Vec::new();
        let mut kinds2 = Vec::new();

        for _ in 0..200 {
            let piece1 = manager1.create(&mut rng1, 0, 0);
            let piece2 = manager2.create(&mut rng2, 0, 0);
            kinds1.push(piece1.kind);
            kinds2.push(piece2.kind);
        }

        assert_eq!(kinds1, kinds2);
    }

    #[test]
    fn test_default_stream_no_weird_pieces() {
        // Default stream never yields weird pieces over 2000 draws
        let mut manager = PieceManager::new();
        let mut rng = Rng::new(42);

        let weird_kinds = [
            PieceKind::Dog,
            PieceKind::RevDog,
            PieceKind::Cap,
            PieceKind::Wall,
            PieceKind::Tower,
            PieceKind::Star,
            PieceKind::WeirdLong,
            PieceKind::FourByFour,
        ];

        for _ in 0..2000 {
            let piece = manager.create(&mut rng, 0, 0);
            assert!(
                !weird_kinds.contains(&piece.kind),
                "Unexpected weird piece: {:?}",
                piece.kind
            );
        }
    }

    #[test]
    fn test_no_dice_weapon() {
        // After weapon_on(NoDice), no Die appears over 2000 draws
        let mut manager = PieceManager::new();
        let mut rng = Rng::new(123);

        manager.weapon_on(WeaponToken::NoDice);

        for _ in 0..2000 {
            let piece = manager.create(&mut rng, 0, 0);
            assert_ne!(piece.kind, PieceKind::Die, "Die appeared when NoDice is active");
        }

        // After weapon_off(NoDice), dice appear again
        manager.weapon_off(WeaponToken::NoDice);
        let mut found_die = false;
        for _ in 0..200 {
            let piece = manager.create(&mut rng, 0, 0);
            if piece.kind == PieceKind::Die {
                found_die = true;
                break;
            }
        }
        assert!(found_die, "Die should appear after NoDice is off");
    }

    #[test]
    fn test_feared_weird_weapon() {
        // After weapon_on(FearedWeird), only weird pieces appear over 500 draws
        // Standard pieces should not appear
        let mut manager = PieceManager::new();
        let mut rng = Rng::new(456);

        manager.weapon_on(WeaponToken::FearedWeird);

        let standard_kinds = [
            PieceKind::El,
            PieceKind::RevEl,
            PieceKind::SlideLeft,
            PieceKind::SlideRight,
            PieceKind::Long,
            PieceKind::Plug,
            PieceKind::Box,
        ];

        let weird_kinds = [
            PieceKind::Dog,
            PieceKind::RevDog,
            PieceKind::Cap,
            PieceKind::Wall,
            PieceKind::Tower,
            PieceKind::Star,
            PieceKind::WeirdLong,
        ];

        let mut found_weird = false;

        for _ in 0..500 {
            let piece = manager.create(&mut rng, 0, 0);
            assert!(
                !standard_kinds.contains(&piece.kind),
                "Standard piece {:?} appeared when FearedWeird is active",
                piece.kind
            );
            if weird_kinds.contains(&piece.kind) {
                found_weird = true;
            }
        }

        assert!(found_weird, "Should find at least one weird piece");
    }

    #[test]
    fn test_nice_day_weapon() {
        // After weapon_on(NiceDay), one create yields Happy, hap_on returns to 0 after
        let mut manager = PieceManager::new();
        let mut rng = Rng::new(789);

        manager.weapon_on(WeaponToken::NiceDay);

        let piece = manager.create(&mut rng, 0, 0);
        assert_eq!(piece.kind, PieceKind::Happy);
        assert_eq!(manager.hap_on, 0);
    }

    #[test]
    fn test_die_pip_value() {
        // Die pieces have pip values in 1..=6
        let mut manager = PieceManager::new();
        let mut rng = Rng::new(111);

        let mut found_die = false;
        for _ in 0..200 {
            let piece = manager.create(&mut rng, 0, 0);
            if piece.kind == PieceKind::Die {
                found_die = true;
                // Check the cell at (1, 1) which should have the die value
                if let Some(cell) = piece.cells[1][1] {
                    match cell.kind {
                        CellKind::Die(v) => {
                            assert!(v >= 1 && v <= 6, "Die value {} out of range", v);
                        }
                        _ => panic!("Expected Die cell at (1, 1)"),
                    }
                } else {
                    panic!("Expected Some cell at (1, 1) for Die piece");
                }
            }
        }
        assert!(found_die, "Should find at least one die piece");
    }

    #[test]
    fn test_four_by_four_weapon() {
        // After weapon_on(FourByFour): the box piece is replaced by the 4x4 -
        // no Box appears, and FourByFour does (BTPieceManager::receive).
        let mut manager = PieceManager::new();
        let mut rng = Rng::new(456);
        manager.weapon_on(WeaponToken::FourByFour);

        let mut found_4x4 = false;
        for _ in 0..2000 {
            let piece = manager.create(&mut rng, 0, 0);
            assert_ne!(piece.kind, PieceKind::Box, "Box appeared while Four-by-Four active");
            if piece.kind == PieceKind::FourByFour {
                found_4x4 = true;
            }
        }
        assert!(found_4x4, "Four-by-Four pieces should appear");

        manager.weapon_off(WeaponToken::FourByFour);
        let mut found_box = false;
        for _ in 0..400 {
            if manager.create(&mut rng, 0, 0).kind == PieceKind::Box {
                found_box = true;
                break;
            }
        }
        assert!(found_box, "the Box returns once Four-by-Four is off");
    }

    #[test]
    fn test_so_long_weapon() {
        // After weapon_on(SoLong): no Long pieces; they return when it's off.
        let mut manager = PieceManager::new();
        let mut rng = Rng::new(321);
        manager.weapon_on(WeaponToken::SoLong);

        for _ in 0..2000 {
            assert_ne!(
                manager.create(&mut rng, 0, 0).kind,
                PieceKind::Long,
                "Long appeared while So Long active"
            );
        }

        manager.weapon_off(WeaponToken::SoLong);
        let mut found_long = false;
        for _ in 0..600 {
            if manager.create(&mut rng, 0, 0).kind == PieceKind::Long {
                found_long = true;
                break;
            }
        }
        assert!(found_long, "Long pieces return once So Long is off");
    }

    #[test]
    fn broken_on_pristine_manager_does_not_panic() {
        // The degenerate case the guard protects: Broken active before any
        // piece has spawned (old_piece still 0). Must yield a valid piece.
        let mut manager = PieceManager::new();
        let mut rng = Rng::new(1);
        manager.weapon_on(WeaponToken::Broken);
        let piece = manager.create(&mut rng, 0, 0); // must not panic
        assert!(matches!(PieceKind::from_id(BT_EL_PIECE), Some(_)));
        let _ = piece.kind; // a real, constructed piece
    }

    #[test]
    fn test_broken_record_repeats_pieces() {
        // Broken Record: the same piece repeats; it only changes ~1 in
        // BT_BROKEN_PROB draws, so the vast majority of consecutive draws match.
        let mut manager = PieceManager::new();
        let mut rng = Rng::new(654);
        // Establish a valid current piece first: Broken Record is always received
        // mid-game (a Game spawns a piece before any weapon arrives), so
        // `old_piece` is valid by the time it activates. Activating it on a
        // pristine manager would try to repeat the unset old_piece (id 0).
        let _ = manager.create(&mut rng, 0, 0);
        manager.weapon_on(WeaponToken::Broken);

        let mut prev = manager.create(&mut rng, 0, 0).kind;
        let mut same = 0;
        const N: usize = 400;
        for _ in 0..N {
            let kind = manager.create(&mut rng, 0, 0).kind;
            if kind == prev {
                same += 1;
            }
            prev = kind;
        }
        // Change probability ~1/10, so expect well over 70% repeats.
        assert!(
            same as f64 / N as f64 > 0.7,
            "Broken Record should mostly repeat: only {same}/{N} consecutive matches"
        );
    }
}

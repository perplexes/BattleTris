//! The player's weapon arsenal, a faithful port of `BTArsenal`
//! (`usr/src/game/BTArsenal.{H,C}`): `BT_ARSENAL_SIZE` (10) slots, each holding
//! a weapon token and a quantity. New purchases stack onto a matching slot or
//! fill the first empty one.

use crate::constants::BT_ARSENAL_SIZE;
use crate::weapons::WeaponToken;

/// A fixed bank of weapon slots. Parallel arrays rather than a `Vec` of
/// `(token, count)` pairs because slot POSITION is meaningful: the UI shows a
/// stable row of slots and Lazy Susan swaps whole arsenals, so slots must keep
/// their index even when emptied, and the capacity is a hard cap.
#[derive(Clone, Debug)]
pub struct Arsenal {
    /// Which weapon occupies each slot (`None` = empty).
    rep: [Option<WeaponToken>; BT_ARSENAL_SIZE],
    /// How many copies of that weapon are stacked in the slot.
    quantity: [u16; BT_ARSENAL_SIZE],
}

impl Default for Arsenal {
    fn default() -> Self {
        Arsenal::new()
    }
}

impl Arsenal {
    /// An empty arsenal (every slot vacant).
    pub fn new() -> Arsenal {
        Arsenal { rep: [None; BT_ARSENAL_SIZE], quantity: [0; BT_ARSENAL_SIZE] }
    }

    /// `BTArsenal::buyWeapon`: stack onto a matching slot, else the first empty
    /// slot. Returns false if the arsenal is full.
    pub fn buy(&mut self, w: WeaponToken) -> bool {
        for i in 0..BT_ARSENAL_SIZE {
            if self.rep[i] == Some(w) {
                self.quantity[i] += 1;
                return true;
            }
            if self.rep[i].is_none() {
                self.rep[i] = Some(w);
                self.quantity[i] += 1;
                return true;
            }
        }
        false
    }

    /// Remove one of weapon `w` from the arsenal (the bazaar "Remove" / sell).
    /// Returns true if one was present.
    pub fn sell(&mut self, w: WeaponToken) -> bool {
        for i in 0..BT_ARSENAL_SIZE {
            if self.rep[i] == Some(w) && self.quantity[i] > 0 {
                self.use_slot(i);
                return true;
            }
        }
        false
    }

    /// `BTArsenal::useWeapon`: consume one from slot `index`; empties the slot
    /// when the quantity hits zero.
    pub fn use_slot(&mut self, index: usize) {
        if index >= BT_ARSENAL_SIZE || self.quantity[index] == 0 {
            return;
        }
        self.quantity[index] -= 1;
        if self.quantity[index] == 0 {
            self.rep[index] = None;
        }
    }

    /// Set slot `index` directly to `token` × `qty`, preserving the exact slot
    /// layout (used by Lazy Susan's restore, where rebuilding via `buy` would
    /// compact holes and shift positions). `None`/`0` empties the slot.
    pub fn set_slot(&mut self, index: usize, token: Option<WeaponToken>, qty: u16) {
        if index >= BT_ARSENAL_SIZE {
            return;
        }
        if token.is_none() || qty == 0 {
            self.rep[index] = None;
            self.quantity[index] = 0;
        } else {
            self.rep[index] = token;
            self.quantity[index] = qty;
        }
    }

    /// The weapon in slot `index`, if any. Out-of-range indices read as empty
    /// so callers can iterate fixed slot rows without bounds juggling.
    pub fn token(&self, index: usize) -> Option<WeaponToken> {
        self.rep.get(index).copied().flatten()
    }

    /// How many copies are stacked in slot `index` (0 for empty or out of range).
    pub fn quantity(&self, index: usize) -> u16 {
        self.quantity.get(index).copied().unwrap_or(0)
    }

    /// Empty every slot, used when a game resets.
    pub fn clear(&mut self) {
        self.rep = [None; BT_ARSENAL_SIZE];
        self.quantity = [0; BT_ARSENAL_SIZE];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buy_stacks_and_fills() {
        let mut a = Arsenal::new();
        assert!(a.buy(WeaponToken::RiseUp));
        assert!(a.buy(WeaponToken::RiseUp));
        assert_eq!(a.token(0), Some(WeaponToken::RiseUp));
        assert_eq!(a.quantity(0), 2);
        assert!(a.buy(WeaponToken::Blind));
        assert_eq!(a.token(1), Some(WeaponToken::Blind));
        assert_eq!(a.quantity(1), 1);
    }

    #[test]
    fn use_empties_slot_at_zero() {
        let mut a = Arsenal::new();
        a.buy(WeaponToken::Swap);
        a.use_slot(0);
        assert_eq!(a.token(0), None);
        assert_eq!(a.quantity(0), 0);
    }

    #[test]
    fn full_arsenal_rejects_new_kinds() {
        let mut a = Arsenal::new();
        let kinds = [
            WeaponToken::FearedWeird, WeaponToken::FourByFour, WeaponToken::Hatter,
            WeaponToken::Upbyside, WeaponToken::FallOut, WeaponToken::Swap,
            WeaponToken::Lawyers, WeaponToken::RiseUp, WeaponToken::FlipOut,
            WeaponToken::Speedy,
        ];
        for k in kinds {
            assert!(a.buy(k));
        }
        // 11th distinct weapon: no empty slot left.
        assert!(!a.buy(WeaponToken::Blind));
        // but stacking an existing one still works
        assert!(a.buy(WeaponToken::Speedy));
    }
}

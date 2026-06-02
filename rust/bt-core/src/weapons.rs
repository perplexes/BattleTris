//! Weapon tokens, active-weapon flags, and the weapon database.
//!
//! `WeaponToken` is ported verbatim from the `BTWeaponToken` enum in
//! `usr/src/game/BTProtocol.H` (order and discriminants matter — they index
//! `keep_prob_`, the arsenal, and the `BTActive[]` array).
//!
//! Weapon *effects* live in the board ([`crate::board`]), piece manager and
//! game state machine; this module is the data + the active-flag bookkeeping.

/// `BTWeaponToken` — 34 weapons (0..=33). Discriminants are load-bearing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum WeaponToken {
    FearedWeird = 0,
    FourByFour = 1,
    Hatter = 2,
    Upbyside = 3,
    FallOut = 4,
    Swap = 5,
    Lawyers = 6,
    RiseUp = 7,
    FlipOut = 8,
    Speedy = 9,
    Missing = 10,
    PieceIt = 11,
    Blind = 12,
    Mondale = 13,
    Keating = 14,
    Carter = 15,
    Reagan = 16,
    Ames = 17,
    Ace = 18,
    Condor = 19,
    NiceDay = 20,
    SoLong = 21,
    NoDice = 22,
    Bug = 23,
    Bottle = 24,
    NoSlide = 25,
    Susan = 26,
    Meadow = 27,
    Mirror = 28,
    Twilight = 29,
    Slick = 30,
    Broken = 31,
    Force = 32,
    Gimp = 33,
}

/// `BT_MAX_WEAPONS` — number of real weapon tokens.
pub const BT_MAX_WEAPONS: usize = 34;

impl WeaponToken {
    /// All weapon tokens in protocol order.
    pub const ALL: [WeaponToken; BT_MAX_WEAPONS] = {
        use WeaponToken::*;
        [
            FearedWeird, FourByFour, Hatter, Upbyside, FallOut, Swap, Lawyers,
            RiseUp, FlipOut, Speedy, Missing, PieceIt, Blind, Mondale, Keating,
            Carter, Reagan, Ames, Ace, Condor, NiceDay, SoLong, NoDice, Bug,
            Bottle, NoSlide, Susan, Meadow, Mirror, Twilight, Slick, Broken,
            Force, Gimp,
        ]
    };

    #[inline]
    pub fn index(self) -> usize {
        self as i32 as usize
    }

    pub fn from_index(i: i32) -> Option<WeaponToken> {
        if (0..BT_MAX_WEAPONS as i32).contains(&i) {
            Some(WeaponToken::ALL[i as usize])
        } else {
            None
        }
    }
}

/// The `BTActive[]` array: how many copies of each weapon are currently active.
///
/// In the original this is incremented on `BT_WPN_ON` and decremented on
/// `BT_WPN_OFF`; gameplay checks `if (BTActive[token])` i.e. "non-zero".
#[derive(Clone, Debug)]
pub struct ActiveFlags {
    counts: [i32; BT_MAX_WEAPONS],
}

impl Default for ActiveFlags {
    fn default() -> Self {
        ActiveFlags {
            counts: [0; BT_MAX_WEAPONS],
        }
    }
}

impl ActiveFlags {
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn is_active(&self, token: WeaponToken) -> bool {
        self.counts[token.index()] != 0
    }

    #[inline]
    pub fn count(&self, token: WeaponToken) -> i32 {
        self.counts[token.index()]
    }

    pub fn activate(&mut self, token: WeaponToken) {
        self.counts[token.index()] += 1;
    }

    pub fn deactivate(&mut self, token: WeaponToken) {
        self.counts[token.index()] -= 1;
    }

    /// Set a weapon's active flag as a boolean (`BTActive[token] = on`), matching
    /// the original (which sets 1 on WPN_ON and 0 on WPN_OFF, not a counter).
    pub fn set(&mut self, token: WeaponToken, on: bool) {
        self.counts[token.index()] = if on { 1 } else { 0 };
    }

    pub fn clear(&mut self) {
        self.counts = [0; BT_MAX_WEAPONS];
    }
}

/// Weapon metadata, mirroring `BTWeapon` (`usr/src/game/BTWeapon.H`) and the
/// rows of `usr/src/share/btweapons.db`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponInfo {
    pub token: WeaponToken,
    pub name: &'static str,
    pub description: &'static str,
    /// Cost in funds.
    pub price: u16,
    /// Effect duration, measured in lines.
    pub duration: u16,
}

/// The full weapon table, in `WeaponToken::ALL` order.
///
/// Generated verbatim from `usr/src/share/btweapons.db` (name + description)
/// and `usr/src/share/btweaponsp.db` (price + duration), matching the loader in
/// `BTPimp::load`. Duration is measured in lines.
pub fn weapon_table() -> [WeaponInfo; BT_MAX_WEAPONS] {
    [
        WeaponInfo { token: WeaponToken::FearedWeird, name: "The Feared Weird", description: "Gives your opponent bizarre, disjointed pieces. None of the pieces are easily placed; particularly deadly when used in conjunction with either the Mad Hatter or No Dice.", price: 400, duration: 3 },
        WeaponInfo { token: WeaponToken::FourByFour, name: "Four-by-Four", description: "Evil incarnate. Replaces your opponent's box piece with one that is a four block by four block hollow box.", price: 425, duration: 10 },
        WeaponInfo { token: WeaponToken::Hatter, name: "The Mad Hatter", description: "Your opponent's pieces never stop spinning (unless pinned up against the wall). Quickly frustrates opponent. Very good combination weapon.", price: 375, duration: 5 },
        WeaponInfo { token: WeaponToken::Upbyside, name: "Upbyside-down", description: "Flips your opponent's screen upside-down. Their direction keys are reversed, and pieces rotate the opposite way.", price: 125, duration: 10 },
        WeaponInfo { token: WeaponToken::FallOut, name: "Fallout", description: "The middle six columns of your board \"fall out.\" The gap that they leave represents a black hole. Any pieces dropped into this black hole will just disappear. The player must instead build a \"bridge\" of pieces over the hole in order to get lines.", price: 250, duration: 10 },
        WeaponInfo { token: WeaponToken::Swap, name: "Swap meet", description: "Swaps your screen with your opponent's. Screw up your own board, and then swap it out. Of course, the other opponent may launch another Swap, but such is life.", price: 1200, duration: 0 },
        WeaponInfo { token: WeaponToken::Lawyers, name: "Lawyer's delite", description: "Outright stolen from the original 2-player arcade version of tetris. Every line you get, your opponent's screen \"rises\" up by one line.", price: 350, duration: 5 },
        WeaponInfo { token: WeaponToken::RiseUp, name: "Rise up", description: "Raises the opponent's screen one level (the bottom level will be solid with one, random, block missing).", price: 75, duration: 0 },
        WeaponInfo { token: WeaponToken::FlipOut, name: "Flip out", description: "Flips your opponents screen on a vertical axis.  Can be extremely annoying if done often enough.", price: 15, duration: 0 },
        WeaponInfo { token: WeaponToken::Speedy, name: "Speedy Gonzales", description: "Doubles the speed of the opponent's game. Several of these launched at once get make things pretty interesting for your opponent.", price: 275, duration: 10 },
        WeaponInfo { token: WeaponToken::Missing, name: "Missing Pieces", description: "Randomly removes one of your opponent's blocks.", price: 50, duration: 0 },
        WeaponInfo { token: WeaponToken::PieceIt, name: "Piece It Together", description: "Randomly adds a piece to your opponent's board. More than one great player has fallen on a lucky Piece It Together.", price: 100, duration: 0 },
        WeaponInfo { token: WeaponToken::Blind, name: "The Blind Cleric", description: "Bombs a region of your opponent's screen. Can be particularly annoying when an elaborate setup develops a large hole in its center.", price: 400, duration: 0 },
        WeaponInfo { token: WeaponToken::Mondale, name: "Mondale '96", description: "Taxes your opponent with a hefty 30 percent rate. Whenever they get funds, you swipe a certain percentage.", price: 150, duration: 50 },
        WeaponInfo { token: WeaponToken::Keating, name: "Keating Five", description: "Your opponent's funds are all taken away...and given to you.", price: 425, duration: 0 },
        WeaponInfo { token: WeaponToken::Carter, name: "Carter Years", description: "Relives the inflationary years of Jimmy Carter -- the prices double at your opponent's bazaar.", price: 250, duration: 20 },
        WeaponInfo { token: WeaponToken::Reagan, name: "Reagan Era", description: "Relives that era of debt -- your opponent's funds are multiplied by -1.", price: 425, duration: 0 },
        WeaponInfo { token: WeaponToken::Ames, name: "William Ames", description: "Displays your opponent's screen and your opponent's funds next to your own.  Remember that cheap spies are easily bought and sold...", price: 50, duration: 20 },
        WeaponInfo { token: WeaponToken::Ace, name: "Ace of Spies", description: "Send Reilly over the border.  A more expensive spy, but such is the price of greater accuracy.  Reilly's still human though...you never know when he's going to flake out on the Russian border.", price: 100, duration: 30 },
        WeaponInfo { token: WeaponToken::Condor, name: "The Condor", description: "Launch the world's most advanced spy satellite.  Guaranteed accuracy, but you probably had to sell arms to the Contras in order to afford it.", price: 225, duration: 40 },
        WeaponInfo { token: WeaponToken::NiceDay, name: "Have a Nice Day", description: "Gives your opponent a smiley face. Why give your opponent the opportunity to make an extra 150 beans? Hit them with a Reagan Era shortly after. God Bless America.", price: 50, duration: 0 },
        WeaponInfo { token: WeaponToken::SoLong, name: "So Long", description: "Deprives your opponent of long pieces.", price: 100, duration: 10 },
        WeaponInfo { token: WeaponToken::NoDice, name: "No Dice", description: "Deprives your opponent of dice.", price: 600, duration: 35 },
        WeaponInfo { token: WeaponToken::Bug, name: "Bug Report", description: "Like Piece It Together, except the block is invisible (which leads your opponent to file a bug report).", price: 320, duration: 0 },
        WeaponInfo { token: WeaponToken::Bottle, name: "Bottle neck", description: "Your opponent's board suddenly develops a 4-block wide bottle neck.", price: 150, duration: 10 },
        WeaponInfo { token: WeaponToken::NoSlide, name: "Slide Denied", description: "Take the famous BattleTris slide out of your opponent's diet.", price: 125, duration: 10 },
        WeaponInfo { token: WeaponToken::Susan, name: "Lazy Susan", description: "Turns the tables on you opponent by swapping your arsenal with theirs.", price: 600, duration: 0 },
        WeaponInfo { token: WeaponToken::Meadow, name: "Meadow", description: "This weapon lineup simulates Meadow running on your opponent's machine:  the drop speed of their pieces is halved.", price: 475, duration: 10 },
        WeaponInfo { token: WeaponToken::Mirror, name: "Mirror Mirror", description: "An oft requested defensive weapon:  when launched, your opponent's weapons will be reflected back on to them.  Note that some weapons (Swap Meet, Keating Five, Have a Nice Day, etc.) are simply nullified.", price: 500, duration: 10 },
        WeaponInfo { token: WeaponToken::Twilight, name: "The Twilight Zone", description: "All of the bricks in your opponents screen becomes invisible.", price: 450, duration: 0 },
        WeaponInfo { token: WeaponToken::Slick, name: "Slick Willy", description: "Your opponent's pieces move endlessly from left to right and back.", price: 650, duration: 3 },
        WeaponInfo { token: WeaponToken::Broken, name: "Broken Record", description: "Gives your opponent the same piece the same piece the same piece ...", price: 325, duration: 5 },
        WeaponInfo { token: WeaponToken::Force, name: "The Force", description: "When your opponent gets a line, his board won't drop to fill the empty space.", price: 325, duration: 5 },
        WeaponInfo { token: WeaponToken::Gimp, name: "The Gimp", description: "Distracts your opponent from the game.", price: 25, duration: 0 },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_complete_and_ordered() {
        let t = weapon_table();
        assert_eq!(t.len(), BT_MAX_WEAPONS);
        // Every row's token matches its position in WeaponToken::ALL.
        for (i, info) in t.iter().enumerate() {
            assert_eq!(info.token, WeaponToken::ALL[i]);
            assert!(!info.name.is_empty());
            assert!(!info.description.is_empty());
        }
        // Spot-check a few known values from the DB.
        assert_eq!(t[WeaponToken::Swap.index()].price, 1200);
        assert_eq!(t[WeaponToken::Slick.index()].price, 650);
        assert_eq!(t[WeaponToken::FearedWeird.index()].duration, 3);
        assert_eq!(t[WeaponToken::NoDice.index()].duration, 35);
    }

    #[test]
    fn active_flags_track_on_off() {
        let mut a = ActiveFlags::new();
        assert!(!a.is_active(WeaponToken::Force));
        a.activate(WeaponToken::Force);
        assert!(a.is_active(WeaponToken::Force));
        a.deactivate(WeaponToken::Force);
        assert!(!a.is_active(WeaponToken::Force));
    }

    #[test]
    fn set_is_boolean_not_a_counter() {
        // BTActive[token] is 0/1, not a count: launching the same weapon twice
        // and expiring it once must leave it inactive (regression guard for the
        // "duration weapon stuck active forever" bug).
        let mut a = ActiveFlags::new();
        a.set(WeaponToken::Speedy, true);
        a.set(WeaponToken::Speedy, true);
        assert_eq!(a.count(WeaponToken::Speedy), 1);
        a.set(WeaponToken::Speedy, false);
        assert!(!a.is_active(WeaponToken::Speedy));
    }
}

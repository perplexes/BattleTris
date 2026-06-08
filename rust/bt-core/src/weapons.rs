//! Weapon tokens, active-weapon flags, and the weapon database.
//!
//! `WeaponToken` is ported verbatim from the `BTWeaponToken` enum in
//! `usr/src/game/BTProtocol.H` (order and discriminants matter — they index the
//! active-flag array, the per-weapon duration array, and [`weapon_table`], and
//! identify the token on the arsenal/wire protocol).
//!
//! Weapon *effects* live in the board ([`crate::board`]), piece manager and
//! game state machine; this module is the data + the active-flag bookkeeping.

/// The 34 weapons, identified by token.
///
/// The discriminants are load-bearing, not cosmetic: a token's `i32` value is
/// its index into the active-flag array, the per-weapon duration array, and
/// [`weapon_table`], and it is the token's identity on the arsenal/wire protocol.
/// They must stay `0..=33` in exactly this order, matching the protocol every
/// consumer shares. Full flavor/price/duration for each lives in
/// [`weapon_table`]; the one-liners here name the gameplay effect so the variant
/// is legible at a call site.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum WeaponToken {
    /// Floods the opponent with hard-to-place "weird" pieces.
    FearedWeird = 0,
    /// Replaces the opponent's box piece with a hollow 4x4 ring.
    FourByFour = 1,
    /// The opponent's pieces spin nonstop (Mad Hatter).
    Hatter = 2,
    /// Flips the opponent's board upside-down and reverses their controls.
    Upbyside = 3,
    /// Opens a hole in the middle of the opponent's floor (Fallout).
    FallOut = 4,
    /// Exchanges the two boards (cross-player; never queued).
    Swap = 5,
    /// Every line you clear raises the opponent's stack one row (Lawyer's Delite).
    Lawyers = 6,
    /// Raises the opponent's stack one solid row with a random gap.
    RiseUp = 7,
    /// Mirrors the opponent's board left↔right.
    FlipOut = 8,
    /// Doubles the opponent's drop speed (Speedy Gonzales).
    Speedy = 9,
    /// Removes one of the opponent's blocks at random.
    Missing = 10,
    /// Drops a random block onto the opponent's board (Piece It Together).
    PieceIt = 11,
    /// Bombs out a region of the opponent's board (Blind Cleric).
    Blind = 12,
    /// Taxes the opponent's earned funds to you (Mondale '96).
    Mondale = 13,
    /// Seizes all the opponent's funds and hands them to you (Keating Five).
    Keating = 14,
    /// Doubles the prices at the opponent's bazaar (Carter Years).
    Carter = 15,
    /// Negates the opponent's funds (Reagan Era).
    Reagan = 16,
    /// Cheapest spy, shortest reveal of the opponent's board/funds (William Ames).
    Ames = 17,
    /// Mid-cost spy, longer reveal (Ace of Spies).
    Ace = 18,
    /// Priciest spy, longest reveal (The Condor).
    Condor = 19,
    /// Gives the opponent a smiley piece (Have a Nice Day).
    NiceDay = 20,
    /// Denies the opponent long pieces (So Long).
    SoLong = 21,
    /// Denies the opponent dice (No Dice).
    NoDice = 22,
    /// Drops an INVISIBLE block onto the opponent's board (Bug Report).
    Bug = 23,
    /// Squeezes the opponent's board to a narrow neck (Bottle neck).
    Bottle = 24,
    /// Removes the opponent's slide window — pieces lock instantly (Slide Denied).
    NoSlide = 25,
    /// Exchanges the two arsenals (cross-player Lazy Susan; never queued).
    Susan = 26,
    /// Halves the opponent's drop speed (Meadow).
    Meadow = 27,
    /// Backfires most weapons the launcher fires while cursed onto the launcher;
    /// some simply fizzle (see `mirror_nullifies`).
    Mirror = 28,
    /// Cloaks every block on the opponent's board (Twilight Zone).
    Twilight = 29,
    /// The opponent's piece slides side to side endlessly (Slick Willy).
    Slick = 30,
    /// The opponent keeps getting the same piece (Broken Record).
    Broken = 31,
    /// The opponent's board won't collapse after a line clears (The Force).
    Force = 32,
    /// Distracts the opponent with a cosmetic gimp overlay (The Gimp).
    Gimp = 33,
}

/// Number of weapon tokens — the length of every per-weapon array, so all of
/// [`WeaponToken::ALL`], the active-flag counts, and [`weapon_table`] stay in
/// lockstep.
pub const BT_MAX_WEAPONS: usize = 34;

impl WeaponToken {
    /// Every token in discriminant order. The canonical iteration order, and the
    /// reverse of [`WeaponToken::index`] (entry `i` has index `i`).
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

    /// The token's array index — its discriminant as a `usize`. The one place
    /// the load-bearing discriminant is consumed as a subscript.
    #[inline]
    pub fn index(self) -> usize {
        self as i32 as usize
    }

    /// The token for index `i`, or `None` if out of range — the guard that turns
    /// an untrusted wire/keyframe integer into a valid token.
    pub fn from_index(i: i32) -> Option<WeaponToken> {
        if (0..BT_MAX_WEAPONS as i32).contains(&i) {
            Some(WeaponToken::ALL[i as usize])
        } else {
            None
        }
    }
}

/// Which weapons are currently in effect — the `BTActive[]` array.
///
/// Active is a per-weapon flag, not a stack: a weapon is on or off, so launching
/// the same weapon twice and expiring it once must leave it off. Gameplay
/// therefore sets it as a boolean ([`ActiveFlags::set`]) and tests "in effect"
/// with [`ActiveFlags::is_active`]. The slot is a count rather than a `bool`
/// only so it can serialize uniformly in a keyframe and offer the more general
/// increment primitive; the live game never relies on the magnitude.
#[derive(Clone, Debug)]
pub struct ActiveFlags {
    /// Per-token active state, indexed by [`WeaponToken::index`]. Nonzero = on.
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
    /// All weapons off.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether `token`'s effect is currently in force.
    #[inline]
    pub fn is_active(&self, token: WeaponToken) -> bool {
        self.counts[token.index()] != 0
    }

    /// The raw count for `token` — the general primitive behind the boolean
    /// view; live gameplay only ever cares whether it is nonzero.
    #[inline]
    pub fn count(&self, token: WeaponToken) -> i32 {
        self.counts[token.index()]
    }

    /// Counting activate/deactivate. The live game does not use these — it sets a
    /// boolean flag via [`ActiveFlags::set`]; these are the general counting
    /// primitive (so a caller that genuinely needs nesting can pair them).
    pub fn activate(&mut self, token: WeaponToken) {
        self.counts[token.index()] += 1;
    }

    /// See [`ActiveFlags::activate`].
    pub fn deactivate(&mut self, token: WeaponToken) {
        self.counts[token.index()] -= 1;
    }

    /// Set `token` on or off as a flat boolean. This is what the game uses, so
    /// that relaunching an active weapon does not deepen a count that a single
    /// expiry could never unwind.
    pub fn set(&mut self, token: WeaponToken, on: bool) {
        self.counts[token.index()] = if on { 1 } else { 0 };
    }

    /// Turn every weapon off — used on game reset.
    pub fn clear(&mut self) {
        self.counts = [0; BT_MAX_WEAPONS];
    }

    /// The raw per-weapon flags — for a full-game keyframe. Pair with
    /// [`ActiveFlags::set_raw`].
    pub fn raw(&self) -> [i32; BT_MAX_WEAPONS] {
        self.counts
    }

    /// Restore the raw flags captured by [`ActiveFlags::raw`].
    pub fn set_raw(&mut self, counts: [i32; BT_MAX_WEAPONS]) {
        self.counts = counts;
    }
}

/// The display + economy data for one weapon: its name, bazaar description, the
/// price to buy it, and how long its effect lasts. Bundled per-weapon so the
/// bazaar UI and the duration bookkeeping read from one table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponInfo {
    /// Which weapon this row describes (equals its position in the table).
    pub token: WeaponToken,
    /// Display name shown in the bazaar.
    pub name: &'static str,
    /// Bazaar blurb describing the effect.
    pub description: &'static str,
    /// Cost in funds to buy in the bazaar.
    pub price: u16,
    /// Effect lifetime, counted DOWN in lines cleared (0 = instant/one-shot).
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
        // Active is a flag, not a stack: launching the same weapon twice and
        // expiring it once must leave it inactive, so `set` clamps to 0/1.
        let mut a = ActiveFlags::new();
        a.set(WeaponToken::Speedy, true);
        a.set(WeaponToken::Speedy, true);
        assert_eq!(a.count(WeaponToken::Speedy), 1);
        a.set(WeaponToken::Speedy, false);
        assert!(!a.is_active(WeaponToken::Speedy));
    }
}

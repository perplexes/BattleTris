//! The board grid and its mechanics — the faithful analogue of
//! `BTBoardManager` (`usr/src/game/BTBoardManager.{H,C}`).
//!
//! Owned by the main port (correctness-critical). The grid is `width * height`
//! squares; `None` is empty (C++ `map_[x][y] == NULL`). Several mechanics key
//! off active-weapon flags ([`ActiveFlags`]): `occupied` (FALL_OUT),
//! `remove_line`/`check_lines` (FORCE/BOTTLE/UPBYSIDE).

use crate::cell::Cell;
use crate::constants::*;
use crate::rng::Rng;
use crate::weapons::{ActiveFlags, WeaponToken};

/// Result of [`Board::check_lines`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct LineClear {
    /// Number of simultaneous lines cleared (`lines.inc()`).
    pub lines: i32,
    /// Sum of cell values across all cleared lines.
    pub value: i32,
    /// Funds awarded = `value * lines`.
    pub funds: i32,
}

/// The board. `map_[x][y]` becomes `self.get(x, y)`.
#[derive(Clone, Debug)]
pub struct Board {
    pub width: i32,
    pub height: i32,
    /// Row-major: index = `y * width + x`. `None` = empty.
    cells: Vec<Option<Cell>>,
    /// `BTActive[]` — active-weapon flags consulted by mechanics here.
    pub active: ActiveFlags,
    /// `upside_` — board currently flipped (Upbyside).
    pub upside: bool,
    /// `computer_` — true for the AI's board (changes a few branches).
    pub computer: bool,
    /// `idiot_` / `reason_` — set by `landed`/`check_lines`, drained by the game.
    pub idiot: bool,
    pub reason: i16,
    /// Board indices filled during the current placement (`new_fill_`), used by
    /// idiot detection in [`Board::landed`].
    new_fill: Vec<usize>,
}

impl Board {
    /// `BTBoardManager::BTBoardManager` — an empty board.
    pub fn new(width: i32, height: i32, computer: bool) -> Board {
        Board {
            width,
            height,
            cells: vec![None; (width * height) as usize],
            active: ActiveFlags::new(),
            upside: false,
            computer,
            idiot: false,
            reason: 0,
            new_fill: Vec::new(),
        }
    }

    /// A standard 10x28 board.
    pub fn standard(computer: bool) -> Board {
        Board::new(BT_BOARD_WTH, BT_BOARD_HGT, computer)
    }

    #[inline]
    fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.width && y >= 0 && y < self.height
    }

    #[inline]
    fn index(&self, x: i32, y: i32) -> usize {
        (y * self.width + x) as usize
    }

    /// The cell at `(x, y)`, or `None` if empty or out of bounds.
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Option<Cell> {
        if self.in_bounds(x, y) {
            self.cells[self.index(x, y)]
        } else {
            None
        }
    }

    /// Set `(x, y)` directly (no bookkeeping). Out-of-bounds writes are ignored.
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, cell: Option<Cell>) {
        if self.in_bounds(x, y) {
            let i = self.index(x, y);
            self.cells[i] = cell;
        }
    }

    #[inline]
    fn is_active(&self, t: WeaponToken) -> bool {
        self.active.is_active(t)
    }

    /// `BTBoardManager::occupied` (inline, `BTBoardManager.H:71-86`).
    ///
    /// Returns true if `(x, y)` is blocked — out of bounds or filled. With the
    /// FALL_OUT weapon active the floor/ceiling open up except for a ledge of
    /// width `BT_FALL_OUT_LEDGE` at each side.
    pub fn occupied(&self, x: i32, y: i32) -> bool {
        if !self.is_active(WeaponToken::FallOut) {
            if x < 0 || x >= self.width || y >= self.height || y < 0 {
                return true;
            }
            return self.get(x, y).is_some();
        }
        // FALL_OUT: the middle of the floor/ceiling is open.
        if x < 0
            || x >= self.width
            || (y >= self.height
                && (x < BT_FALL_OUT_LEDGE || x >= self.width - BT_FALL_OUT_LEDGE))
            || (y < 0 && (x < BT_FALL_OUT_LEDGE || x >= self.width - BT_FALL_OUT_LEDGE))
        {
            return true;
        }
        if y < self.height && y >= 0 && self.get(x, y).is_some() {
            return true;
        }
        if y < 0 - BT_PIECE_HEIGHT as i32 || y > self.height + BT_PIECE_HEIGHT as i32 {
            return true;
        }
        false
    }

    /// `BTBoardManager::fill` — place `cell` at `(x, y)` and record it for idiot
    /// detection. A box that lands off-board (e.g. via FALL_OUT) is discarded.
    pub fn fill(&mut self, x: i32, y: i32, cell: Cell) {
        if self.in_bounds(x, y) {
            let i = self.index(x, y);
            self.cells[i] = Some(cell);
            self.new_fill.push(i);
        }
        // else: fell off the board — discard (C++ `delete new_box`).
    }

    /// `BTBoardManager::setIdiot`.
    pub fn set_idiot(&mut self, reason: i16) {
        self.idiot = true;
        self.reason = reason;
    }

    /// `BTBoardManager::flushIdiot` — read & clear the idiot flag, returning the
    /// reason if one was set.
    pub fn flush_idiot(&mut self) -> Option<i16> {
        let out = if self.idiot { Some(self.reason) } else { None };
        self.idiot = false;
        out
    }

    /// `BTBoardManager::clear` — empty the board.
    pub fn clear(&mut self) {
        for c in self.cells.iter_mut() {
            *c = None;
        }
        self.new_fill.clear();
    }

    /// The board index of an occupied, in-bounds neighbor (for idiot detection,
    /// where the original compares box-pointer identity against `new_fill_`).
    #[inline]
    fn fill_index(&self, x: i32, y: i32) -> Option<usize> {
        if self.in_bounds(x, y) {
            let i = self.index(x, y);
            if self.cells[i].is_some() {
                return Some(i);
            }
        }
        None
    }

    /// `BTBoardManager::landed` — "bad move" (idiot) detection after a piece
    /// locks at `(x, y)`: an empty square surrounded on left, right and logical
    /// top by boxes, where at least one of those was just placed this turn.
    pub fn landed(&mut self, x: i32, y: i32) {
        let upside = self.is_active(WeaponToken::Upbyside);
        for i in 0..BT_PIECE_WIDTH as i32 {
            for j in 0..BT_PIECE_HEIGHT as i32 {
                let cx = x + i;
                let cy = y + j;
                if self.occupied(cx, cy) {
                    continue; // only consider empty squares
                }
                if !self.occupied(cx - 1, cy) {
                    continue;
                }
                let left = self.fill_index(cx - 1, cy);
                if !self.occupied(cx + 1, cy) {
                    continue;
                }
                let right = self.fill_index(cx + 1, cy);
                let top = if !upside {
                    if !self.occupied(cx, cy - 1) {
                        continue;
                    }
                    self.fill_index(cx, cy - 1)
                } else {
                    if !self.occupied(cx, cy + 1) {
                        continue;
                    }
                    self.fill_index(cx, cy + 1)
                };
                // Surrounded; is any neighbor a box placed this turn?
                let hit = self
                    .new_fill
                    .iter()
                    .any(|&nf| Some(nf) == left || Some(nf) == right || Some(nf) == top);
                if hit {
                    self.idiot = true;
                    self.reason = BT_BAD_MOVE;
                }
            }
        }
        self.new_fill.clear();
    }

    /// `BTBoardManager::checkLines` — detect & clear full lines, award funds
    /// (`value * lines`), and set idiot flags (missed-smiley / near-death).
    pub fn check_lines(&mut self) -> LineClear {
        let force = self.is_active(WeaponToken::Force);
        let mut value = 0i32;
        let mut lines = 0i32; // BTLine increment_

        // Near-death gauge: scan up from the bottom; `min` is the highest
        // occupied row before the first fully empty row (BTBoardManager.C:561).
        let mut min = self.height - 1;
        {
            let mut j = self.height - 1;
            while j > 0 {
                let mut i = 0;
                while i < self.width {
                    if self.get(i, j).is_some() {
                        if j < min {
                            min = j;
                        }
                        break;
                    }
                    i += 1;
                }
                if i == self.width {
                    break;
                }
                j -= 1;
            }
        }

        // Main pass: from the bottom up, clearing full lines.
        let mut j = self.height - 1;
        while j >= 0 {
            // Count consecutive non-empty cells from the left, summing values.
            let mut nvalue = 0;
            let mut i = 0;
            while i < self.width {
                match self.get(i, j) {
                    Some(c) => {
                        nvalue += c.value();
                        i += 1;
                    }
                    None => break,
                }
            }

            if i == self.width {
                // Full line.
                lines += 1;
                value += nvalue;
                self.remove_line(j, 0, self.width);
                if !force {
                    j += 1; // re-examine this row (board shifted down into it)
                }
            } else {
                // Not a line: a happy face that lands here turns into a frown.
                for i2 in 0..self.width {
                    if let Some(mut c) = self.get(i2, j) {
                        if c.value() == BT_HAPPY_VAL {
                            c.landed();
                            self.set(i2, j, Some(c));
                            self.idiot = true;
                            self.reason = BT_MISSED_SMILEY;
                        }
                    }
                }
            }
            j -= 1;
        }

        if min < 8 {
            self.idiot = true;
            self.reason = BT_NEAR_DEATH;
        }

        if lines == 0 {
            return LineClear::default();
        }

        // Cleared at least one line — not an idiot after all.
        self.idiot = false;
        let funds = value * lines;
        LineClear {
            lines,
            value,
            funds,
        }
    }

    /// `BTBoardManager::removeLine` — drop the board into `line` over columns
    /// `[x1, x2)` (negative `x2` means full width). Honors FORCE (no shift),
    /// BOTTLE (narrow to the neck) and UPBYSIDE (shift the other way).
    fn remove_line(&mut self, line: i32, mut x1: i32, mut x2: i32) {
        if x2 < 0 {
            x2 = self.width;
        }
        let force = self.is_active(WeaponToken::Force);
        let bottle = self.is_active(WeaponToken::Bottle);
        let h = BT_BOARD_HGT;

        if !self.is_active(WeaponToken::Upbyside) || self.computer {
            let mut i = line;
            while i > 0 {
                if bottle && i <= h / 2 + BT_BOTTLE_Y && i >= h / 2 - BT_BOTTLE_Y {
                    x1 = BT_BOTTLE_X;
                    x2 = self.width - BT_BOTTLE_X;
                }
                for j in x1..x2 {
                    if force {
                        if i == line && self.get(j, i).is_some() {
                            self.set(j, i, None);
                        }
                        continue;
                    }
                    let above = self.get(j, i - 1);
                    self.set(j, i, above);
                    self.set(j, i - 1, None);
                }
                i -= 1;
            }
            if !force {
                for i2 in x1..x2 {
                    self.set(i2, 0, None);
                }
            }
        } else {
            let mut i = line;
            while i < self.height - 1 {
                if bottle && i <= h / 2 + BT_BOTTLE_Y && i >= h / 2 - BT_BOTTLE_Y - 1 {
                    x1 = BT_BOTTLE_X;
                    x2 = self.width - BT_BOTTLE_X;
                }
                for j in x1..x2 {
                    if force {
                        if i == line && self.get(j, i).is_some() {
                            self.set(j, i, None);
                        }
                        continue;
                    }
                    let below = self.get(j, i + 1);
                    self.set(j, i, below);
                    self.set(j, i + 1, None);
                }
                i += 1;
            }
            if !force {
                for i2 in x1..x2 {
                    self.set(i2, self.height - 1, None);
                }
            }
        }
    }

    /// `BTBoardManager::insertLine` — a rise-up / Lawyers' Delite garbage line:
    /// push the stack up (or down when upside-down) and insert a solid row of
    /// green boxes with one random gap. Garbage boxes carry no funds value.
    pub fn insert_line(&mut self, rng: &mut Rng) {
        let hole = rng.rand_below(self.width);
        let h = BT_BOARD_HGT;
        let bottle = self.is_active(WeaponToken::Bottle);

        if !self.is_active(WeaponToken::Upbyside) || self.computer {
            let mut x1 = 0;
            let mut x2 = self.width;
            for i in 0..self.height - 1 {
                x1 = 0;
                x2 = self.width;
                if bottle && i < h / 2 + BT_BOTTLE_Y {
                    x1 = BT_BOTTLE_X;
                    x2 = self.width - BT_BOTTLE_X;
                }
                for j in x1..x2 {
                    let below = self.get(j, i + 1);
                    self.set(j, i, below);
                    self.set(j, i + 1, None);
                }
            }
            for i in x1..x2 {
                if i != hole {
                    self.set(i, self.height - 1, Some(Cell::color(BT_GREEN)));
                }
            }
        } else {
            let mut x1 = 0;
            let mut x2 = self.width;
            for i in (1..self.height).rev() {
                x1 = 0;
                x2 = self.width;
                if bottle && i >= h / 2 - BT_BOTTLE_Y {
                    x1 = BT_BOTTLE_X;
                    x2 = self.width - BT_BOTTLE_X;
                }
                for j in x1..x2 {
                    let above = self.get(j, i - 1);
                    self.set(j, i, above);
                    self.set(j, i - 1, None);
                }
            }
            for i in x1..x2 {
                if i != hole {
                    self.set(i, 0, Some(Cell::color(BT_GREEN)));
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Weapon effects on the board — `BTBoardManager::receive` (BT_WPN_ON /
    // BT_WPN_OFF). The active-flag bookkeeping is done by [`Board::set_active`];
    // these apply the one-shot board mutation.
    // -----------------------------------------------------------------------

    /// Set/clear an active-weapon flag the board logic consults (`BTActive[]` is
    /// boolean in the original, not a counter).
    pub fn set_active(&mut self, token: WeaponToken, on: bool) {
        self.active.set(token, on);
    }

    /// Swap Meet: exchange this board's grid with `other`'s. Only the cells
    /// move; the active flags, `upside`, `computer` and idiot state stay put
    /// (Swap clears Bottle/Upbyside separately, at the game level). Dimensions
    /// must match.
    pub fn swap_cells(&mut self, other: &mut Board) {
        debug_assert_eq!(
            (self.width, self.height),
            (other.width, other.height),
            "swap_cells requires equal dimensions"
        );
        std::mem::swap(&mut self.cells, &mut other.cells);
    }

    fn swap(&mut self, x1: i32, y1: i32, x2: i32, y2: i32) {
        let a = self.get(x1, y1);
        let b = self.get(x2, y2);
        self.set(x1, y1, b);
        self.set(x2, y2, a);
    }

    /// `BTBoardManager::flipOnHoriz` — mirror top↔bottom (Upbyside).
    fn flip_horiz(&mut self) {
        for i in 0..self.height / 2 {
            for j in 0..self.width {
                self.swap(j, i, j, self.height - 1 - i);
            }
        }
    }

    /// `BTBoardManager::flipOnVert` — mirror left↔right (Flip Out).
    fn flip_vert(&mut self) {
        for i in 0..self.width / 2 {
            for j in 0..self.height {
                self.swap(self.width - 1 - i, j, i, j);
            }
        }
    }

    /// The one-shot board mutation for a weapon turning on (`BT_WPN_ON`).
    /// (Set the active flag via [`Board::set_active`] first.)
    pub fn apply_weapon(&mut self, token: WeaponToken, rng: &mut Rng) {
        let h = BT_BOARD_HGT;
        match token {
            WeaponToken::Upbyside => {
                if !self.upside && !self.computer {
                    self.flip_horiz();
                }
                self.upside = true;
            }
            WeaponToken::PieceIt | WeaponToken::Bug => {
                // A box at a random empty spot in the middle two quarters.
                let (mut i, mut j);
                loop {
                    i = rng.rand_below(self.width);
                    j = rng.rand_below(self.height / 2) + self.height / 4;
                    if !self.occupied(i, j) {
                        break;
                    }
                }
                let cell = if token == WeaponToken::Bug {
                    Cell::color(BT_INVISIBLE)
                } else {
                    Cell::color(rng.rand_below(BT_NEUTRAL - 1) + 1)
                };
                self.set(i, j, Some(cell));
            }
            WeaponToken::Missing => {
                // Remove the first removable box scanning from a random origin.
                let sx = rng.rand_below(self.width);
                let sy = rng.rand_below(self.height);
                'outer: for dy in 0..self.height {
                    let y = (sy + dy) % self.height;
                    for dx in 0..self.width {
                        let x = (sx + dx) % self.width;
                        if let Some(c) = self.get(x, y) {
                            if c.is_removable() {
                                self.set(x, y, None);
                                break 'outer;
                            }
                        }
                    }
                }
            }
            WeaponToken::Blind => {
                for y in 0..self.height {
                    for x in 0..self.width {
                        if let Some(c) = self.get(x, y) {
                            if c.is_removable() && rng.rand_below(2) == 0 {
                                self.set(x, y, None);
                            }
                        }
                    }
                }
            }
            WeaponToken::Gimp => {
                for y in 0..self.height {
                    for x in 0..self.width {
                        if let Some(c) = self.get(x, y) {
                            if c.is_removable() {
                                self.set(x, y, Some(Cell::gimp(c.value())));
                            }
                        }
                    }
                }
            }
            WeaponToken::Twilight => {
                for y in 0..self.height {
                    for x in 0..self.width {
                        if let Some(mut c) = self.get(x, y) {
                            c.hide();
                            self.set(x, y, Some(c));
                        }
                    }
                }
            }
            WeaponToken::FlipOut => self.flip_vert(),
            WeaponToken::FallOut => {
                // The middle columns "fall out": repeatedly drop the bottom
                // (or top, if upside-down) line over the non-ledge columns.
                for _ in 0..self.height {
                    let line = if !self.upside { self.height - 1 } else { 0 };
                    self.remove_line(line, BT_FALL_OUT_LEDGE, self.width - BT_FALL_OUT_LEDGE);
                }
            }
            WeaponToken::Bottle => {
                for x in 0..BT_BOTTLE_X {
                    for y in (h / 2 - BT_BOTTLE_Y)..(h / 2 + BT_BOTTLE_Y) {
                        self.set(x, y, Some(Cell::structure()));
                        self.set(self.width - x - 1, y, Some(Cell::structure()));
                    }
                }
                self.check_lines();
            }
            WeaponToken::RiseUp => self.insert_line(rng),
            _ => {}
        }
    }

    /// The board mutation for a weapon turning off (`BT_WPN_OFF`).
    pub fn revert_weapon(&mut self, token: WeaponToken) {
        let h = BT_BOARD_HGT;
        match token {
            WeaponToken::Upbyside => {
                self.upside = false;
                if !self.computer {
                    self.flip_horiz();
                }
            }
            WeaponToken::Bottle => {
                for x in 0..BT_BOTTLE_X {
                    for y in (h / 2 - BT_BOTTLE_Y)..(h / 2 + BT_BOTTLE_Y) {
                        self.set(x, y, None);
                        self.set(self.width - x - 1, y, None);
                    }
                }
            }
            _ => {}
        }
    }
}

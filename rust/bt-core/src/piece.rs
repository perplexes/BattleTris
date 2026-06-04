//! Falling pieces — the faithful analogue of `BTPiece` and its subclasses in
//! `usr/src/game/BTPiece.{H,C}`.
//!
//! Each piece carries an 8x8 grid of cells (`map_` in the original), an
//! absolute board position `(x, y)`, a `color`, a rotation extent `rot`
//! (0 = no rotation, otherwise the side length of the rotated sub-square), and
//! `orientation`/`orientations`/`state` for rotation bookkeeping. The grid is
//! indexed `cells[x_local][y_local]`, matching C++ `map_[i][j]`.
//!
//! ## Contract (do not change these public signatures — other modules depend
//! on them; only fill in the bodies):
//!   * [`PieceKind`] + `id`/`from_id`
//!   * [`Piece`] fields and the listed methods
//!
//! `construct` takes a `die_value` (1..=6) used only by [`PieceKind::Die`];
//! callers compute it via the RNG so this module stays RNG-independent.

use crate::board::Board;
use crate::cell::Cell;
use crate::constants::*;

/// The 18 piece kinds, in `BT_*_PIECE` id order (see `BTConstants.H`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PieceKind {
    El,          // BT_EL_PIECE = 1
    RevEl,       // BT_REL_PIECE = 2
    SlideRight,  // BT_SL_RT_PIECE = 3
    SlideLeft,   // BT_SL_LF_PIECE = 4
    Long,        // BT_LONG_PIECE = 5
    Plug,        // BT_PLUG_PIECE = 6
    Box,         // BT_BOX_PIECE = 7
    Die,         // BT_DIE_PIECE = 8
    Happy,       // BT_HAP_PIECE = 9
    Dog,         // BT_DOG_PIECE = 10
    RevDog,      // BT_RDOG_PIECE = 11
    Cap,         // BT_CAP_PIECE = 12
    Wall,        // BT_WALL_PIECE = 13
    Tower,       // BT_TOWER_PIECE = 14
    Star,        // BT_STAR_PIECE = 15
    WeirdLong,   // BT_WLONG_PIECE = 16
    FourByFour,  // BT_4X4_PIECE = 17
    LongDong,    // BT_LONG_DONG_PIECE = 18
}

impl PieceKind {
    /// The `BT_*_PIECE` id (1..=18) used to index `keep_prob_`.
    pub fn id(self) -> i32 {
        match self {
            PieceKind::El => BT_EL_PIECE,
            PieceKind::RevEl => BT_REL_PIECE,
            PieceKind::SlideRight => BT_SL_RT_PIECE,
            PieceKind::SlideLeft => BT_SL_LF_PIECE,
            PieceKind::Long => BT_LONG_PIECE,
            PieceKind::Plug => BT_PLUG_PIECE,
            PieceKind::Box => BT_BOX_PIECE,
            PieceKind::Die => BT_DIE_PIECE,
            PieceKind::Happy => BT_HAP_PIECE,
            PieceKind::Dog => BT_DOG_PIECE,
            PieceKind::RevDog => BT_RDOG_PIECE,
            PieceKind::Cap => BT_CAP_PIECE,
            PieceKind::Wall => BT_WALL_PIECE,
            PieceKind::Tower => BT_TOWER_PIECE,
            PieceKind::Star => BT_STAR_PIECE,
            PieceKind::WeirdLong => BT_WLONG_PIECE,
            PieceKind::FourByFour => BT_4X4_PIECE,
            PieceKind::LongDong => BT_LONG_DONG_PIECE,
        }
    }

    pub fn from_id(id: i32) -> Option<PieceKind> {
        Some(match id {
            BT_EL_PIECE => PieceKind::El,
            BT_REL_PIECE => PieceKind::RevEl,
            BT_SL_RT_PIECE => PieceKind::SlideRight,
            BT_SL_LF_PIECE => PieceKind::SlideLeft,
            BT_LONG_PIECE => PieceKind::Long,
            BT_PLUG_PIECE => PieceKind::Plug,
            BT_BOX_PIECE => PieceKind::Box,
            BT_DIE_PIECE => PieceKind::Die,
            BT_HAP_PIECE => PieceKind::Happy,
            BT_DOG_PIECE => PieceKind::Dog,
            BT_RDOG_PIECE => PieceKind::RevDog,
            BT_CAP_PIECE => PieceKind::Cap,
            BT_WALL_PIECE => PieceKind::Wall,
            BT_TOWER_PIECE => PieceKind::Tower,
            BT_STAR_PIECE => PieceKind::Star,
            BT_WLONG_PIECE => PieceKind::WeirdLong,
            BT_4X4_PIECE => PieceKind::FourByFour,
            BT_LONG_DONG_PIECE => PieceKind::LongDong,
            _ => return None,
        })
    }
}

/// A falling piece. `cells[i][j]` is the local grid (`map_[i][j]`); a `Some`
/// entry is an occupied square of the piece at board position `(x + i, y + j)`.
#[derive(Clone, Debug)]
pub struct Piece {
    pub kind: PieceKind,
    pub x: i32,
    pub y: i32,
    pub color: i32,
    /// `rot_`: 0 = cannot rotate; otherwise the side length of the rotated
    /// sub-square (3 for most, 4 for Long/Cap/Wall/WeirdLong, 8 for LongDong).
    pub rot: usize,
    /// `orientation_`.
    pub orientation: i32,
    /// `orientations_`: 4 for most, 2 for Star, 6 for WeirdLong.
    pub orientations: i32,
    /// `state_` for the custom-rotation pieces (Wall, Star, WeirdLong).
    pub state: i32,
    /// `map_[BT_PIECE_WIDTH][BT_PIECE_HEIGHT]`, indexed `cells[x][y]`.
    pub cells: [[Option<Cell>; BT_PIECE_HEIGHT]; BT_PIECE_WIDTH],
}

impl Piece {
    /// Build a fresh piece of `kind` with its top-left at board `(x, y)`.
    /// `die_value` (1..=6) is used only for [`PieceKind::Die`].
    ///
    /// Faithful to each `BT*Piece::BT*Piece` ctor + `construct` in `BTPiece.C`:
    /// sets `color`, `rot`, `orientations`, `state`, and fills `cells`.
    pub fn construct(kind: PieceKind, x: i32, y: i32, die_value: u8) -> Piece {
        let mut cells = [[None; BT_PIECE_HEIGHT]; BT_PIECE_WIDTH];
        let (color, rot, orientations, state) = match kind {
            PieceKind::El => {
                cells[1][0] = Some(Cell::color(2));
                cells[1][1] = Some(Cell::color(2));
                cells[1][2] = Some(Cell::color(2));
                cells[2][2] = Some(Cell::color(2));
                (2, 3, 4, 0)
            }
            PieceKind::RevEl => {
                cells[2][0] = Some(Cell::color(3));
                cells[2][1] = Some(Cell::color(3));
                cells[2][2] = Some(Cell::color(3));
                cells[1][2] = Some(Cell::color(3));
                (3, 3, 4, 0)
            }
            PieceKind::SlideLeft => {
                cells[0][1] = Some(Cell::color(5));
                cells[1][1] = Some(Cell::color(5));
                cells[1][2] = Some(Cell::color(5));
                cells[2][2] = Some(Cell::color(5));
                (5, 3, 4, 0)
            }
            PieceKind::SlideRight => {
                cells[0][2] = Some(Cell::color(4));
                cells[1][2] = Some(Cell::color(4));
                cells[1][1] = Some(Cell::color(4));
                cells[2][1] = Some(Cell::color(4));
                (4, 3, 4, 0)
            }
            PieceKind::Long => {
                cells[0][1] = Some(Cell::color(6));
                cells[1][1] = Some(Cell::color(6));
                cells[2][1] = Some(Cell::color(6));
                cells[3][1] = Some(Cell::color(6));
                (6, 4, 4, 0)
            }
            PieceKind::Plug => {
                cells[0][2] = Some(Cell::color(7));
                cells[1][2] = Some(Cell::color(7));
                cells[1][1] = Some(Cell::color(7));
                cells[2][2] = Some(Cell::color(7));
                (7, 3, 4, 0)
            }
            PieceKind::Box => {
                cells[1][1] = Some(Cell::color(8));
                cells[1][2] = Some(Cell::color(8));
                cells[2][1] = Some(Cell::color(8));
                cells[2][2] = Some(Cell::color(8));
                (8, 0, 4, 0)
            }
            PieceKind::Die => {
                cells[1][1] = Some(Cell::die(die_value));
                (BT_IVORY, 0, 4, 0)
            }
            PieceKind::Happy => {
                cells[1][1] = Some(Cell::happy());
                (BT_IVORY, 0, 4, 0)
            }
            PieceKind::Dog => {
                cells[0][0] = Some(Cell::color(2));
                cells[1][1] = Some(Cell::color(2));
                cells[2][1] = Some(Cell::color(2));
                cells[2][2] = Some(Cell::color(2));
                (2, 3, 4, 0)
            }
            PieceKind::RevDog => {
                cells[0][1] = Some(Cell::color(3));
                cells[0][2] = Some(Cell::color(3));
                cells[1][1] = Some(Cell::color(3));
                cells[2][2] = Some(Cell::color(3));
                (3, 3, 4, 0)
            }
            PieceKind::Cap => {
                cells[0][2] = Some(Cell::color(4));
                cells[1][1] = Some(Cell::color(4));
                cells[2][1] = Some(Cell::color(4));
                cells[3][2] = Some(Cell::color(4));
                (4, 4, 4, 0)
            }
            PieceKind::Wall => {
                cells[0][1] = Some(Cell::color(5));
                cells[0][2] = Some(Cell::color(5));
                cells[3][1] = Some(Cell::color(5));
                cells[3][2] = Some(Cell::color(5));
                (5, 4, 4, 0)
            }
            PieceKind::Tower => {
                cells[2][0] = Some(Cell::color(6));
                cells[1][1] = Some(Cell::color(6));
                cells[0][1] = Some(Cell::color(6));
                cells[2][2] = Some(Cell::color(6));
                (6, 3, 4, 0)
            }
            PieceKind::Star => {
                cells[1][0] = Some(Cell::color(7));
                cells[0][1] = Some(Cell::color(7));
                cells[1][2] = Some(Cell::color(7));
                cells[2][1] = Some(Cell::color(7));
                (7, 3, 2, 0)
            }
            PieceKind::WeirdLong => {
                cells[1][0] = Some(Cell::color(8));
                cells[1][1] = Some(Cell::color(8));
                cells[2][2] = Some(Cell::color(8));
                cells[2][3] = Some(Cell::color(8));
                (8, 4, 6, 0)
            }
            PieceKind::FourByFour => {
                // Top row
                cells[0][0] = Some(Cell::color(8));
                cells[1][0] = Some(Cell::color(8));
                cells[2][0] = Some(Cell::color(8));
                cells[3][0] = Some(Cell::color(8));
                // Bottom row
                cells[0][3] = Some(Cell::color(8));
                cells[1][3] = Some(Cell::color(8));
                cells[2][3] = Some(Cell::color(8));
                cells[3][3] = Some(Cell::color(8));
                // Left side (middle)
                cells[0][1] = Some(Cell::color(8));
                cells[0][2] = Some(Cell::color(8));
                // Right side (middle)
                cells[3][1] = Some(Cell::color(8));
                cells[3][2] = Some(Cell::color(8));
                (8, 0, 4, 0)
            }
            PieceKind::LongDong => {
                cells[0][0] = Some(Cell::color(6));
                cells[1][0] = Some(Cell::color(6));
                cells[2][0] = Some(Cell::color(6));
                cells[3][0] = Some(Cell::color(6));
                cells[4][0] = Some(Cell::color(6));
                cells[5][0] = Some(Cell::color(6));
                cells[6][0] = Some(Cell::color(6));
                cells[7][0] = Some(Cell::color(6));
                (6, 8, 4, 0)
            }
        };

        Piece {
            kind,
            x,
            y,
            color,
            rot,
            orientation: 0,
            orientations,
            state,
            cells,
        }
    }

    /// `BTPiece::isMapped`.
    pub fn is_mapped(&self, x: usize, y: usize) -> bool {
        self.cells[x][y].is_some()
    }

    /// `BTPiece::canMoveTo` — can the piece occupy board position `(x, y)`?
    pub fn can_move_to(&self, board: &Board, x: i32, y: i32) -> bool {
        for i in 0..BT_PIECE_WIDTH {
            for j in 0..BT_PIECE_HEIGHT {
                if self.cells[i][j].is_some() && board.occupied(x + i as i32, y + j as i32) {
                    return false;
                }
            }
        }
        true
    }

    /// `BTPiece::moveTo` — move if legal; returns false (and does nothing) if
    /// blocked.
    pub fn move_to(&mut self, board: &Board, x: i32, y: i32) -> bool {
        if self.can_move_to(board, x, y) {
            self.x = x;
            self.y = y;
            true
        } else {
            false
        }
    }

    /// `BTPiece::canRotate`.
    pub fn can_rotate(&self, board: &Board, x: i32, y: i32) -> bool {
        if self.rot == 0 {
            return false;
        }
        for i in 0..self.rot {
            for j in 0..self.rot {
                if self.cells[self.rot - 1 - j][i].is_some()
                    && board.occupied(x + i as i32, y + j as i32)
                {
                    return false;
                }
            }
        }
        true
    }

    /// `BTPiece::rotate` (and the Wall/Star/WeirdLong overrides). `reverse`
    /// rotates counter-clockwise. Returns false if blocked.
    pub fn rotate(&mut self, board: &Board, reverse: bool) -> bool {
        // Special cases for Wall, Star, WeirdLong; generic for all others
        match self.kind {
            PieceKind::Wall => self.rotate_wall(board, reverse),
            PieceKind::Star => self.rotate_star(board, reverse),
            PieceKind::WeirdLong => self.rotate_weirding(board, reverse),
            _ => self.rotate_generic(board, reverse),
        }
    }

    fn rotate_generic(&mut self, board: &Board, reverse: bool) -> bool {
        if self.rot == 0 {
            return false;
        }

        // Build the rotated map
        let mut rot_map = [[None; BT_PIECE_HEIGHT]; BT_PIECE_WIDTH];
        for i in 0..self.rot {
            for j in 0..self.rot {
                rot_map[i][j] = if reverse {
                    self.cells[j][self.rot - 1 - i]
                } else {
                    self.cells[self.rot - 1 - j][i]
                };

                // Check for conflicts
                if rot_map[i][j].is_some()
                    && board.occupied(self.x + i as i32, self.y + j as i32)
                {
                    return false;
                }
            }
        }

        // Write back the rotated subgrid
        for i in 0..self.rot {
            for j in 0..self.rot {
                self.cells[i][j] = rot_map[i][j];
            }
        }

        // Update orientation
        if reverse {
            self.orientation = (self.orientation - 1 + self.orientations) % self.orientations;
        } else {
            self.orientation = (self.orientation + 1) % self.orientations;
        }

        true
    }

    fn rotate_wall(&mut self, board: &Board, reverse: bool) -> bool {
        let new_state = if reverse {
            (self.state - 1 + self.orientations) % self.orientations
        } else {
            (self.state + 1) % self.orientations
        };

        match new_state {
            0 => {
                if !reverse {
                    if board.occupied(self.x, self.y + 2)
                        || board.occupied(self.x + 3, self.y + 1)
                    {
                        return false;
                    }
                    self.cells[0][2] = self.cells[1][0];
                    self.cells[1][0] = None;
                    self.cells[3][1] = self.cells[2][3];
                    self.cells[2][3] = None;
                } else {
                    if board.occupied(self.x, self.y + 1)
                        || board.occupied(self.x + 3, self.y + 2)
                    {
                        return false;
                    }
                    self.cells[0][1] = self.cells[1][3];
                    self.cells[1][3] = None;
                    self.cells[3][2] = self.cells[2][0];
                    self.cells[2][0] = None;
                }
            }
            1 => {
                if !reverse {
                    if board.occupied(self.x + 1, self.y + 3)
                        || board.occupied(self.x + 2, self.y)
                    {
                        return false;
                    }
                    self.cells[1][3] = self.cells[0][1];
                    self.cells[0][1] = None;
                    self.cells[2][0] = self.cells[3][2];
                    self.cells[3][2] = None;
                } else {
                    if board.occupied(self.x, self.y + 2)
                        || board.occupied(self.x + 3, self.y + 1)
                    {
                        return false;
                    }
                    self.cells[0][2] = self.cells[2][3];
                    self.cells[2][3] = None;
                    self.cells[3][1] = self.cells[1][0];
                    self.cells[1][0] = None;
                }
            }
            2 => {
                if !reverse {
                    if board.occupied(self.x + 2, self.y + 3)
                        || board.occupied(self.x + 1, self.y)
                    {
                        return false;
                    }
                    self.cells[2][3] = self.cells[0][2];
                    self.cells[0][2] = None;
                    self.cells[1][0] = self.cells[3][1];
                    self.cells[3][1] = None;
                } else {
                    if board.occupied(self.x + 2, self.y) || board.occupied(self.x + 1, self.y + 3) {
                        return false;
                    }
                    self.cells[2][0] = self.cells[0][1];
                    self.cells[0][1] = None;
                    self.cells[1][3] = self.cells[3][2];
                    self.cells[3][2] = None;
                }
            }
            3 => {
                if !reverse {
                    if board.occupied(self.x, self.y + 1)
                        || board.occupied(self.x + 3, self.y + 2)
                    {
                        return false;
                    }
                    self.cells[0][1] = self.cells[2][0];
                    self.cells[2][0] = None;
                    self.cells[3][2] = self.cells[1][3];
                    self.cells[1][3] = None;
                } else {
                    if board.occupied(self.x + 1, self.y) || board.occupied(self.x + 2, self.y + 3) {
                        return false;
                    }
                    self.cells[1][0] = self.cells[0][2];
                    self.cells[0][2] = None;
                    self.cells[2][3] = self.cells[3][1];
                    self.cells[3][1] = None;
                }
            }
            _ => {}
        }

        self.state = new_state;
        if reverse {
            self.orientation = (self.orientation - 1 + self.orientations) % self.orientations;
        } else {
            self.orientation = (self.orientation + 1) % self.orientations;
        }
        true
    }

    fn rotate_star(&mut self, board: &Board, _reverse: bool) -> bool {
        // Star ignores rotation direction (faithful to BTStarPiece::rotate).
        if self.state == 0 {
            if board.occupied(self.x, self.y)
                || board.occupied(self.x + 2, self.y)
                || board.occupied(self.x, self.y + 2)
                || board.occupied(self.x + 2, self.y + 2)
            {
                return false;
            }
            self.cells[0][0] = self.cells[1][0];
            self.cells[1][0] = None;
            self.cells[2][0] = self.cells[2][1];
            self.cells[2][1] = None;
            self.cells[0][2] = self.cells[0][1];
            self.cells[0][1] = None;
            self.cells[2][2] = self.cells[1][2];
            self.cells[1][2] = None;
        } else {
            if board.occupied(self.x + 1, self.y)
                || board.occupied(self.x, self.y + 1)
                || board.occupied(self.x + 1, self.y + 2)
                || board.occupied(self.x + 2, self.y + 1)
            {
                return false;
            }
            self.cells[1][0] = self.cells[0][0];
            self.cells[0][0] = None;
            self.cells[0][1] = self.cells[2][0];
            self.cells[2][0] = None;
            self.cells[1][2] = self.cells[0][2];
            self.cells[0][2] = None;
            self.cells[2][1] = self.cells[2][2];
            self.cells[2][2] = None;
        }
        // NB: faithful to BTStarPiece::rotate — it advances only `state_`,
        // never `orientation_` (unlike Wall/WeirdLong).
        self.state = (self.state + 1) % 2;
        true
    }

    fn rotate_weirding(&mut self, board: &Board, reverse: bool) -> bool {
        let new_state = if reverse {
            (self.state - 1 + self.orientations) % self.orientations
        } else {
            (self.state + 1) % self.orientations
        };

        match new_state {
            0 => {
                if !reverse {
                    if board.occupied(self.x + 1, self.y)
                        || board.occupied(self.x + 1, self.y + 1)
                        || board.occupied(self.x + 2, self.y + 2)
                        || board.occupied(self.x + 2, self.y + 3)
                    {
                        return false;
                    }
                    self.cells[1][0] = self.cells[2][0];
                    self.cells[2][0] = None;
                    self.cells[1][1] = self.cells[2][1];
                    self.cells[2][1] = None;
                    self.cells[2][2] = self.cells[1][2];
                    self.cells[1][2] = None;
                    self.cells[2][3] = self.cells[1][3];
                    self.cells[1][3] = None;
                } else {
                    if board.occupied(self.x + 1, self.y) || board.occupied(self.x + 2, self.y + 3) {
                        return false;
                    }
                    self.cells[1][0] = self.cells[0][0];
                    self.cells[0][0] = None;
                    self.cells[2][3] = self.cells[3][3];
                    self.cells[3][3] = None;
                }
            }
            1 => {
                if !reverse {
                    if board.occupied(self.x, self.y) || board.occupied(self.x + 3, self.y + 3) {
                        return false;
                    }
                    self.cells[0][0] = self.cells[1][0];
                    self.cells[1][0] = None;
                    self.cells[3][3] = self.cells[2][3];
                    self.cells[2][3] = None;
                } else {
                    if board.occupied(self.x, self.y) || board.occupied(self.x + 3, self.y + 3) {
                        return false;
                    }
                    self.cells[0][0] = self.cells[0][1];
                    self.cells[0][1] = None;
                    self.cells[3][3] = self.cells[3][2];
                    self.cells[3][2] = None;
                }
            }
            2 => {
                if !reverse {
                    if board.occupied(self.x, self.y + 1) || board.occupied(self.x + 3, self.y + 2) {
                        return false;
                    }
                    self.cells[0][1] = self.cells[0][0];
                    self.cells[0][0] = None;
                    self.cells[3][2] = self.cells[3][3];
                    self.cells[3][3] = None;
                } else {
                    if board.occupied(self.x, self.y + 1)
                        || board.occupied(self.x + 1, self.y + 1)
                        || board.occupied(self.x + 2, self.y + 2)
                        || board.occupied(self.x + 3, self.y + 2)
                    {
                        return false;
                    }
                    self.cells[0][1] = self.cells[0][2];
                    self.cells[0][2] = None;
                    self.cells[1][1] = self.cells[1][2];
                    self.cells[1][2] = None;
                    self.cells[2][2] = self.cells[2][1];
                    self.cells[2][1] = None;
                    self.cells[3][2] = self.cells[3][1];
                    self.cells[3][1] = None;
                }
            }
            3 => {
                if !reverse {
                    if board.occupied(self.x, self.y + 2)
                        || board.occupied(self.x + 1, self.y + 2)
                        || board.occupied(self.x + 2, self.y + 1)
                        || board.occupied(self.x + 3, self.y + 1)
                    {
                        return false;
                    }
                    self.cells[0][2] = self.cells[0][1];
                    self.cells[0][1] = None;
                    self.cells[1][2] = self.cells[1][1];
                    self.cells[1][1] = None;
                    self.cells[2][1] = self.cells[2][2];
                    self.cells[2][2] = None;
                    self.cells[3][1] = self.cells[3][2];
                    self.cells[3][2] = None;
                } else {
                    if board.occupied(self.x + 3, self.y + 1) || board.occupied(self.x, self.y + 2) {
                        return false;
                    }
                    self.cells[3][1] = self.cells[3][0];
                    self.cells[3][0] = None;
                    self.cells[0][2] = self.cells[0][3];
                    self.cells[0][3] = None;
                }
            }
            4 => {
                if !reverse {
                    if board.occupied(self.x + 3, self.y) || board.occupied(self.x, self.y + 3) {
                        return false;
                    }
                    self.cells[3][0] = self.cells[3][1];
                    self.cells[3][1] = None;
                    self.cells[0][3] = self.cells[0][2];
                    self.cells[0][2] = None;
                } else {
                    if board.occupied(self.x + 3, self.y) || board.occupied(self.x, self.y + 3) {
                        return false;
                    }
                    self.cells[3][0] = self.cells[2][0];
                    self.cells[2][0] = None;
                    self.cells[0][3] = self.cells[1][3];
                    self.cells[1][3] = None;
                }
            }
            5 => {
                if !reverse {
                    if board.occupied(self.x + 2, self.y) || board.occupied(self.x + 1, self.y + 3) {
                        return false;
                    }
                    self.cells[2][0] = self.cells[3][0];
                    self.cells[3][0] = None;
                    self.cells[1][3] = self.cells[0][3];
                    self.cells[0][3] = None;
                } else {
                    if board.occupied(self.x + 2, self.y)
                        || board.occupied(self.x + 2, self.y + 1)
                        || board.occupied(self.x + 1, self.y + 2)
                        || board.occupied(self.x + 1, self.y + 3)
                    {
                        return false;
                    }
                    self.cells[2][0] = self.cells[1][0];
                    self.cells[1][0] = None;
                    self.cells[2][1] = self.cells[1][1];
                    self.cells[1][1] = None;
                    self.cells[1][2] = self.cells[2][2];
                    self.cells[2][2] = None;
                    self.cells[1][3] = self.cells[2][3];
                    self.cells[2][3] = None;
                }
            }
            _ => {}
        }

        self.state = new_state;
        if reverse {
            self.orientation = (self.orientation - 1 + self.orientations) % self.orientations;
        } else {
            self.orientation = (self.orientation + 1) % self.orientations;
        }
        true
    }

    /// `BTPiece::reset` — clears the grid and orientation (position handled by
    /// the caller / `construct`).
    pub fn reset(&mut self) {
        for i in 0..BT_PIECE_WIDTH {
            for j in 0..BT_PIECE_HEIGHT {
                self.cells[i][j] = None;
            }
        }
        self.orientation = 0;
        self.state = 0;
    }

    /// `BTPiece::landed` — copy the piece's cells into the board and run the
    /// board's idiot/landing bookkeeping. Mirrors `BTPiece::landed`, which
    /// fills each occupied square via `board.fill` then calls `board.landed`.
    pub fn land(&mut self, board: &mut Board) {
        for i in 0..BT_PIECE_WIDTH {
            for j in 0..BT_PIECE_HEIGHT {
                if let Some(cell) = self.cells[i][j] {
                    board.fill(self.x + i as i32, self.y + j as i32, cell);
                }
            }
        }
        board.landed(self.x, self.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    fn count_cells(piece: &Piece) -> usize {
        piece
            .cells
            .iter()
            .flatten()
            .filter(|c| c.is_some())
            .count()
    }

    #[test]
    fn test_el_construct() {
        let piece = Piece::construct(PieceKind::El, 10, 10, 1);
        assert_eq!(piece.x, 10);
        assert_eq!(piece.y, 10);
        assert_eq!(piece.color, 2);
        assert_eq!(piece.rot, 3);
        assert_eq!(piece.orientations, 4);
        assert_eq!(count_cells(&piece), 4);
        assert!(piece.is_mapped(1, 0));
        assert!(piece.is_mapped(1, 1));
        assert!(piece.is_mapped(1, 2));
        assert!(piece.is_mapped(2, 2));
    }

    #[test]
    fn test_rel_construct() {
        let piece = Piece::construct(PieceKind::RevEl, 10, 10, 1);
        assert_eq!(piece.color, 3);
        assert_eq!(count_cells(&piece), 4);
        assert!(piece.is_mapped(2, 0));
        assert!(piece.is_mapped(2, 1));
        assert!(piece.is_mapped(2, 2));
        assert!(piece.is_mapped(1, 2));
    }

    #[test]
    fn test_long_construct() {
        let piece = Piece::construct(PieceKind::Long, 10, 10, 1);
        assert_eq!(piece.color, 6);
        assert_eq!(piece.rot, 4);
        assert_eq!(count_cells(&piece), 4);
        assert!(piece.is_mapped(0, 1));
        assert!(piece.is_mapped(1, 1));
        assert!(piece.is_mapped(2, 1));
        assert!(piece.is_mapped(3, 1));
    }

    #[test]
    fn test_long_dong_construct() {
        let piece = Piece::construct(PieceKind::LongDong, 10, 10, 1);
        assert_eq!(piece.color, 6);
        assert_eq!(piece.rot, 8);
        assert_eq!(count_cells(&piece), 8);
        for i in 0..8 {
            assert!(piece.is_mapped(i, 0));
        }
    }

    #[test]
    fn test_four_by_four_construct() {
        let piece = Piece::construct(PieceKind::FourByFour, 10, 10, 1);
        assert_eq!(count_cells(&piece), 12);
        // Top row
        assert!(piece.is_mapped(0, 0));
        assert!(piece.is_mapped(1, 0));
        assert!(piece.is_mapped(2, 0));
        assert!(piece.is_mapped(3, 0));
        // Bottom row
        assert!(piece.is_mapped(0, 3));
        assert!(piece.is_mapped(1, 3));
        assert!(piece.is_mapped(2, 3));
        assert!(piece.is_mapped(3, 3));
        // Left and right sides
        assert!(piece.is_mapped(0, 1));
        assert!(piece.is_mapped(0, 2));
        assert!(piece.is_mapped(3, 1));
        assert!(piece.is_mapped(3, 2));
    }

    #[test]
    fn test_die_construct() {
        let piece = Piece::construct(PieceKind::Die, 10, 10, 3);
        assert_eq!(piece.color, BT_IVORY);
        assert_eq!(piece.rot, 0);
        assert_eq!(count_cells(&piece), 1);
        assert!(piece.is_mapped(1, 1));
        if let Some(cell) = piece.cells[1][1] {
            match cell.kind {
                crate::cell::CellKind::Die(v) => assert_eq!(v, 3),
                _ => panic!("Expected Die cell"),
            }
        } else {
            panic!("Expected Some cell at (1, 1)");
        }
    }

    #[test]
    fn test_happy_construct() {
        let piece = Piece::construct(PieceKind::Happy, 10, 10, 1);
        assert_eq!(piece.color, BT_IVORY);
        assert_eq!(piece.rot, 0);
        assert_eq!(count_cells(&piece), 1);
        assert!(piece.is_mapped(1, 1));
        if let Some(cell) = piece.cells[1][1] {
            match cell.kind {
                crate::cell::CellKind::Happy { landed } => assert!(!landed),
                _ => panic!("Expected Happy cell"),
            }
        } else {
            panic!("Expected Some cell at (1, 1)");
        }
    }

    #[test]
    fn test_rotate_returns_false_for_no_rot() {
        let board = Board::new(40, 40, false);
        let mut piece = Piece::construct(PieceKind::Box, 10, 10, 1);
        assert_eq!(piece.rot, 0);
        assert!(!piece.rotate(&board, false));
    }

    #[test]
    fn test_rotate_cycle_el() {
        let board = Board::new(40, 40, false);
        let mut piece = Piece::construct(PieceKind::El, 10, 10, 1);
        let original = piece.cells;

        // Rotate 4 times should restore
        for _ in 0..4 {
            assert!(piece.rotate(&board, false));
        }
        assert_eq!(piece.cells, original);
    }

    #[test]
    fn test_rotate_cycle_long() {
        let board = Board::new(40, 40, false);
        let mut piece = Piece::construct(PieceKind::Long, 10, 10, 1);
        let original = piece.cells;

        for _ in 0..4 {
            assert!(piece.rotate(&board, false));
        }
        assert_eq!(piece.cells, original);
    }

    #[test]
    fn test_rotate_cycle_tower() {
        let board = Board::new(40, 40, false);
        let mut piece = Piece::construct(PieceKind::Tower, 10, 10, 1);
        let original = piece.cells;

        for _ in 0..4 {
            assert!(piece.rotate(&board, false));
        }
        assert_eq!(piece.cells, original);
    }

    #[test]
    fn test_rotate_cycle_star() {
        let board = Board::new(40, 40, false);
        let mut piece = Piece::construct(PieceKind::Star, 10, 10, 1);
        let original = piece.cells;

        // Star has orientations=2
        for _ in 0..2 {
            assert!(piece.rotate(&board, false));
        }
        assert_eq!(piece.cells, original);
    }

    #[test]
    fn test_rotate_cycle_weirding() {
        let board = Board::new(40, 40, false);
        let mut piece = Piece::construct(PieceKind::WeirdLong, 10, 10, 1);
        let original = piece.cells;

        // WeirdLong has orientations=6
        for _ in 0..6 {
            assert!(piece.rotate(&board, false));
        }
        assert_eq!(piece.cells, original);
    }

    #[test]
    fn test_rotate_cycle_wall() {
        let board = Board::new(40, 40, false);
        let mut piece = Piece::construct(PieceKind::Wall, 10, 10, 1);
        let original = piece.cells;

        // Wall has orientations=4
        for _ in 0..4 {
            assert!(piece.rotate(&board, false));
        }
        assert_eq!(piece.cells, original);
    }

    #[test]
    fn test_move_to_empty_space() {
        let board = Board::new(40, 40, false);
        let mut piece = Piece::construct(PieceKind::Long, 10, 10, 1);
        assert_eq!(piece.x, 10);
        assert_eq!(piece.y, 10);

        assert!(piece.move_to(&board, 15, 20));
        assert_eq!(piece.x, 15);
        assert_eq!(piece.y, 20);
    }

    #[test]
    fn test_move_to_off_left_edge() {
        let board = Board::new(40, 40, false);
        let mut piece = Piece::construct(PieceKind::Long, 10, 10, 1);
        // Long piece has cells at (0, 1), (1, 1), (2, 1), (3, 1)
        // If x = -1, then cell at (0, 1) would be at board (-1, 11) which is off-board
        let result = piece.move_to(&board, -1, 10);
        assert!(!result);
        assert_eq!(piece.x, 10);
        assert_eq!(piece.y, 10);
    }

    #[test]
    fn test_reset() {
        let mut piece = Piece::construct(PieceKind::Long, 10, 10, 1);
        piece.orientation = 2;
        piece.state = 1;

        piece.reset();

        assert_eq!(count_cells(&piece), 0);
        assert_eq!(piece.orientation, 0);
        assert_eq!(piece.state, 0);
    }

    #[test]
    fn test_plug_construct() {
        let piece = Piece::construct(PieceKind::Plug, 10, 10, 1);
        assert_eq!(piece.color, 7);
        assert_eq!(count_cells(&piece), 4);
        assert!(piece.is_mapped(0, 2));
        assert!(piece.is_mapped(1, 2));
        assert!(piece.is_mapped(1, 1));
        assert!(piece.is_mapped(2, 2));
    }

    #[test]
    fn test_box_construct() {
        let piece = Piece::construct(PieceKind::Box, 10, 10, 1);
        assert_eq!(piece.color, 8);
        assert_eq!(piece.rot, 0);
        assert_eq!(count_cells(&piece), 4);
        assert!(piece.is_mapped(1, 1));
        assert!(piece.is_mapped(1, 2));
        assert!(piece.is_mapped(2, 1));
        assert!(piece.is_mapped(2, 2));
    }
}

//! A single occupied board square — the faithful analogue of `BTBox` and its
//! subclasses in `usr/src/game/BTBox.H`.
//!
//! In the original, the board grid is `BTBox ***map_` where a `NULL` pointer
//! means "empty". We model an occupied square as a [`Cell`] and an empty one as
//! `None` (the grid is `Vec<Option<Cell>>`).
//!
//! `BTBox` semantics we reproduce:
//!    * `value()` — funds contribution when cleared (0 for normal boxes, die pip
//!      value, 150 for an un-landed happy face).
//!    * `id()` — render id; returns -1 when `hidden_` (Twilight Zone).
//!    * `isRemoveable()` — false for the bottle-neck structure boxes.
//!    * `landed()` — a happy face that lands without clearing turns into a frown
//!      (`BTHappyBox::landed`), dropping its value to 0.

use crate::constants::*;

/// The kind of box occupying a square, mirroring the `BTBox` subclasses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellKind {
    /// Ordinary colored box; `id == color`, `value == 0`.
    /// Colors are `BT_BLACK..=BT_MAX_COLORS` plus `BT_INVISIBLE`.
    Color(i32),
    /// Die box (`BTDieBox`) with a pip value 1..=6.
    Die(u8),
    /// Happy face (`BTHappyBox`). `landed == false` => value 150 (un-landed,
    /// id `BT_HAPPY`); `landed == true` => value 0 (frown, id `BT_UNHAPPY`).
    Happy { landed: bool },
    /// Bottle-neck structure box (`BTStructureBox`); not removable.
    Structure,
    /// Gimp box (`BTGimpBox`) carrying an underlying value.
    Gimp(i32),
    /// Invisible box (`BTInvisiBox`) with an explicit id + value; used by the
    /// Bug weapon and opponent-board reconstruction.
    Invisible { id: i32, value: i32 },
}

impl CellKind {
    /// Funds contribution — `BTBox::value()` and overrides.
    pub fn value(&self) -> i32 {
        match *self {
            CellKind::Die(v) => v as i32,
            CellKind::Happy { landed } => {
                if landed {
                    0
                } else {
                    BT_HAPPY_VAL
                }
            }
            CellKind::Gimp(v) => v,
            CellKind::Invisible { value, .. } => value,
            CellKind::Color(_) | CellKind::Structure => 0,
        }
    }

    /// Render id ignoring the hidden flag — `BTBox::id()` without `hidden_`.
    pub fn raw_id(&self) -> i32 {
        match *self {
            CellKind::Color(c) => c,
            CellKind::Die(v) => BT_DIE_1 + v as i32 - 1,
            CellKind::Happy { landed } => {
                if landed {
                    BT_UNHAPPY
                } else {
                    BT_HAPPY
                }
            }
            CellKind::Structure => BT_STRUCT,
            CellKind::Gimp(_) => BT_GIMP_ID,
            CellKind::Invisible { id, .. } => id,
        }
    }

    /// `BTBox::isRemoveable()` — structure boxes resist line clears / weapons.
    pub fn is_removable(&self) -> bool {
        !matches!(self, CellKind::Structure)
    }
}

/// An occupied board square: a [`CellKind`] plus the `hidden_` flag.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell {
    pub kind: CellKind,
    /// `hidden_` — set by the Twilight Zone weapon; makes `id()` return -1.
    pub hidden: bool,
}

impl Cell {
    pub fn new(kind: CellKind) -> Self {
        Cell { kind, hidden: false }
    }

    /// A normal colored box.
    pub fn color(color: i32) -> Self {
        Cell::new(CellKind::Color(color))
    }
    /// A die box with `value` pips (1..=6).
    pub fn die(value: u8) -> Self {
        Cell::new(CellKind::Die(value))
    }
    /// A happy face (un-landed by default).
    pub fn happy() -> Self {
        Cell::new(CellKind::Happy { landed: false })
    }
    /// A bottle-neck structure box.
    pub fn structure() -> Self {
        Cell::new(CellKind::Structure)
    }
    pub fn gimp(value: i32) -> Self {
        Cell::new(CellKind::Gimp(value))
    }

    /// `BTBox::value()`.
    pub fn value(&self) -> i32 {
        self.kind.value()
    }

    /// `BTBox::id()` — returns -1 when hidden.
    pub fn id(&self) -> i32 {
        if self.hidden {
            -1
        } else {
            self.kind.raw_id()
        }
    }

    /// `BTBox::isRemoveable()`.
    pub fn is_removable(&self) -> bool {
        self.kind.is_removable()
    }

    /// `BTBox::hide()`.
    pub fn hide(&mut self) {
        self.hidden = true;
    }

    /// `BTHappyBox::landed()` — a happy face that lands becomes a frown.
    pub fn landed(&mut self) {
        if let CellKind::Happy { landed } = &mut self.kind {
            *landed = true;
        }
    }

    /// Encode as `[tag, a, b, hidden]` for cross-player board transfer (Swap and
    /// the spies send a whole grid over the wire). Round-trips via [`Cell::decode`].
    pub fn encode(&self) -> [i32; 4] {
        let h = self.hidden as i32;
        match self.kind {
            CellKind::Color(c) => [1, c, 0, h],
            CellKind::Die(v) => [2, v as i32, 0, h],
            CellKind::Happy { landed } => [3, landed as i32, 0, h],
            CellKind::Structure => [4, 0, 0, h],
            CellKind::Gimp(v) => [5, v, 0, h],
            CellKind::Invisible { id, value } => [6, id, value, h],
        }
    }

    /// Decode `[tag, a, b, hidden]`; `tag == 0` is an empty square (`None`).
    pub fn decode(q: [i32; 4]) -> Option<Cell> {
        let kind = match q[0] {
            1 => CellKind::Color(q[1]),
            2 => CellKind::Die(q[1] as u8),
            3 => CellKind::Happy { landed: q[1] != 0 },
            4 => CellKind::Structure,
            5 => CellKind::Gimp(q[1]),
            6 => CellKind::Invisible { id: q[1], value: q[2] },
            _ => return None,
        };
        Some(Cell { kind, hidden: q[3] != 0 })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_encode_decode_round_trips_every_kind() {
        let cases = [
            Cell::color(3),
            Cell::die(6),
            Cell::happy(),
            Cell::structure(),
            Cell::gimp(5),
            Cell::new(CellKind::Invisible { id: 7, value: 42 }),
            {
                let mut c = Cell::die(4);
                c.hide();
                c
            },
        ];
        for c in cases {
            assert_eq!(Cell::decode(c.encode()), Some(c), "{c:?} must round-trip");
        }
        // tag 0 decodes to an empty square.
        assert_eq!(Cell::decode([0, 0, 0, 0]), None);
    }
}

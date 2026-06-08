//! Constants ported verbatim from the C++ source.
//!
//! Primary source: `usr/src/game/BTConstants.H` (Bryan Cantrill, 1994), plus a
//! few from `BTBoardManager.H` (bottle), `BTGame.H` (timing) and
//! `BTPieceManager.C` (keep probabilities) and `BTScoreManager.C` (bazaar).
//!
//! These are the ground truth for the faithful port — do not "improve" them.

// ---------------------------------------------------------------------------
// Version
// ---------------------------------------------------------------------------
/// Human-readable version banner.
pub const BT_VERSION: &str = "BattleTris v1.0";
pub const BT_MAJOR_VER: i32 = 1;
pub const BT_MINOR_VER: i32 = 0;

// ---------------------------------------------------------------------------
// Ranking
// ---------------------------------------------------------------------------
/// The rank value a brand-new player starts at. The rules engine itself does
/// not rank; this lives here so every consumer agrees on the same baseline.
pub const BT_ELO_START: i64 = 1200;

// ---------------------------------------------------------------------------
// Colors / box ids   (BTConstants.H:29-68)
//
// A box's render id IS its color for ordinary boxes, so colors and box ids
// share one numeric space. The bright colors occupy `0..=9`; each has a "dark"
// twin exactly `BT_MAX_DIF_COLORS` (9) higher, so a single offset converts a
// color to its shaded variant. Non-color box kinds (structure, faces, dice)
// follow above the color range. The renderer keys sprites off these ids; the
// rules engine treats them as opaque tags.
// ---------------------------------------------------------------------------
/// Span of the bright color palette, and the stride to each color's dark twin.
pub const BT_MAX_DIF_COLORS: i32 = 9;

/// Sentinel color for a box that renders nothing — used by the Bug weapon's
/// hidden block and by board reconstruction where the true color is unknown.
pub const BT_INVISIBLE: i32 = -1;
pub const BT_BLACK: i32 = 0;
pub const BT_IVORY: i32 = 1;
pub const BT_YELLOW: i32 = 2;
pub const BT_RED: i32 = 3;
pub const BT_BLUE: i32 = 4;
pub const BT_ORANGE: i32 = 5;
pub const BT_GREEN: i32 = 6;
pub const BT_CYAN: i32 = 7;
pub const BT_PURPLE: i32 = 8;
/// The garbage/neutral fill color (e.g. inserted rise-up rows).
pub const BT_NEUTRAL: i32 = 9;

// Dark twins: each bright color plus the palette stride. Derived rather than
// hard-coded so the two halves of the palette can never drift apart.
pub const BT_GRAY: i32 = BT_IVORY + BT_MAX_DIF_COLORS;
pub const BT_DYELLOW: i32 = BT_YELLOW + BT_MAX_DIF_COLORS;
pub const BT_DRED: i32 = BT_RED + BT_MAX_DIF_COLORS;
pub const BT_DBLUE: i32 = BT_BLUE + BT_MAX_DIF_COLORS;
pub const BT_DORANGE: i32 = BT_ORANGE + BT_MAX_DIF_COLORS;
pub const BT_DGREEN: i32 = BT_GREEN + BT_MAX_DIF_COLORS;
pub const BT_DCYAN: i32 = BT_CYAN + BT_MAX_DIF_COLORS;
pub const BT_DPURPLE: i32 = BT_PURPLE + BT_MAX_DIF_COLORS;
/// One past the last color id — where the non-color box ids begin.
pub const BT_MAX_COLORS: i32 = BT_NEUTRAL + BT_MAX_DIF_COLORS;

/// Bottle-neck structure box — an immovable wall, distinct from any color.
pub const BT_STRUCT: i32 = 20;

/// An un-landed smiley (worth funds); becomes [`BT_UNHAPPY`] once it locks
/// without completing a line.
pub const BT_HAPPY: i32 = 21;
/// A frown — a smiley that landed without paying out.
pub const BT_UNHAPPY: i32 = 22;
pub const BT_GIMP_ID: i32 = 23;

// Die faces are six consecutive ids so a pip value maps to an id by addition.
pub const BT_DIE_1: i32 = 24;
pub const BT_DIE_2: i32 = 25;
pub const BT_DIE_3: i32 = 26;
pub const BT_DIE_4: i32 = 27;
pub const BT_DIE_5: i32 = 28;
pub const BT_DIE_6: i32 = 29;

/// Count of distinct box kinds — the upper bound on any render id.
pub const BT_MAX_BOXES: i32 = 30;

// Box geometry in pixels. The rules engine is resolution-independent; these
// exist so the WASM front-end and the native game agree on the cell size.
pub const BT_BOX_WTH: i32 = 23;
pub const BT_BOX_HGT: i32 = 23;
pub const BT_BOX_BRDR: i32 = 3;

/// Funds an un-landed smiley pays when cleared in a line. Chosen high enough to
/// make catching the smiley a meaningful play (and tempting to bury via a Reagan
/// Era hit right after a Have-a-Nice-Day).
pub const BT_HAPPY_VAL: i32 = 150;

// Id offsets that separate box families when ids are packed onto the wire, so a
// die/face/gimp id never collides with a color id in the same field.
pub const BT_BOX_ID_OFFS: i32 = 0;
pub const BT_DIE_ID_OFFS: i32 = 100;
pub const BT_HAPPY_ID_OFFS: i32 = 200;
pub const BT_GIMP_ID_OFFS: i32 = 300;

// ---------------------------------------------------------------------------
// Board geometry
//
// The playfield is 10 wide and 28 tall. The board is taller than it looks: the
// top rows are spawn/overflow space, so a piece can rotate and settle above the
// visible stack before the top-out test fires.
// ---------------------------------------------------------------------------
pub const BT_BOARD_WTH: i32 = 10;
pub const BT_BOARD_HGT: i32 = 28;

// ---------------------------------------------------------------------------
// Timing (milliseconds)   (BTConstants.H:92-94, BTGame.C)
// ---------------------------------------------------------------------------
/// Gravity interval once fast-drop is engaged — near-instant descent.
pub const BT_FAST_DROP_TIME: i32 = 10;
/// Baseline gravity interval between automatic one-row falls. Weapons scale
/// this (Speedy halves it, Meadow doubles it).
pub const BT_DROP_TIME: i32 = 512;
/// The lock delay: once a piece can fall no further it gets this long to be slid
/// or rotated before it locks, which is what makes the signature "slide" and
/// "airslide" tucks possible. No Slide reduces it to zero (instant lock).
pub const BT_SLIDE_TIME: i32 = 150;

/// Baseline probability a randomly drawn standard piece is kept rather than
/// re-rolled — the knob that shapes the default piece distribution.
pub const BT_BASE_PROB: f64 = 0.21;

// Where a fresh piece's local grid is anchored on the board before its
// rotation extent is centered (see `Game::spawn`).
pub const BT_DEFAULT_X: i32 = 5;
pub const BT_DEFAULT_Y: i32 = 0;

// ---------------------------------------------------------------------------
// Pieces   (BTConstants.H:101-126)
// ---------------------------------------------------------------------------
// Every piece carries an 8x8 local grid even though no piece fills it. The
// uniform extent lets one rotation routine and one collision test serve all
// pieces, from the single-cell die to the eight-wide Long Dong.
pub const BT_PIECE_WIDTH: usize = 8;
pub const BT_PIECE_HEIGHT: usize = 8;

// Piece ids double as indices into the keep-probability table, so they are
// dense and 1-based (index 0 is unused). The blocks below partition the ids
// into the families that selection and the weapons treat as a group.

// Standard pieces — the seven that make up the default stream.
pub const BT_EL_PIECE: i32 = 1;
pub const BT_REL_PIECE: i32 = 2;
pub const BT_SL_RT_PIECE: i32 = 3;
pub const BT_SL_LF_PIECE: i32 = 4;
pub const BT_LONG_PIECE: i32 = 5;
pub const BT_PLUG_PIECE: i32 = 6;
pub const BT_BOX_PIECE: i32 = 7;

// Special single-cell pieces that pay funds.
pub const BT_DIE_PIECE: i32 = 8;
pub const BT_HAP_PIECE: i32 = 9;

/// The boundary above which ids are the "weird" pieces — Feared Weird turns the
/// stream on by zeroing the standard block and enabling everything past here.
pub const BT_WEIRD_OFFS: i32 = 9;
pub const BT_DOG_PIECE: i32 = 10;
pub const BT_RDOG_PIECE: i32 = 11;
pub const BT_CAP_PIECE: i32 = 12;
pub const BT_WALL_PIECE: i32 = 13;
pub const BT_TOWER_PIECE: i32 = 14;
pub const BT_STAR_PIECE: i32 = 15;
pub const BT_WLONG_PIECE: i32 = 16;

pub const BT_4X4_PIECE: i32 = 17;
pub const BT_LONG_DONG_PIECE: i32 = 18;
/// Highest valid piece id; also the upper bound of the selection roll.
pub const BT_MAX_PIECES: i32 = 18;

// Keep probabilities   (BTPieceManager.C:16-19)
//
// Selection rolls a uniform id then keeps it with probability `keep_prob[id]`,
// re-rolling otherwise. A piece's rarity is therefore its keep probability, and
// disabling a piece is just setting its probability to 0 — which is how the
// piece-stream weapons work.
/// Keep probability for the seven standard pieces.
pub const BT_DEFAULT_KEEP_PROB: f64 = 0.21;
/// Keep probability for the rare treats (smiley, Long Dong) — seldom, never never.
pub const BT_EXOTIC_KEEP_PROB: f64 = 0.02;
/// The die is always kept once rolled, so its frequency is purely its 1-in-18
/// roll probability.
pub const BT_DIE_KEEP_PROB: f64 = 1.0;
/// Broken Record reroll divisor: a Broken-cursed stream breaks its repeat only
/// about 1 draw in this many, so the same piece keeps coming.
pub const BT_BROKEN_PROB: i64 = 10;

// ---------------------------------------------------------------------------
// Idiot reasons   (BTConstants.H:129-131)
//
// The "idiot" signal lets the front-end heckle a player. Each value names why
// the engine flagged the last lock; the board sets one as a side effect of
// landing / line-checking.
// ---------------------------------------------------------------------------
/// Sealed an empty square under freshly placed boxes.
pub const BT_BAD_MOVE: i16 = 0;
/// The stack is dangerously high.
pub const BT_NEAR_DEATH: i16 = 1;
/// A smiley landed without completing a line, forfeiting its funds.
pub const BT_MISSED_SMILEY: i16 = 2;

// ---------------------------------------------------------------------------
// Weapons / board structure
// ---------------------------------------------------------------------------
/// Width of the ledge left at each side when Fall Out opens the floor, so the
/// stack has something to rest on rather than emptying entirely.
pub const BT_FALL_OUT_LEDGE: i32 = 2;
/// Distinct weapon slots a player can hold (purchases of the same weapon stack
/// within one slot).
pub const BT_ARSENAL_SIZE: usize = 10;

// Bottle neck   (BTBoardManager.H:12-13)
// The Bottle weapon plants structure walls `BT_BOTTLE_X` cells deep on each
// side across the middle `±BT_BOTTLE_Y` rows, squeezing the playable width to a
// narrow neck there.
pub const BT_BOTTLE_X: i32 = 3;
pub const BT_BOTTLE_Y: i32 = 4;

/// The bazaar opens each time the two players' COMBINED line count crosses a
/// multiple of this — tying shopping to shared progress so both stop together.
pub const BT_LINES_TIL_BAZ: i32 = 20;

/// Mondale '96 skims this fraction of the victim's newly banked funds to the
/// attacker.
pub const BT_MONDALE_RATE: f64 = 0.30;

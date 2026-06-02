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
pub const BT_VERSION: &str = "BattleTris v1.0";
pub const BT_MAJOR_VER: i32 = 1;
pub const BT_MINOR_VER: i32 = 0;

// ---------------------------------------------------------------------------
// Ranking (original ELO; replaced by TrueSkill 2, kept for parity)
// ---------------------------------------------------------------------------
pub const BT_ELO_START: i64 = 1200;

// ---------------------------------------------------------------------------
// Colors / box ids   (BTConstants.H:29-68)
// ---------------------------------------------------------------------------
pub const BT_MAX_DIF_COLORS: i32 = 9;

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
pub const BT_NEUTRAL: i32 = 9;

pub const BT_GRAY: i32 = BT_IVORY + BT_MAX_DIF_COLORS;
pub const BT_DYELLOW: i32 = BT_YELLOW + BT_MAX_DIF_COLORS;
pub const BT_DRED: i32 = BT_RED + BT_MAX_DIF_COLORS;
pub const BT_DBLUE: i32 = BT_BLUE + BT_MAX_DIF_COLORS;
pub const BT_DORANGE: i32 = BT_ORANGE + BT_MAX_DIF_COLORS;
pub const BT_DGREEN: i32 = BT_GREEN + BT_MAX_DIF_COLORS;
pub const BT_DCYAN: i32 = BT_CYAN + BT_MAX_DIF_COLORS;
pub const BT_DPURPLE: i32 = BT_PURPLE + BT_MAX_DIF_COLORS;
pub const BT_MAX_COLORS: i32 = BT_NEUTRAL + BT_MAX_DIF_COLORS;

pub const BT_STRUCT: i32 = 20;

pub const BT_HAPPY: i32 = 21;
pub const BT_UNHAPPY: i32 = 22;
pub const BT_GIMP_ID: i32 = 23;

pub const BT_DIE_1: i32 = 24;
pub const BT_DIE_2: i32 = 25;
pub const BT_DIE_3: i32 = 26;
pub const BT_DIE_4: i32 = 27;
pub const BT_DIE_5: i32 = 28;
pub const BT_DIE_6: i32 = 29;

pub const BT_MAX_BOXES: i32 = 30;

// Box geometry (rendering only; kept for the WASM front-end)
pub const BT_BOX_WTH: i32 = 23;
pub const BT_BOX_HGT: i32 = 23;
pub const BT_BOX_BRDR: i32 = 3;

pub const BT_HAPPY_VAL: i32 = 150;

// Id offsets used by the original box ids over the wire
pub const BT_BOX_ID_OFFS: i32 = 0;
pub const BT_DIE_ID_OFFS: i32 = 100;
pub const BT_HAPPY_ID_OFFS: i32 = 200;
pub const BT_GIMP_ID_OFFS: i32 = 300;

// ---------------------------------------------------------------------------
// Board geometry
// ---------------------------------------------------------------------------
pub const BT_BOARD_WTH: i32 = 10;
pub const BT_BOARD_HGT: i32 = 28;

// ---------------------------------------------------------------------------
// Timing (milliseconds)   (BTConstants.H:92-94, BTGame.C)
// ---------------------------------------------------------------------------
pub const BT_FAST_DROP_TIME: i32 = 10;
pub const BT_DROP_TIME: i32 = 512;
pub const BT_SLIDE_TIME: i32 = 150;

pub const BT_BASE_PROB: f64 = 0.21;

// Spawn position
pub const BT_DEFAULT_X: i32 = 5;
pub const BT_DEFAULT_Y: i32 = 0;

// ---------------------------------------------------------------------------
// Pieces   (BTConstants.H:101-126)
// ---------------------------------------------------------------------------
pub const BT_PIECE_WIDTH: usize = 8;
pub const BT_PIECE_HEIGHT: usize = 8;

pub const BT_EL_PIECE: i32 = 1;
pub const BT_REL_PIECE: i32 = 2;
pub const BT_SL_RT_PIECE: i32 = 3;
pub const BT_SL_LF_PIECE: i32 = 4;
pub const BT_LONG_PIECE: i32 = 5;
pub const BT_PLUG_PIECE: i32 = 6;
pub const BT_BOX_PIECE: i32 = 7;

pub const BT_DIE_PIECE: i32 = 8;
pub const BT_HAP_PIECE: i32 = 9;

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
pub const BT_MAX_PIECES: i32 = 18;

// Keep probabilities   (BTPieceManager.C:16-19)
pub const BT_DEFAULT_KEEP_PROB: f64 = 0.21;
pub const BT_EXOTIC_KEEP_PROB: f64 = 0.02;
pub const BT_DIE_KEEP_PROB: f64 = 1.0;
pub const BT_BROKEN_PROB: i64 = 10;

// ---------------------------------------------------------------------------
// Idiot reasons   (BTConstants.H:129-131)
// ---------------------------------------------------------------------------
pub const BT_BAD_MOVE: i16 = 0;
pub const BT_NEAR_DEATH: i16 = 1;
pub const BT_MISSED_SMILEY: i16 = 2;

// ---------------------------------------------------------------------------
// Weapons / board structure
// ---------------------------------------------------------------------------
pub const BT_FALL_OUT_LEDGE: i32 = 2;
pub const BT_ARSENAL_SIZE: usize = 10;

// Bottle neck   (BTBoardManager.H:12-13)
pub const BT_BOTTLE_X: i32 = 3;
pub const BT_BOTTLE_Y: i32 = 4;

// Bazaar trigger: combined lines between the two players   (BTScoreManager.C)
pub const BT_LINES_TIL_BAZ: i32 = 20;

// Mondale '96 tax rate (BTScoreManager.C: BT_MONDALE_RATE .30)
pub const BT_MONDALE_RATE: f64 = 0.30;

//! `bt-core` — a faithful, deterministic port of the BattleTris game logic.
//!
//! Ported from the 1994 C++ source under `usr/src/game/` (Bryan Cantrill et
//! al.). No platform, UI, or network dependencies — this crate is the pure
//! rules engine consumed by the WASM front-end, the AI, and the netcode.
//!
//! Module map mirrors the original classes:
//!   * [`constants`] — `BTConstants.H` etc.
//!   * [`cell`]      — `BTBox` + subclasses
//!   * [`piece`]     — `BTPiece` + subclasses
//!   * [`board`]     — `BTBoardManager`
//!   * [`weapons`]   — `BTWeaponToken`, `BTActive[]`, `BTWeapon`
//!   * [`rng`]       — `rand` / `drand48` / `lrand48`

pub mod board;
pub mod cell;
pub mod constants;
pub mod game;
pub mod piece;
pub mod piece_manager;
pub mod rng;
pub mod weapons;

pub use board::{Board, LineClear};
pub use cell::{Cell, CellKind};
pub use game::{Game, GameEvent, Score};
pub use piece::{Piece, PieceKind};
pub use piece_manager::PieceManager;
pub use rng::Rng;
pub use weapons::{ActiveFlags, WeaponToken};

//! Core chess representation, bitboards, move generation, FEN, SAN, and PGN.

pub mod bitboard;
pub mod board;
pub mod fen;
pub mod movegen;
pub mod pgn;
pub mod types;
pub mod zobrist;

pub use board::{Board, UndoState};
pub use fen::STARTPOS_FEN;
pub use types::{CastlingRights, Color, CoreError, Move, MoveList, PieceType, Square};

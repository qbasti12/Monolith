//! Monolith UCI chess engine library.

pub mod eval;
pub mod search;
pub mod skill;
pub mod tt;
pub mod uci;

pub use eval::{eval_breakdown, evaluate, EvalBreakdown};
pub use tt::{Bound, TranspositionTable, MATE_BOUND, MATE_SCORE};

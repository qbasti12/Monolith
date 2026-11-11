//! Monolith UCI chess engine library.

pub mod eval;
pub mod search;
pub mod skill;
pub mod tt;
pub mod uci;

pub use eval::{eval_breakdown, evaluate, EvalBreakdown};
pub use search::{Score, SearchInfo, SearchLimits, SearchResult, Searcher, MAX_MULTI_PV, MAX_PLY};
pub use tt::{Bound, TranspositionTable, MATE_BOUND, MATE_SCORE};

//! Monolith UCI chess engine library.

pub mod eval;
pub mod search;
pub mod skill;
pub mod tt;
pub mod uci;

pub use eval::{eval_breakdown, evaluate, see_ge, EvalBreakdown};
pub use search::{Score, SearchInfo, SearchLimits, SearchResult, Searcher, MAX_MULTI_PV, MAX_PLY};
pub use skill::{select_move, SkillProfile, SkillRng};
pub use tt::{Bound, TranspositionTable, MATE_BOUND, MATE_SCORE};
pub use uci::{
    parse_uci_info_line, run_uci_loop, run_uci_stdio, UciError, UciEvent, UciInfo, UciProcess,
};

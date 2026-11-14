//! Search limit configuration, PV/result structs, and zero-alloc helper utilities.

use std::time::{Duration, Instant};

use monolith_core::types::{Color, Move, MoveList, PieceType};
use monolith_core::Board;

use crate::eval::SEE_VALUES;
use crate::tt::{MATE_BOUND, MATE_SCORE};

pub const MAX_PLY: usize = 128;
pub const MAX_MULTI_PV: usize = 16;

/// Search termination limits and mode flags.
#[derive(Debug, Clone)]
pub struct SearchLimits {
    pub max_depth: u8,
    pub max_nodes: Option<u64>,
    pub movetime: Option<Duration>,
    pub wtime: Option<Duration>,
    pub btime: Option<Duration>,
    pub winc: Option<Duration>,
    pub binc: Option<Duration>,
    pub movestogo: Option<u32>,
    pub infinite: bool,
    pub multi_pv: usize,
    pub search_moves: MoveList,
}

impl Default for SearchLimits {
    fn default() -> Self {
        Self {
            max_depth: 64,
            max_nodes: None,
            movetime: None,
            wtime: None,
            btime: None,
            winc: None,
            binc: None,
            movestogo: None,
            infinite: false,
            multi_pv: 1,
            search_moves: MoveList::new(),
        }
    }
}

/// Score formatted as centipawns or mate distance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Score {
    Cp(i32),
    Mate(i32),
}

impl Score {
    pub fn from_raw(raw: i32) -> Self {
        if raw >= MATE_BOUND {
            let plies = (MATE_SCORE - raw).max(1);
            Score::Mate((plies + 1) / 2)
        } else if raw <= -MATE_BOUND {
            let plies = (MATE_SCORE + raw).max(1);
            Score::Mate(-((plies + 1) / 2))
        } else {
            Score::Cp(raw)
        }
    }
}

/// One principal variation line from a search iteration (zero heap allocation).
#[derive(Clone, Copy)]
pub struct PvLine {
    pub moves: [Move; MAX_PLY],
    pub len: usize,
    pub score: i32,
    pub depth: u8,
    pub seldepth: u8,
    pub multipv_idx: usize,
}

impl Default for PvLine {
    fn default() -> Self {
        Self {
            moves: [Move::NONE; MAX_PLY],
            len: 0,
            score: 0,
            depth: 0,
            seldepth: 0,
            multipv_idx: 0,
        }
    }
}

impl PvLine {
    #[inline]
    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len]
    }
}

/// Progress snapshot reported at each completed depth / Multi-PV pass.
#[derive(Clone, Copy)]
pub struct SearchInfo {
    pub depth: u8,
    pub seldepth: u8,
    pub multipv: usize,
    pub score: i32,
    pub nodes: u64,
    pub nps: u64,
    pub hashfull: u32,
    pub time_ms: u64,
    pub pv: PvLine,
}

/// Final search result containing best move, Multi-PV candidates, and statistics.
#[derive(Clone)]
pub struct SearchResult {
    pub best_move: Move,
    pub ponder_move: Move,
    pub score: i32,
    pub depth: u8,
    pub seldepth: u8,
    pub nodes: u64,
    pub time_ms: u64,
    pub pv_lines: [PvLine; MAX_MULTI_PV],
    pub pv_count: usize,
}

#[inline]
pub(crate) fn apply_history_delta(entry: &mut i32, delta: i32) {
    let clamped = delta.clamp(-16_384, 16_384);
    *entry += clamped - (*entry * clamped.abs()) / 16_384;
}

#[inline]
pub(crate) fn pick_best_move(
    list: &mut MoveList,
    scores: &mut [i32; MoveList::CAPACITY],
    start: usize,
) -> Move {
    let len = list.len();
    let mut best_idx = start;
    let mut best_score = scores[start];
    for (j, &sc) in scores.iter().enumerate().take(len).skip(start + 1) {
        if sc > best_score {
            best_score = sc;
            best_idx = j;
        }
    }
    if best_idx != start {
        list.swap(start, best_idx);
        scores.swap(start, best_idx);
    }
    list.get(start).unwrap_or(Move::NONE)
}

#[inline]
pub(crate) fn captured_piece_value(board: &Board, mv: Move) -> i32 {
    if mv.is_en_passant() {
        SEE_VALUES[PieceType::Pawn.index()]
    } else if let Some((_, pt)) = board.piece_on(mv.to_sq()) {
        SEE_VALUES[pt.index()]
    } else {
        0
    }
}

pub(crate) fn compute_deadlines(
    now: Instant,
    stm: Color,
    limits: &SearchLimits,
) -> (Option<Instant>, Option<Instant>) {
    if limits.infinite {
        return (None, None);
    }
    if let Some(mt) = limits.movetime {
        let hard = mt
            .saturating_sub(Duration::from_millis(5))
            .max(Duration::from_millis(1));
        return (Some(now + hard), Some(now + hard));
    }

    let (time_opt, inc_opt) = match stm {
        Color::White => (limits.wtime, limits.winc),
        Color::Black => (limits.btime, limits.binc),
    };

    if let Some(remaining) = time_opt {
        let mtg = limits.movestogo.unwrap_or(28).max(1) as u64;
        let inc_ms = inc_opt.unwrap_or(Duration::ZERO).as_millis() as u64;
        let rem_ms = remaining.as_millis() as u64;
        let move_overhead = 45u64;
        let safe_max = rem_ms.saturating_sub(move_overhead).max(1);
        let base = if rem_ms < 120 {
            (safe_max / 3).max(1)
        } else {
            ((rem_ms / mtg) + (inc_ms * 3) / 4).min(safe_max / 2).max(4)
        };
        let soft = (base * 6 / 10).min(safe_max).max(1);
        let hard = (base * 2).min(safe_max).max(1);
        return (
            Some(now + Duration::from_millis(soft)),
            Some(now + Duration::from_millis(hard)),
        );
    }

    (None, None)
}

//! Alpha-beta search with iterative deepening, PVS, aspiration windows,
//! quiescence search, SEE-light move ordering, NMP, RFP, LMR, and Multi-PV.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use monolith_core::movegen::{generate_legal, generate_noisy};
use monolith_core::types::{Color, Move, MoveList, PieceType};
use monolith_core::Board;

pub use crate::eval::see_ge;
use crate::eval::{evaluate, MG_PIECE_VALUES, SEE_VALUES};
use crate::tt::{Bound, TranspositionTable, MATE_BOUND, MATE_SCORE};

pub const MAX_PLY: usize = 128;
pub const MAX_MULTI_PV: usize = 16;
const INF: i32 = 31_000;

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
    Mate(i32), // positive = engine mates in N moves, negative = mated in N moves
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

#[derive(Clone, Copy, Default)]
struct StackEntry {
    static_eval: i32,
}

/// Reusable chess searcher holding preallocated tables so `search()` performs 0 heap allocations.
pub struct Searcher {
    tt: TranspositionTable,
    killers: Vec<[Move; 2]>,
    history: Box<[[[i32; 64]; 64]; 2]>,
    pv_table: Vec<[Move; MAX_PLY]>,
    pv_len: [usize; MAX_PLY],
    stack: Vec<StackEntry>,
    lmr_table: [[u8; 64]; 64],
    nodes: u64,
    seldepth: u8,
    stopped: bool,
    start_time: Instant,
    soft_deadline: Option<Instant>,
    hard_deadline: Option<Instant>,
    max_nodes: Option<u64>,
    stop_flag: Arc<AtomicBool>,
}

impl Searcher {
    /// Allocates all search structures once with fixed capacity.
    pub fn new(tt_mb: usize) -> Self {
        let mut lmr_table = [[0u8; 64]; 64];
        for (d, row) in lmr_table.iter_mut().enumerate().skip(1) {
            for (m, cell) in row.iter_mut().enumerate().skip(1) {
                let r = 0.75 + (d as f64).ln() * (m as f64).ln() / 2.25;
                *cell = (r.max(0.0) as u8).min(63);
            }
        }

        Self {
            tt: TranspositionTable::new(tt_mb),
            killers: vec![[Move::NONE; 2]; MAX_PLY],
            history: Box::new([[[0i32; 64]; 64]; 2]),
            pv_table: vec![[Move::NONE; MAX_PLY]; MAX_PLY],
            pv_len: [0usize; MAX_PLY],
            stack: vec![StackEntry::default(); MAX_PLY],
            lmr_table,
            nodes: 0,
            seldepth: 0,
            stopped: false,
            start_time: Instant::now(),
            soft_deadline: None,
            hard_deadline: None,
            max_nodes: None,
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn tt(&self) -> &TranspositionTable {
        &self.tt
    }

    pub fn tt_mut(&mut self) -> &mut TranspositionTable {
        &mut self.tt
    }

    pub fn set_stop_flag(&mut self, stop_flag: Arc<AtomicBool>) {
        self.stop_flag = stop_flag;
    }

    pub fn clear_state(&mut self) {
        self.tt.clear();
        self.killers.fill([Move::NONE; 2]);
        for side in self.history.iter_mut() {
            for from in side.iter_mut() {
                from.fill(0);
            }
        }
    }

    /// Runs iterative deepening search without performing any heap allocations.
    pub fn search<F>(
        &mut self,
        board: &mut Board,
        limits: &SearchLimits,
        mut on_info: F,
    ) -> SearchResult
    where
        F: FnMut(&SearchInfo),
    {
        self.prepare_search(board, limits);

        let mut root_moves = MoveList::new();
        generate_legal(board, &mut root_moves);
        if !limits.search_moves.is_empty() {
            let mut filtered = MoveList::new();
            for &mv in root_moves.iter() {
                if limits.search_moves.contains(mv) {
                    filtered.push(mv);
                }
            }
            if !filtered.is_empty() {
                root_moves = filtered;
            }
        }

        let mut result = SearchResult {
            best_move: root_moves.get(0).unwrap_or(Move::NONE),
            ponder_move: Move::NONE,
            score: 0,
            depth: 0,
            seldepth: 0,
            nodes: 0,
            time_ms: 0,
            pv_lines: [PvLine::default(); MAX_MULTI_PV],
            pv_count: 0,
        };

        if root_moves.is_empty() {
            return result;
        }

        let max_depth = limits.max_depth.clamp(1, (MAX_PLY - 4) as u8);
        let multi_pv = limits.multi_pv.clamp(1, MAX_MULTI_PV).min(root_moves.len());
        let mut prev_best_score = 0i32;

        for depth in 1..=max_depth {
            if depth > 1 {
                if self.stopped || self.stop_flag.load(Ordering::Relaxed) {
                    break;
                }
                if let Some(soft) = self.soft_deadline {
                    if Instant::now() >= soft {
                        break;
                    }
                }
            }

            self.seldepth = depth;
            let mut excluded = MoveList::new();
            let mut depth_lines = [PvLine::default(); MAX_MULTI_PV];
            let mut completed_pvs = 0usize;

            for (pv_idx, slot) in depth_lines.iter_mut().enumerate().take(multi_pv) {
                let (score, best_mv) = if pv_idx == 0 && multi_pv == 1 && depth >= 4 {
                    self.aspiration_search(board, depth, prev_best_score, &root_moves, &excluded)
                } else {
                    self.root_search(board, depth, -INF, INF, &root_moves, &excluded)
                };

                if self.stopped && depth > 1 {
                    break;
                }

                if best_mv.is_none() {
                    break;
                }

                excluded.push(best_mv);
                let mut line = PvLine {
                    moves: [Move::NONE; MAX_PLY],
                    len: self.pv_len[0].min(MAX_PLY),
                    score,
                    depth,
                    seldepth: self.seldepth,
                    multipv_idx: pv_idx + 1,
                };
                line.moves[..line.len].copy_from_slice(&self.pv_table[0][..line.len]);
                if line.len == 0 {
                    line.moves[0] = best_mv;
                    line.len = 1;
                }
                *slot = line;
                completed_pvs += 1;

                let elapsed_ms = self.start_time.elapsed().as_millis() as u64;
                let nps = if elapsed_ms > 0 {
                    (self.nodes * 1000) / elapsed_ms
                } else {
                    self.nodes * 1000
                };

                let info = SearchInfo {
                    depth,
                    seldepth: self.seldepth,
                    multipv: pv_idx + 1,
                    score,
                    nodes: self.nodes,
                    nps,
                    hashfull: self.tt.hashfull(),
                    time_ms: elapsed_ms,
                    pv: line,
                };
                on_info(&info);

                if pv_idx == 0 {
                    prev_best_score = score;
                }
            }

            if completed_pvs > 0 && (!self.stopped || depth == 1) {
                result.pv_lines[..completed_pvs].copy_from_slice(&depth_lines[..completed_pvs]);
                result.pv_count = completed_pvs;
                result.best_move = depth_lines[0].moves[0];
                result.ponder_move = if depth_lines[0].len >= 2 {
                    depth_lines[0].moves[1]
                } else {
                    Move::NONE
                };
                result.score = depth_lines[0].score;
                result.depth = depth;
                result.seldepth = self.seldepth;
            }

            if self.stopped {
                break;
            }
            if multi_pv == 1 && prev_best_score.abs() >= MATE_BOUND {
                let mate_plies = MATE_SCORE - prev_best_score.abs();
                if (depth as i32) >= mate_plies + 2 {
                    break;
                }
            }
        }

        result.nodes = self.nodes;
        result.time_ms = self.start_time.elapsed().as_millis() as u64;
        result
    }

    fn prepare_search(&mut self, board: &Board, limits: &SearchLimits) {
        self.tt.new_search();
        self.nodes = 0;
        self.seldepth = 0;
        self.stopped = false;
        self.start_time = Instant::now();
        self.max_nodes = limits.max_nodes;

        // Age history softly instead of zeroing to keep useful quiet ordering across moves.
        for side in self.history.iter_mut() {
            for from in side.iter_mut() {
                for val in from.iter_mut() {
                    *val /= 2;
                }
            }
        }
        self.killers.fill([Move::NONE; 2]);

        let (soft, hard) = compute_deadlines(self.start_time, board.side_to_move(), limits);
        self.soft_deadline = soft;
        self.hard_deadline = hard;
    }

    fn aspiration_search(
        &mut self,
        board: &mut Board,
        depth: u8,
        prev_score: i32,
        root_moves: &MoveList,
        excluded: &MoveList,
    ) -> (i32, Move) {
        if prev_score.abs() >= MATE_BOUND - 100 {
            return self.root_search(board, depth, -INF, INF, root_moves, excluded);
        }

        let mut delta = 25i32;
        let mut alpha = (prev_score - delta).max(-INF);
        let mut beta = (prev_score + delta).min(INF);

        loop {
            let (score, mv) = self.root_search(board, depth, alpha, beta, root_moves, excluded);
            if self.stopped {
                return (score, mv);
            }
            if score <= alpha {
                beta = (alpha + beta) / 2;
                alpha = (score - delta).max(-INF);
            } else if score >= beta {
                beta = (score + delta).min(INF);
            } else {
                return (score, mv);
            }
            delta *= 2;
            if delta > 500 {
                return self.root_search(board, depth, -INF, INF, root_moves, excluded);
            }
        }
    }

    fn root_search(
        &mut self,
        board: &mut Board,
        depth: u8,
        mut alpha: i32,
        beta: i32,
        root_moves: &MoveList,
        excluded: &MoveList,
    ) -> (i32, Move) {
        self.pv_len[0] = 0;
        let tt_move = self
            .tt
            .probe(board.zobrist_key(), 0)
            .map(|h| h.best_move)
            .unwrap_or(Move::NONE);

        let mut candidates = MoveList::new();
        for &mv in root_moves.iter() {
            if !excluded.contains(mv) {
                candidates.push(mv);
            }
        }
        if candidates.is_empty() {
            return (0, Move::NONE);
        }

        let mut scores = [0i32; MoveList::CAPACITY];
        self.score_moves(board, &candidates, &mut scores, tt_move, 0);

        let mut best_score = -INF;
        let mut best_move = candidates.get(0).unwrap_or(Move::NONE);
        let in_check = board.in_check();
        let orig_alpha = alpha;

        for idx in 0..candidates.len() {
            let mv = pick_best_move(&mut candidates, &mut scores, idx);
            board.make_move(mv);
            self.nodes += 1;

            let child_ext = u8::from(in_check);
            let next_depth = depth.saturating_sub(1).saturating_add(child_ext);

            let score = if idx == 0 {
                -self.negamax(board, next_depth, -beta, -alpha, 1, true)
            } else {
                let mut s = -self.negamax(board, next_depth, -alpha - 1, -alpha, 1, true);
                if s > alpha && s < beta && !self.stopped {
                    s = -self.negamax(board, next_depth, -beta, -alpha, 1, true);
                }
                s
            };
            board.unmake_move();

            if self.stopped && depth > 1 {
                return (best_score, best_move);
            }

            if score > best_score {
                best_score = score;
                best_move = mv;
                self.update_pv(0, mv);
            }
            if score > alpha {
                alpha = score;
                if alpha >= beta {
                    break;
                }
            }
        }

        if !self.stopped && excluded.is_empty() {
            let bound = if best_score <= orig_alpha {
                Bound::Upper
            } else if best_score >= beta {
                Bound::Lower
            } else {
                Bound::Exact
            };
            self.tt
                .store(board.zobrist_key(), depth, best_score, bound, best_move, 0);
        }

        (best_score, best_move)
    }

    fn negamax(
        &mut self,
        board: &mut Board,
        depth: u8,
        mut alpha: i32,
        mut beta: i32,
        ply: usize,
        allow_null: bool,
    ) -> i32 {
        self.pv_len[ply] = 0;

        if self.should_stop() {
            self.stopped = true;
            return 0;
        }

        if ply > self.seldepth as usize {
            self.seldepth = ply.min(u8::MAX as usize) as u8;
        }

        if ply >= MAX_PLY - 2 || board.history().len() >= Board::HISTORY_PREALLOC - 4 {
            return evaluate(board);
        }

        // Draw detection inside search tree.
        if ply > 0
            && (board.is_fifty_move_draw()
                || board.is_repetition()
                || board.is_insufficient_material())
        {
            return 0;
        }

        // Mate-distance pruning.
        alpha = alpha.max(-MATE_SCORE + ply as i32);
        beta = beta.min(MATE_SCORE - ply as i32 - 1);
        if alpha >= beta {
            return alpha;
        }

        let in_check = board.in_check();
        if depth == 0 {
            return self.qsearch(board, alpha, beta, ply);
        }

        let pv_node = beta - alpha > 1;
        let key = board.zobrist_key();
        let mut tt_move = Move::NONE;

        if let Some(hit) = self.tt.probe(key, ply) {
            tt_move = hit.best_move;
            if !pv_node && hit.depth >= depth {
                match hit.bound {
                    Bound::Exact => return hit.score,
                    Bound::Lower if hit.score >= beta => return hit.score,
                    Bound::Upper if hit.score <= alpha => return hit.score,
                    _ => {}
                }
            }
        }

        let static_eval = if in_check { -INF } else { evaluate(board) };
        self.stack[ply].static_eval = static_eval;

        // Reverse Futility Pruning (static null move).
        if !pv_node && !in_check && depth <= 6 && beta.abs() < MATE_BOUND {
            let margin = 85 * depth as i32;
            if static_eval - margin >= beta {
                return static_eval - margin;
            }
        }

        // Null-Move Pruning.
        if allow_null
            && !pv_node
            && !in_check
            && depth >= 3
            && static_eval >= beta
            && board.has_non_pawn_material(board.side_to_move())
        {
            let r = 2 + u8::from(depth >= 6);
            let nmp_depth = depth.saturating_sub(1 + r);
            board.make_null_move();
            self.nodes += 1;
            let score = -self.negamax(board, nmp_depth, -beta, -beta + 1, ply + 1, false);
            board.unmake_null_move();
            if self.stopped {
                return 0;
            }
            if score >= beta {
                return if score >= MATE_BOUND { beta } else { score };
            }
        }

        let mut legal = MoveList::new();
        generate_legal(board, &mut legal);
        if legal.is_empty() {
            return if in_check {
                -MATE_SCORE + ply as i32
            } else {
                0
            };
        }

        let mut scores = [0i32; MoveList::CAPACITY];
        self.score_moves(board, &legal, &mut scores, tt_move, ply);

        let orig_alpha = alpha;
        let mut best_score = -INF;
        let mut best_move = Move::NONE;
        let mut quiet_tried = MoveList::new();
        let ext = u8::from(in_check && ply < 32);

        for idx in 0..legal.len() {
            let mv = pick_best_move(&mut legal, &mut scores, idx);
            let is_quiet = !mv.is_capture() && !mv.is_promotion();

            board.make_move(mv);
            self.nodes += 1;

            let gives_check = board.in_check();
            let base_depth = depth.saturating_sub(1).saturating_add(ext);

            let mut score;
            if idx == 0 {
                score = -self.negamax(board, base_depth, -beta, -alpha, ply + 1, true);
            } else {
                // Late Move Reductions (LMR).
                let can_reduce = depth >= 3
                    && idx >= 3
                    && !in_check
                    && is_quiet
                    && !gives_check
                    && mv != self.killers[ply][0]
                    && mv != self.killers[ply][1];

                let reduction = if can_reduce {
                    let mut r = self.lmr_table[depth.min(63) as usize][idx.min(63)];
                    if pv_node {
                        r = r.saturating_sub(1);
                    }
                    r.min(base_depth.saturating_sub(1))
                } else {
                    0
                };

                let reduced_depth = base_depth.saturating_sub(reduction);
                score = -self.negamax(board, reduced_depth, -alpha - 1, -alpha, ply + 1, true);

                if reduction > 0 && score > alpha && !self.stopped {
                    score = -self.negamax(board, base_depth, -alpha - 1, -alpha, ply + 1, true);
                }
                if score > alpha && score < beta && !self.stopped {
                    score = -self.negamax(board, base_depth, -beta, -alpha, ply + 1, true);
                }
            }

            board.unmake_move();
            if self.stopped {
                return 0;
            }

            if score > best_score {
                best_score = score;
                best_move = mv;
            }

            if score > alpha {
                alpha = score;
                if pv_node {
                    self.update_pv(ply, mv);
                }
                if alpha >= beta {
                    if is_quiet {
                        self.store_killer(ply, mv);
                        self.update_history(board.side_to_move(), mv, depth, &quiet_tried);
                    }
                    break;
                }
            }

            if is_quiet {
                quiet_tried.push(mv);
            }
        }

        let bound = if best_score <= orig_alpha {
            Bound::Upper
        } else if best_score >= beta {
            Bound::Lower
        } else {
            Bound::Exact
        };
        self.tt.store(key, depth, best_score, bound, best_move, ply);

        best_score
    }

    fn qsearch(&mut self, board: &mut Board, mut alpha: i32, beta: i32, ply: usize) -> i32 {
        self.pv_len[ply] = 0;

        if self.should_stop() {
            self.stopped = true;
            return 0;
        }

        if ply > self.seldepth as usize {
            self.seldepth = ply.min(u8::MAX as usize) as u8;
        }

        if ply >= MAX_PLY - 2 || board.history().len() >= Board::HISTORY_PREALLOC - 4 {
            return evaluate(board);
        }

        let in_check = board.in_check();
        let mut stand_pat = -INF;
        if !in_check {
            stand_pat = evaluate(board);
            if stand_pat >= beta {
                return beta;
            }
            if stand_pat > alpha {
                alpha = stand_pat;
            }
        }

        let mut list = MoveList::new();
        if in_check {
            generate_legal(board, &mut list);
            if list.is_empty() {
                return -MATE_SCORE + ply as i32;
            }
        } else {
            generate_noisy(board, &mut list);
        }

        let mut scores = [0i32; MoveList::CAPACITY];
        self.score_moves(board, &list, &mut scores, Move::NONE, ply);

        for idx in 0..list.len() {
            let mv = pick_best_move(&mut list, &mut scores, idx);

            if !in_check {
                // Delta pruning on non-promoting captures.
                if !mv.is_promotion() {
                    let cap_val = captured_piece_value(board, mv);
                    if stand_pat + cap_val + 200 < alpha {
                        continue;
                    }
                    if !see_ge(board, mv, -50) {
                        continue;
                    }
                }
            }

            board.make_move(mv);
            self.nodes += 1;
            let score = -self.qsearch(board, -beta, -alpha, ply + 1);
            board.unmake_move();

            if self.stopped {
                return 0;
            }

            if score >= beta {
                return beta;
            }
            if score > alpha {
                alpha = score;
            }
        }

        alpha
    }

    #[inline]
    fn should_stop(&self) -> bool {
        if (self.nodes & 2047) != 0 {
            return false;
        }
        if self.stop_flag.load(Ordering::Relaxed) {
            return true;
        }
        if let Some(max_n) = self.max_nodes {
            if self.nodes >= max_n {
                return true;
            }
        }
        if let Some(hard) = self.hard_deadline {
            if Instant::now() >= hard {
                return true;
            }
        }
        false
    }

    fn update_pv(&mut self, ply: usize, mv: Move) {
        self.pv_table[ply][0] = mv;
        let child_len = if ply + 1 < MAX_PLY {
            self.pv_len[ply + 1].min(MAX_PLY - 1)
        } else {
            0
        };
        for i in 0..child_len {
            self.pv_table[ply][i + 1] = self.pv_table[ply + 1][i];
        }
        self.pv_len[ply] = child_len + 1;
    }

    fn store_killer(&mut self, ply: usize, mv: Move) {
        if self.killers[ply][0] != mv {
            self.killers[ply][1] = self.killers[ply][0];
            self.killers[ply][0] = mv;
        }
    }

    fn update_history(&mut self, stm: Color, best: Move, depth: u8, quiet_tried: &MoveList) {
        let bonus = (depth as i32) * (depth as i32);
        let side = &mut self.history[stm.index()];
        apply_history_delta(
            &mut side[best.from_sq().index()][best.to_sq().index()],
            bonus,
        );
        for &q in quiet_tried.iter() {
            apply_history_delta(&mut side[q.from_sq().index()][q.to_sq().index()], -bonus);
        }
    }

    fn score_moves(
        &self,
        board: &Board,
        list: &MoveList,
        scores: &mut [i32; MoveList::CAPACITY],
        tt_move: Move,
        ply: usize,
    ) {
        let stm_idx = board.side_to_move().index();
        let k0 = self.killers[ply][0];
        let k1 = self.killers[ply][1];

        for (i, &mv) in list.iter().enumerate() {
            if mv == tt_move {
                scores[i] = 2_000_000;
                continue;
            }

            if mv.is_capture() || mv.is_promotion() {
                let victim_val = if mv.is_en_passant() {
                    MG_PIECE_VALUES[PieceType::Pawn.index()]
                } else if mv.is_capture() {
                    board
                        .piece_on(mv.to_sq())
                        .map(|(_, pt)| MG_PIECE_VALUES[pt.index()])
                        .unwrap_or(0)
                } else {
                    0
                };
                let attacker_val = board
                    .piece_on(mv.from_sq())
                    .map(|(_, pt)| MG_PIECE_VALUES[pt.index()])
                    .unwrap_or(100);
                let promo_val = mv
                    .promotion_piece()
                    .map(|pt| MG_PIECE_VALUES[pt.index()])
                    .unwrap_or(0);
                let mvv_lva = victim_val * 16 - attacker_val + promo_val;
                if see_ge(board, mv, 0) {
                    scores[i] = 1_000_000 + mvv_lva;
                } else {
                    scores[i] = -50_000 + mvv_lva;
                }
            } else if mv == k0 {
                scores[i] = 900_000;
            } else if mv == k1 {
                scores[i] = 800_000;
            } else {
                scores[i] = self.history[stm_idx][mv.from_sq().index()][mv.to_sq().index()];
            }
        }
    }
}

#[inline]
fn apply_history_delta(entry: &mut i32, delta: i32) {
    let clamped = delta.clamp(-16_384, 16_384);
    *entry += clamped - (*entry * clamped.abs()) / 16_384;
}

#[inline]
fn pick_best_move(
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
fn captured_piece_value(board: &Board, mv: Move) -> i32 {
    if mv.is_en_passant() {
        SEE_VALUES[PieceType::Pawn.index()]
    } else if let Some((_, pt)) = board.piece_on(mv.to_sq()) {
        SEE_VALUES[pt.index()]
    } else {
        0
    }
}

fn compute_deadlines(
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
        let safe_max = rem_ms.saturating_sub(30).max(1);
        let base = ((rem_ms / mtg) + (inc_ms * 3) / 4).min(safe_max / 2).max(5);
        let soft = (base * 6 / 10).min(safe_max).max(1);
        let hard = (base * 2).min(safe_max).max(1);
        return (
            Some(now + Duration::from_millis(soft)),
            Some(now + Duration::from_millis(hard)),
        );
    }

    (None, None)
}

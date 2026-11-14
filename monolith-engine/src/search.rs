//! Alpha-beta search with iterative deepening, PVS, aspiration windows,
//! incremental NNUE accumulators, NMP, RFP, Razoring, IIR, LMP, LMR, and Multi-PV.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use monolith_core::movegen::{generate_legal, generate_noisy};
use monolith_core::types::{Move, MoveList};
use monolith_core::Board;

use crate::eval::evaluate;
pub use crate::eval::see_ge;
use crate::move_order::OrderTables;
use crate::nnue::{NnueAccumulator, NnueNet};
use crate::search_types::{captured_piece_value, compute_deadlines, pick_best_move};
pub use crate::search_types::{
    PvLine, Score, SearchInfo, SearchLimits, SearchResult, MAX_MULTI_PV, MAX_PLY,
};
use crate::tt::{Bound, TranspositionTable, MATE_BOUND, MATE_SCORE};

const INF: i32 = 31_000;

#[derive(Clone, Copy, Default)]
struct StackEntry {
    static_eval: i32,
    mv: Move,
}

/// Reusable chess searcher holding preallocated tables so `search()` performs 0 heap allocations.
pub struct Searcher {
    tt: Arc<TranspositionTable>,
    nnue: Option<Arc<NnueNet>>,
    use_nnue: bool,
    acc_stack: Vec<NnueAccumulator>,
    order: OrderTables,
    pv_table: Vec<[Move; MAX_PLY]>,
    pv_len: [usize; MAX_PLY],
    stack: Vec<StackEntry>,
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
        Self::with_shared_tt(Arc::new(TranspositionTable::new(tt_mb)))
    }

    /// Creates a worker searcher sharing an existing lock-free [`TranspositionTable`].
    pub fn with_shared_tt(tt: Arc<TranspositionTable>) -> Self {
        Self {
            tt,
            nnue: None,
            use_nnue: true,
            acc_stack: vec![NnueAccumulator::default(); MAX_PLY + 4],
            order: OrderTables::new(),
            pv_table: vec![[Move::NONE; MAX_PLY]; MAX_PLY],
            pv_len: [0usize; MAX_PLY],
            stack: vec![StackEntry::default(); MAX_PLY],
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

    pub fn shared_tt(&self) -> Arc<TranspositionTable> {
        Arc::clone(&self.tt)
    }

    pub fn resize_tt(&mut self, size_mb: usize) {
        self.tt = Arc::new(TranspositionTable::new(size_mb));
    }

    pub fn set_nnue(&mut self, nnue: Option<Arc<NnueNet>>) {
        self.nnue = nnue;
    }

    pub fn nnue(&self) -> Option<&Arc<NnueNet>> {
        self.nnue.as_ref()
    }

    pub fn set_use_nnue(&mut self, use_nnue: bool) {
        self.use_nnue = use_nnue;
    }

    pub fn use_nnue(&self) -> bool {
        self.use_nnue
    }

    #[inline]
    fn eval_at(&self, board: &Board, acc_ply: usize) -> i32 {
        if self.use_nnue {
            if let Some(net) = &self.nnue {
                return net.evaluate_accumulator(&self.acc_stack[acc_ply], board.side_to_move());
            }
        }
        evaluate(board)
    }

    #[inline]
    fn push_acc(&mut self, board: &Board, mv: Move, acc_ply: usize) -> usize {
        if self.use_nnue {
            if let Some(net) = &self.nnue {
                if acc_ply + 1 < self.acc_stack.len() {
                    if let Some(delta) = NnueNet::capture_delta(board, mv) {
                        let (left, right) = self.acc_stack.split_at_mut(acc_ply + 1);
                        net.apply_delta(&left[acc_ply], &mut right[0], &delta);
                        return acc_ply + 1;
                    }
                }
            }
        }
        acc_ply
    }

    pub fn set_stop_flag(&mut self, stop_flag: Arc<AtomicBool>) {
        self.stop_flag = stop_flag;
    }

    pub fn clear_state(&mut self) {
        self.tt.clear();
        self.order.clear();
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
                if limits.search_moves.iter().any(|sm| {
                    sm.from_sq() == mv.from_sq()
                        && sm.to_sq() == mv.to_sq()
                        && sm.promotion_piece() == mv.promotion_piece()
                }) {
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
        self.order.age_for_search();
        self.stack[0] = StackEntry::default();

        if self.use_nnue {
            if let Some(net) = &self.nnue {
                net.refresh_accumulator(board, &mut self.acc_stack[0]);
            }
        }

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

        let mut delta = 20i32;
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
        self.order
            .score_moves(board, &candidates, &mut scores, tt_move, 0, Move::NONE);

        let mut best_score = -INF;
        let mut best_move = candidates.get(0).unwrap_or(Move::NONE);
        let in_check = board.in_check();
        let orig_alpha = alpha;

        for idx in 0..candidates.len() {
            let mv = pick_best_move(&mut candidates, &mut scores, idx);
            let next_acc = self.push_acc(board, mv, 0);
            self.stack[1].mv = mv;
            board.make_move(mv);
            self.nodes += 1;

            let child_ext = u8::from(in_check);
            let next_depth = depth.saturating_sub(1).saturating_add(child_ext);

            let score = if idx == 0 {
                -self.negamax(board, next_depth, -beta, -alpha, 1, next_acc, true)
            } else {
                let mut s = -self.negamax(board, next_depth, -alpha - 1, -alpha, 1, next_acc, true);
                if s > alpha && s < beta && !self.stopped {
                    s = -self.negamax(board, next_depth, -beta, -alpha, 1, next_acc, true);
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
            }
            if alpha >= beta {
                break;
            }
        }

        if excluded.is_empty() && !self.stopped {
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

    #[allow(clippy::too_many_arguments)]
    fn negamax(
        &mut self,
        board: &mut Board,
        mut depth: u8,
        mut alpha: i32,
        beta: i32,
        ply: usize,
        acc_ply: usize,
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

        if ply >= MAX_PLY - 2 || board.history_spare_capacity() <= 4 {
            return self.eval_at(board, acc_ply);
        }

        if ply > 0
            && (board.is_fifty_move_draw()
                || board.is_repetition()
                || board.is_insufficient_material())
        {
            return 0;
        }

        alpha = alpha.max(-MATE_SCORE + ply as i32);
        let beta_clamped = beta.min(MATE_SCORE - ply as i32 - 1);
        if alpha >= beta_clamped {
            return alpha;
        }
        let beta = beta_clamped;

        let in_check = board.in_check();
        if depth == 0 {
            return self.qsearch(board, alpha, beta, ply, acc_ply);
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

        // Internal Iterative Reductions (IIR) when no TT move is present.
        if depth >= 4 && tt_move.is_none() && !in_check {
            depth -= 1;
        }

        let static_eval = if in_check {
            -INF
        } else {
            self.eval_at(board, acc_ply)
        };
        self.stack[ply].static_eval = static_eval;
        let improving = ply >= 2
            && !in_check
            && self.stack[ply - 2].static_eval != -INF
            && static_eval > self.stack[ply - 2].static_eval;

        // Reverse Futility Pruning (static null move).
        if !pv_node && !in_check && depth <= 6 && beta.abs() < MATE_BOUND {
            let margin = (if improving { 65 } else { 85 }) * depth as i32;
            if static_eval - margin >= beta {
                return static_eval - margin;
            }
        }

        // Razoring at shallow depths.
        if !pv_node && !in_check && depth <= 2 && alpha.abs() < MATE_BOUND {
            let margin = 280 * depth as i32;
            if static_eval + margin <= alpha {
                let r_score = self.qsearch(board, alpha, alpha + 1, ply, acc_ply);
                if self.stopped {
                    return 0;
                }
                if r_score <= alpha {
                    return r_score;
                }
            }
        }

        // Null-Move Pruning (acc_ply stays identical since pieces don't move).
        if allow_null
            && !pv_node
            && !in_check
            && depth >= 3
            && static_eval >= beta
            && board.has_non_pawn_material(board.side_to_move())
        {
            let r = 2 + u8::from(depth >= 6) + u8::from(static_eval - beta >= 200);
            let nmp_depth = depth.saturating_sub(1 + r);
            self.stack[ply + 1].mv = Move::NONE;
            board.make_null_move();
            self.nodes += 1;
            let score = -self.negamax(board, nmp_depth, -beta, -beta + 1, ply + 1, acc_ply, false);
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

        let prev_move = self.stack[ply].mv;
        let k0 = self.order.killers[ply][0];
        let k1 = self.order.killers[ply][1];
        let cm = self.order.countermove_for(prev_move);

        let mut scores = [0i32; MoveList::CAPACITY];
        self.order
            .score_moves(board, &legal, &mut scores, tt_move, ply, prev_move);

        let orig_alpha = alpha;
        let mut best_score = -INF;
        let mut best_move = Move::NONE;
        let mut quiet_tried = MoveList::new();
        let mut quiets_cnt = 0usize;
        let ext = u8::from(in_check && ply < 32);
        let lmp_limit = if improving {
            5 + (depth as usize) * (depth as usize)
        } else {
            3 + (depth as usize) * (depth as usize)
        };

        for idx in 0..legal.len() {
            let mv = pick_best_move(&mut legal, &mut scores, idx);
            let is_quiet = !mv.is_capture() && !mv.is_promotion();
            let is_special = mv == tt_move || mv == k0 || mv == k1 || mv == cm;

            // SEE pruning for clearly losing captures at low depth.
            if !pv_node
                && !in_check
                && !is_quiet
                && !mv.is_promotion()
                && depth <= 3
                && idx > 0
                && best_score > -MATE_BOUND
                && !see_ge(board, mv, -85 * depth as i32)
            {
                continue;
            }

            if is_quiet {
                quiets_cnt += 1;
            }

            // Quiet pruning check after playing move to verify `gives_check` cleanly.
            let delta_opt = if self.use_nnue {
                self.nnue
                    .as_ref()
                    .and_then(|_| NnueNet::capture_delta(board, mv))
            } else {
                None
            };

            self.stack[ply + 1].mv = mv;
            board.make_move(mv);
            let gives_check = board.in_check();

            if !pv_node
                && !in_check
                && is_quiet
                && !gives_check
                && !is_special
                && best_score > -MATE_BOUND
            {
                if depth <= 4 && quiets_cnt > lmp_limit {
                    board.unmake_move();
                    continue;
                }
                if depth <= 4
                    && alpha.abs() < MATE_BOUND
                    && static_eval + 110 + 95 * depth as i32 <= alpha
                {
                    board.unmake_move();
                    continue;
                }
            }

            self.nodes += 1;
            let next_acc = if let (true, Some(net), Some(delta)) =
                (acc_ply + 1 < self.acc_stack.len(), &self.nnue, delta_opt)
            {
                let (left, right) = self.acc_stack.split_at_mut(acc_ply + 1);
                net.apply_delta(&left[acc_ply], &mut right[0], &delta);
                acc_ply + 1
            } else {
                acc_ply
            };

            let base_depth = depth.saturating_sub(1).saturating_add(ext);
            let stm_after = board.side_to_move();

            let mut score;
            if idx == 0 {
                score = -self.negamax(board, base_depth, -beta, -alpha, ply + 1, next_acc, true);
            } else {
                let can_reduce = depth >= 3
                    && idx >= 3
                    && !in_check
                    && is_quiet
                    && !gives_check
                    && mv != k0
                    && mv != k1;

                let reduction = if can_reduce {
                    let mut r = self.order.lmr_table[depth.min(63) as usize][idx.min(63)] as i32;
                    if pv_node {
                        r -= 1;
                    }
                    if !improving {
                        r += 1;
                    }
                    let hist = self.order.history[stm_after.flip().index()][mv.from_sq().index()]
                        [mv.to_sq().index()];
                    r -= (hist / 8192).clamp(-1, 2);
                    (r.max(0) as u8).min(base_depth.saturating_sub(1))
                } else {
                    0
                };

                let reduced_depth = base_depth.saturating_sub(reduction);
                score = -self.negamax(
                    board,
                    reduced_depth,
                    -alpha - 1,
                    -alpha,
                    ply + 1,
                    next_acc,
                    true,
                );

                if reduction > 0 && score > alpha && !self.stopped {
                    score = -self.negamax(
                        board,
                        base_depth,
                        -alpha - 1,
                        -alpha,
                        ply + 1,
                        next_acc,
                        true,
                    );
                }
                if score > alpha && score < beta && !self.stopped {
                    score =
                        -self.negamax(board, base_depth, -beta, -alpha, ply + 1, next_acc, true);
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
                        self.order.store_killer(ply, mv);
                        self.order.store_countermove(prev_move, mv);
                        self.order
                            .update_history(board.side_to_move(), mv, depth, &quiet_tried);
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

    fn qsearch(
        &mut self,
        board: &mut Board,
        mut alpha: i32,
        beta: i32,
        ply: usize,
        acc_ply: usize,
    ) -> i32 {
        self.pv_len[ply] = 0;

        if self.should_stop() {
            self.stopped = true;
            return 0;
        }

        if ply > self.seldepth as usize {
            self.seldepth = ply.min(u8::MAX as usize) as u8;
        }

        if ply >= MAX_PLY - 2 || board.history_spare_capacity() <= 4 {
            return self.eval_at(board, acc_ply);
        }

        let in_check = board.in_check();
        let mut stand_pat = -INF;
        if !in_check {
            stand_pat = self.eval_at(board, acc_ply);
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
        self.order
            .score_moves(board, &list, &mut scores, Move::NONE, ply, Move::NONE);

        for idx in 0..list.len() {
            let mv = pick_best_move(&mut list, &mut scores, idx);

            if !in_check && !mv.is_promotion() {
                let cap_val = captured_piece_value(board, mv);
                if stand_pat + cap_val + 200 < alpha {
                    continue;
                }
                if !see_ge(board, mv, -50) {
                    continue;
                }
            }

            let next_acc = self.push_acc(board, mv, acc_ply);
            board.make_move(mv);
            self.nodes += 1;
            let score = -self.qsearch(board, -beta, -alpha, ply + 1, next_acc);
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
        if let Some(max_n) = self.max_nodes {
            if self.nodes >= max_n {
                return true;
            }
        }
        if (self.nodes & 1023) != 0 {
            return false;
        }
        if self.stop_flag.load(Ordering::Relaxed) {
            return true;
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
}

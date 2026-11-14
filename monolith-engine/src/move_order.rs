//! Preallocated move ordering tables (TT, SEE captures, killers, countermoves, quiet history, LMR).

use monolith_core::types::{Color, Move, MoveList};
use monolith_core::Board;

use crate::eval::{see_ge, MG_PIECE_VALUES};
use crate::search_types::{apply_history_delta, captured_piece_value, MAX_PLY};

/// Fixed-capacity move ordering state owned by [`crate::search::Searcher`].
pub(crate) struct OrderTables {
    pub killers: Vec<[Move; 2]>,
    pub history: Box<[[[i32; 64]; 64]; 2]>,
    pub countermoves: Box<[[Move; 64]; 64]>,
    pub lmr_table: [[u8; 64]; 64],
}

impl OrderTables {
    pub fn new() -> Self {
        let mut lmr_table = [[0u8; 64]; 64];
        for (d, row) in lmr_table.iter_mut().enumerate().skip(1) {
            for (m, cell) in row.iter_mut().enumerate().skip(1) {
                let r = 0.65 + (d as f64).ln() * (m as f64).ln() / 2.10;
                *cell = (r.max(0.0) as u8).min(63);
            }
        }

        Self {
            killers: vec![[Move::NONE; 2]; MAX_PLY],
            history: Box::new([[[0i32; 64]; 64]; 2]),
            countermoves: Box::new([[Move::NONE; 64]; 64]),
            lmr_table,
        }
    }

    pub fn clear(&mut self) {
        self.killers.fill([Move::NONE; 2]);
        for side in self.history.iter_mut() {
            for from in side.iter_mut() {
                from.fill(0);
            }
        }
        for row in self.countermoves.iter_mut() {
            row.fill(Move::NONE);
        }
    }

    pub fn age_for_search(&mut self) {
        for side in self.history.iter_mut() {
            for from in side.iter_mut() {
                for val in from.iter_mut() {
                    *val /= 2;
                }
            }
        }
        self.killers.fill([Move::NONE; 2]);
    }

    #[inline]
    pub fn store_killer(&mut self, ply: usize, mv: Move) {
        if self.killers[ply][0] != mv {
            self.killers[ply][1] = self.killers[ply][0];
            self.killers[ply][0] = mv;
        }
    }

    #[inline]
    pub fn store_countermove(&mut self, prev_move: Move, mv: Move) {
        if !prev_move.is_none() {
            self.countermoves[prev_move.from_sq().index()][prev_move.to_sq().index()] = mv;
        }
    }

    #[inline]
    pub fn countermove_for(&self, prev_move: Move) -> Move {
        if prev_move.is_none() {
            Move::NONE
        } else {
            self.countermoves[prev_move.from_sq().index()][prev_move.to_sq().index()]
        }
    }

    pub fn update_history(&mut self, stm: Color, best: Move, depth: u8, quiet_tried: &MoveList) {
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

    pub fn score_moves(
        &self,
        board: &Board,
        list: &MoveList,
        scores: &mut [i32; MoveList::CAPACITY],
        tt_move: Move,
        ply: usize,
        prev_move: Move,
    ) {
        let stm_idx = board.side_to_move().index();
        let k0 = self.killers[ply][0];
        let k1 = self.killers[ply][1];
        let cm = self.countermove_for(prev_move);

        for (i, &mv) in list.iter().enumerate() {
            if mv == tt_move {
                scores[i] = 2_000_000;
                continue;
            }

            if mv.is_capture() || mv.is_promotion() {
                let victim_val = if mv.is_capture() {
                    captured_piece_value(board, mv)
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
            } else if mv == cm {
                scores[i] = 750_000;
            } else {
                scores[i] = self.history[stm_idx][mv.from_sq().index()][mv.to_sq().index()];
            }
        }
    }
}

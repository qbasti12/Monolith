//! Background PGN game analysis worker producing per-ply move classifications.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use monolith_core::pgn::{classify_move, move_to_san, ClassifyInput, MoveClassification, PgnGame};
use monolith_core::{Board, Color};
use monolith_engine::eval::eval_breakdown;
use monolith_engine::search::{Score, SearchLimits, Searcher};

/// Incremental update emitted by the background PGN analyzer.
#[derive(Debug, Clone)]
pub enum AnalysisEvent {
    Started {
        total_plies: usize,
    },
    Step {
        ply_index: usize,
        total_plies: usize,
        classification: MoveClassification,
        best_move_san: Option<String>,
        white_eval_cp: i32,
    },
    Finished,
    Cancelled,
}

/// Active background PGN analysis task with cancellation support.
pub struct PgnAnalysisHandle {
    pub rx: Receiver<AnalysisEvent>,
    cancel_flag: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl PgnAnalysisHandle {
    /// Signals cancellation and joins the background worker thread.
    pub fn cancel(&mut self) {
        self.cancel_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.join.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for PgnAnalysisHandle {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn raw_score_to_cp(raw: i32) -> i32 {
    match Score::from_raw(raw) {
        Score::Cp(cp) => cp,
        Score::Mate(m) => {
            if m > 0 {
                10_000 - (m * 20)
            } else {
                -10_000 - (m * 20)
            }
        }
    }
}

fn white_material_cp(board: &Board) -> i32 {
    eval_breakdown(board).material_cp
}

fn stm_material_cp(board: &Board) -> i32 {
    let w = white_material_cp(board);
    if board.side_to_move() == Color::White {
        w
    } else {
        -w
    }
}

/// Spawns a background thread analyzing every half-move in `game` up to `search_depth`.
pub fn start_pgn_analysis(game: PgnGame, search_depth: u8) -> PgnAnalysisHandle {
    let (tx, rx) = mpsc::channel();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel_flag);

    let join = thread::spawn(move || {
        let total_plies = game.moves.len();
        let _ = tx.send(AnalysisEvent::Started { total_plies });

        let mut searcher = Searcher::new(16);
        searcher.set_stop_flag(Arc::clone(&worker_cancel));
        let depth = search_depth.clamp(3, 12);

        for (idx, pgn_mv) in game.moves.iter().enumerate() {
            if worker_cancel.load(Ordering::Relaxed) {
                let _ = tx.send(AnalysisEvent::Cancelled);
                return;
            }

            let Ok(mut before) = Board::from_fen(&pgn_mv.fen_before) else {
                continue;
            };
            let mover = before.side_to_move();
            let mat_before_mover = stm_material_cp(&before);

            let limits_before = SearchLimits {
                max_depth: depth,
                multi_pv: 2,
                ..SearchLimits::default()
            };
            let res_before = searcher.search(&mut before, &limits_before, |_| {});
            if worker_cancel.load(Ordering::Relaxed) {
                let _ = tx.send(AnalysisEvent::Cancelled);
                return;
            }

            let eval_before_best_cp = raw_score_to_cp(res_before.score);
            let second_best_cp = if res_before.pv_count >= 2 {
                Some(raw_score_to_cp(res_before.pv_lines[1].score))
            } else {
                None
            };
            let top1_mv = res_before.pv_lines[0].moves[0];
            let top2_mv = if res_before.pv_count >= 2 {
                res_before.pv_lines[1].moves[0]
            } else {
                monolith_core::Move::NONE
            };
            let is_engine_top_move = pgn_mv.mv == top1_mv;
            let is_top_two_move = is_engine_top_move || pgn_mv.mv == top2_mv;
            let best_san = if top1_mv.is_none() {
                None
            } else {
                move_to_san(&before, top1_mv).ok()
            };

            let mut after = before.clone();
            after.make_move(pgn_mv.mv);

            let (eval_after_played_cp, mat_after_reply_white) = if is_engine_top_move {
                let mut probe = after.clone();
                let reply = res_before.pv_lines[0].moves[1];
                if !reply.is_none() {
                    probe.make_move(reply);
                }
                (eval_before_best_cp, white_material_cp(&probe))
            } else {
                let limits_after = SearchLimits {
                    max_depth: depth.saturating_sub(1).max(3),
                    multi_pv: 1,
                    ..SearchLimits::default()
                };
                let res_after = searcher.search(&mut after, &limits_after, |_| {});
                if worker_cancel.load(Ordering::Relaxed) {
                    let _ = tx.send(AnalysisEvent::Cancelled);
                    return;
                }
                let mut probe = after.clone();
                if !res_after.best_move.is_none() {
                    probe.make_move(res_after.best_move);
                }
                (-raw_score_to_cp(res_after.score), white_material_cp(&probe))
            };

            let mat_immediate_white = white_material_cp(&after);
            let (mat_imm_mover, mat_rep_mover) = if mover == Color::White {
                (mat_immediate_white, mat_after_reply_white)
            } else {
                (-mat_immediate_white, -mat_after_reply_white)
            };
            let material_delta_cp =
                (mat_imm_mover - mat_before_mover).min(mat_rep_mover - mat_before_mover);

            let classification = classify_move(&ClassifyInput {
                eval_before_best_cp,
                eval_after_played_cp,
                second_best_cp,
                is_engine_top_move,
                is_top_two_move,
                material_delta_cp,
                best_san: best_san.clone(),
            });

            let white_eval_cp = if mover == Color::White {
                eval_after_played_cp
            } else {
                -eval_after_played_cp
            };

            if tx
                .send(AnalysisEvent::Step {
                    ply_index: idx,
                    total_plies,
                    classification,
                    best_move_san: best_san,
                    white_eval_cp,
                })
                .is_err()
            {
                return;
            }
        }

        let _ = tx.send(AnalysisEvent::Finished);
    });

    PgnAnalysisHandle {
        rx,
        cancel_flag,
        join: Some(join),
    }
}

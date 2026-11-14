//! Lazy SMP multi-threaded search coordinator sharing a lock-free [`TranspositionTable`].

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

use monolith_core::Board;

use crate::search::{SearchInfo, SearchLimits, SearchResult, Searcher};

/// Runs a single-threaded or Lazy-SMP multi-threaded search using `threads` workers.
pub fn search_parallel<F>(
    main_searcher: &mut Searcher,
    board: &mut Board,
    limits: &SearchLimits,
    threads: usize,
    stop_flag: &Arc<AtomicBool>,
    on_info: F,
) -> SearchResult
where
    F: FnMut(&SearchInfo),
{
    let workers = threads.clamp(1, 64);
    main_searcher.set_stop_flag(Arc::clone(stop_flag));
    if workers == 1 {
        return main_searcher.search(board, limits, on_info);
    }

    let tt = main_searcher.shared_tt();
    let nnue = main_searcher.nnue().cloned();
    let use_nnue = main_searcher.use_nnue();
    let helper_nodes = AtomicU64::new(0);
    let was_stopped_before = stop_flag.load(Ordering::SeqCst);

    let mut result = thread::scope(|scope| {
        for tid in 1..workers {
            let tt_ref = Arc::clone(&tt);
            let nnue_ref = nnue.clone();
            let stop_ref = Arc::clone(stop_flag);
            let nodes_ref = &helper_nodes;
            let mut helper_board = board.clone();
            let mut helper_limits = limits.clone();
            helper_limits.multi_pv = 1;
            if tid % 2 == 1 {
                helper_limits.max_depth = helper_limits.max_depth.saturating_add(1);
            }

            scope.spawn(move || {
                let mut worker = Searcher::with_shared_tt(tt_ref);
                worker.set_nnue(nnue_ref);
                worker.set_use_nnue(use_nnue);
                worker.set_stop_flag(stop_ref);
                let res = worker.search(&mut helper_board, &helper_limits, |_| {});
                nodes_ref.fetch_add(res.nodes, Ordering::Relaxed);
            });
        }

        let main_res = main_searcher.search(board, limits, on_info);
        stop_flag.store(true, Ordering::SeqCst);
        main_res
    });

    if !was_stopped_before {
        stop_flag.store(false, Ordering::SeqCst);
    }

    result.nodes = result
        .nodes
        .saturating_add(helper_nodes.load(Ordering::Relaxed));
    result
}

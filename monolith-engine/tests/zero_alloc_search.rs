use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use monolith_core::movegen::generate_legal;
use monolith_core::types::MoveList;
use monolith_core::{Board, STARTPOS_FEN};
use monolith_engine::search::{SearchLimits, Searcher};

struct CountingAllocator;

static COUNT_ACTIVE: AtomicBool = AtomicBool::new(false);
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

// SAFETY: Delegates directly to `std::alloc::System` while atomically counting allocations.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNT_ACTIVE.load(Ordering::Relaxed) {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNT_ACTIVE.load(Ordering::Relaxed) {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

#[test]
fn search_performs_zero_heap_allocations() {
    let mut searcher = Searcher::new(4);
    let mut startpos = Board::from_fen(STARTPOS_FEN).expect("startpos");
    let mut kiwipete =
        Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1")
            .expect("kiwipete");
    let mut iq = Board::from_fen("4k3/8/8/8/8/4p3/QQQQQQQQ/QQQ1KQQQ w - - 0 1").expect("iq");

    let limits = SearchLimits {
        max_depth: 6,
        multi_pv: 3,
        ..Default::default()
    };

    ALLOC_COUNT.store(0, Ordering::SeqCst);
    COUNT_ACTIVE.store(true, Ordering::SeqCst);

    let r1 = searcher.search(&mut startpos, &limits, |_| {});
    let r2 = searcher.search(&mut kiwipete, &limits, |_| {});
    let iq_limits = SearchLimits {
        max_depth: 3,
        ..Default::default()
    };
    let r3 = searcher.search(&mut iq, &iq_limits, |_| {});

    COUNT_ACTIVE.store(false, Ordering::SeqCst);
    let allocs = ALLOC_COUNT.load(Ordering::SeqCst);

    assert_eq!(allocs, 0, "search performed {} heap allocations", allocs);

    for (board, mv) in [
        (&startpos, r1.best_move),
        (&kiwipete, r2.best_move),
        (&iq, r3.best_move),
    ] {
        let mut legal = MoveList::new();
        generate_legal(board, &mut legal);
        assert!(legal.contains(mv));
    }
}

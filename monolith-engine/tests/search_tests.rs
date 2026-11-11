use std::time::Instant;

use monolith_core::movegen::{generate_legal, parse_uci_move};
use monolith_core::types::MoveList;
use monolith_core::Board;
use monolith_engine::search::{Score, SearchLimits, Searcher};

fn solve_best(fen: &str, depth: u8) -> (String, Score, u64) {
    let mut searcher = Searcher::new(16);
    let mut board = Board::from_fen(fen).expect("valid fen");
    let limits = SearchLimits {
        max_depth: depth,
        ..Default::default()
    };
    let res = searcher.search(&mut board, &limits, |_| {});
    (
        res.best_move.to_uci(),
        Score::from_raw(res.score),
        res.nodes,
    )
}

#[test]
fn finds_mate_in_1_2_and_3() {
    // Mate in 1: Ra1-a8#
    let (m1, s1, _) = solve_best("6k1/5ppp/8/8/8/8/8/R3K3 w - - 0 1", 4);
    assert_eq!(m1, "a1a8");
    assert_eq!(s1, Score::Mate(1));

    // Mate in 1: Scholar's Mate Qxf7#
    let (m1b, s1b, _) = solve_best(
        "r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4",
        4,
    );
    assert_eq!(m1b, "h5f7");
    assert_eq!(s1b, Score::Mate(1));

    // Mate in 2: Morphy Opera Mate (1. Qb8+ Nxb8 2. Rd8#)
    let (m2a, s2a, _) = solve_best("4kb1r/p2n1ppp/4q3/4p1B1/4P3/1Q6/PPP2PPP/2KR4 w k - 0 1", 6);
    assert_eq!(m2a, "b3b8");
    assert_eq!(s2a, Score::Mate(2));

    // Mate in 2: Corridor exchange (1. Qe8+ Rxe8 2. Rxe8#)
    let (m2b, s2b, _) = solve_best("3r2k1/5ppp/8/8/8/8/4Q3/4R1K1 w - - 0 1", 6);
    assert_eq!(m2b, "e2e8");
    assert_eq!(s2b, Score::Mate(2));

    // Mate in 3: Smothered Mate (1. Nh6+ Kh8 2. Qg8+ Rxg8 3. Nf7#)
    let (m3a, s3a, _) = solve_best("5rk1/6pp/8/5N2/8/1Q6/8/6K1 w - - 0 1", 7);
    assert_eq!(m3a, "f5h6");
    assert_eq!(s3a, Score::Mate(3));

    // Mate in 3: Back-rank battery (1. Qxe8+ Rxe8 2. Rxe8+ Qxe8 3. Rxe8#)
    let (m3b, s3b, _) = solve_best("2r1r1k1/3q1ppp/8/8/8/4Q3/4R3/4R1K1 w - - 0 1", 7);
    assert_eq!(m3b, "e3e8");
    assert_eq!(s3b, Score::Mate(3));
}

#[test]
fn solves_tactical_mini_suite_at_least_80_percent() {
    // 16 tactical positions: (name, FEN, accepted best UCI moves)
    let suite: &[(&str, &str, &[&str])] = &[
        // 1. Knight royal fork on f7
        (
            "royal_fork_f7",
            "7k/8/3q4/6N1/8/8/8/4K3 w - - 0 1",
            &["g5f7"],
        ),
        // 2. Knight fork on c7 winning rook on a8
        (
            "knight_fork_c7",
            "r3k3/8/8/1N6/8/8/8/4K3 w - - 0 1",
            &["b5c7"],
        ),
        // 3. Pin along e-file winning defended queen
        (
            "pin_e_file_queen",
            "4k3/3p1p2/4q3/8/8/8/3R4/4K3 w - - 0 1",
            &["d2e2"],
        ),
        // 4. Skewer/pin along e-file winning bishop
        (
            "pin_win_bishop",
            "4k3/8/8/8/4b3/8/8/3R2K1 w - - 0 1",
            &["d1e1"],
        ),
        // 5. Skewer on rank 8 winning rook
        ("skewer_rank8", "4k2r/8/8/8/8/8/8/R3K3 w - - 0 1", &["a1a8"]),
        // 6. Diagonal pin winning rook
        (
            "bishop_skewer",
            "6k1/8/4r3/8/8/3B4/8/6K1 w - - 0 1",
            &["d3c4"],
        ),
        // 7. Discovered check winning queen
        (
            "discovered_check_queen",
            "4k3/8/4q3/8/4N3/8/4R3/4K3 w - - 0 1",
            &["e4f6", "e4d6", "e4c5", "e4g5", "e4c3", "e4g3"],
        ),
        // 8. Hanging knight capture with tempo
        (
            "win_hanging_piece",
            "4k3/8/8/3n4/4P3/8/8/4K3 w - - 0 1",
            &["e4d5"],
        ),
        // 9. Pawn fork winning knight or rook
        (
            "pawn_fork",
            "4k3/8/2n1r3/8/3P4/8/6K1/8 w - - 0 1",
            &["d4d5"],
        ),
        // 10. Deflection of queen defending back rank
        (
            "deflection_back_rank",
            "3q2k1/5ppp/8/8/8/8/3R4/4R1K1 w - - 0 1",
            &["d2d8", "e2e8"],
        ),
        // 11. Promotion tactic winning game
        (
            "promotion_win",
            "8/4P1k1/8/8/8/8/8/4K3 w - - 0 1",
            &["e7e8q"],
        ),
        // 12. Back-rank mate threat / execution
        (
            "back_rank_mate",
            "6k1/5ppp/8/8/8/8/3R4/6K1 w - - 0 1",
            &["d2d8"],
        ),
        // 13. Double attack with Queen (check on e4 + win rook on a8)
        (
            "queen_double_attack",
            "r3k3/8/8/8/8/5Q2/8/4K3 w - - 0 1",
            &["f3e4", "f3a8"],
        ),
        // 14. Knight fork on f7 winning queen/rook
        (
            "knight_fork_qf7",
            "r2q3k/ppp2ppp/2n5/3np1N1/8/8/PPPP1PPP/R1BQ1RK1 w - - 0 1",
            &["g5f7"],
        ),
        // 15. Removing the defender (Bxc6 winning knight defending d8 rook)
        (
            "remove_defender",
            "3r2k1/2p2ppp/2n5/1B6/8/8/2QR4/6K1 w - - 0 1",
            &["b5c6"],
        ),
        // 16. Rook skewer along 8th rank winning rook on h8
        (
            "rook_skewer_file",
            "4k2r/8/8/8/8/8/1R6/4K3 w - - 0 1",
            &["b2b8"],
        ),
    ];

    let mut searcher = Searcher::new(16);
    let mut solved = 0usize;
    let total = suite.len();

    for &(name, fen, expected) in suite {
        searcher.clear_state();
        let mut board = Board::from_fen(fen).expect("valid tactical fen");
        let limits = SearchLimits {
            max_depth: 8,
            ..Default::default()
        };
        let res = searcher.search(&mut board, &limits, |_| {});
        let mv_str = res.best_move.to_uci();
        if expected.contains(&mv_str.as_str()) {
            solved += 1;
        } else {
            eprintln!(
                "Tactic '{}' missed: got {}, expected {:?}",
                name, mv_str, expected
            );
        }
    }

    let pct = (solved * 100) / total;
    eprintln!("Tactical mini-suite: {}/{} ({}%)", solved, total, pct);
    assert!(
        pct >= 80,
        "expected >= 80% on tactical suite, got {}/{} ({}%)",
        solved,
        total,
        pct
    );
}

#[test]
fn infinite_queens_search_returns_legal_move_under_5s() {
    let mut searcher = Searcher::new(16);
    let fens = [
        "4k3/8/8/8/8/8/QQQQQQQQ/QQQ1KQQQ w - - 0 1",
        "4k3/8/8/8/8/4p3/QQQQQQQQ/QQQ1KQQQ w - - 0 1",
    ];

    for fen in fens {
        let mut board = Board::from_fen(fen).expect("iq fen");
        let start = Instant::now();
        let limits = SearchLimits {
            max_depth: 4,
            movetime: Some(std::time::Duration::from_millis(1500)),
            ..Default::default()
        };
        let res = searcher.search(&mut board, &limits, |_| {});
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs_f64() < 5.0,
            "infinite queens search took too long: {:?}",
            elapsed
        );

        let mut legal = MoveList::new();
        generate_legal(&board, &mut legal);
        assert!(legal.contains(res.best_move));
        assert!(parse_uci_move(&board, &res.best_move.to_uci()).is_ok());
    }
}

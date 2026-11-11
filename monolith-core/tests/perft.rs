use std::time::Instant;

use monolith_core::movegen::{generate_legal, perft, perft_divide};
use monolith_core::types::{MoveList, PieceType};
use monolith_core::{zobrist, Board, Color, STARTPOS_FEN};

fn verify_make_unmake_tree(board: &mut Board, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }

    let snapshot_fen = board.to_fen();
    let snapshot_key = board.zobrist_key();

    let mut list = MoveList::new();
    generate_legal(board, &mut list);

    let mut nodes = 0u64;
    for &mv in list.iter() {
        board.make_move(mv);
        assert_eq!(
            board.zobrist_key(),
            zobrist::compute_key(board),
            "Zobrist mismatch after move {}",
            mv
        );
        nodes += verify_make_unmake_tree(board, depth - 1);
        board.unmake_move();
        assert_eq!(
            board.to_fen(),
            snapshot_fen,
            "FEN mismatch after unmake {}",
            mv
        );
        assert_eq!(
            board.zobrist_key(),
            snapshot_key,
            "Zobrist mismatch after unmake {}",
            mv
        );
    }
    nodes
}

#[test]
fn perft_startpos_d1_to_d5() {
    let mut board = Board::from_fen(STARTPOS_FEN).expect("startpos");
    let expected = [
        (1, 20u64),
        (2, 400),
        (3, 8_902),
        (4, 197_281),
        (5, 4_865_609),
    ];

    let start = Instant::now();
    let mut total_nodes = 0u64;
    for (depth, want) in expected {
        let got = perft(&mut board, depth);
        if got != want {
            let div = perft_divide(&mut board, depth);
            panic!(
                "startpos perft d{} mismatch: got {}, expected {} (divide len {})",
                depth,
                got,
                want,
                div.len()
            );
        }
        total_nodes += got;
    }
    let elapsed = start.elapsed().as_secs_f64();
    eprintln!(
        "startpos perft d1-5 total={} nodes in {:.3}s ({:.2} MN/s)",
        total_nodes,
        elapsed,
        (total_nodes as f64 / elapsed) / 1_000_000.0
    );
}

#[test]
fn perft_kiwipete_d1_to_d4() {
    let mut board =
        Board::from_fen("r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1")
            .expect("kiwipete");
    let expected = [(1, 48u64), (2, 2_039), (3, 97_862), (4, 4_085_603)];
    for (depth, want) in expected {
        assert_eq!(perft(&mut board, depth), want, "kiwipete d{}", depth);
    }
}

#[test]
fn perft_pos3_d1_to_d5() {
    let mut board = Board::from_fen("8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1").expect("pos 3");
    let expected = [(1, 14u64), (2, 191), (3, 2_812), (4, 43_238), (5, 674_624)];
    for (depth, want) in expected {
        assert_eq!(perft(&mut board, depth), want, "pos3 d{}", depth);
    }
}

#[test]
fn perft_pos4_d1_to_d4() {
    let mut board =
        Board::from_fen("r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1")
            .expect("pos 4");
    let expected = [(1, 6u64), (2, 264), (3, 9_467), (4, 422_333)];
    for (depth, want) in expected {
        assert_eq!(perft(&mut board, depth), want, "pos4 d{}", depth);
    }
}

#[test]
fn perft_pos5_d1_to_d4() {
    let mut board = Board::from_fen("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8")
        .expect("pos 5");
    let expected = [(1, 44u64), (2, 1_486), (3, 62_379), (4, 2_103_487)];
    for (depth, want) in expected {
        assert_eq!(perft(&mut board, depth), want, "pos5 d{}", depth);
    }
}

#[test]
fn perft_pos6_d1_to_d4() {
    let mut board =
        Board::from_fen("r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10")
            .expect("pos 6");
    let expected = [(1, 46u64), (2, 2_079), (3, 89_890), (4, 3_894_594)];
    for (depth, want) in expected {
        assert_eq!(perft(&mut board, depth), want, "pos6 d{}", depth);
    }
}

#[test]
fn perft_infinite_queens_and_make_unmake_consistency() {
    let mut iq_preset =
        Board::from_fen("4k3/8/8/8/8/8/QQQQQQQQ/QQQ1KQQQ w - - 0 1").expect("iq preset");
    assert_eq!(
        iq_preset
            .piece_bb(Color::White, PieceType::Queen)
            .count_ones(),
        14
    );
    let d3_preset = perft(&mut iq_preset, 3);
    assert!(d3_preset > 0);
    let verified_preset = verify_make_unmake_tree(&mut iq_preset, 2);
    assert_eq!(verified_preset, perft(&mut iq_preset, 2));

    // Legal position where black is not initially in check and white has 14 queens.
    let mut iq_legal =
        Board::from_fen("4k3/8/8/8/8/4p3/QQQQQQQQ/QQQ1KQQQ w - - 0 1").expect("iq legal");
    assert!(!iq_legal.is_square_attacked(iq_legal.king_square(Color::Black), Color::White));
    let d3_legal = perft(&mut iq_legal, 3);
    assert!(d3_legal > 0);
    assert_eq!(
        verify_make_unmake_tree(&mut iq_legal, 2),
        perft(&mut iq_legal, 2)
    );

    // Position with > 256 legal moves verifying MoveList capacity 512 without overflow.
    // Border of white queens around an open center with shielded black king produces > 256 legal moves.
    let mut many_moves = Board::from_fen("QQQQQQnk/Q5pp/Q6Q/Q6Q/Q6Q/Q6Q/Q6Q/KQQQQQQQ w - - 0 1")
        .expect("many queens");
    let mut list = MoveList::new();
    generate_legal(&many_moves, &mut list);
    eprintln!("many_queens legal moves = {}", list.len());
    assert!(
        list.len() > 256,
        "expected > 256 legal moves, got {}",
        list.len()
    );
    assert_eq!(perft(&mut many_moves, 1), list.len() as u64);
    assert!(perft(&mut many_moves, 2) > list.len() as u64);
    assert_eq!(
        verify_make_unmake_tree(&mut many_moves, 2),
        perft(&mut many_moves, 2)
    );
}

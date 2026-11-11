use monolith_core::types::{Move, Square};
use monolith_core::{Board, STARTPOS_FEN};
use monolith_engine::eval::{eval_breakdown, evaluate, MAX_EVAL_CP};
use monolith_engine::tt::{Bound, TranspositionTable, MATE_SCORE};

fn mirror_fen(fen: &str) -> String {
    let parts: Vec<&str> = fen.split_whitespace().collect();
    let ranks: Vec<String> = parts[0]
        .split('/')
        .rev()
        .map(|r| {
            r.chars()
                .map(|c| {
                    if c.is_ascii_uppercase() {
                        c.to_ascii_lowercase()
                    } else if c.is_ascii_lowercase() {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    }
                })
                .collect()
        })
        .collect();

    let side = if parts[1] == "w" { "b" } else { "w" };
    let castling = if parts[2] == "-" {
        "-".to_string()
    } else {
        let mut s = String::new();
        if parts[2].contains('k') {
            s.push('K');
        }
        if parts[2].contains('q') {
            s.push('Q');
        }
        if parts[2].contains('K') {
            s.push('k');
        }
        if parts[2].contains('Q') {
            s.push('q');
        }
        s
    };
    let ep = if parts[3] == "-" {
        "-".to_string()
    } else {
        let mut chars = parts[3].chars();
        let f = chars.next().expect("file");
        let r = chars.next().expect("rank");
        let flipped_r = match r {
            '3' => '6',
            '6' => '3',
            other => other,
        };
        format!("{}{}", f, flipped_r)
    };

    format!(
        "{} {} {} {} {} {}",
        ranks.join("/"),
        side,
        castling,
        ep,
        parts[4],
        parts[5]
    )
}

#[test]
fn eval_symmetry_across_diverse_positions() {
    let fens = [
        STARTPOS_FEN,
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        "r1bqk2r/pp1pppbp/2n2np1/8/3NP3/2N1B3/PPP1BPPP/R2QK2R b KQkq - 3 7",
        "4k3/2p2ppp/8/3P4/8/8/5PPP/4K1R1 w - - 0 30",
    ];

    for fen in fens {
        let b1 = Board::from_fen(fen).expect("fen");
        let m_fen = mirror_fen(fen);
        let b2 = Board::from_fen(&m_fen).expect("mirrored fen");

        let e1 = evaluate(&b1);
        let e2 = evaluate(&b2);
        assert_eq!(e1, e2, "eval symmetry failed for {}", fen);

        let bd1 = eval_breakdown(&b1);
        let bd2 = eval_breakdown(&b2);
        assert_eq!(bd1.white_pov_cp, -bd2.white_pov_cp);
        assert_eq!(bd1.phase, bd2.phase);
    }
}

#[test]
fn eval_infinite_queens_no_overflow_and_phase_clamped() {
    let iq_fens = [
        "4k3/8/8/8/8/8/QQQQQQQQ/QQQ1KQQQ w - - 0 1",
        "4k3/8/8/8/8/8/QQQQQQQQ/QQQQKQQQ w - - 0 1",
        "QQQQQQQk/QQQQQQQQ/QQQQQQQQ/QQQQQQQQ/QQQQQQQQ/QQQQQQQQ/QQQQQQQQ/KQQQQQQQ w - - 0 1",
    ];

    for fen in iq_fens {
        let b = Board::from_fen(fen).expect("iq fen");
        let bd = eval_breakdown(&b);
        assert_eq!(bd.phase, 24);
        assert!(bd.stm_pov_cp > 5_000 && bd.stm_pov_cp <= MAX_EVAL_CP);

        let mb = Board::from_fen(&mirror_fen(fen)).expect("mirrored iq fen");
        assert_eq!(evaluate(&b), evaluate(&mb));
    }
}

#[test]
fn transposition_table_store_probe_mate_and_age() {
    let mut tt = TranspositionTable::new(1);
    let mv = Move::new(Square::E2, Square::E4, Move::FLAG_DOUBLE_PUSH);

    // Basic store and probe.
    tt.store(0x1234_5678_9ABC_DEF0, 6, 145, Bound::Exact, mv, 0);
    let hit = tt.probe(0x1234_5678_9ABC_DEF0, 0).expect("tt hit");
    assert_eq!(hit.best_move, mv);
    assert_eq!(hit.score, 145);
    assert_eq!(hit.depth, 6);
    assert_eq!(hit.bound, Bound::Exact);

    // Mate score ply normalization (positive and negative).
    let mate_key = 0xCAFE_BABE_1111_2222;
    tt.store(mate_key, 8, MATE_SCORE - 5, Bound::Exact, mv, 2);
    let hit_ply4 = tt.probe(mate_key, 4).expect("mate hit");
    assert_eq!(hit_ply4.score, MATE_SCORE - 7);

    let mated_key = 0xDEAD_BEEF_3333_4444;
    tt.store(mated_key, 8, -MATE_SCORE + 4, Bound::Exact, mv, 1);
    let hit_ply3 = tt.probe(mated_key, 3).expect("mated hit");
    assert_eq!(hit_ply3.score, -MATE_SCORE + 6);

    // Aging replacement: entry from old age should be overwritten by shallower entry.
    tt.new_search();
    assert_eq!(tt.hashfull(), 0);
    let mv2 = Move::new(Square::D2, Square::D4, Move::FLAG_DOUBLE_PUSH);
    tt.store(0x1234_5678_9ABC_DEF0, 1, 20, Bound::Lower, mv2, 0);
    let updated = tt.probe(0x1234_5678_9ABC_DEF0, 0).expect("updated");
    assert_eq!(updated.best_move, mv2);
    assert_eq!(updated.depth, 1);

    // Hashfull reflects populated entries and resets on clear.
    for i in 1..=2000u64 {
        tt.store(
            i.wrapping_mul(0x9E37_79B9_7F4A_7C15),
            2,
            10,
            Bound::Upper,
            mv,
            0,
        );
    }
    assert!(tt.hashfull() > 0);
    tt.clear();
    assert_eq!(tt.hashfull(), 0);
    assert!(tt.probe(mate_key, 0).is_none());

    // Resize clamps cleanly.
    tt.resize(0);
    assert!(tt.len() >= 1024);
}

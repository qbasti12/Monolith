use monolith_core::movegen::generate_legal;
use monolith_core::pgn::{
    classify_move, move_to_san, parse_pgn, parse_san, ClassifyInput, MoveClass,
};
use monolith_core::types::MoveList;
use monolith_core::{Board, CoreError, STARTPOS_FEN};

fn verify_san_roundtrip_depth(board: &mut Board, depth: u32) {
    let mut legal = MoveList::new();
    generate_legal(board, &mut legal);
    for &mv in legal.iter() {
        let san = move_to_san(board, mv).expect("move_to_san should succeed for legal move");
        let parsed = parse_san(board, &san).expect("parse_san should roundtrip");
        assert_eq!(
            parsed,
            mv,
            "SAN roundtrip mismatch for {} on FEN {}",
            san,
            board.to_fen()
        );
        if depth > 1 {
            board.make_move(mv);
            verify_san_roundtrip_depth(board, depth - 1);
            board.unmake_move();
        }
    }
}

#[test]
fn san_roundtrip_on_all_perft_and_queens_positions() {
    let fens = [
        STARTPOS_FEN,
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
        "r4rk1/1pp1qppp/p1np1n2/2b1p1B1/2B1P1b1/P1NP1N2/1PP1QPPP/R4RK1 w - - 0 10",
        "4k3/8/8/8/8/8/QQQQQQQQ/QQQ1KQQQ w - - 0 1",
    ];

    for fen in fens {
        let mut board = Board::from_fen(fen).expect("valid fen");
        let depth = if fen == STARTPOS_FEN { 2 } else { 1 };
        verify_san_roundtrip_depth(&mut board, depth);
    }

    // Also test depth 2 on Pos 5 (promotions, captures, checks, castling).
    let mut pos5 =
        Board::from_fen("rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8").expect("pos5");
    verify_san_roundtrip_depth(&mut pos5, 2);
}

#[test]
fn parse_pgn_games_with_castling_ep_promotion_comments_and_variations() {
    // Game 1: Scholar's / tactical game with kingside castling, EP, comments, NAGs, nested variations, and trailing second game.
    let pgn1 = r#"
[Event "Test Game 1"]
[Site "Berlin"]
[White "Player A"]
[Black "Player B"]
[Result "1-0"]

1. e4 {King's pawn opening} e5 (1... c5 2. Nf3 (2. d4 cxd4) d6) 2. Nf3 $1 Nc6 3. Bb5 a6
4. Ba4 Nf6 5. O-O Be7 ; line comment after Be7
6. d4 exd4 7. e5 Ne4 8. c3 d5 9. cxd4 O-O 10. Bc2 f5 11. exf6 Bxf6 1-0

[Event "Second Game Ignored"]
1. d4 d5 1/2-1/2
"#;

    let game1 = parse_pgn(pgn1).expect("pgn1 should parse");
    assert_eq!(game1.header("Event"), Some("Test Game 1"));
    assert_eq!(game1.result.as_deref(), Some("1-0"));
    assert_eq!(game1.moves.len(), 22);
    assert_eq!(
        game1.moves[0].comment.as_deref(),
        Some("King's pawn opening")
    );
    assert_eq!(game1.moves[8].san, "O-O");
    assert_eq!(game1.moves[20].san, "exf6");

    // Game 2: Custom [FEN] setup featuring queenside castling and pawn promotion to Queen with checkmate/check.
    let pgn2 = r#"
[Event "Endgame Study"]
[SetUp "1"]
[FEN "r3k2r/4P3/8/8/8/8/8/R3K2R w KQkq - 0 25"]

25. O-O-O Kf7 26. e8=Q+ Raxe8 27. Rd7+ Kf6 28. Rf1+ Kg6 0-1
"#;

    let game2 = parse_pgn(pgn2).expect("pgn2 should parse");
    assert_eq!(game2.moves.len(), 8);
    assert_eq!(game2.moves[0].san, "O-O-O");
    assert_eq!(game2.moves[0].move_number, 25);
    assert_eq!(game2.moves[2].san, "e8=Q+");
    assert_eq!(game2.result.as_deref(), Some("0-1"));
}

#[test]
fn invalid_pgn_reports_error_with_move_number() {
    let bad_pgn = "1. e4 e5 2. Nf3 Qxh8 1-0";
    let err = parse_pgn(bad_pgn).expect_err("illegal move should fail");
    match err {
        CoreError::InvalidPgn { move_number, .. } => assert_eq!(move_number, 2),
        other => panic!("unexpected error variant: {:?}", other),
    }

    assert!(matches!(parse_pgn("   \n  "), Err(CoreError::EmptyPgn)));
    assert!(parse_pgn("[Event \"Unclosed").is_err());
    assert!(parse_pgn("1. e4 (1... c5").is_err());
    assert!(parse_pgn("1. e4 (1... c5 {unterminated comment").is_err());

    // Relaxed SAN pawn captures (`ed5`, `ed`, `exd`).
    let scandi =
        Board::from_fen("rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2").unwrap();
    let mv_exd5 = parse_san(&scandi, "exd5").expect("exd5");
    assert_eq!(parse_san(&scandi, "ed5").expect("ed5"), mv_exd5);
    assert_eq!(parse_san(&scandi, "ed").expect("ed"), mv_exd5);
    assert_eq!(parse_san(&scandi, "exd").expect("exd"), mv_exd5);
}

#[test]
fn move_classification_thresholds() {
    // Brilliant sacrifice (losing >= 200 cp material while keeping soundness, not already > +600).
    let brilliant = classify_move(&ClassifyInput {
        eval_before_best_cp: 80,
        eval_after_played_cp: 75,
        second_best_cp: Some(20),
        is_engine_top_move: true,
        is_top_two_move: true,
        material_delta_cp: -300,
        best_san: Some("Nxf7".to_string()),
    });
    assert_eq!(brilliant.class, MoveClass::Brilliant);
    assert_eq!(brilliant.class.symbol(), "!!");

    // Sacrifice when already +750 cp should NOT be Brilliant, instead Best.
    let not_brilliant = classify_move(&ClassifyInput {
        eval_before_best_cp: 750,
        eval_after_played_cp: 745,
        second_best_cp: Some(700),
        is_engine_top_move: true,
        is_top_two_move: true,
        material_delta_cp: -300,
        best_san: None,
    });
    assert_eq!(not_brilliant.class, MoveClass::Best);

    // Great move: > 150 cp gap over 2nd best.
    let great = classify_move(&ClassifyInput {
        eval_before_best_cp: 210,
        eval_after_played_cp: 210,
        second_best_cp: Some(10),
        is_engine_top_move: true,
        is_top_two_move: true,
        material_delta_cp: 0,
        best_san: Some("Qd5".to_string()),
    });
    assert_eq!(great.class, MoveClass::Great);

    // Good (loss 35), Inaccuracy (loss 90), Mistake (loss 220), Blunder (loss 480).
    let good = classify_move(&ClassifyInput {
        eval_before_best_cp: 50,
        eval_after_played_cp: 15,
        second_best_cp: None,
        is_engine_top_move: false,
        is_top_two_move: false,
        material_delta_cp: 0,
        best_san: None,
    });
    assert_eq!(good.class, MoveClass::Good);

    let inaccuracy = classify_move(&ClassifyInput {
        eval_before_best_cp: 100,
        eval_after_played_cp: 10,
        second_best_cp: None,
        is_engine_top_move: false,
        is_top_two_move: false,
        material_delta_cp: 0,
        best_san: Some("d4".to_string()),
    });
    assert_eq!(inaccuracy.class, MoveClass::Inaccuracy);

    let mistake = classify_move(&ClassifyInput {
        eval_before_best_cp: 100,
        eval_after_played_cp: -120,
        second_best_cp: None,
        is_engine_top_move: false,
        is_top_two_move: false,
        material_delta_cp: 0,
        best_san: Some("Be2".to_string()),
    });
    assert_eq!(mistake.class, MoveClass::Mistake);

    let blunder = classify_move(&ClassifyInput {
        eval_before_best_cp: 100,
        eval_after_played_cp: -380,
        second_best_cp: None,
        is_engine_top_move: false,
        is_top_two_move: false,
        material_delta_cp: 0,
        best_san: Some("Rf1".to_string()),
    });
    assert_eq!(blunder.class, MoveClass::Blunder);
}

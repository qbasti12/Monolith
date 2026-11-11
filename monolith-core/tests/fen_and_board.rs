use monolith_core::bitboard;
use monolith_core::types::{Color, Move, PieceType, Square};
use monolith_core::{zobrist, Board, STARTPOS_FEN};

#[test]
fn fen_roundtrip_standard_and_infinite_queens() {
    let fens = [
        STARTPOS_FEN,
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
        "4k3/8/8/8/8/8/QQQQQQQQ/QQQ1KQQQ w - - 0 1",
        "4k3/8/8/8/8/8/QQQQQQQQ/QQQQKQQQ w - - 0 1",
        "QQQQQQQk/QQQQQQQQ/QQQQQQQQ/QQQQQQQQ/QQQQQQQQ/QQQQQQQQ/QQQQQQQQ/KQQQQQQQ w - - 0 1",
    ];

    for fen in fens {
        let board = Board::from_fen(fen).expect("valid FEN should parse");
        assert_eq!(board.to_fen(), fen);
        assert_eq!(board.zobrist_key(), zobrist::compute_key(&board));
    }

    let iq14 = Board::from_fen("4k3/8/8/8/8/8/QQQQQQQQ/QQQ1KQQQ w - - 0 1").expect("iq14 fen");
    assert_eq!(
        iq14.piece_bb(Color::White, PieceType::Queen).count_ones(),
        14
    );
    let iq15 = Board::from_fen("4k3/8/8/8/8/8/QQQQQQQQ/QQQQKQQQ w - - 0 1").expect("iq15 fen");
    assert_eq!(
        iq15.piece_bb(Color::White, PieceType::Queen).count_ones(),
        15
    );
    let iq62 = Board::from_fen(
        "QQQQQQQk/QQQQQQQQ/QQQQQQQQ/QQQQQQQQ/QQQQQQQQ/QQQQQQQQ/QQQQQQQQ/KQQQQQQQ w - - 0 1",
    )
    .expect("iq62 fen");
    assert_eq!(
        iq62.piece_bb(Color::White, PieceType::Queen).count_ones(),
        62
    );
}

#[test]
fn invalid_fens_return_err_without_panic() {
    let invalid = [
        "",
        "garbage input not a fen",
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0", // 5 fields
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP w KQkq - 0 1",        // 7 ranks
        "rnbqkbnr/pppppppp/9/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", // width 9
        "rnbqkbnr/pppppppp/43/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", // adjacent digits
        "rnbq1bnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", // missing black king
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNK w KQkq - 0 1", // two white kings
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 0", // fullmove 0
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/PNBQKBNR w KQkq - 0 1", // white pawn on rank 1
        "pnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", // black pawn on rank 8
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR x KQkq - 0 1", // bad side
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KK - 0 1", // duplicate castling
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBN1 w KQkq - 0 1", // K without h1 rook
        "4k3/8/8/8/8/8/8/4K2R w Q - 0 1",                         // Q without a1 rook
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq e3 0 1", // implausible EP
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e3 0 1", // EP rank mismatch for white
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPPPPPP/RNBQKBNR b KQkq e3 0 1", // e2 still occupied
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq z9 0 1", // bad EP square
    ];

    for fen in invalid {
        assert!(Board::from_fen(fen).is_err(), "expected Err for '{}'", fen);
    }
}

#[test]
fn make_unmake_and_draw_detection() {
    let mut board = Board::startpos();
    let initial_fen = board.to_fen();
    let initial_key = board.zobrist_key();

    // Play Nf3, Nf6, Ng1, Ng8 twice -> repetition.
    let e2 = "g1".parse::<Square>().expect("g1");
    let f3 = "f3".parse::<Square>().expect("f3");
    let g8 = "g8".parse::<Square>().expect("g8");
    let f6 = "f6".parse::<Square>().expect("f6");

    let w_out = Move::new(e2, f3, Move::FLAG_QUIET);
    let w_back = Move::new(f3, e2, Move::FLAG_QUIET);
    let b_out = Move::new(g8, f6, Move::FLAG_QUIET);
    let b_back = Move::new(f6, g8, Move::FLAG_QUIET);

    for _ in 0..2 {
        board.make_move(w_out);
        board.make_move(b_out);
        board.make_move(w_back);
        board.make_move(b_back);
    }
    assert!(board.is_repetition());
    assert!(board.is_threefold_repetition());

    for _ in 0..8 {
        board.unmake_move();
    }
    assert_eq!(board.to_fen(), initial_fen);
    assert_eq!(board.zobrist_key(), initial_key);

    // Null move consistency.
    board.make_null_move();
    assert_eq!(board.side_to_move(), Color::Black);
    board.unmake_null_move();
    assert_eq!(board.to_fen(), initial_fen);
    assert_eq!(board.zobrist_key(), initial_key);

    // Insufficient material cases.
    assert!(Board::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1")
        .expect("k vs k")
        .is_insufficient_material());
    assert!(Board::from_fen("4k3/8/8/8/8/8/8/4KB2 w - - 0 1")
        .expect("kb vs k")
        .is_insufficient_material());
    assert!(Board::from_fen("4k3/8/8/8/8/8/8/4KN2 w - - 0 1")
        .expect("kn vs k")
        .is_insufficient_material());
    assert!(Board::from_fen("4kb2/8/8/8/8/8/8/4K1B1 w - - 0 1")
        .expect("same color bishops")
        .is_insufficient_material());
    assert!(!Board::from_fen("4kb2/8/8/8/8/8/8/4KB2 w - - 0 1")
        .expect("opposite color bishops")
        .is_insufficient_material());
    assert!(!Board::from_fen("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1")
        .expect("pawn")
        .is_insufficient_material());

    // Fifty-move draw.
    let fifty = Board::from_fen("4k3/8/8/8/8/8/4R3/4K3 w - - 100 60").expect("50 move");
    assert!(fifty.is_fifty_move_draw());

    // Magic bitboard sanity check.
    let occ = Square::E4.bit() | Square::B1.bit() | Square::H7.bit();
    let rook_e1 = bitboard::rook_attacks(Square::E1, occ);
    assert!((rook_e1 & Square::E4.bit()) != 0);
    assert!((rook_e1 & Square::E5.bit()) == 0);
}

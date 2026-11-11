//! Direct legal move generation, UCI move parsing, and perft/divide utilities.

use crate::bitboard::{
    between_bb, bishop_attacks, king_attacks, knight_attacks, line_bb, lsb, pawn_attacks, pop_lsb,
    rook_attacks, RANK_2, RANK_7,
};
use crate::board::Board;
use crate::types::{CastlingRights, Color, CoreError, Move, MoveList, PieceType, Square};

/// Generates all legal moves in the current position into `list` without heap allocation.
pub fn generate_legal(board: &Board, list: &mut MoveList) {
    generate_filtered(board, list, false, false);
}

/// Generates all legal capture moves (including en passant and capture-promotions) into `list`.
pub fn generate_captures(board: &Board, list: &mut MoveList) {
    generate_filtered(board, list, true, false);
}

/// Generates all legal noisy moves (all captures + quiet promotions) for quiescence search.
pub fn generate_noisy(board: &Board, list: &mut MoveList) {
    generate_filtered(board, list, true, true);
}

fn generate_filtered(
    board: &Board,
    list: &mut MoveList,
    captures_only: bool,
    include_quiet_promos: bool,
) {
    list.clear();

    let stm = board.side_to_move();
    let opp = stm.flip();
    let king_sq = board.king_square(stm);
    let own = board.color_occupancy(stm);
    let enemy = board.color_occupancy(opp);
    let occ = board.all_occupancy();
    let opp_king_bb = board.piece_bb(opp, PieceType::King);
    let valid_enemy = enemy & !opp_king_bb;

    let opp_pawns = board.piece_bb(opp, PieceType::Pawn);
    let opp_knights = board.piece_bb(opp, PieceType::Knight);
    let opp_bishops = board.piece_bb(opp, PieceType::Bishop);
    let opp_rooks = board.piece_bb(opp, PieceType::Rook);
    let opp_queens = board.piece_bb(opp, PieceType::Queen);
    let opp_bq = opp_bishops | opp_queens;
    let opp_rq = opp_rooks | opp_queens;

    // 1. Find checking pieces.
    let checkers = (pawn_attacks(stm, king_sq) & opp_pawns)
        | (knight_attacks(king_sq) & opp_knights)
        | (bishop_attacks(king_sq, occ) & opp_bq)
        | (rook_attacks(king_sq, occ) & opp_rq);

    // 2. King moves (always legal if destination is not owned and not attacked without our king).
    let occ_no_king = occ ^ king_sq.bit();
    let king_targets = if captures_only {
        valid_enemy
    } else {
        !own & !opp_king_bb
    };
    let mut king_moves = king_attacks(king_sq) & king_targets;
    while king_moves != 0 {
        let to = pop_lsb(&mut king_moves);
        if !board.is_square_attacked_occ(to, opp, occ_no_king) {
            let flag = if (valid_enemy & to.bit()) != 0 {
                Move::FLAG_CAPTURE
            } else {
                Move::FLAG_QUIET
            };
            list.push(Move::new(king_sq, to, flag));
        }
    }

    let num_checkers = checkers.count_ones();
    if num_checkers >= 2 {
        // Double check: only king evasions are legal.
        return;
    }

    // 3. Compute pinned own pieces along rays from `king_sq`.
    let mut pinned = 0u64;
    let mut snipers = (rook_attacks(king_sq, 0) & opp_rq) | (bishop_attacks(king_sq, 0) & opp_bq);
    while snipers != 0 {
        let sniper_sq = pop_lsb(&mut snipers);
        let btw = between_bb(king_sq, sniper_sq) & occ;
        if btw.count_ones() == 1 && (btw & own) != 0 {
            pinned |= btw;
        }
    }

    // 4. Check resolution mask (all squares if not in check).
    let (in_check, checker_sq, check_mask) = if num_checkers == 1 {
        let c_sq = lsb(checkers);
        (true, Some(c_sq), between_bb(king_sq, c_sq) | c_sq.bit())
    } else {
        (false, None, !0u64)
    };

    let target_mask = if captures_only {
        valid_enemy & check_mask
    } else {
        (!own & !opp_king_bb) & check_mask
    };

    // 5. Castling (only when not in check and not captures-only).
    if !in_check && !captures_only {
        let cr = board.castling_rights();
        match stm {
            Color::White => {
                if cr.has(CastlingRights::WHITE_KING)
                    && (occ & (Square::F1.bit() | Square::G1.bit())) == 0
                    && !board.is_square_attacked(Square::F1, Color::Black)
                    && !board.is_square_attacked(Square::G1, Color::Black)
                {
                    list.push(Move::new(Square::E1, Square::G1, Move::FLAG_KING_CASTLE));
                }
                if cr.has(CastlingRights::WHITE_QUEEN)
                    && (occ & (Square::B1.bit() | Square::C1.bit() | Square::D1.bit())) == 0
                    && !board.is_square_attacked(Square::D1, Color::Black)
                    && !board.is_square_attacked(Square::C1, Color::Black)
                {
                    list.push(Move::new(Square::E1, Square::C1, Move::FLAG_QUEEN_CASTLE));
                }
            }
            Color::Black => {
                if cr.has(CastlingRights::BLACK_KING)
                    && (occ & (Square::F8.bit() | Square::G8.bit())) == 0
                    && !board.is_square_attacked(Square::F8, Color::White)
                    && !board.is_square_attacked(Square::G8, Color::White)
                {
                    list.push(Move::new(Square::E8, Square::G8, Move::FLAG_KING_CASTLE));
                }
                if cr.has(CastlingRights::BLACK_QUEEN)
                    && (occ & (Square::B8.bit() | Square::C8.bit() | Square::D8.bit())) == 0
                    && !board.is_square_attacked(Square::D8, Color::White)
                    && !board.is_square_attacked(Square::C8, Color::White)
                {
                    list.push(Move::new(Square::E8, Square::C8, Move::FLAG_QUEEN_CASTLE));
                }
            }
        }
    }

    // 6. Knights (pinned knights cannot move).
    let mut knights = board.piece_bb(stm, PieceType::Knight) & !pinned;
    while knights != 0 {
        let from = pop_lsb(&mut knights);
        let mut moves = knight_attacks(from) & target_mask;
        while moves != 0 {
            let to = pop_lsb(&mut moves);
            let flag = if (valid_enemy & to.bit()) != 0 {
                Move::FLAG_CAPTURE
            } else {
                Move::FLAG_QUIET
            };
            list.push(Move::new(from, to, flag));
        }
    }

    // 7. Diagonal sliders (Bishops + Queens).
    let mut diag_sliders =
        board.piece_bb(stm, PieceType::Bishop) | board.piece_bb(stm, PieceType::Queen);
    while diag_sliders != 0 {
        let from = pop_lsb(&mut diag_sliders);
        let pin_mask = if (pinned & from.bit()) != 0 {
            line_bb(king_sq, from)
        } else {
            !0u64
        };
        let mut moves = bishop_attacks(from, occ) & target_mask & pin_mask;
        while moves != 0 {
            let to = pop_lsb(&mut moves);
            let flag = if (valid_enemy & to.bit()) != 0 {
                Move::FLAG_CAPTURE
            } else {
                Move::FLAG_QUIET
            };
            list.push(Move::new(from, to, flag));
        }
    }

    // 8. Orthogonal sliders (Rooks + Queens).
    let mut ortho_sliders =
        board.piece_bb(stm, PieceType::Rook) | board.piece_bb(stm, PieceType::Queen);
    while ortho_sliders != 0 {
        let from = pop_lsb(&mut ortho_sliders);
        let pin_mask = if (pinned & from.bit()) != 0 {
            line_bb(king_sq, from)
        } else {
            !0u64
        };
        let mut moves = rook_attacks(from, occ) & target_mask & pin_mask;
        while moves != 0 {
            let to = pop_lsb(&mut moves);
            let flag = if (valid_enemy & to.bit()) != 0 {
                Move::FLAG_CAPTURE
            } else {
                Move::FLAG_QUIET
            };
            list.push(Move::new(from, to, flag));
        }
    }

    // 9. Pawns.
    generate_pawn_moves(
        board,
        list,
        stm,
        king_sq,
        occ,
        valid_enemy,
        pinned,
        check_mask,
        checker_sq,
        opp_rq,
        opp_bq,
        captures_only,
        include_quiet_promos,
    );
}

#[allow(clippy::too_many_arguments)]
fn generate_pawn_moves(
    board: &Board,
    list: &mut MoveList,
    stm: Color,
    king_sq: Square,
    occ: u64,
    valid_enemy: u64,
    pinned: u64,
    check_mask: u64,
    checker_sq: Option<Square>,
    opp_rq: u64,
    opp_bq: u64,
    captures_only: bool,
    include_quiet_promos: bool,
) {
    let mut pawns = board.piece_bb(stm, PieceType::Pawn);
    let promo_rank_mask = match stm {
        Color::White => RANK_7,
        Color::Black => RANK_2,
    };
    let start_rank_mask = match stm {
        Color::White => RANK_2,
        Color::Black => RANK_7,
    };
    let push_delta: i8 = match stm {
        Color::White => 8,
        Color::Black => -8,
    };

    while pawns != 0 {
        let from = pop_lsb(&mut pawns);
        let from_bit = from.bit();
        let is_promo = (from_bit & promo_rank_mask) != 0;
        let pin_mask = if (pinned & from_bit) != 0 {
            line_bb(king_sq, from)
        } else {
            !0u64
        };

        // Captures.
        let mut caps = pawn_attacks(stm, from) & valid_enemy & check_mask & pin_mask;
        while caps != 0 {
            let to = pop_lsb(&mut caps);
            if is_promo {
                for promo in PieceType::PROMOTIONS {
                    list.push(Move::promotion(from, to, promo, true));
                }
            } else {
                list.push(Move::new(from, to, Move::FLAG_CAPTURE));
            }
        }

        // Single and double pushes.
        if !captures_only || (include_quiet_promos && is_promo) {
            let one_sq = Square((from.0 as i8 + push_delta) as u8);
            if (occ & one_sq.bit()) == 0 {
                if (one_sq.bit() & check_mask & pin_mask) != 0 {
                    if is_promo {
                        for promo in PieceType::PROMOTIONS {
                            list.push(Move::promotion(from, one_sq, promo, false));
                        }
                    } else if !captures_only {
                        list.push(Move::new(from, one_sq, Move::FLAG_QUIET));
                    }
                }

                if !captures_only && (from_bit & start_rank_mask) != 0 {
                    let two_sq = Square((one_sq.0 as i8 + push_delta) as u8);
                    if (occ & two_sq.bit()) == 0 && (two_sq.bit() & check_mask & pin_mask) != 0 {
                        list.push(Move::new(from, two_sq, Move::FLAG_DOUBLE_PUSH));
                    }
                }
            }
        }
    }

    // En passant captures.
    if let Some(ep_sq) = board.en_passant() {
        let cap_sq = match stm {
            Color::White => Square(ep_sq.0 - 8),
            Color::Black => Square(ep_sq.0 + 8),
        };

        // When in check, EP is only allowed if it captures the checking pawn or blocks the check ray.
        if let Some(c_sq) = checker_sq {
            if c_sq != cap_sq && (ep_sq.bit() & check_mask) == 0 {
                return;
            }
        }

        let mut ep_candidates =
            pawn_attacks(stm.flip(), ep_sq) & board.piece_bb(stm, PieceType::Pawn);
        while ep_candidates != 0 {
            let from = pop_lsb(&mut ep_candidates);
            let occ_after = (occ ^ from.bit() ^ cap_sq.bit()) | ep_sq.bit();
            let king_exposed = (rook_attacks(king_sq, occ_after) & opp_rq) != 0
                || (bishop_attacks(king_sq, occ_after) & opp_bq) != 0;
            if !king_exposed {
                list.push(Move::new(from, ep_sq, Move::FLAG_EN_PASSANT));
            }
        }
    }
}

/// Parses a UCI move string (e.g. `"e2e4"` or `"e7e8q"`) and validates legality on `board`.
pub fn parse_uci_move(board: &Board, uci: &str) -> Result<Move, CoreError> {
    let s = uci.trim();
    if s.len() != 4 && s.len() != 5 {
        return Err(CoreError::InvalidUciMove(uci.to_string()));
    }
    let from: Square = s[0..2]
        .parse()
        .map_err(|_| CoreError::InvalidUciMove(uci.to_string()))?;
    let to: Square = s[2..4]
        .parse()
        .map_err(|_| CoreError::InvalidUciMove(uci.to_string()))?;
    let promo = if s.len() == 5 {
        let ch = s.as_bytes()[4] as char;
        let pt = match ch.to_ascii_lowercase() {
            'n' => PieceType::Knight,
            'b' => PieceType::Bishop,
            'r' => PieceType::Rook,
            'q' => PieceType::Queen,
            _ => return Err(CoreError::InvalidUciMove(uci.to_string())),
        };
        Some(pt)
    } else {
        None
    };

    let mut list = MoveList::new();
    generate_legal(board, &mut list);
    for &mv in list.iter() {
        if mv.from_sq() == from && mv.to_sq() == to && mv.promotion_piece() == promo {
            return Ok(mv);
        }
    }

    Err(CoreError::IllegalMove(uci.to_string()))
}

/// Counts legal leaf nodes at `depth` from `board` (with bulk counting at depth 1).
pub fn perft(board: &mut Board, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }

    let mut list = MoveList::new();
    generate_legal(board, &mut list);
    if depth == 1 {
        return list.len() as u64;
    }

    let mut nodes = 0u64;
    for &mv in list.iter() {
        board.make_move(mv);
        nodes += perft(board, depth - 1);
        board.unmake_move();
    }
    nodes
}

/// Computes perft node counts broken down per root legal move.
pub fn perft_divide(board: &mut Board, depth: u32) -> Vec<(Move, u64)> {
    let mut list = MoveList::new();
    generate_legal(board, &mut list);
    let mut out = Vec::with_capacity(list.len());

    if depth == 0 {
        return out;
    }

    for &mv in list.iter() {
        let count = if depth == 1 {
            1
        } else {
            board.make_move(mv);
            let n = perft(board, depth - 1);
            board.unmake_move();
            n
        };
        out.push((mv, count));
    }
    out
}

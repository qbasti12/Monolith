//! FEN parser and serializer with strict structural validation and no piece-count limits.

use crate::bitboard::{RANK_1, RANK_8};
use crate::board::Board;
use crate::types::{CastlingRights, Color, CoreError, PieceType, Square};

pub const STARTPOS_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

impl Board {
    /// Parses a FEN string into a validated [`Board`].
    pub fn from_fen(fen: &str) -> Result<Self, CoreError> {
        parse_fen(fen)
    }

    /// Exports the current board state to a standard 6-field FEN string.
    pub fn to_fen(&self) -> String {
        export_fen(self)
    }
}

/// Parses and validates a standard 6-field (or 4-field clockless) FEN string.
pub fn parse_fen(fen: &str) -> Result<Board, CoreError> {
    let parts: Vec<&str> = fen.split_whitespace().collect();
    if parts.len() != 6 && parts.len() != 4 {
        return Err(CoreError::InvalidFen(format!(
            "expected 4 or 6 whitespace-separated fields, got {}",
            parts.len()
        )));
    }

    let (placement, side_str, castling_str, ep_str, halfmove_str, fullmove_str) =
        if parts.len() == 6 {
            (parts[0], parts[1], parts[2], parts[3], parts[4], parts[5])
        } else {
            (parts[0], parts[1], parts[2], parts[3], "0", "1")
        };

    let mut board = Board::empty();

    let ranks: Vec<&str> = placement.split('/').collect();
    if ranks.len() != 8 {
        return Err(CoreError::InvalidFen(format!(
            "expected 8 ranks separated by '/', got {}",
            ranks.len()
        )));
    }

    for (rank_idx, rank_str) in ranks.iter().enumerate() {
        let rank = 7u8 - (rank_idx as u8);
        let mut file = 0u8;
        let mut prev_digit = false;

        for ch in rank_str.chars() {
            if let Some(empty) = ch.to_digit(10) {
                if !(1..=8).contains(&empty) || prev_digit {
                    return Err(CoreError::InvalidFen(format!(
                        "invalid empty-square digit '{}' in rank {}",
                        ch,
                        rank + 1
                    )));
                }
                file = file.saturating_add(empty as u8);
                prev_digit = true;
            } else if let Some((color, pt)) = PieceType::from_char(ch) {
                if file >= 8 {
                    return Err(CoreError::InvalidFen(format!(
                        "rank {} exceeds 8 squares",
                        rank + 1
                    )));
                }
                board.set_piece_raw(color, pt, Square::from_coords(file, rank));
                file += 1;
                prev_digit = false;
            } else {
                return Err(CoreError::InvalidFen(format!(
                    "unexpected character '{}' in piece placement",
                    ch
                )));
            }
        }

        if file != 8 {
            return Err(CoreError::InvalidFen(format!(
                "rank {} has width {} instead of 8",
                rank + 1,
                file
            )));
        }
    }

    // Rule: exactly one king per side.
    if board.piece_bb(Color::White, PieceType::King).count_ones() != 1 {
        return Err(CoreError::InvalidFen(
            "white must have exactly 1 king".to_string(),
        ));
    }
    if board.piece_bb(Color::Black, PieceType::King).count_ones() != 1 {
        return Err(CoreError::InvalidFen(
            "black must have exactly 1 king".to_string(),
        ));
    }

    // Rule: no pawns on rank 1 or rank 8.
    let pawns = board.piece_bb(Color::White, PieceType::Pawn)
        | board.piece_bb(Color::Black, PieceType::Pawn);
    if (pawns & (RANK_1 | RANK_8)) != 0 {
        return Err(CoreError::InvalidFen(
            "pawns cannot be placed on rank 1 or rank 8".to_string(),
        ));
    }

    let side_to_move = match side_str {
        "w" => Color::White,
        "b" => Color::Black,
        _ => {
            return Err(CoreError::InvalidFen(format!(
                "invalid side to move '{}'",
                side_str
            )))
        }
    };

    // Castling rights + consistency against king and rook positions.
    let castling_rights = parse_castling(&board, castling_str)?;

    // En passant square + plausibility validation.
    let en_passant = parse_en_passant(&board, side_to_move, ep_str)?;

    let halfmove_clock: u16 = halfmove_str
        .parse()
        .map_err(|_| CoreError::InvalidFen(format!("invalid halfmove clock '{}'", halfmove_str)))?;
    let fullmove_number: u16 = fullmove_str.parse().map_err(|_| {
        CoreError::InvalidFen(format!("invalid fullmove number '{}'", fullmove_str))
    })?;
    if fullmove_number == 0 {
        return Err(CoreError::InvalidFen(
            "fullmove number must be >= 1".to_string(),
        ));
    }

    board.set_state_raw(
        side_to_move,
        castling_rights,
        en_passant,
        halfmove_clock,
        fullmove_number,
    );
    board.finalize_setup();

    Ok(board)
}

fn parse_castling(board: &Board, s: &str) -> Result<CastlingRights, CoreError> {
    if s == "-" {
        return Ok(CastlingRights::NONE);
    }
    if s.is_empty() {
        return Err(CoreError::InvalidFen("empty castling field".to_string()));
    }

    let mut mask = 0u8;
    for ch in s.chars() {
        let flag = match ch {
            'K' => CastlingRights::WHITE_KING,
            'Q' => CastlingRights::WHITE_QUEEN,
            'k' => CastlingRights::BLACK_KING,
            'q' => CastlingRights::BLACK_QUEEN,
            _ => {
                return Err(CoreError::InvalidFen(format!(
                    "invalid castling character '{}'",
                    ch
                )))
            }
        };
        if (mask & flag) != 0 {
            return Err(CoreError::InvalidFen(format!(
                "duplicate castling flag '{}'",
                ch
            )));
        }
        mask |= flag;
    }

    let cr = CastlingRights(mask);
    if cr.has(CastlingRights::WHITE_KING)
        && (board.piece_on(Square::E1) != Some((Color::White, PieceType::King))
            || board.piece_on(Square::H1) != Some((Color::White, PieceType::Rook)))
    {
        return Err(CoreError::InvalidFen(
            "white kingside castling requires K on e1 and R on h1".to_string(),
        ));
    }
    if cr.has(CastlingRights::WHITE_QUEEN)
        && (board.piece_on(Square::E1) != Some((Color::White, PieceType::King))
            || board.piece_on(Square::A1) != Some((Color::White, PieceType::Rook)))
    {
        return Err(CoreError::InvalidFen(
            "white queenside castling requires K on e1 and R on a1".to_string(),
        ));
    }
    if cr.has(CastlingRights::BLACK_KING)
        && (board.piece_on(Square::E8) != Some((Color::Black, PieceType::King))
            || board.piece_on(Square::H8) != Some((Color::Black, PieceType::Rook)))
    {
        return Err(CoreError::InvalidFen(
            "black kingside castling requires k on e8 and r on h8".to_string(),
        ));
    }
    if cr.has(CastlingRights::BLACK_QUEEN)
        && (board.piece_on(Square::E8) != Some((Color::Black, PieceType::King))
            || board.piece_on(Square::A8) != Some((Color::Black, PieceType::Rook)))
    {
        return Err(CoreError::InvalidFen(
            "black queenside castling requires k on e8 and r on a8".to_string(),
        ));
    }

    Ok(cr)
}

fn parse_en_passant(
    board: &Board,
    side_to_move: Color,
    s: &str,
) -> Result<Option<Square>, CoreError> {
    if s == "-" {
        return Ok(None);
    }

    let ep_sq: Square = s
        .parse()
        .map_err(|_| CoreError::InvalidFen(format!("invalid en passant square '{}'", s)))?;

    let (expected_rank, pushed_pawn_sq, origin_sq, pushed_color) = match side_to_move {
        Color::White => (
            5u8, // rank 6 (0-based 5)
            Square::from_coords(ep_sq.file(), 4),
            Square::from_coords(ep_sq.file(), 6),
            Color::Black,
        ),
        Color::Black => (
            2u8, // rank 3 (0-based 2)
            Square::from_coords(ep_sq.file(), 3),
            Square::from_coords(ep_sq.file(), 1),
            Color::White,
        ),
    };

    if ep_sq.rank() != expected_rank {
        return Err(CoreError::InvalidFen(format!(
            "en passant square {} has invalid rank for side {}",
            ep_sq, side_to_move
        )));
    }
    if board.piece_on(ep_sq).is_some() || board.piece_on(origin_sq).is_some() {
        return Err(CoreError::InvalidFen(format!(
            "en passant path through {} is occupied",
            ep_sq
        )));
    }
    if board.piece_on(pushed_pawn_sq) != Some((pushed_color, PieceType::Pawn)) {
        return Err(CoreError::InvalidFen(format!(
            "en passant square {} lacks double-pushed pawn on {}",
            ep_sq, pushed_pawn_sq
        )));
    }

    Ok(Some(ep_sq))
}

/// Serializes a [`Board`] into a 6-field FEN string.
pub fn export_fen(board: &Board) -> String {
    let mut out = String::with_capacity(92);

    for rank in (0..8u8).rev() {
        let mut empty = 0u8;
        for file in 0..8u8 {
            let sq = Square::from_coords(file, rank);
            match board.piece_on(sq) {
                Some((color, pt)) => {
                    if empty > 0 {
                        out.push((b'0' + empty) as char);
                        empty = 0;
                    }
                    out.push(pt.to_char(color));
                }
                None => empty += 1,
            }
        }
        if empty > 0 {
            out.push((b'0' + empty) as char);
        }
        if rank > 0 {
            out.push('/');
        }
    }

    out.push(' ');
    out.push_str(match board.side_to_move() {
        Color::White => "w",
        Color::Black => "b",
    });
    out.push(' ');
    out.push_str(&board.castling_rights().to_string());
    out.push(' ');
    match board.en_passant() {
        Some(ep) => out.push_str(&ep.to_string()),
        None => out.push('-'),
    }
    out.push_str(&format!(
        " {} {}",
        board.halfmove_clock(),
        board.fullmove_number()
    ));
    out
}

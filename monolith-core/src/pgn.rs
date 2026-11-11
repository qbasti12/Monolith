//! SAN parser/generator, PGN reader, and move quality classification.

use crate::board::Board;
use crate::movegen::generate_legal;
use crate::types::{Color, CoreError, Move, MoveList, PieceType, Square};

/// Converts a legal [`Move`] on `board` into Standard Algebraic Notation (SAN).
pub fn move_to_san(board: &Board, mv: Move) -> Result<String, CoreError> {
    let mut clone = board.clone();
    move_to_san_mut(&mut clone, mv)
}

/// Converts a legal [`Move`] on `board` into SAN without cloning the board.
pub fn move_to_san_mut(board: &mut Board, mv: Move) -> Result<String, CoreError> {
    let mut legal = MoveList::new();
    generate_legal(board, &mut legal);
    if !legal.contains(mv) {
        return Err(CoreError::IllegalMove(mv.to_uci()));
    }

    let mut out = if mv.is_king_castle() {
        "O-O".to_string()
    } else if mv.is_queen_castle() {
        "O-O-O".to_string()
    } else {
        let from = mv.from_sq();
        let to = mv.to_sq();
        let stm = board.side_to_move();
        let (_, moving_pt) = board
            .piece_on(from)
            .ok_or_else(|| CoreError::IllegalMove(mv.to_uci()))?;

        let mut san = String::with_capacity(8);

        if let Some(letter) = moving_pt.san_letter() {
            san.push(letter);

            // Disambiguate if another legal move of the same piece type reaches `to`.
            let mut same_dest = false;
            let mut same_file = false;
            let mut same_rank = false;

            for &other in legal.iter() {
                if other == mv || other.to_sq() != to {
                    continue;
                }
                if board.piece_on(other.from_sq()) == Some((stm, moving_pt)) {
                    same_dest = true;
                    if other.from_sq().file() == from.file() {
                        same_file = true;
                    }
                    if other.from_sq().rank() == from.rank() {
                        same_rank = true;
                    }
                }
            }

            if same_dest {
                if !same_file {
                    san.push(from.file_char());
                } else if !same_rank {
                    san.push(from.rank_char());
                } else {
                    san.push(from.file_char());
                    san.push(from.rank_char());
                }
            }

            if mv.is_capture() {
                san.push('x');
            }
        } else if mv.is_capture() {
            san.push(from.file_char());
            san.push('x');
        }

        san.push_str(&to.to_string());

        if let Some(promo) = mv.promotion_piece() {
            san.push('=');
            if let Some(letter) = promo.san_letter() {
                san.push(letter);
            }
        }
        san
    };

    board.make_move(mv);
    if board.in_check() {
        let mut replies = MoveList::new();
        generate_legal(board, &mut replies);
        if replies.is_empty() {
            out.push('#');
        } else {
            out.push('+');
        }
    }
    board.unmake_move();

    Ok(out)
}

/// Parses a SAN string (e.g. `"Nf3"`, `"exd5"`, `"O-O"`, `"e8=Q+"`) into a legal [`Move`].
pub fn parse_san(board: &Board, san: &str) -> Result<Move, CoreError> {
    let raw = san.trim();
    if raw.is_empty() {
        return Err(CoreError::InvalidSan {
            san: san.to_string(),
            reason: "empty move token".to_string(),
        });
    }

    // Strip optional trailing move quality glyphs (`!`, `?`) and single check/mate marker (`+`, `#`).
    let mut s = raw.trim_end_matches(['!', '?']);
    if let Some(stripped) = s.strip_suffix('+').or_else(|| s.strip_suffix('#')) {
        s = stripped;
    }

    let mut legal = MoveList::new();
    generate_legal(board, &mut legal);

    if s == "O-O" || s == "0-0" {
        return legal
            .iter()
            .copied()
            .find(|m| m.is_king_castle())
            .ok_or_else(|| CoreError::InvalidSan {
                san: raw.to_string(),
                reason: "kingside castling is not legal".to_string(),
            });
    }
    if s == "O-O-O" || s == "0-0-0" {
        return legal
            .iter()
            .copied()
            .find(|m| m.is_queen_castle())
            .ok_or_else(|| CoreError::InvalidSan {
                san: raw.to_string(),
                reason: "queenside castling is not legal".to_string(),
            });
    }

    let mut work = s;
    let mut promo: Option<PieceType> = None;

    // Parse promotion suffix (`=Q` or `Q` at the end).
    if let Some(eq_idx) = work.rfind('=') {
        let promo_str = &work[eq_idx + 1..];
        promo = Some(parse_promo_str(promo_str, raw)?);
        work = &work[..eq_idx];
    } else if work.len() >= 3 {
        let last = work.as_bytes()[work.len() - 1] as char;
        if matches!(last, 'N' | 'B' | 'R' | 'Q' | 'n' | 'b' | 'r' | 'q') {
            promo = Some(parse_promo_str(&last.to_string(), raw)?);
            work = &work[..work.len() - 1];
        }
    }

    let mut chars = work.chars();
    let first = chars.next().ok_or_else(|| CoreError::InvalidSan {
        san: raw.to_string(),
        reason: "missing move body".to_string(),
    })?;

    let (piece_type, rest) = match first {
        'N' => (PieceType::Knight, &work[1..]),
        'B' => (PieceType::Bishop, &work[1..]),
        'R' => (PieceType::Rook, &work[1..]),
        'Q' => (PieceType::Queen, &work[1..]),
        'K' => (PieceType::King, &work[1..]),
        _ => (PieceType::Pawn, work),
    };

    if rest.len() < 2 {
        return Err(CoreError::InvalidSan {
            san: raw.to_string(),
            reason: "missing destination square".to_string(),
        });
    }

    let dest_str = &rest[rest.len() - 2..];
    let to_sq: Square = dest_str.parse().map_err(|_| CoreError::InvalidSan {
        san: raw.to_string(),
        reason: format!("invalid destination square '{}'", dest_str),
    })?;

    let mut prefix = &rest[..rest.len() - 2];
    let is_capture = if let Some(stripped) = prefix.strip_suffix('x') {
        prefix = stripped;
        true
    } else {
        false
    };

    let (from_file, from_rank) = parse_disambiguation(prefix, raw)?;

    let stm = board.side_to_move();
    let mut matched: Option<Move> = None;

    for &mv in legal.iter() {
        if mv.to_sq() != to_sq {
            continue;
        }
        if mv.is_capture() != is_capture {
            continue;
        }
        if mv.promotion_piece() != promo {
            continue;
        }
        if board.piece_on(mv.from_sq()) != Some((stm, piece_type)) {
            continue;
        }
        if let Some(f) = from_file {
            if mv.from_sq().file() != f {
                continue;
            }
        }
        if let Some(r) = from_rank {
            if mv.from_sq().rank() != r {
                continue;
            }
        }
        if matched.is_some() {
            return Err(CoreError::InvalidSan {
                san: raw.to_string(),
                reason: "ambiguous SAN move".to_string(),
            });
        }
        matched = Some(mv);
    }

    matched.ok_or_else(|| CoreError::InvalidSan {
        san: raw.to_string(),
        reason: "no matching legal move".to_string(),
    })
}

fn parse_promo_str(s: &str, raw: &str) -> Result<PieceType, CoreError> {
    match s {
        "N" | "n" => Ok(PieceType::Knight),
        "B" | "b" => Ok(PieceType::Bishop),
        "R" | "r" => Ok(PieceType::Rook),
        "Q" | "q" => Ok(PieceType::Queen),
        _ => Err(CoreError::InvalidSan {
            san: raw.to_string(),
            reason: format!("invalid promotion piece '{}'", s),
        }),
    }
}

fn parse_disambiguation(prefix: &str, raw: &str) -> Result<(Option<u8>, Option<u8>), CoreError> {
    let bytes = prefix.as_bytes();
    match bytes.len() {
        0 => Ok((None, None)),
        1 => {
            let b = bytes[0];
            if (b'a'..=b'h').contains(&b) {
                Ok((Some(b - b'a'), None))
            } else if (b'1'..=b'8').contains(&b) {
                Ok((None, Some(b - b'1')))
            } else {
                Err(CoreError::InvalidSan {
                    san: raw.to_string(),
                    reason: format!("invalid disambiguation '{}'", prefix),
                })
            }
        }
        2 => {
            let f = bytes[0];
            let r = bytes[1];
            if (b'a'..=b'h').contains(&f) && (b'1'..=b'8').contains(&r) {
                Ok((Some(f - b'a'), Some(r - b'1')))
            } else {
                Err(CoreError::InvalidSan {
                    san: raw.to_string(),
                    reason: format!("invalid disambiguation '{}'", prefix),
                })
            }
        }
        _ => Err(CoreError::InvalidSan {
            san: raw.to_string(),
            reason: format!("unexpected prefix '{}'", prefix),
        }),
    }
}

/// One ply in a parsed PGN game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgnMove {
    pub ply: usize,
    pub move_number: u16,
    pub side: Color,
    pub mv: Move,
    pub san: String,
    pub fen_before: String,
    pub fen_after: String,
    pub comment: Option<String>,
}

/// Parsed first game from a PGN document.
#[derive(Debug, Clone)]
pub struct PgnGame {
    pub headers: Vec<(String, String)>,
    pub initial_fen: String,
    pub moves: Vec<PgnMove>,
    pub result: Option<String>,
}

impl PgnGame {
    pub fn header(&self, key: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }
}

/// Parses the first chess game from a PGN string.
///
/// Supports header tags (including `[FEN]` / `[SetUp]`), `{}` and `;` comments,
/// nested `()` variations (skipped), `$N` NAGs, and game result tokens.
pub fn parse_pgn(input: &str) -> Result<PgnGame, CoreError> {
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut board: Option<Board> = None;
    let mut initial_fen = crate::fen::STARTPOS_FEN.to_string();
    let mut moves: Vec<PgnMove> = Vec::new();
    let mut result: Option<String> = None;
    let mut started_moves = false;

    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;

    while i < len {
        // Skip whitespace.
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= len {
            break;
        }

        match bytes[i] {
            b'[' => {
                if started_moves {
                    // Start of a subsequent game in multi-game PGN.
                    break;
                }
                i += 1;
                let end = find_header_end(bytes, i).ok_or(CoreError::InvalidPgn {
                    move_number: 1,
                    reason: "unterminated PGN header tag".to_string(),
                })?;
                let tag_body = &input[i..end];
                i = end + 1;
                if let Some((k, v)) = parse_header_tag(tag_body) {
                    if k.eq_ignore_ascii_case("FEN") {
                        let parsed = Board::from_fen(&v).map_err(|e| CoreError::InvalidPgn {
                            move_number: 1,
                            reason: format!("invalid [FEN] header: {}", e),
                        })?;
                        initial_fen = v.clone();
                        board = Some(parsed);
                    }
                    headers.push((k, v));
                } else {
                    return Err(CoreError::InvalidPgn {
                        move_number: 1,
                        reason: format!("malformed header tag '[{}]'", tag_body),
                    });
                }
            }
            b'{' => {
                i += 1;
                let start = i;
                while i < len && bytes[i] != b'}' {
                    i += 1;
                }
                if i >= len {
                    return Err(CoreError::InvalidPgn {
                        move_number: current_move_number(board.as_ref()),
                        reason: "unterminated '{' comment".to_string(),
                    });
                }
                let comment_text = input[start..i].trim().to_string();
                if !comment_text.is_empty() {
                    if let Some(last) = moves.last_mut() {
                        match &mut last.comment {
                            Some(existing) => {
                                existing.push(' ');
                                existing.push_str(&comment_text);
                            }
                            None => last.comment = Some(comment_text),
                        }
                    }
                }
                i += 1;
            }
            b';' => {
                i += 1;
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'(' => {
                // Skip nested variations completely.
                let mut depth = 1usize;
                i += 1;
                while i < len && depth > 0 {
                    match bytes[i] {
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        b'{' => {
                            i += 1;
                            while i < len && bytes[i] != b'}' {
                                i += 1;
                            }
                        }
                        b';' => {
                            while i < len && bytes[i] != b'\n' {
                                i += 1;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                if depth > 0 {
                    return Err(CoreError::InvalidPgn {
                        move_number: current_move_number(board.as_ref()),
                        reason: "unterminated '(' variation".to_string(),
                    });
                }
            }
            b'%' => {
                // PGN escape line.
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            _ => {
                let start = i;
                while i < len
                    && !bytes[i].is_ascii_whitespace()
                    && !matches!(bytes[i], b'{' | b'}' | b'(' | b')' | b';' | b'[')
                {
                    i += 1;
                }
                let token = &input[start..i];
                if token.is_empty() {
                    i += 1;
                    continue;
                }

                if is_result_token(token) {
                    result = Some(token.to_string());
                    break;
                }
                if token.starts_with('$') || is_move_number_token(token) {
                    continue;
                }

                // Handle glued move number + SAN like `1.e4` or `12...Nf6`.
                let san_token = strip_leading_move_number(token);
                if san_token.is_empty() {
                    continue;
                }

                started_moves = true;
                let active_board = board.get_or_insert_with(Board::startpos);
                let move_number = active_board.fullmove_number();
                let side = active_board.side_to_move();
                let fen_before = active_board.to_fen();

                let mv = parse_san(active_board, san_token).map_err(|e| CoreError::InvalidPgn {
                    move_number: move_number as u32,
                    reason: format!("invalid move '{}': {}", san_token, e),
                })?;

                let canonical_san =
                    move_to_san_mut(active_board, mv).map_err(|e| CoreError::InvalidPgn {
                        move_number: move_number as u32,
                        reason: e.to_string(),
                    })?;

                active_board.make_move(mv);
                let fen_after = active_board.to_fen();
                moves.push(PgnMove {
                    ply: moves.len() + 1,
                    move_number,
                    side,
                    mv,
                    san: canonical_san,
                    fen_before,
                    fen_after,
                    comment: None,
                });
            }
        }
    }

    if headers.is_empty() && moves.is_empty() && result.is_none() {
        return Err(CoreError::EmptyPgn);
    }

    Ok(PgnGame {
        headers,
        initial_fen,
        moves,
        result,
    })
}

fn current_move_number(board: Option<&Board>) -> u32 {
    board.map(|b| b.fullmove_number() as u32).unwrap_or(1)
}

fn find_header_end(bytes: &[u8], mut i: usize) -> Option<usize> {
    let mut in_quotes = false;
    let mut escaped = false;
    while i < bytes.len() {
        let b = bytes[i];
        if escaped {
            escaped = false;
        } else if in_quotes {
            if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_quotes = false;
            }
        } else if b == b'"' {
            in_quotes = true;
        } else if b == b']' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn parse_header_tag(body: &str) -> Option<(String, String)> {
    let trimmed = body.trim();
    let quote_start = trimmed.find('"')?;
    let key = trimmed[..quote_start].trim();
    if key.is_empty() || !trimmed.ends_with('"') {
        return None;
    }
    let val_raw = &trimmed[quote_start + 1..trimmed.len() - 1];
    let value = val_raw.replace("\\\"", "\"").replace("\\\\", "\\");
    Some((key.to_string(), value))
}

fn is_result_token(token: &str) -> bool {
    matches!(token, "1-0" | "0-1" | "1/2-1/2" | "*")
}

fn is_move_number_token(token: &str) -> bool {
    let stripped = token.trim_end_matches('.');
    !stripped.is_empty()
        && stripped.len() < token.len()
        && stripped.bytes().all(|b| b.is_ascii_digit())
}

fn strip_leading_move_number(token: &str) -> &str {
    let mut idx = 0usize;
    let bytes = token.as_bytes();
    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }
    if idx > 0 && idx < bytes.len() && bytes[idx] == b'.' {
        while idx < bytes.len() && bytes[idx] == b'.' {
            idx += 1;
        }
        &token[idx..]
    } else {
        token
    }
}

/// Move quality categories with documented centipawn thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveClass {
    /// Net material sacrifice >= 200 cp, top-2 move, eval remains >= -50 cp (and not > +600 beforehand).
    Brilliant,
    /// Top/near-top move holding >= 150 cp lead over the second-best candidate.
    Great,
    /// Engine top move or centipawn loss <= 10 cp.
    Best,
    /// Centipawn loss <= 50 cp.
    Good,
    /// Centipawn loss > 50 cp and <= 150 cp.
    Inaccuracy,
    /// Centipawn loss > 150 cp and <= 300 cp.
    Mistake,
    /// Centipawn loss > 300 cp.
    Blunder,
}

impl MoveClass {
    pub const fn symbol(self) -> &'static str {
        match self {
            MoveClass::Brilliant => "!!",
            MoveClass::Great => "!",
            MoveClass::Best => "Best",
            MoveClass::Good => "Good",
            MoveClass::Inaccuracy => "?!",
            MoveClass::Mistake => "?",
            MoveClass::Blunder => "??",
        }
    }
}

/// Input metrics (in centipawns from the moving player's perspective) for [`classify_move`].
#[derive(Debug, Clone)]
pub struct ClassifyInput {
    pub eval_before_best_cp: i32,
    pub eval_after_played_cp: i32,
    pub second_best_cp: Option<i32>,
    pub is_engine_top_move: bool,
    pub is_top_two_move: bool,
    /// Net material change caused by the move from the mover's perspective (negative = sacrificed).
    pub material_delta_cp: i32,
    pub best_san: Option<String>,
}

/// Result of move classification containing the category, cp loss, and explanatory text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveClassification {
    pub class: MoveClass,
    pub cp_loss: i32,
    pub explanation: String,
}

/// Threshold constants (centipawns) used by [`classify_move`].
pub const THRESHOLD_BEST_CP: i32 = 10;
pub const THRESHOLD_GOOD_CP: i32 = 50;
pub const THRESHOLD_INACCURACY_CP: i32 = 150;
pub const THRESHOLD_MISTAKE_CP: i32 = 300;
pub const THRESHOLD_GREAT_GAP_CP: i32 = 150;
pub const THRESHOLD_BRILLIANT_SAC_CP: i32 = -200;
pub const THRESHOLD_BRILLIANT_MAX_PREV_EVAL_CP: i32 = 600;
pub const THRESHOLD_BRILLIANT_MIN_AFTER_CP: i32 = -50;

/// Classifies a played move using engine Multi-PV evaluations and net material delta.
pub fn classify_move(input: &ClassifyInput) -> MoveClassification {
    let cp_loss = (input.eval_before_best_cp - input.eval_after_played_cp).max(0);
    let best_hint = input
        .best_san
        .as_deref()
        .map(|s| format!(" (besser: {})", s))
        .unwrap_or_default();
    let delta_pawns = -(cp_loss as f64) / 100.0;

    // Brilliant: genuine sacrifice (>= 2 pawns net), top-2 move, position stays sound (>= -50 cp
    // or no worse than before), and position was not already trivially won (> +6.00).
    let is_sacrifice = input.material_delta_cp <= THRESHOLD_BRILLIANT_SAC_CP;
    let stays_sound = input.eval_after_played_cp >= THRESHOLD_BRILLIANT_MIN_AFTER_CP
        || input.eval_after_played_cp >= input.eval_before_best_cp - THRESHOLD_BEST_CP;
    let not_already_won = input.eval_before_best_cp <= THRESHOLD_BRILLIANT_MAX_PREV_EVAL_CP;
    if is_sacrifice && (input.is_top_two_move || cp_loss <= 30) && stays_sound && not_already_won {
        return MoveClassification {
            class: MoveClass::Brilliant,
            cp_loss,
            explanation: format!(
                "Brillantes Opfer ({:+.2} Bauern Material, Bewertung gehalten)",
                (input.material_delta_cp as f64) / 100.0
            ),
        };
    }

    // Great: only good move holding a clear > 150 cp lead over the second-best alternative.
    if (input.is_engine_top_move || cp_loss <= THRESHOLD_BEST_CP)
        && input
            .second_best_cp
            .is_some_and(|second| input.eval_before_best_cp - second > THRESHOLD_GREAT_GAP_CP)
    {
        let gap = input
            .second_best_cp
            .map(|s| (input.eval_before_best_cp - s) as f64 / 100.0)
            .unwrap_or(0.0);
        return MoveClassification {
            class: MoveClass::Great,
            cp_loss,
            explanation: format!("Starker einziger Zug (+{:.2} vor Alternative)", gap),
        };
    }

    if input.is_engine_top_move || cp_loss <= THRESHOLD_BEST_CP {
        MoveClassification {
            class: MoveClass::Best,
            cp_loss,
            explanation: "Stärkster Zug".to_string(),
        }
    } else if cp_loss <= THRESHOLD_GOOD_CP {
        MoveClassification {
            class: MoveClass::Good,
            cp_loss,
            explanation: format!("Solider Zug ({:.2})", delta_pawns),
        }
    } else if cp_loss <= THRESHOLD_INACCURACY_CP {
        MoveClassification {
            class: MoveClass::Inaccuracy,
            cp_loss,
            explanation: format!("Ungenauigkeit ({:.2}){}", delta_pawns, best_hint),
        }
    } else if cp_loss <= THRESHOLD_MISTAKE_CP {
        MoveClassification {
            class: MoveClass::Mistake,
            cp_loss,
            explanation: format!("Fehler ({:.2}){}", delta_pawns, best_hint),
        }
    } else if cp_loss >= 10_000 {
        MoveClassification {
            class: MoveClass::Blunder,
            cp_loss,
            explanation: format!("Grober Patzer: verliert/verpasst Matt{}", best_hint),
        }
    } else {
        MoveClassification {
            class: MoveClass::Blunder,
            cp_loss,
            explanation: format!(
                "Grober Patzer: verliert Vorteil ({:.2}){}",
                delta_pawns, best_hint
            ),
        }
    }
}

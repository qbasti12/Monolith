//! Chess board state, incremental Zobrist hashing, make/unmake, and draw detection.

use crate::bitboard::{self, DARK_SQUARES, LIGHT_SQUARES};
use crate::types::{CastlingRights, Color, Move, PieceType, Square};
use crate::zobrist;

#[rustfmt::skip]
const CASTLING_MASK: [u8; 64] = [
    13, 15, 15, 15, 12, 15, 15, 14, // rank 1: a1=WQ(2)->13, e1=WK|WQ(3)->12, h1=WK(1)->14
    15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15,
    15, 15, 15, 15, 15, 15, 15, 15,
     7, 15, 15, 15,  3, 15, 15, 11, // rank 8: a8=BQ(8)->7, e8=BK|BQ(12)->3, h8=BK(4)->11
];

/// Snapshot saved on `Board.history` for zero-allocation `unmake_move`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UndoState {
    pub mv: Move,
    pub captured: Option<PieceType>,
    pub castling_rights: CastlingRights,
    pub en_passant: Option<Square>,
    pub halfmove_clock: u16,
    pub zobrist_key: u64,
}

/// Complete chess board representation with unlimited piece counts via bitboards.
#[derive(Debug)]
pub struct Board {
    pieces: [[u64; 6]; 2],
    occupancies: [u64; 3],
    mailbox: [Option<(Color, PieceType)>; 64],
    side_to_move: Color,
    castling_rights: CastlingRights,
    en_passant: Option<Square>,
    halfmove_clock: u16,
    fullmove_number: u16,
    zobrist_key: u64,
    history: Vec<UndoState>,
}

impl Clone for Board {
    fn clone(&self) -> Self {
        let cap = (self.history.len() + 256).max(Self::HISTORY_PREALLOC);
        let mut history = Vec::with_capacity(cap);
        history.extend_from_slice(&self.history);
        Self {
            pieces: self.pieces,
            occupancies: self.occupancies,
            mailbox: self.mailbox,
            side_to_move: self.side_to_move,
            castling_rights: self.castling_rights,
            en_passant: self.en_passant,
            halfmove_clock: self.halfmove_clock,
            fullmove_number: self.fullmove_number,
            zobrist_key: self.zobrist_key,
            history,
        }
    }
}

impl Board {
    pub const HISTORY_PREALLOC: usize = 2048;

    /// Creates an empty board with preallocated history and initialized lookup tables.
    pub fn empty() -> Self {
        bitboard::init();
        zobrist::init();
        Self {
            pieces: [[0u64; 6]; 2],
            occupancies: [0u64; 3],
            mailbox: [None; 64],
            side_to_move: Color::White,
            castling_rights: CastlingRights::NONE,
            en_passant: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            zobrist_key: 0,
            history: Vec::with_capacity(Self::HISTORY_PREALLOC),
        }
    }

    /// Standard chess starting position.
    pub fn startpos() -> Self {
        Self::from_fen(crate::fen::STARTPOS_FEN).unwrap_or_else(|_| Self::empty())
    }

    /// Recomputes occupancies and Zobrist hash from `pieces`, side, castling, and EP.
    pub(crate) fn finalize_setup(&mut self) {
        self.occupancies = [0u64; 3];
        for color in Color::ALL {
            let mut color_occ = 0u64;
            for pt in PieceType::ALL {
                color_occ |= self.pieces[color.index()][pt.index()];
            }
            self.occupancies[color.index()] = color_occ;
        }
        self.occupancies[2] = self.occupancies[0] | self.occupancies[1];
        self.zobrist_key = zobrist::compute_key(self);
    }

    pub(crate) fn set_piece_raw(&mut self, color: Color, pt: PieceType, sq: Square) {
        let bit = sq.bit();
        self.pieces[color.index()][pt.index()] |= bit;
        self.mailbox[sq.index()] = Some((color, pt));
    }

    pub(crate) fn set_state_raw(
        &mut self,
        side_to_move: Color,
        castling_rights: CastlingRights,
        en_passant: Option<Square>,
        halfmove_clock: u16,
        fullmove_number: u16,
    ) {
        self.side_to_move = side_to_move;
        self.castling_rights = castling_rights;
        self.en_passant = en_passant;
        self.halfmove_clock = halfmove_clock;
        self.fullmove_number = fullmove_number;
    }

    #[inline]
    pub fn piece_bb(&self, color: Color, pt: PieceType) -> u64 {
        self.pieces[color.index()][pt.index()]
    }

    #[inline]
    pub fn color_occupancy(&self, color: Color) -> u64 {
        self.occupancies[color.index()]
    }

    #[inline]
    pub fn all_occupancy(&self) -> u64 {
        self.occupancies[2]
    }

    #[inline]
    pub fn piece_on(&self, sq: Square) -> Option<(Color, PieceType)> {
        self.mailbox[sq.index()]
    }

    #[inline]
    pub fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    #[inline]
    pub fn castling_rights(&self) -> CastlingRights {
        self.castling_rights
    }

    #[inline]
    pub fn en_passant(&self) -> Option<Square> {
        self.en_passant
    }

    #[inline]
    pub fn halfmove_clock(&self) -> u16 {
        self.halfmove_clock
    }

    #[inline]
    pub fn fullmove_number(&self) -> u16 {
        self.fullmove_number
    }

    #[inline]
    pub fn zobrist_key(&self) -> u64 {
        self.zobrist_key
    }

    #[inline]
    pub fn history(&self) -> &[UndoState] {
        &self.history
    }

    #[inline]
    pub fn history_spare_capacity(&self) -> usize {
        self.history.capacity().saturating_sub(self.history.len())
    }

    #[inline]
    pub fn king_square(&self, color: Color) -> Square {
        let bb = self.piece_bb(color, PieceType::King);
        debug_assert!(bb != 0, "missing king for {:?}", color);
        Square((bb.trailing_zeros() as u8) & 63)
    }

    #[inline]
    fn put_piece(&mut self, color: Color, pt: PieceType, sq: Square) {
        let bit = sq.bit();
        self.pieces[color.index()][pt.index()] |= bit;
        self.occupancies[color.index()] |= bit;
        self.occupancies[2] |= bit;
        self.mailbox[sq.index()] = Some((color, pt));
        self.zobrist_key ^= zobrist::keys().piece[color.index()][pt.index()][sq.index()];
    }

    #[inline]
    fn remove_piece(&mut self, color: Color, pt: PieceType, sq: Square) {
        let bit = sq.bit();
        self.pieces[color.index()][pt.index()] &= !bit;
        self.occupancies[color.index()] &= !bit;
        self.occupancies[2] &= !bit;
        self.mailbox[sq.index()] = None;
        self.zobrist_key ^= zobrist::keys().piece[color.index()][pt.index()][sq.index()];
    }

    #[inline]
    fn move_piece(&mut self, color: Color, pt: PieceType, from: Square, to: Square) {
        let mask = from.bit() | to.bit();
        self.pieces[color.index()][pt.index()] ^= mask;
        self.occupancies[color.index()] ^= mask;
        self.occupancies[2] ^= mask;
        self.mailbox[from.index()] = None;
        self.mailbox[to.index()] = Some((color, pt));
        let k = &zobrist::keys().piece[color.index()][pt.index()];
        self.zobrist_key ^= k[from.index()] ^ k[to.index()];
    }

    #[inline]
    fn put_piece_no_hash(&mut self, color: Color, pt: PieceType, sq: Square) {
        let bit = sq.bit();
        self.pieces[color.index()][pt.index()] |= bit;
        self.occupancies[color.index()] |= bit;
        self.occupancies[2] |= bit;
        self.mailbox[sq.index()] = Some((color, pt));
    }

    #[inline]
    fn remove_piece_no_hash(&mut self, color: Color, pt: PieceType, sq: Square) {
        let bit = sq.bit();
        self.pieces[color.index()][pt.index()] &= !bit;
        self.occupancies[color.index()] &= !bit;
        self.occupancies[2] &= !bit;
        self.mailbox[sq.index()] = None;
    }

    #[inline]
    fn move_piece_no_hash(&mut self, color: Color, pt: PieceType, from: Square, to: Square) {
        let mask = from.bit() | to.bit();
        self.pieces[color.index()][pt.index()] ^= mask;
        self.occupancies[color.index()] ^= mask;
        self.occupancies[2] ^= mask;
        self.mailbox[from.index()] = None;
        self.mailbox[to.index()] = Some((color, pt));
    }

    /// Applies a legal (or pseudo-legal) move onto the board in O(1) without allocating.
    pub fn make_move(&mut self, mv: Move) {
        let keys = zobrist::keys();
        let stm = self.side_to_move;
        let opp = stm.flip();
        let from = mv.from_sq();
        let to = mv.to_sq();

        let moving_pt = match self.mailbox[from.index()] {
            Some((c, pt)) if c == stm => pt,
            _ => {
                debug_assert!(false, "make_move: missing piece on {}", from);
                return;
            }
        };

        let captured = if mv.is_en_passant() {
            Some(PieceType::Pawn)
        } else if mv.is_capture() {
            self.mailbox[to.index()].map(|(_, pt)| pt)
        } else {
            None
        };

        self.history.push(UndoState {
            mv,
            captured,
            castling_rights: self.castling_rights,
            en_passant: self.en_passant,
            halfmove_clock: self.halfmove_clock,
            zobrist_key: self.zobrist_key,
        });

        if let Some(ep) = self.en_passant.take() {
            self.zobrist_key ^= keys.ep_file[ep.file() as usize];
        }

        if moving_pt == PieceType::Pawn || mv.is_capture() {
            self.halfmove_clock = 0;
        } else {
            self.halfmove_clock = self.halfmove_clock.saturating_add(1);
        }

        if mv.is_en_passant() {
            let cap_sq = match stm {
                Color::White => Square(to.0 - 8),
                Color::Black => Square(to.0 + 8),
            };
            self.remove_piece(opp, PieceType::Pawn, cap_sq);
        } else if let Some(cap_pt) = captured {
            self.remove_piece(opp, cap_pt, to);
        }

        if let Some(promo_pt) = mv.promotion_piece() {
            self.remove_piece(stm, PieceType::Pawn, from);
            self.put_piece(stm, promo_pt, to);
        } else {
            self.move_piece(stm, moving_pt, from, to);
        }

        if mv.is_king_castle() {
            match stm {
                Color::White => {
                    self.move_piece(Color::White, PieceType::Rook, Square::H1, Square::F1)
                }
                Color::Black => {
                    self.move_piece(Color::Black, PieceType::Rook, Square::H8, Square::F8)
                }
            }
        } else if mv.is_queen_castle() {
            match stm {
                Color::White => {
                    self.move_piece(Color::White, PieceType::Rook, Square::A1, Square::D1)
                }
                Color::Black => {
                    self.move_piece(Color::Black, PieceType::Rook, Square::A8, Square::D8)
                }
            }
        }

        if mv.is_double_push() {
            let ep_sq = Square((from.0 + to.0) / 2);
            self.en_passant = Some(ep_sq);
            self.zobrist_key ^= keys.ep_file[ep_sq.file() as usize];
        }

        let old_cr = self.castling_rights.mask();
        self.castling_rights.0 &= CASTLING_MASK[from.index()] & CASTLING_MASK[to.index()];
        let new_cr = self.castling_rights.mask();
        if old_cr != new_cr {
            self.zobrist_key ^= keys.castling[old_cr] ^ keys.castling[new_cr];
        }

        if stm == Color::Black {
            self.fullmove_number = self.fullmove_number.saturating_add(1);
        }
        self.side_to_move = opp;
        self.zobrist_key ^= keys.side;

        debug_assert_eq!(self.zobrist_key, zobrist::compute_key(self));
    }

    /// Reverts the most recent move made with [`Board::make_move`].
    pub fn unmake_move(&mut self) -> Option<Move> {
        let undo = self.history.pop()?;
        let mv = undo.mv;

        self.side_to_move = self.side_to_move.flip();
        let stm = self.side_to_move;
        let opp = stm.flip();

        if stm == Color::Black {
            self.fullmove_number = self.fullmove_number.saturating_sub(1);
        }

        self.castling_rights = undo.castling_rights;
        self.en_passant = undo.en_passant;
        self.halfmove_clock = undo.halfmove_clock;
        self.zobrist_key = undo.zobrist_key;

        let from = mv.from_sq();
        let to = mv.to_sq();

        if mv.is_king_castle() {
            match stm {
                Color::White => {
                    self.move_piece_no_hash(Color::White, PieceType::Rook, Square::F1, Square::H1)
                }
                Color::Black => {
                    self.move_piece_no_hash(Color::Black, PieceType::Rook, Square::F8, Square::H8)
                }
            }
        } else if mv.is_queen_castle() {
            match stm {
                Color::White => {
                    self.move_piece_no_hash(Color::White, PieceType::Rook, Square::D1, Square::A1)
                }
                Color::Black => {
                    self.move_piece_no_hash(Color::Black, PieceType::Rook, Square::D8, Square::A8)
                }
            }
        }

        if let Some(promo_pt) = mv.promotion_piece() {
            self.remove_piece_no_hash(stm, promo_pt, to);
            self.put_piece_no_hash(stm, PieceType::Pawn, from);
        } else if let Some((_, moved_pt)) = self.mailbox[to.index()] {
            self.move_piece_no_hash(stm, moved_pt, to, from);
        }

        if mv.is_en_passant() {
            let cap_sq = match stm {
                Color::White => Square(to.0 - 8),
                Color::Black => Square(to.0 + 8),
            };
            self.put_piece_no_hash(opp, PieceType::Pawn, cap_sq);
        } else if let Some(cap_pt) = undo.captured {
            self.put_piece_no_hash(opp, cap_pt, to);
        }

        debug_assert_eq!(self.zobrist_key, zobrist::compute_key(self));
        Some(mv)
    }

    /// Passes the turn (for Null-Move Pruning). Must be paired with [`Board::unmake_null_move`].
    pub fn make_null_move(&mut self) {
        let keys = zobrist::keys();
        self.history.push(UndoState {
            mv: Move::NONE,
            captured: None,
            castling_rights: self.castling_rights,
            en_passant: self.en_passant,
            halfmove_clock: self.halfmove_clock,
            zobrist_key: self.zobrist_key,
        });

        if let Some(ep) = self.en_passant.take() {
            self.zobrist_key ^= keys.ep_file[ep.file() as usize];
        }
        self.halfmove_clock = 0;
        self.side_to_move = self.side_to_move.flip();
        self.zobrist_key ^= keys.side;
        debug_assert_eq!(self.zobrist_key, zobrist::compute_key(self));
    }

    /// Restores board state after [`Board::make_null_move`].
    pub fn unmake_null_move(&mut self) {
        if let Some(undo) = self.history.pop() {
            self.side_to_move = self.side_to_move.flip();
            self.castling_rights = undo.castling_rights;
            self.en_passant = undo.en_passant;
            self.halfmove_clock = undo.halfmove_clock;
            self.zobrist_key = undo.zobrist_key;
            debug_assert_eq!(self.zobrist_key, zobrist::compute_key(self));
        }
    }

    /// Returns true if `sq` is attacked by any piece of color `by_color`.
    #[inline]
    pub fn is_square_attacked(&self, sq: Square, by_color: Color) -> bool {
        self.is_square_attacked_occ(sq, by_color, self.all_occupancy())
    }

    /// Same as [`Board::is_square_attacked`], but using a custom occupancy bitboard.
    #[inline]
    pub fn is_square_attacked_occ(&self, sq: Square, by_color: Color, occ: u64) -> bool {
        let idx = by_color.index();
        if (bitboard::pawn_attacks(by_color.flip(), sq) & self.pieces[idx][PieceType::Pawn.index()])
            != 0
        {
            return true;
        }
        if (bitboard::knight_attacks(sq) & self.pieces[idx][PieceType::Knight.index()]) != 0 {
            return true;
        }
        if (bitboard::king_attacks(sq) & self.pieces[idx][PieceType::King.index()]) != 0 {
            return true;
        }
        let diag_sliders = self.pieces[idx][PieceType::Bishop.index()]
            | self.pieces[idx][PieceType::Queen.index()];
        if (bitboard::bishop_attacks(sq, occ) & diag_sliders) != 0 {
            return true;
        }
        let ortho_sliders =
            self.pieces[idx][PieceType::Rook.index()] | self.pieces[idx][PieceType::Queen.index()];
        if (bitboard::rook_attacks(sq, occ) & ortho_sliders) != 0 {
            return true;
        }
        false
    }

    /// Returns a bitboard of all pieces (both colors) attacking `sq` under `occ`.
    #[inline]
    pub fn attackers_to(&self, sq: Square, occ: u64) -> u64 {
        let wp =
            bitboard::pawn_attacks(Color::Black, sq) & self.piece_bb(Color::White, PieceType::Pawn);
        let bp =
            bitboard::pawn_attacks(Color::White, sq) & self.piece_bb(Color::Black, PieceType::Pawn);
        let knights = bitboard::knight_attacks(sq)
            & (self.piece_bb(Color::White, PieceType::Knight)
                | self.piece_bb(Color::Black, PieceType::Knight));
        let kings = bitboard::king_attacks(sq)
            & (self.piece_bb(Color::White, PieceType::King)
                | self.piece_bb(Color::Black, PieceType::King));
        let diag = bitboard::bishop_attacks(sq, occ)
            & (self.piece_bb(Color::White, PieceType::Bishop)
                | self.piece_bb(Color::Black, PieceType::Bishop)
                | self.piece_bb(Color::White, PieceType::Queen)
                | self.piece_bb(Color::Black, PieceType::Queen));
        let ortho = bitboard::rook_attacks(sq, occ)
            & (self.piece_bb(Color::White, PieceType::Rook)
                | self.piece_bb(Color::Black, PieceType::Rook)
                | self.piece_bb(Color::White, PieceType::Queen)
                | self.piece_bb(Color::Black, PieceType::Queen));
        (wp | bp | knights | kings | diag | ortho) & occ
    }

    /// Returns true if the side to move is currently in check.
    #[inline]
    pub fn in_check(&self) -> bool {
        self.is_square_attacked(
            self.king_square(self.side_to_move),
            self.side_to_move.flip(),
        )
    }

    /// Returns true if the current position has occurred at least `prior_occurrences` times
    /// earlier since the last irreversible move (`prior_occurrences = 1` for 2-fold search draw,
    /// `2` for 3-fold game draw).
    pub fn repetition_count(&self) -> usize {
        let limit = (self.halfmove_clock as usize).min(self.history.len());
        let len = self.history.len();
        let mut count = 0usize;
        let mut dist = 2usize;
        while dist <= limit {
            let undo = &self.history[len - dist];
            if undo.mv.is_none() {
                break;
            }
            if undo.zobrist_key == self.zobrist_key {
                count += 1;
            }
            dist += 2;
        }
        count
    }

    #[inline]
    pub fn is_repetition(&self) -> bool {
        self.repetition_count() >= 1
    }

    #[inline]
    pub fn is_threefold_repetition(&self) -> bool {
        self.repetition_count() >= 2
    }

    #[inline]
    pub fn is_fifty_move_draw(&self) -> bool {
        self.halfmove_clock >= 100
    }

    /// Detects FIDE dead positions (insufficient material to deliver checkmate).
    pub fn is_insufficient_material(&self) -> bool {
        let majors_or_pawns = self.piece_bb(Color::White, PieceType::Pawn)
            | self.piece_bb(Color::Black, PieceType::Pawn)
            | self.piece_bb(Color::White, PieceType::Rook)
            | self.piece_bb(Color::Black, PieceType::Rook)
            | self.piece_bb(Color::White, PieceType::Queen)
            | self.piece_bb(Color::Black, PieceType::Queen);

        if majors_or_pawns != 0 {
            return false;
        }

        let wn = self.piece_bb(Color::White, PieceType::Knight).count_ones();
        let bn = self.piece_bb(Color::Black, PieceType::Knight).count_ones();
        let wb_bb = self.piece_bb(Color::White, PieceType::Bishop);
        let bb_bb = self.piece_bb(Color::Black, PieceType::Bishop);
        let wb = wb_bb.count_ones();
        let bb = bb_bb.count_ones();
        let total_minors = wn + bn + wb + bb;

        if total_minors <= 1 {
            return true;
        }

        if wn == 0 && bn == 0 {
            let all_bishops = wb_bb | bb_bb;
            let on_light = (all_bishops & LIGHT_SQUARES) != 0;
            let on_dark = (all_bishops & DARK_SQUARES) != 0;
            return !(on_light && on_dark);
        }

        false
    }

    /// Returns true if the position has any non-pawn, non-king pieces for `color`
    /// (used to avoid null-move pruning in pure pawn endgames where zugzwang is common).
    #[inline]
    pub fn has_non_pawn_material(&self, color: Color) -> bool {
        let idx = color.index();
        (self.pieces[idx][PieceType::Knight.index()]
            | self.pieces[idx][PieceType::Bishop.index()]
            | self.pieces[idx][PieceType::Rook.index()]
            | self.pieces[idx][PieceType::Queen.index()])
            != 0
    }
}

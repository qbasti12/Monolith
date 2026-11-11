//! Fundamental chess types: [`Color`], [`PieceType`], [`Square`], [`CastlingRights`],
//! [`Move`], [`MoveList`], and [`CoreError`].

use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Errors returned by `monolith-core` parsing and validation routines.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CoreError {
    #[error("invalid square: {0}")]
    InvalidSquare(String),
    #[error("invalid piece character: {0}")]
    InvalidPiece(char),
    #[error("invalid FEN ({0})")]
    InvalidFen(String),
    #[error("invalid UCI move: {0}")]
    InvalidUciMove(String),
    #[error("illegal move: {0}")]
    IllegalMove(String),
    #[error("invalid SAN move '{san}': {reason}")]
    InvalidSan { san: String, reason: String },
    #[error("invalid PGN at move {move_number}: {reason}")]
    InvalidPgn { move_number: u32, reason: String },
    #[error("PGN contains no moves")]
    EmptyPgn,
}

/// Side to move or piece color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Color {
    White = 0,
    Black = 1,
}

impl Color {
    pub const ALL: [Color; 2] = [Color::White, Color::Black];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn flip(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    #[inline]
    pub const fn from_index(idx: usize) -> Color {
        if idx & 1 == 0 {
            Color::White
        } else {
            Color::Black
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Color::White => "w",
            Color::Black => "b",
        })
    }
}

/// Chess piece type (excluding color).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PieceType {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5,
}

impl PieceType {
    pub const ALL: [PieceType; 6] = [
        PieceType::Pawn,
        PieceType::Knight,
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Queen,
        PieceType::King,
    ];

    pub const PROMOTIONS: [PieceType; 4] = [
        PieceType::Queen,
        PieceType::Rook,
        PieceType::Bishop,
        PieceType::Knight,
    ];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn from_index(idx: usize) -> Option<PieceType> {
        match idx {
            0 => Some(PieceType::Pawn),
            1 => Some(PieceType::Knight),
            2 => Some(PieceType::Bishop),
            3 => Some(PieceType::Rook),
            4 => Some(PieceType::Queen),
            5 => Some(PieceType::King),
            _ => None,
        }
    }

    #[inline]
    pub const fn to_char(self, color: Color) -> char {
        let base = match self {
            PieceType::Pawn => 'P',
            PieceType::Knight => 'N',
            PieceType::Bishop => 'B',
            PieceType::Rook => 'R',
            PieceType::Queen => 'Q',
            PieceType::King => 'K',
        };
        match color {
            Color::White => base,
            Color::Black => base.to_ascii_lowercase(),
        }
    }

    #[inline]
    pub const fn from_char(c: char) -> Option<(Color, PieceType)> {
        let color = if c.is_ascii_uppercase() {
            Color::White
        } else {
            Color::Black
        };
        let piece = match c.to_ascii_uppercase() {
            'P' => PieceType::Pawn,
            'N' => PieceType::Knight,
            'B' => PieceType::Bishop,
            'R' => PieceType::Rook,
            'Q' => PieceType::Queen,
            'K' => PieceType::King,
            _ => return None,
        };
        Some((color, piece))
    }

    #[inline]
    pub const fn promo_char(self) -> Option<char> {
        match self {
            PieceType::Knight => Some('n'),
            PieceType::Bishop => Some('b'),
            PieceType::Rook => Some('r'),
            PieceType::Queen => Some('q'),
            _ => None,
        }
    }

    #[inline]
    pub const fn san_letter(self) -> Option<char> {
        match self {
            PieceType::Knight => Some('N'),
            PieceType::Bishop => Some('B'),
            PieceType::Rook => Some('R'),
            PieceType::Queen => Some('Q'),
            PieceType::King => Some('K'),
            PieceType::Pawn => None,
        }
    }
}

/// Board square using little-endian rank-file mapping (`a1 = 0`, `h1 = 7`, `a8 = 56`, `h8 = 63`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Square(pub u8);

impl Square {
    pub const A1: Square = Square(0);
    pub const B1: Square = Square(1);
    pub const C1: Square = Square(2);
    pub const D1: Square = Square(3);
    pub const E1: Square = Square(4);
    pub const F1: Square = Square(5);
    pub const G1: Square = Square(6);
    pub const H1: Square = Square(7);
    pub const A2: Square = Square(8);
    pub const B2: Square = Square(9);
    pub const C2: Square = Square(10);
    pub const D2: Square = Square(11);
    pub const E2: Square = Square(12);
    pub const F2: Square = Square(13);
    pub const G2: Square = Square(14);
    pub const H2: Square = Square(15);
    pub const A3: Square = Square(16);
    pub const B3: Square = Square(17);
    pub const C3: Square = Square(18);
    pub const D3: Square = Square(19);
    pub const E3: Square = Square(20);
    pub const F3: Square = Square(21);
    pub const G3: Square = Square(22);
    pub const H3: Square = Square(23);
    pub const A4: Square = Square(24);
    pub const B4: Square = Square(25);
    pub const C4: Square = Square(26);
    pub const D4: Square = Square(27);
    pub const E4: Square = Square(28);
    pub const F4: Square = Square(29);
    pub const G4: Square = Square(30);
    pub const H4: Square = Square(31);
    pub const A5: Square = Square(32);
    pub const B5: Square = Square(33);
    pub const C5: Square = Square(34);
    pub const D5: Square = Square(35);
    pub const E5: Square = Square(36);
    pub const F5: Square = Square(37);
    pub const G5: Square = Square(38);
    pub const H5: Square = Square(39);
    pub const A6: Square = Square(40);
    pub const B6: Square = Square(41);
    pub const C6: Square = Square(42);
    pub const D6: Square = Square(43);
    pub const E6: Square = Square(44);
    pub const F6: Square = Square(45);
    pub const G6: Square = Square(46);
    pub const H6: Square = Square(47);
    pub const A7: Square = Square(48);
    pub const B7: Square = Square(49);
    pub const C7: Square = Square(50);
    pub const D7: Square = Square(51);
    pub const E7: Square = Square(52);
    pub const F7: Square = Square(53);
    pub const G7: Square = Square(54);
    pub const H7: Square = Square(55);
    pub const A8: Square = Square(56);
    pub const B8: Square = Square(57);
    pub const C8: Square = Square(58);
    pub const D8: Square = Square(59);
    pub const E8: Square = Square(60);
    pub const F8: Square = Square(61);
    pub const G8: Square = Square(62);
    pub const H8: Square = Square(63);

    #[inline]
    pub const fn from_u8(raw: u8) -> Square {
        debug_assert!(raw < 64);
        Square(raw & 63)
    }

    #[inline]
    pub const fn from_coords(file: u8, rank: u8) -> Square {
        debug_assert!(file < 8 && rank < 8);
        Square((rank << 3) | file)
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn file(self) -> u8 {
        self.0 & 7
    }

    #[inline]
    pub const fn rank(self) -> u8 {
        self.0 >> 3
    }

    #[inline]
    pub const fn bit(self) -> u64 {
        1u64 << self.0
    }

    #[inline]
    pub const fn flip_rank(self) -> Square {
        Square(self.0 ^ 56)
    }

    #[inline]
    pub const fn is_light(self) -> bool {
        (self.file() + self.rank()) % 2 == 1
    }

    #[inline]
    pub fn file_char(self) -> char {
        (b'a' + self.file()) as char
    }

    #[inline]
    pub fn rank_char(self) -> char {
        (b'1' + self.rank()) as char
    }
}

impl FromStr for Square {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = s.as_bytes();
        if bytes.len() != 2 {
            return Err(CoreError::InvalidSquare(s.to_string()));
        }
        let file = bytes[0];
        let rank = bytes[1];
        if !(b'a'..=b'h').contains(&file) || !(b'1'..=b'8').contains(&rank) {
            return Err(CoreError::InvalidSquare(s.to_string()));
        }
        Ok(Square::from_coords(file - b'a', rank - b'1'))
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.file_char(), self.rank_char())
    }
}

/// 4-bit castling availability bitmask (`K=1`, `Q=2`, `k=4`, `q=8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct CastlingRights(pub u8);

impl CastlingRights {
    pub const NONE: CastlingRights = CastlingRights(0);
    pub const WHITE_KING: u8 = 0b0001;
    pub const WHITE_QUEEN: u8 = 0b0010;
    pub const BLACK_KING: u8 = 0b0100;
    pub const BLACK_QUEEN: u8 = 0b1000;
    pub const ALL: CastlingRights = CastlingRights(0b1111);

    #[inline]
    pub const fn has(self, flag: u8) -> bool {
        (self.0 & flag) != 0
    }

    #[inline]
    pub const fn mask(self) -> usize {
        (self.0 & 0x0F) as usize
    }

    #[inline]
    pub const fn remove(&mut self, mask: u8) {
        self.0 &= mask;
    }
}

impl fmt::Display for CastlingRights {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            return f.write_str("-");
        }
        if self.has(Self::WHITE_KING) {
            f.write_str("K")?;
        }
        if self.has(Self::WHITE_QUEEN) {
            f.write_str("Q")?;
        }
        if self.has(Self::BLACK_KING) {
            f.write_str("k")?;
        }
        if self.has(Self::BLACK_QUEEN) {
            f.write_str("q")?;
        }
        Ok(())
    }
}

/// Compact 16-bit move encoding:
///
/// ```text
/// bits  0..=5 : from square (0..63)
/// bits  6..=11: to square   (0..63)
/// bits 12..=15: flags (4 bits)
///   0 (0000): Quiet move
///   1 (0001): Double pawn push
///   2 (0010): King-side castle
///   3 (0011): Queen-side castle
///   4 (0100): Capture
///   5 (0101): En passant capture
///   8 (1000): Knight promotion (quiet)
///   9 (1001): Bishop promotion (quiet)
///  10 (1010): Rook promotion   (quiet)
///  11 (1011): Queen promotion  (quiet)
///  12 (1100): Knight promotion capture
///  13 (1101): Bishop promotion capture
///  14 (1110): Rook promotion capture
///  15 (1111): Queen promotion capture
/// ```
///
/// Notice bit 3 (`0b1000`) marks any promotion, bit 2 (`0b0100`) marks any capture
/// (including en passant and promotion captures), and bits `0..=1` encode the
/// promoted piece type (`0=N`, `1=B`, `2=R`, `3=Q`) when bit 3 is set.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct Move(pub u16);

impl Move {
    pub const NONE: Move = Move(0);

    pub const FLAG_QUIET: u16 = 0b0000;
    pub const FLAG_DOUBLE_PUSH: u16 = 0b0001;
    pub const FLAG_KING_CASTLE: u16 = 0b0010;
    pub const FLAG_QUEEN_CASTLE: u16 = 0b0011;
    pub const FLAG_CAPTURE: u16 = 0b0100;
    pub const FLAG_EN_PASSANT: u16 = 0b0101;
    pub const FLAG_PROMO_BASE: u16 = 0b1000;

    #[inline]
    pub const fn new(from: Square, to: Square, flags: u16) -> Move {
        Move((from.0 as u16) | ((to.0 as u16) << 6) | ((flags & 0x0F) << 12))
    }

    #[inline]
    pub const fn promotion(from: Square, to: Square, promo: PieceType, is_capture: bool) -> Move {
        let promo_bits = match promo {
            PieceType::Knight => 0,
            PieceType::Bishop => 1,
            PieceType::Rook => 2,
            PieceType::Queen => 3,
            _ => 3,
        };
        let cap_bit = if is_capture { 0b0100 } else { 0 };
        Self::new(from, to, Self::FLAG_PROMO_BASE | cap_bit | promo_bits)
    }

    #[inline]
    pub const fn is_none(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn from_sq(self) -> Square {
        Square((self.0 & 0x3F) as u8)
    }

    #[inline]
    pub const fn to_sq(self) -> Square {
        Square(((self.0 >> 6) & 0x3F) as u8)
    }

    #[inline]
    pub const fn flags(self) -> u16 {
        (self.0 >> 12) & 0x0F
    }

    #[inline]
    pub const fn is_capture(self) -> bool {
        (self.flags() & 0b0100) != 0
    }

    #[inline]
    pub const fn is_promotion(self) -> bool {
        (self.flags() & 0b1000) != 0
    }

    #[inline]
    pub const fn is_double_push(self) -> bool {
        self.flags() == Self::FLAG_DOUBLE_PUSH
    }

    #[inline]
    pub const fn is_en_passant(self) -> bool {
        self.flags() == Self::FLAG_EN_PASSANT
    }

    #[inline]
    pub const fn is_castle(self) -> bool {
        let f = self.flags();
        f == Self::FLAG_KING_CASTLE || f == Self::FLAG_QUEEN_CASTLE
    }

    #[inline]
    pub const fn is_king_castle(self) -> bool {
        self.flags() == Self::FLAG_KING_CASTLE
    }

    #[inline]
    pub const fn is_queen_castle(self) -> bool {
        self.flags() == Self::FLAG_QUEEN_CASTLE
    }

    #[inline]
    pub const fn promotion_piece(self) -> Option<PieceType> {
        if !self.is_promotion() {
            return None;
        }
        match self.flags() & 0b0011 {
            0 => Some(PieceType::Knight),
            1 => Some(PieceType::Bishop),
            2 => Some(PieceType::Rook),
            _ => Some(PieceType::Queen),
        }
    }

    pub fn to_uci(self) -> String {
        if self.is_none() {
            return "0000".to_string();
        }
        let mut s = format!("{}{}", self.from_sq(), self.to_sq());
        if let Some(promo) = self.promotion_piece() {
            if let Some(ch) = promo.promo_char() {
                s.push(ch);
            }
        }
        s
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_uci())
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_uci())
    }
}

/// Stack-allocated fixed-capacity list of moves (capacity 512 to accommodate positions
/// with many promoted queens where legal move counts can exceed 256).
#[derive(Clone)]
pub struct MoveList {
    moves: [Move; Self::CAPACITY],
    len: usize,
}

impl MoveList {
    pub const CAPACITY: usize = 512;

    #[inline]
    pub const fn new() -> Self {
        Self {
            moves: [Move::NONE; Self::CAPACITY],
            len: 0,
        }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.len = 0;
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Appends a move with bounds protection (`debug_assert!` in debug builds,
    /// saturating no-op in release builds so neither panics nor UB can occur).
    #[inline]
    pub fn push(&mut self, mv: Move) {
        debug_assert!(
            self.len < Self::CAPACITY,
            "MoveList capacity (512) exceeded"
        );
        if self.len < Self::CAPACITY {
            self.moves[self.len] = mv;
            self.len += 1;
        }
    }

    #[inline]
    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len]
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [Move] {
        &mut self.moves[..self.len]
    }

    #[inline]
    pub fn get(&self, idx: usize) -> Option<Move> {
        self.as_slice().get(idx).copied()
    }

    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, Move> {
        self.as_slice().iter()
    }

    #[inline]
    pub fn swap(&mut self, a: usize, b: usize) {
        self.as_mut_slice().swap(a, b);
    }

    #[inline]
    pub fn contains(&self, mv: Move) -> bool {
        self.as_slice().contains(&mv)
    }
}

impl Default for MoveList {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for MoveList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.as_slice()).finish()
    }
}

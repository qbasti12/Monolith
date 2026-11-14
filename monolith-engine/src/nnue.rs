//! Quantized perspective NNUE evaluation (`(768 -> 256) x 2 -> 1`, SCReLU)
//! and 32-byte `bulletformat`-compatible training record encoding.

use std::fs;
use std::io;
use std::path::Path;

use monolith_core::bitboard::{lsb, pop_lsb};
use monolith_core::types::{Color, PieceType, Square};
use monolith_core::Board;

use crate::eval::MAX_EVAL_CP;

pub const NNUE_INPUTS: usize = 768;
pub const NNUE_HL: usize = 256;
pub const NNUE_QA: i32 = 255;
pub const NNUE_QB: i32 = 64;
pub const NNUE_SCALE: i32 = 400;

const MNUE_MAGIC: &[u8; 8] = b"MNUEv001";

/// Quantized `(768 -> 256) x 2 -> 1` perspective NNUE weights.
#[derive(Clone)]
pub struct NnueNet {
    pub ft_weights: Box<[i16; NNUE_INPUTS * NNUE_HL]>,
    pub ft_biases: Box<[i16; NNUE_HL]>,
    pub out_weights: Box<[i16; 2 * NNUE_HL]>,
    pub out_bias: i32,
}

/// Dual-perspective accumulator (`[White, Black]`) for zero-allocation inference.
#[derive(Clone, Copy)]
pub struct NnueAccumulator {
    pub vals: [[i16; NNUE_HL]; 2],
}

impl Default for NnueAccumulator {
    fn default() -> Self {
        Self {
            vals: [[0; NNUE_HL]; 2],
        }
    }
}

impl NnueNet {
    /// Returns the piece-square feature index (`0..768`) from `perspective`.
    #[inline]
    pub fn feature_index(
        perspective: Color,
        piece_color: Color,
        pt: PieceType,
        sq: Square,
    ) -> usize {
        let same_side = usize::from(piece_color != perspective);
        let mapped_sq = if perspective == Color::White {
            sq.index()
        } else {
            sq.flip_rank().index()
        };
        same_side * 384 + pt.index() * 64 + mapped_sq
    }

    /// Recomputes both perspective accumulators from scratch for `board`.
    pub fn refresh_accumulator(&self, board: &Board, acc: &mut NnueAccumulator) {
        acc.vals[0].copy_from_slice(&*self.ft_biases);
        acc.vals[1].copy_from_slice(&*self.ft_biases);

        for color in [Color::White, Color::Black] {
            for pt in PieceType::ALL {
                let mut bb = board.piece_bb(color, pt);
                while bb != 0 {
                    let sq = pop_lsb(&mut bb);
                    let w_feat = Self::feature_index(Color::White, color, pt, sq);
                    let b_feat = Self::feature_index(Color::Black, color, pt, sq);
                    add_feature(&mut acc.vals[0], &self.ft_weights[w_feat * NNUE_HL..]);
                    add_feature(&mut acc.vals[1], &self.ft_weights[b_feat * NNUE_HL..]);
                }
            }
        }
    }

    /// Evaluates `board` from scratch using quantized SCReLU (`0` heap allocations).
    #[inline]
    pub fn evaluate_board(&self, board: &Board) -> i32 {
        let mut acc = NnueAccumulator::default();
        self.refresh_accumulator(board, &mut acc);
        self.evaluate_accumulator(&acc, board.side_to_move())
    }

    /// Evaluates precomputed perspective accumulators for `stm` (side-to-move relative cp).
    #[inline]
    pub fn evaluate_accumulator(&self, acc: &NnueAccumulator, stm: Color) -> i32 {
        let (us_idx, them_idx) = if stm == Color::White { (0, 1) } else { (1, 0) };
        let us = &acc.vals[us_idx];
        let them = &acc.vals[them_idx];

        let mut sum: i64 = 0;
        for i in 0..NNUE_HL {
            let vu = (us[i] as i32).clamp(0, NNUE_QA) as i64;
            let vt = (them[i] as i32).clamp(0, NNUE_QA) as i64;
            sum += vu * vu * self.out_weights[i] as i64;
            sum += vt * vt * self.out_weights[NNUE_HL + i] as i64;
        }

        let eval = (sum / NNUE_QA as i64 + self.out_bias as i64) * NNUE_SCALE as i64
            / (NNUE_QA as i64 * NNUE_QB as i64);
        (eval as i32).clamp(-MAX_EVAL_CP, MAX_EVAL_CP)
    }

    /// Serializes quantized network weights into the `.mnue` binary format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let total = 16 + (NNUE_INPUTS * NNUE_HL + NNUE_HL + 2 * NNUE_HL) * 2 + 4;
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(MNUE_MAGIC);
        out.extend_from_slice(&(NNUE_HL as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        for &w in self.ft_weights.iter() {
            out.extend_from_slice(&w.to_le_bytes());
        }
        for &b in self.ft_biases.iter() {
            out.extend_from_slice(&b.to_le_bytes());
        }
        for &w in self.out_weights.iter() {
            out.extend_from_slice(&w.to_le_bytes());
        }
        out.extend_from_slice(&self.out_bias.to_le_bytes());
        out
    }

    /// Loads quantized network weights from `.mnue` bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let expected = 16 + (NNUE_INPUTS * NNUE_HL + NNUE_HL + 2 * NNUE_HL) * 2 + 4;
        if bytes.len() != expected {
            return Err(format!(
                "invalid NNUE file size: expected {} bytes, got {}",
                expected,
                bytes.len()
            ));
        }
        if &bytes[0..8] != MNUE_MAGIC {
            return Err("invalid NNUE magic header (expected MNUEv001)".to_string());
        }
        let hl = u32::from_le_bytes(bytes[8..12].try_into().map_err(|_| "header err")?) as usize;
        if hl != NNUE_HL {
            return Err(format!(
                "unsupported NNUE hidden size {hl}, expected {NNUE_HL}"
            ));
        }

        let mut cursor = 16;
        let read_i16 = |buf: &[u8], off: &mut usize| -> i16 {
            let v = i16::from_le_bytes([buf[*off], buf[*off + 1]]);
            *off += 2;
            v
        };

        let mut ft_weights = vec![0i16; NNUE_INPUTS * NNUE_HL];
        for slot in &mut ft_weights {
            *slot = read_i16(bytes, &mut cursor);
        }
        let mut ft_biases = vec![0i16; NNUE_HL];
        for slot in &mut ft_biases {
            *slot = read_i16(bytes, &mut cursor);
        }
        let mut out_weights = vec![0i16; 2 * NNUE_HL];
        for slot in &mut out_weights {
            *slot = read_i16(bytes, &mut cursor);
        }
        let out_bias = i32::from_le_bytes(
            bytes[cursor..cursor + 4]
                .try_into()
                .map_err(|_| "bias err")?,
        );

        Ok(Self {
            ft_weights: ft_weights
                .into_boxed_slice()
                .try_into()
                .map_err(|_| "ft_weights box err")?,
            ft_biases: ft_biases
                .into_boxed_slice()
                .try_into()
                .map_err(|_| "ft_biases box err")?,
            out_weights: out_weights
                .into_boxed_slice()
                .try_into()
                .map_err(|_| "out_weights box err")?,
            out_bias,
        })
    }

    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        let bytes =
            fs::read(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        Self::from_bytes(&bytes)
    }

    pub fn save_to_file(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(path, self.to_bytes())
    }
}

#[inline]
fn add_feature(acc: &mut [i16; NNUE_HL], weights: &[i16]) {
    for i in 0..NNUE_HL {
        acc[i] = acc[i].saturating_add(weights[i]);
    }
}

/// 32-byte `bulletformat::ChessBoard`-compatible training record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BulletRecord {
    pub occ: u64,
    pub pcs: [u8; 16],
    /// White-relative centipawn score.
    pub score_white_cp: i16,
    /// `0` = Black win, `1` = Draw, `2` = White win.
    pub result_wdl: u8,
    pub white_king_sq: u8,
    pub black_king_sq: u8,
}

impl BulletRecord {
    pub const BYTE_SIZE: usize = 32;

    /// Encodes a standard chess board (<= 32 pieces) into a 32-byte `BulletRecord`.
    pub fn from_board(board: &Board, score_white_cp: i16, result_wdl: u8) -> Option<Self> {
        let occ = board.all_occupancy();
        if occ.count_ones() > 32 {
            return None;
        }
        let mut pcs = [0u8; 16];
        let mut idx = 0usize;
        let mut bb = occ;
        while bb != 0 {
            let sq = pop_lsb(&mut bb);
            let (color, pt) = board.piece_on(sq)?;
            let code = ((u8::from(color == Color::Black)) << 3) | (pt.index() as u8 & 0x07);
            if idx.is_multiple_of(2) {
                pcs[idx / 2] |= code;
            } else {
                pcs[idx / 2] |= code << 4;
            }
            idx += 1;
        }

        let w_king = board.piece_bb(Color::White, PieceType::King);
        let b_king = board.piece_bb(Color::Black, PieceType::King);
        if w_king == 0 || b_king == 0 {
            return None;
        }

        Some(Self {
            occ,
            pcs,
            score_white_cp,
            result_wdl,
            white_king_sq: lsb(w_king).index() as u8,
            black_king_sq: lsb(b_king).index() as u8,
        })
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        let mut buf = [0u8; 32];
        buf[0..8].copy_from_slice(&self.occ.to_le_bytes());
        buf[8..24].copy_from_slice(&self.pcs);
        buf[24..26].copy_from_slice(&self.score_white_cp.to_le_bytes());
        buf[26] = self.result_wdl;
        buf[27] = self.white_king_sq;
        buf[28] = self.black_king_sq;
        buf
    }

    pub fn from_bytes(buf: &[u8; 32]) -> Self {
        let mut pcs = [0u8; 16];
        pcs.copy_from_slice(&buf[8..24]);
        Self {
            occ: u64::from_le_bytes(buf[0..8].try_into().unwrap_or([0; 8])),
            pcs,
            score_white_cp: i16::from_le_bytes([buf[24], buf[25]]),
            result_wdl: buf[26],
            white_king_sq: buf[27],
            black_king_sq: buf[28],
        }
    }

    /// Iterates `(piece_color, piece_type, square)` stored in this record.
    pub fn iter_pieces(&self) -> impl Iterator<Item = (Color, PieceType, Square)> + '_ {
        struct Iter<'a> {
            occ: u64,
            pcs: &'a [u8; 16],
            idx: usize,
        }
        impl<'a> Iterator for Iter<'a> {
            type Item = (Color, PieceType, Square);
            fn next(&mut self) -> Option<Self::Item> {
                if self.occ == 0 {
                    return None;
                }
                let sq = pop_lsb(&mut self.occ);
                let byte = self.pcs[self.idx / 2];
                let nibble = if self.idx.is_multiple_of(2) {
                    byte & 0x0F
                } else {
                    (byte >> 4) & 0x0F
                };
                self.idx += 1;
                let color = if (nibble & 0x08) != 0 {
                    Color::Black
                } else {
                    Color::White
                };
                let pt = match nibble & 0x07 {
                    0 => PieceType::Pawn,
                    1 => PieceType::Knight,
                    2 => PieceType::Bishop,
                    3 => PieceType::Rook,
                    4 => PieceType::Queen,
                    _ => PieceType::King,
                };
                Some((color, pt, sq))
            }
        }
        Iter {
            occ: self.occ,
            pcs: &self.pcs,
            idx: 0,
        }
    }
}

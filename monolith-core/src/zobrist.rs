//! Deterministic Zobrist hashing keys generated via SplitMix64.

use std::sync::OnceLock;

use crate::board::Board;
use crate::types::{Color, PieceType};

/// Precomputed Zobrist keys for pieces, side to move, castling rights, and EP files.
pub struct ZobristKeys {
    pub piece: [[[u64; 64]; 6]; 2],
    pub side: u64,
    pub castling: [u64; 16],
    pub ep_file: [u64; 8],
}

struct SplitMix64(u64);

impl SplitMix64 {
    #[inline]
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

static KEYS: OnceLock<ZobristKeys> = OnceLock::new();

/// Returns the global deterministic Zobrist key table.
#[inline]
pub fn keys() -> &'static ZobristKeys {
    KEYS.get_or_init(generate_keys)
}

/// Initializes the Zobrist key table eagerly.
pub fn init() {
    let _ = keys();
}

fn generate_keys() -> ZobristKeys {
    let mut rng = SplitMix64(0x4D4F_4E4F_4C49_5448); // "MONOLITH"
    let mut piece = [[[0u64; 64]; 6]; 2];
    for side in &mut piece {
        for pt in side {
            for sq in pt {
                *sq = rng.next_u64();
            }
        }
    }
    let side = rng.next_u64();
    let mut castling = [0u64; 16];
    for c in &mut castling {
        *c = rng.next_u64();
    }
    let mut ep_file = [0u64; 8];
    for ep in &mut ep_file {
        *ep = rng.next_u64();
    }
    ZobristKeys {
        piece,
        side,
        castling,
        ep_file,
    }
}

/// Computes the full Zobrist hash of a board from scratch.
pub fn compute_key(board: &Board) -> u64 {
    let k = keys();
    let mut hash = 0u64;

    for color in Color::ALL {
        for pt in PieceType::ALL {
            let mut bb = board.piece_bb(color, pt);
            while bb != 0 {
                let sq = bb.trailing_zeros() as usize;
                bb &= bb - 1;
                hash ^= k.piece[color.index()][pt.index()][sq];
            }
        }
    }

    if board.side_to_move() == Color::Black {
        hash ^= k.side;
    }

    hash ^= k.castling[board.castling_rights().mask()];

    if let Some(ep) = board.en_passant() {
        hash ^= k.ep_file[ep.file() as usize];
    }

    hash
}

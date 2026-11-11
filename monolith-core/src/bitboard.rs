//! Bitboard primitives, precomputed attack tables, ray/between tables,
//! and deterministically generated magic bitboards.

use std::sync::OnceLock;

use crate::types::{Color, Square};

pub const FILE_A: u64 = 0x0101_0101_0101_0101;
pub const FILE_B: u64 = FILE_A << 1;
pub const FILE_C: u64 = FILE_A << 2;
pub const FILE_D: u64 = FILE_A << 3;
pub const FILE_E: u64 = FILE_A << 4;
pub const FILE_F: u64 = FILE_A << 5;
pub const FILE_G: u64 = FILE_A << 6;
pub const FILE_H: u64 = FILE_A << 7;

pub const NOT_FILE_A: u64 = !FILE_A;
pub const NOT_FILE_H: u64 = !FILE_H;
pub const NOT_FILE_AB: u64 = !(FILE_A | FILE_B);
pub const NOT_FILE_GH: u64 = !(FILE_G | FILE_H);

pub const RANK_1: u64 = 0x0000_0000_0000_00FF;
pub const RANK_2: u64 = RANK_1 << 8;
pub const RANK_3: u64 = RANK_1 << 16;
pub const RANK_4: u64 = RANK_1 << 24;
pub const RANK_5: u64 = RANK_1 << 32;
pub const RANK_6: u64 = RANK_1 << 40;
pub const RANK_7: u64 = RANK_1 << 48;
pub const RANK_8: u64 = RANK_1 << 56;

pub const LIGHT_SQUARES: u64 = 0x55AA_55AA_55AA_55AA;
pub const DARK_SQUARES: u64 = !LIGHT_SQUARES;

pub const FILES: [u64; 8] = [
    FILE_A, FILE_B, FILE_C, FILE_D, FILE_E, FILE_F, FILE_G, FILE_H,
];
pub const RANKS: [u64; 8] = [
    RANK_1, RANK_2, RANK_3, RANK_4, RANK_5, RANK_6, RANK_7, RANK_8,
];

/// Pops and returns the least-significant set bit as a [`Square`].
///
/// Caller must ensure `bb != 0`.
#[inline]
pub fn pop_lsb(bb: &mut u64) -> Square {
    debug_assert!(*bb != 0);
    let sq = bb.trailing_zeros() as u8;
    *bb &= *bb - 1;
    Square(sq)
}

/// Returns the least-significant set bit as a [`Square`].
///
/// Caller must ensure `bb != 0`.
#[inline]
pub fn lsb(bb: u64) -> Square {
    debug_assert!(bb != 0);
    Square(bb.trailing_zeros() as u8)
}

/// Returns the number of set bits in a bitboard.
#[inline]
pub const fn popcount(bb: u64) -> u32 {
    bb.count_ones()
}

#[inline]
pub const fn file_bb(file: u8) -> u64 {
    FILES[(file & 7) as usize]
}

#[inline]
pub const fn rank_bb(rank: u8) -> u64 {
    RANKS[(rank & 7) as usize]
}

#[derive(Clone, Copy)]
struct MagicEntry {
    mask: u64,
    magic: u64,
    shift: u32,
    offset: usize,
}

struct Tables {
    pawn_attacks: [[u64; 64]; 2],
    knight_attacks: [u64; 64],
    king_attacks: [u64; 64],
    between: [[u64; 64]; 64],
    line: [[u64; 64]; 64],
    rook_magics: [MagicEntry; 64],
    bishop_magics: [MagicEntry; 64],
    rook_table: Vec<u64>,
    bishop_table: Vec<u64>,
}

struct Prng(u64);

impl Prng {
    #[inline]
    fn next_u64(&mut self) -> u64 {
        // Xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    #[inline]
    fn sparse_u64(&mut self) -> u64 {
        self.next_u64() & self.next_u64() & self.next_u64()
    }
}

static TABLES: OnceLock<Tables> = OnceLock::new();

#[inline]
fn tables() -> &'static Tables {
    TABLES.get_or_init(build_tables)
}

/// Initializes all precomputed attack and magic bitboard tables eagerly.
pub fn init() {
    let _ = tables();
}

#[inline]
pub fn pawn_attacks(color: Color, sq: Square) -> u64 {
    tables().pawn_attacks[color.index()][sq.index()]
}

#[inline]
pub fn knight_attacks(sq: Square) -> u64 {
    tables().knight_attacks[sq.index()]
}

#[inline]
pub fn king_attacks(sq: Square) -> u64 {
    tables().king_attacks[sq.index()]
}

#[inline]
pub fn between_bb(s1: Square, s2: Square) -> u64 {
    tables().between[s1.index()][s2.index()]
}

#[inline]
pub fn line_bb(s1: Square, s2: Square) -> u64 {
    tables().line[s1.index()][s2.index()]
}

#[inline]
pub fn rook_attacks(sq: Square, occ: u64) -> u64 {
    let t = tables();
    let entry = &t.rook_magics[sq.index()];
    let idx = ((occ & entry.mask).wrapping_mul(entry.magic) >> entry.shift) as usize;
    t.rook_table[entry.offset + idx]
}

#[inline]
pub fn bishop_attacks(sq: Square, occ: u64) -> u64 {
    let t = tables();
    let entry = &t.bishop_magics[sq.index()];
    let idx = ((occ & entry.mask).wrapping_mul(entry.magic) >> entry.shift) as usize;
    t.bishop_table[entry.offset + idx]
}

#[inline]
pub fn queen_attacks(sq: Square, occ: u64) -> u64 {
    rook_attacks(sq, occ) | bishop_attacks(sq, occ)
}

/// Computes pawn capture mask for an entire bitboard of pawns.
#[inline]
pub const fn pawn_set_attacks(pawns: u64, color: Color) -> u64 {
    match color {
        Color::White => ((pawns & NOT_FILE_A) << 7) | ((pawns & NOT_FILE_H) << 9),
        Color::Black => ((pawns & NOT_FILE_A) >> 9) | ((pawns & NOT_FILE_H) >> 7),
    }
}

fn rook_mask(sq: usize) -> u64 {
    let r = (sq / 8) as i32;
    let f = (sq % 8) as i32;
    let mut mask = 0u64;
    for rr in (r + 1)..7 {
        mask |= 1u64 << (rr * 8 + f);
    }
    for rr in 1..r {
        mask |= 1u64 << (rr * 8 + f);
    }
    for ff in (f + 1)..7 {
        mask |= 1u64 << (r * 8 + ff);
    }
    for ff in 1..f {
        mask |= 1u64 << (r * 8 + ff);
    }
    mask
}

fn bishop_mask(sq: usize) -> u64 {
    let r = (sq / 8) as i32;
    let f = (sq % 8) as i32;
    let mut mask = 0u64;
    for (dr, df) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
        let mut rr = r + dr;
        let mut ff = f + df;
        while (1..=6).contains(&rr) && (1..=6).contains(&ff) {
            mask |= 1u64 << (rr * 8 + ff);
            rr += dr;
            ff += df;
        }
    }
    mask
}

fn sliding_attacks_on_the_fly(sq: usize, occ: u64, dirs: &[(i32, i32)]) -> u64 {
    let r = (sq / 8) as i32;
    let f = (sq % 8) as i32;
    let mut attacks = 0u64;
    for &(dr, df) in dirs {
        let mut rr = r + dr;
        let mut ff = f + df;
        while (0..8).contains(&rr) && (0..8).contains(&ff) {
            let bit = 1u64 << (rr * 8 + ff);
            attacks |= bit;
            if (occ & bit) != 0 {
                break;
            }
            rr += dr;
            ff += df;
        }
    }
    attacks
}

fn rook_attacks_on_the_fly(sq: usize, occ: u64) -> u64 {
    sliding_attacks_on_the_fly(sq, occ, &[(1, 0), (-1, 0), (0, 1), (0, -1)])
}

fn bishop_attacks_on_the_fly(sq: usize, occ: u64) -> u64 {
    sliding_attacks_on_the_fly(sq, occ, &[(1, 1), (1, -1), (-1, 1), (-1, -1)])
}

fn find_magics(is_rook: bool, rng: &mut Prng) -> ([MagicEntry; 64], Vec<u64>) {
    let empty_entry = MagicEntry {
        mask: 0,
        magic: 0,
        shift: 0,
        offset: 0,
    };
    let mut entries = [empty_entry; 64];
    let mut total_size = 0usize;

    for sq in 0..64 {
        let mask = if is_rook {
            rook_mask(sq)
        } else {
            bishop_mask(sq)
        };
        total_size += 1usize << mask.count_ones();
    }

    let mut table = vec![0u64; total_size];
    let mut occs = [0u64; 4096];
    let mut ref_attacks = [0u64; 4096];
    let mut used = [0u64; 4096];
    let mut epoch = [0u32; 4096];
    let mut current_epoch = 0u32;
    let mut offset = 0usize;

    for (sq, entry_slot) in entries.iter_mut().enumerate() {
        let mask = if is_rook {
            rook_mask(sq)
        } else {
            bishop_mask(sq)
        };
        let bits = mask.count_ones();
        let size = 1usize << bits;
        let shift = 64 - bits;

        let mut subset = 0u64;
        let mut n = 0usize;
        loop {
            occs[n] = subset;
            ref_attacks[n] = if is_rook {
                rook_attacks_on_the_fly(sq, subset)
            } else {
                bishop_attacks_on_the_fly(sq, subset)
            };
            n += 1;
            subset = subset.wrapping_sub(mask) & mask;
            if subset == 0 {
                break;
            }
        }

        loop {
            let magic = rng.sparse_u64();
            if ((mask.wrapping_mul(magic)) >> 56).count_ones() < 6 {
                continue;
            }

            current_epoch = current_epoch.wrapping_add(1);
            if current_epoch == 0 {
                epoch.fill(0);
                current_epoch = 1;
            }

            let mut fail = false;
            for i in 0..size {
                let idx = ((occs[i].wrapping_mul(magic)) >> shift) as usize;
                if epoch[idx] != current_epoch {
                    epoch[idx] = current_epoch;
                    used[idx] = ref_attacks[i];
                } else if used[idx] != ref_attacks[i] {
                    fail = true;
                    break;
                }
            }

            if !fail {
                *entry_slot = MagicEntry {
                    mask,
                    magic,
                    shift,
                    offset,
                };
                table[offset..offset + size].copy_from_slice(&used[..size]);
                offset += size;
                break;
            }
        }
    }

    (entries, table)
}

fn build_tables() -> Tables {
    let mut pawn_attacks = [[0u64; 64]; 2];
    let mut knight_attacks = [0u64; 64];
    let mut king_attacks = [0u64; 64];
    let mut between = [[0u64; 64]; 64];
    let mut line = [[0u64; 64]; 64];

    for sq in 0..64 {
        let b = 1u64 << sq;
        pawn_attacks[Color::White.index()][sq] = ((b & NOT_FILE_A) << 7) | ((b & NOT_FILE_H) << 9);
        pawn_attacks[Color::Black.index()][sq] = ((b & NOT_FILE_A) >> 9) | ((b & NOT_FILE_H) >> 7);

        let r = (sq / 8) as i32;
        let f = (sq % 8) as i32;

        let mut kn = 0u64;
        for (dr, df) in [
            (2, 1),
            (2, -1),
            (-2, 1),
            (-2, -1),
            (1, 2),
            (1, -2),
            (-1, 2),
            (-1, -2),
        ] {
            let rr = r + dr;
            let ff = f + df;
            if (0..8).contains(&rr) && (0..8).contains(&ff) {
                kn |= 1u64 << (rr * 8 + ff);
            }
        }
        knight_attacks[sq] = kn;

        let mut kg = 0u64;
        for dr in -1..=1 {
            for df in -1..=1 {
                if dr == 0 && df == 0 {
                    continue;
                }
                let rr = r + dr;
                let ff = f + df;
                if (0..8).contains(&rr) && (0..8).contains(&ff) {
                    kg |= 1u64 << (rr * 8 + ff);
                }
            }
        }
        king_attacks[sq] = kg;
    }

    for s1 in 0..64 {
        let r1 = (s1 / 8) as i32;
        let f1 = (s1 % 8) as i32;
        for s2 in 0..64 {
            if s1 == s2 {
                continue;
            }
            let r2 = (s2 / 8) as i32;
            let f2 = (s2 % 8) as i32;
            let dr = (r2 - r1).signum();
            let df = (f2 - f1).signum();
            let aligned_ortho = r1 == r2 || f1 == f2;
            let aligned_diag = (r2 - r1).abs() == (f2 - f1).abs();
            if !aligned_ortho && !aligned_diag {
                continue;
            }

            let mut btw = 0u64;
            let mut rr = r1 + dr;
            let mut ff = f1 + df;
            while rr != r2 || ff != f2 {
                btw |= 1u64 << (rr * 8 + ff);
                rr += dr;
                ff += df;
            }
            between[s1][s2] = btw;

            let mut ln = 0u64;
            for step in [-1, 1] {
                let mut r_cur = r1;
                let mut f_cur = f1;
                while (0..8).contains(&r_cur) && (0..8).contains(&f_cur) {
                    ln |= 1u64 << (r_cur * 8 + f_cur);
                    r_cur += dr * step;
                    f_cur += df * step;
                }
            }
            line[s1][s2] = ln;
        }
    }

    let mut rng = Prng(1070372);
    let (rook_magics, rook_table) = find_magics(true, &mut rng);
    let (bishop_magics, bishop_table) = find_magics(false, &mut rng);

    Tables {
        pawn_attacks,
        knight_attacks,
        king_attacks,
        between,
        line,
        rook_magics,
        bishop_magics,
        rook_table,
        bishop_table,
    }
}

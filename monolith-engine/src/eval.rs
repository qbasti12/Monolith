//! Static evaluation with PeSTO piece-square tables, tapered game phase,
//! pawn structure, mobility, bishop pair, and king safety.

use monolith_core::bitboard::{
    self, bishop_attacks, file_bb, knight_attacks, lsb, pawn_set_attacks, pop_lsb, rook_attacks,
    FILES,
};
use monolith_core::types::{Color, Move, PieceType, Square};
use monolith_core::Board;

pub const SEE_VALUES: [i32; 6] = [100, 320, 330, 500, 950, 20_000];

pub const MAX_EVAL_CP: i32 = 28_000;

pub const MG_PIECE_VALUES: [i32; 6] = [82, 337, 365, 477, 1025, 0];
pub const EG_PIECE_VALUES: [i32; 6] = [94, 281, 297, 512, 936, 0];

const PHASE_WEIGHTS: [i32; 6] = [0, 1, 1, 2, 4, 0];
const MAX_PHASE: i32 = 24;

// PeSTO piece-square tables ordered rank 8 -> rank 1 (a8=0 .. h1=63).
// White looks up `sq.flip_rank()`, Black looks up `sq`.
#[rustfmt::skip]
const MG_PST: [[i32; 64]; 6] = [
    // Pawn
    [
          0,   0,   0,   0,   0,   0,   0,   0,
         98, 134,  61,  95,  68, 126,  34, -11,
         -6,   7,  26,  31,  65,  56,  25, -20,
        -14,  13,   6,  21,  23,  12,  17, -23,
        -27,  -2,  -5,  12,  17,   6,  10, -25,
        -26,  -4,  -4, -10,   3,   3,  33, -12,
        -35,  -1, -20, -23, -15,  24,  38, -22,
          0,   0,   0,   0,   0,   0,   0,   0,
    ],
    // Knight
    [
       -167, -89, -34, -49,  61, -97, -15,-107,
        -73, -41,  72,  36,  23,  62,   7, -17,
        -47,  60,  37,  65,  84, 129,  73,  44,
         -9,  17,  19,  53,  37,  69,  18,  22,
        -13,   4,  16,  13,  28,  19,  21,  -8,
        -23,  -9,  12,  10,  19,  17,  25, -16,
        -29, -53, -12,  -3,  -1,  18, -14, -19,
       -105, -21, -58, -33, -17, -28, -19, -23,
    ],
    // Bishop
    [
        -29,   4, -82, -37, -25, -42,   7,  -8,
        -26,  16, -18, -13,  30,  59,  18, -47,
        -16,  37,  43,  40,  35,  50,  37,  -2,
         -4,   5,  19,  50,  37,  37,   7,  -2,
         -6,  13,  13,  26,  34,  12,  10,   4,
          0,  15,  15,  15,  14,  27,  18,  10,
          4,  15,  16,   0,   7,  21,  33,   1,
        -33,  -3, -14, -21, -13, -12, -39, -21,
    ],
    // Rook
    [
         32,  42,  32,  51,  63,   9,  31,  43,
         27,  32,  58,  62,  80,  67,  26,  44,
         -5,  19,  26,  36,  17,  45,  61,  16,
        -24, -11,   7,  26,  24,  35,  -8, -20,
        -36, -26, -12,  -1,   9,  -7,   6, -23,
        -45, -25, -16, -17,   3,   0,  -5, -33,
        -44, -16, -20,  -9,  -1,  11,  -6, -71,
        -19, -13,   1,  17,  16,   7, -37, -26,
    ],
    // Queen
    [
        -28,   0,  29,  12,  59,  44,  43,  45,
        -24, -39,  -5,   1, -16,  57,  28,  54,
        -13, -17,   7,   8,  29,  56,  47,  57,
        -27, -27, -16, -16,  -1,  17,  -2,   1,
         -9, -26,  -9, -10,  -2,  -4,   3,  -3,
        -14,   2, -11,  -2,  -5,   2,  14,   5,
        -35,  -8,  11,   2,   8,  15,  -3,   1,
         -1, -18,  -9,  10, -15, -25, -31, -50,
    ],
    // King
    [
        -65,  23,  16, -15, -56, -34,   2,  13,
         29,  -1, -20,  -7,  -8,  -4, -38, -29,
         -9,  24,   2, -16, -20,   6,  22, -22,
        -17, -20, -12, -27, -30, -25, -14, -36,
        -49,  -1, -27, -39, -46, -44, -33, -51,
        -14, -14, -22, -46, -44, -30, -15, -27,
          1,   7,  -8, -64, -43, -16,   9,   8,
        -15,  36,  12, -54,   8, -28,  24,  14,
    ],
];

#[rustfmt::skip]
const EG_PST: [[i32; 64]; 6] = [
    // Pawn
    [
          0,   0,   0,   0,   0,   0,   0,   0,
        178, 173, 158, 134, 147, 132, 165, 187,
         94, 100,  85,  67,  56,  53,  82,  84,
         32,  24,  13,   5,  -2,   4,  17,  17,
         13,   9,  -3,  -7,  -7,  -8,   3,  -1,
          4,   7,  -6,   1,   0,  -5,  -1,  -8,
         13,   8,   8,  10,  13,   0,   2,  -7,
          0,   0,   0,   0,   0,   0,   0,   0,
    ],
    // Knight
    [
        -58, -38, -13, -28, -31, -27, -63, -99,
        -25,  -8, -25,  -2,  -9, -25, -24, -52,
        -24, -20,  10,   9,  -1,  -9, -19, -41,
        -17,   3,  22,  22,  22,  11,   8, -18,
        -18,  -6,  16,  25,  16,  17,   4, -18,
        -23,  -3,  -1,  15,  10,  -3, -20, -22,
        -42, -20, -10,  -5,  -2, -20, -23, -44,
        -29, -51, -23, -15, -22, -18, -50, -64,
    ],
    // Bishop
    [
        -14, -21, -11,  -8,  -7,  -9, -17, -24,
         -8,  -4,   7, -12,  -3, -13,  -4, -14,
          2,  -8,   0,  -1,  -2,   6,   0,   4,
         -3,   9,  12,   9,  14,  10,   3,   2,
         -6,   3,  13,  19,   7,  10,  -3,  -9,
        -12,  -3,   8,  10,  13,   3,  -7, -15,
        -14, -18,  -7,  -1,   4,  -9, -15, -27,
        -23,  -9, -23,  -5,  -9, -16,  -5, -17,
    ],
    // Rook
    [
         13,  10,  18,  15,  12,  12,   8,   5,
         11,  13,  13,  11,  -3,   3,   8,   3,
          7,   7,   7,   5,   4,  -3,  -5,  -3,
          4,   3,  13,   1,   2,   1,  -1,   2,
          3,   5,   8,   4,  -5,  -6,  -8, -11,
         -4,   0,  -5,  -1,  -7, -12,  -8, -16,
         -6,  -6,   0,   2,  -9,  -9, -11,  -3,
         -9,   2,   3,  -1,  -5, -13,   4, -20,
    ],
    // Queen
    [
         -9,  22,  22,  27,  27,  19,  10,  20,
        -17,  20,  32,  41,  58,  25,  30,   0,
        -20,   6,   9,  49,  47,  35,  19,   9,
          3,  22,  24,  45,  57,  40,  57,  36,
        -18,  28,  19,  47,  31,  34,  39,  23,
        -16, -27,  15,   6,   9,  17,  10,   5,
        -22, -23, -30, -16, -16, -23, -36, -32,
        -33, -28, -22, -43,  -5, -32, -20, -41,
    ],
    // King
    [
        -74, -35, -18, -18, -11,  15,   4, -17,
        -12,  17,  14,  17,  17,  38,  23,  11,
         10,  17,  23,  15,  20,  45,  44,  13,
         -8,  22,  24,  27,  26,  33,  26,   3,
        -18,  -4,  21,  24,  27,  23,   9, -11,
        -19,  -3,  11,  21,  23,  16,   7,  -9,
        -27, -11,   4,  13,  14,   4,  -5, -17,
        -53, -34, -21, -11, -28, -14, -24, -43,
    ],
];

const PASSED_PAWN_MG: [i32; 8] = [0, 8, 12, 20, 38, 68, 110, 0];
const PASSED_PAWN_EG: [i32; 8] = [0, 14, 22, 36, 65, 110, 175, 0];

#[derive(Debug, Clone, Copy, Default)]
struct ScorePair {
    mg: i32,
    eg: i32,
}

impl ScorePair {
    #[inline]
    const fn new(mg: i32, eg: i32) -> Self {
        Self { mg, eg }
    }

    #[inline]
    fn add(&mut self, other: ScorePair) {
        self.mg += other.mg;
        self.eg += other.eg;
    }

    #[inline]
    fn sub(&mut self, other: ScorePair) {
        self.mg -= other.mg;
        self.eg -= other.eg;
    }

    #[inline]
    fn taper(self, mg_phase: i32) -> i32 {
        let eg_phase = MAX_PHASE - mg_phase;
        let num = (self.mg as i64) * (mg_phase as i64) + (self.eg as i64) * (eg_phase as i64);
        (num / (MAX_PHASE as i64)).clamp(-MAX_EVAL_CP as i64, MAX_EVAL_CP as i64) as i32
    }
}

/// Detailed centipawn breakdown of a static position evaluation (White-POV components + final STM eval).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EvalBreakdown {
    pub material_cp: i32,
    pub pst_cp: i32,
    pub pawn_structure_cp: i32,
    pub king_safety_cp: i32,
    pub mobility_cp: i32,
    pub bishop_pair_cp: i32,
    pub phase: i32,
    pub white_pov_cp: i32,
    pub stm_pov_cp: i32,
}

/// Evaluates `board` statically in centipawns from the side-to-move's perspective.
pub fn evaluate(board: &Board) -> i32 {
    eval_breakdown(board).stm_pov_cp
}

/// Computes the full evaluation breakdown without heap allocations.
pub fn eval_breakdown(board: &Board) -> EvalBreakdown {
    let mut phase_raw = 0i32;
    let mut mat = [ScorePair::default(); 2];
    let mut pst = [ScorePair::default(); 2];

    for color in Color::ALL {
        let ci = color.index();
        for pt in PieceType::ALL {
            let pti = pt.index();
            let mut bb = board.piece_bb(color, pt);
            let count = bb.count_ones() as i32;
            phase_raw += count * PHASE_WEIGHTS[pti];
            mat[ci].mg += count * MG_PIECE_VALUES[pti];
            mat[ci].eg += count * EG_PIECE_VALUES[pti];

            while bb != 0 {
                let sq = pop_lsb(&mut bb);
                let idx = match color {
                    Color::White => sq.flip_rank().index(),
                    Color::Black => sq.index(),
                };
                pst[ci].mg += MG_PST[pti][idx];
                pst[ci].eg += EG_PST[pti][idx];
            }
        }
    }

    let phase = phase_raw.clamp(0, MAX_PHASE);

    let bishop_pair = [
        if board.piece_bb(Color::White, PieceType::Bishop).count_ones() >= 2 {
            ScorePair::new(30, 45)
        } else {
            ScorePair::default()
        },
        if board.piece_bb(Color::Black, PieceType::Bishop).count_ones() >= 2 {
            ScorePair::new(30, 45)
        } else {
            ScorePair::default()
        },
    ];

    let pawn_struct = [
        eval_pawn_structure(board, Color::White),
        eval_pawn_structure(board, Color::Black),
    ];

    let king_safety = [
        eval_king_safety(board, Color::White),
        eval_king_safety(board, Color::Black),
    ];

    let mobility = [
        eval_mobility(board, Color::White),
        eval_mobility(board, Color::Black),
    ];

    let mat_diff = ScorePair::new(mat[0].mg - mat[1].mg, mat[0].eg - mat[1].eg);
    let pst_diff = ScorePair::new(pst[0].mg - pst[1].mg, pst[0].eg - pst[1].eg);
    let pawn_diff = ScorePair::new(
        pawn_struct[0].mg - pawn_struct[1].mg,
        pawn_struct[0].eg - pawn_struct[1].eg,
    );
    let king_diff = ScorePair::new(
        king_safety[0].mg - king_safety[1].mg,
        king_safety[0].eg - king_safety[1].eg,
    );
    let mob_diff = ScorePair::new(
        mobility[0].mg - mobility[1].mg,
        mobility[0].eg - mobility[1].eg,
    );
    let bp_diff = ScorePair::new(
        bishop_pair[0].mg - bishop_pair[1].mg,
        bishop_pair[0].eg - bishop_pair[1].eg,
    );

    let mut total_white = ScorePair::default();
    total_white.add(mat_diff);
    total_white.add(pst_diff);
    total_white.add(pawn_diff);
    total_white.add(king_diff);
    total_white.add(mob_diff);
    total_white.add(bp_diff);

    // Small tempo bonus for the side to move (symmetric under color + STM flip).
    let tempo = match board.side_to_move() {
        Color::White => ScorePair::new(10, 6),
        Color::Black => ScorePair::new(-10, -6),
    };
    total_white.add(tempo);

    let white_pov_cp = total_white.taper(phase);
    let stm_pov_cp = match board.side_to_move() {
        Color::White => white_pov_cp,
        Color::Black => -white_pov_cp,
    };

    EvalBreakdown {
        material_cp: mat_diff.taper(phase),
        pst_cp: pst_diff.taper(phase),
        pawn_structure_cp: pawn_diff.taper(phase),
        king_safety_cp: king_diff.taper(phase),
        mobility_cp: mob_diff.taper(phase),
        bishop_pair_cp: bp_diff.taper(phase),
        phase,
        white_pov_cp,
        stm_pov_cp,
    }
}

fn eval_pawn_structure(board: &Board, color: Color) -> ScorePair {
    let own_pawns = board.piece_bb(color, PieceType::Pawn);
    let enemy_pawns = board.piece_bb(color.flip(), PieceType::Pawn);
    let mut score = ScorePair::default();

    // Doubled pawns per file.
    for &file_mask in &FILES {
        let on_file = (own_pawns & file_mask).count_ones() as i32;
        if on_file > 1 {
            score.sub(ScorePair::new(10 * (on_file - 1), 18 * (on_file - 1)));
        }
    }

    let mut bb = own_pawns;
    while bb != 0 {
        let sq = pop_lsb(&mut bb);
        let file = sq.file() as usize;
        let rank = sq.rank() as usize;

        let adj_files = adjacent_files_mask(file);
        if (own_pawns & adj_files) == 0 {
            score.sub(ScorePair::new(12, 16));
        }

        let front_span = forward_span_mask(color, file, rank);
        if (enemy_pawns & front_span) == 0 {
            let rel_rank = match color {
                Color::White => rank,
                Color::Black => 7 - rank,
            };
            score.add(ScorePair::new(
                PASSED_PAWN_MG[rel_rank],
                PASSED_PAWN_EG[rel_rank],
            ));
        }
    }

    score
}

fn eval_king_safety(board: &Board, color: Color) -> ScorePair {
    let king_sq = board.king_square(color);
    let kf = king_sq.file() as usize;
    let kr = king_sq.rank() as usize;
    let own_pawns = board.piece_bb(color, PieceType::Pawn);
    let enemy = color.flip();
    let enemy_pawns = board.piece_bb(enemy, PieceType::Pawn);
    let has_enemy_majors =
        (board.piece_bb(enemy, PieceType::Queen) | board.piece_bb(enemy, PieceType::Rook)) != 0;

    let mut score = ScorePair::default();
    let (ahead1, ahead2) = match color {
        Color::White => (
            kr.checked_add(1).filter(|&r| r < 8),
            kr.checked_add(2).filter(|&r| r < 8),
        ),
        Color::Black => (kr.checked_sub(1), kr.checked_sub(2)),
    };

    let f_start = kf.saturating_sub(1);
    let f_end = (kf + 1).min(7);
    for f in f_start..=f_end {
        let f_mask = file_bb(f as u8);
        if let Some(r1) = ahead1 {
            if (own_pawns & f_mask & bitboard::rank_bb(r1 as u8)) != 0 {
                score.add(ScorePair::new(12, 0));
            } else if let Some(r2) = ahead2 {
                if (own_pawns & f_mask & bitboard::rank_bb(r2 as u8)) != 0 {
                    score.add(ScorePair::new(6, 0));
                }
            }
        }

        if has_enemy_majors && (own_pawns & f_mask) == 0 {
            let open_bonus = if (enemy_pawns & f_mask) == 0 { 22 } else { 12 };
            let scale = if f == kf { open_bonus } else { open_bonus / 2 };
            score.sub(ScorePair::new(scale, 0));
        }
    }

    score
}

fn eval_mobility(board: &Board, color: Color) -> ScorePair {
    let own = board.color_occupancy(color);
    let occ = board.all_occupancy();
    let enemy_pawns = board.piece_bb(color.flip(), PieceType::Pawn);
    let pawn_danger = pawn_set_attacks(enemy_pawns, color.flip());
    let safe_targets = !own & !pawn_danger;

    let mut score = ScorePair::default();

    let mut knights = board.piece_bb(color, PieceType::Knight);
    while knights != 0 {
        let sq = pop_lsb(&mut knights);
        let mob = (knight_attacks(sq) & safe_targets).count_ones() as i32;
        score.add(ScorePair::new(mob * 4, mob * 4));
    }

    let mut bishops = board.piece_bb(color, PieceType::Bishop);
    while bishops != 0 {
        let sq = pop_lsb(&mut bishops);
        let mob = (bishop_attacks(sq, occ) & safe_targets).count_ones() as i32;
        score.add(ScorePair::new(mob * 5, mob * 5));
    }

    let mut rooks = board.piece_bb(color, PieceType::Rook);
    while rooks != 0 {
        let sq = pop_lsb(&mut rooks);
        let mob = (rook_attacks(sq, occ) & !own).count_ones() as i32;
        score.add(ScorePair::new(mob * 3, mob * 4));
    }

    let mut queens = board.piece_bb(color, PieceType::Queen);
    while queens != 0 {
        let sq = pop_lsb(&mut queens);
        let mob =
            ((bishop_attacks(sq, occ) | rook_attacks(sq, occ)) & safe_targets).count_ones() as i32;
        score.add(ScorePair::new(mob, mob * 2));
    }

    score
}

#[inline]
const fn adjacent_files_mask(file: usize) -> u64 {
    let left = if file > 0 { FILES[file - 1] } else { 0 };
    let right = if file < 7 { FILES[file + 1] } else { 0 };
    left | right
}

#[inline]
fn forward_span_mask(color: Color, file: usize, rank: usize) -> u64 {
    let file_and_adj = FILES[file] | adjacent_files_mask(file);
    let ahead_ranks = match color {
        Color::White => {
            if rank >= 7 {
                0
            } else {
                !0u64 << ((rank + 1) * 8)
            }
        }
        Color::Black => {
            if rank == 0 {
                0
            } else {
                (1u64 << (rank * 8)) - 1
            }
        }
    };
    file_and_adj & ahead_ranks
}

/// Static Exchange Evaluation test: returns true if the capture/promotion sequence on `mv.to_sq()`
/// achieves at least `threshold` centipawns.
pub fn see_ge(board: &Board, mv: Move, threshold: i32) -> bool {
    let from = mv.from_sq();
    let to = mv.to_sq();

    let mut gain = [0i32; 32];
    let mut d = 0usize;

    let initial_cap = if mv.is_en_passant() {
        SEE_VALUES[PieceType::Pawn.index()]
    } else if let Some((_, pt)) = board.piece_on(to) {
        SEE_VALUES[pt.index()]
    } else {
        0
    };

    let promo_bonus = mv
        .promotion_piece()
        .map(|pt| SEE_VALUES[pt.index()] - SEE_VALUES[PieceType::Pawn.index()])
        .unwrap_or(0);

    gain[0] = initial_cap + promo_bonus;
    if gain[0] < threshold && promo_bonus == 0 && !mv.is_capture() {
        return false;
    }

    let mut occ = board.all_occupancy() ^ from.bit();
    if mv.is_en_passant() {
        let ep_cap_sq = match board.side_to_move() {
            Color::White => Square(to.0 - 8),
            Color::Black => Square(to.0 + 8),
        };
        occ ^= ep_cap_sq.bit();
    }

    let mut attacker_pt = match mv.promotion_piece() {
        Some(pt) => pt,
        None => board
            .piece_on(from)
            .map(|(_, pt)| pt)
            .unwrap_or(PieceType::Pawn),
    };

    let diag_sliders = board.piece_bb(Color::White, PieceType::Bishop)
        | board.piece_bb(Color::Black, PieceType::Bishop)
        | board.piece_bb(Color::White, PieceType::Queen)
        | board.piece_bb(Color::Black, PieceType::Queen);
    let ortho_sliders = board.piece_bb(Color::White, PieceType::Rook)
        | board.piece_bb(Color::Black, PieceType::Rook)
        | board.piece_bb(Color::White, PieceType::Queen)
        | board.piece_bb(Color::Black, PieceType::Queen);

    let mut attackers = board.attackers_to(to, occ);
    let mut stm = board.side_to_move().flip();

    loop {
        d += 1;
        if d >= 31 {
            break;
        }
        gain[d] = SEE_VALUES[attacker_pt.index()] - gain[d - 1];

        let stm_attackers = attackers & board.color_occupancy(stm) & occ;
        if stm_attackers == 0 {
            break;
        }

        let mut next_pt = None;
        let mut next_sq = Square::A1;
        for pt in PieceType::ALL {
            let subset = stm_attackers & board.piece_bb(stm, pt);
            if subset != 0 {
                next_pt = Some(pt);
                next_sq = lsb(subset);
                break;
            }
        }
        let Some(picked_pt) = next_pt else {
            break;
        };

        occ ^= next_sq.bit();
        if matches!(
            picked_pt,
            PieceType::Pawn | PieceType::Bishop | PieceType::Queen
        ) {
            attackers |= bishop_attacks(to, occ) & diag_sliders;
        }
        if matches!(picked_pt, PieceType::Rook | PieceType::Queen) {
            attackers |= rook_attacks(to, occ) & ortho_sliders;
        }
        attackers &= occ;

        attacker_pt = picked_pt;
        stm = stm.flip();

        if attacker_pt == PieceType::King && (attackers & board.color_occupancy(stm)) != 0 {
            break;
        }
    }

    let mut idx = d.saturating_sub(1);
    while idx > 0 {
        gain[idx - 1] = -((-gain[idx - 1]).max(gain[idx]));
        idx -= 1;
    }

    gain[0] >= threshold
}

//! Elo-based strength throttling (`800..=3200`) and controlled Multi-PV move selection.
//!
//! Documented Elo profile table:
//! - `800`:  depth=1, nodes=40,    multi_pv=6, noise=350 cp, temp=260 cp, mate1_find=0.45
//! - `1000`: depth=2, nodes=120,   multi_pv=5, noise=240 cp, temp=180 cp, mate1_find=0.65
//! - `1200`: depth=2, nodes=300,   multi_pv=5, noise=170 cp, temp=120 cp, mate1_find=0.78
//! - `1400`: depth=3, nodes=800,   multi_pv=4, noise=110 cp, temp=80  cp, mate1_find=0.88
//! - `1600`: depth=4, nodes=2500,  multi_pv=3, noise=65  cp, temp=45  cp, mate1_find=0.94
//! - `1800`: depth=5, nodes=8000,  multi_pv=3, noise=35  cp, temp=25  cp, mate1_find=0.97
//! - `2000`: depth=6, nodes=25000, multi_pv=2, noise=18  cp, temp=12  cp, mate1_find=0.99
//! - `2400`: depth=10, nodes=None, multi_pv=1, noise=0   cp, temp=0   cp, mate1_find=1.00
//! - `2800`: depth=18, nodes=None, multi_pv=1, noise=0   cp, temp=0   cp, mate1_find=1.00
//! - `>=3000`: unthrottled (depth=64, nodes=None, multi_pv=1, noise=0, temp=0, mate1_find=1.00)

use monolith_core::types::Move;

use crate::search::{Score, SearchLimits, SearchResult};

pub const MIN_ELO: u32 = 800;
pub const MAX_ELO: u32 = 3200;
pub const UNTHROTTLED_ELO: u32 = 3000;

/// Strength throttling parameters derived from a target Elo rating.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillProfile {
    pub elo: u32,
    pub depth_limit: u8,
    pub node_limit: Option<u64>,
    pub multi_pv_k: usize,
    pub noise_cp: i32,
    pub softmax_temp_cp: f64,
    pub mate1_find_prob: f64,
}

const ELO_TABLE: [SkillProfile; 10] = [
    SkillProfile {
        elo: 800,
        depth_limit: 1,
        node_limit: Some(40),
        multi_pv_k: 6,
        noise_cp: 350,
        softmax_temp_cp: 260.0,
        mate1_find_prob: 0.45,
    },
    SkillProfile {
        elo: 1000,
        depth_limit: 2,
        node_limit: Some(120),
        multi_pv_k: 5,
        noise_cp: 240,
        softmax_temp_cp: 180.0,
        mate1_find_prob: 0.65,
    },
    SkillProfile {
        elo: 1200,
        depth_limit: 2,
        node_limit: Some(300),
        multi_pv_k: 5,
        noise_cp: 170,
        softmax_temp_cp: 120.0,
        mate1_find_prob: 0.78,
    },
    SkillProfile {
        elo: 1400,
        depth_limit: 3,
        node_limit: Some(800),
        multi_pv_k: 4,
        noise_cp: 110,
        softmax_temp_cp: 80.0,
        mate1_find_prob: 0.88,
    },
    SkillProfile {
        elo: 1600,
        depth_limit: 4,
        node_limit: Some(2500),
        multi_pv_k: 3,
        noise_cp: 65,
        softmax_temp_cp: 45.0,
        mate1_find_prob: 0.94,
    },
    SkillProfile {
        elo: 1800,
        depth_limit: 5,
        node_limit: Some(8000),
        multi_pv_k: 3,
        noise_cp: 35,
        softmax_temp_cp: 25.0,
        mate1_find_prob: 0.97,
    },
    SkillProfile {
        elo: 2000,
        depth_limit: 6,
        node_limit: Some(25_000),
        multi_pv_k: 2,
        noise_cp: 18,
        softmax_temp_cp: 12.0,
        mate1_find_prob: 0.99,
    },
    SkillProfile {
        elo: 2400,
        depth_limit: 10,
        node_limit: None,
        multi_pv_k: 1,
        noise_cp: 0,
        softmax_temp_cp: 0.0,
        mate1_find_prob: 1.0,
    },
    SkillProfile {
        elo: 2800,
        depth_limit: 18,
        node_limit: None,
        multi_pv_k: 1,
        noise_cp: 0,
        softmax_temp_cp: 0.0,
        mate1_find_prob: 1.0,
    },
    SkillProfile {
        elo: 3000,
        depth_limit: 64,
        node_limit: None,
        multi_pv_k: 1,
        noise_cp: 0,
        softmax_temp_cp: 0.0,
        mate1_find_prob: 1.0,
    },
];

impl SkillProfile {
    /// Returns the interpolated strength profile for `elo` (clamped to `800..=3200`).
    pub fn for_elo(elo: u32) -> Self {
        let clamped = elo.clamp(MIN_ELO, MAX_ELO);
        if clamped >= UNTHROTTLED_ELO {
            return SkillProfile {
                elo: clamped,
                ..ELO_TABLE[ELO_TABLE.len() - 1]
            };
        }

        for pair in ELO_TABLE.windows(2) {
            let lo = pair[0];
            let hi = pair[1];
            if clamped >= lo.elo && clamped <= hi.elo {
                let t = (clamped - lo.elo) as f64 / (hi.elo - lo.elo) as f64;
                let depth_limit = ((lo.depth_limit as f64)
                    + t * ((hi.depth_limit - lo.depth_limit) as f64))
                    .round() as u8;
                let node_limit = match (lo.node_limit, hi.node_limit) {
                    (Some(a), Some(b)) => {
                        let log_a = (a as f64).ln();
                        let log_b = (b as f64).ln();
                        Some((log_a + t * (log_b - log_a)).exp().round() as u64)
                    }
                    (Some(a), None) if t < 0.85 => {
                        Some(((a as f64) * (1.0 + t * 4.0)).round() as u64)
                    }
                    _ => None,
                };
                let multi_pv_k = ((lo.multi_pv_k as f64)
                    + t * ((hi.multi_pv_k as f64) - (lo.multi_pv_k as f64)))
                    .round()
                    .max(1.0) as usize;
                let noise_cp = ((lo.noise_cp as f64) + t * ((hi.noise_cp - lo.noise_cp) as f64))
                    .round() as i32;
                let softmax_temp_cp =
                    lo.softmax_temp_cp + t * (hi.softmax_temp_cp - lo.softmax_temp_cp);
                let mate1_find_prob =
                    lo.mate1_find_prob + t * (hi.mate1_find_prob - lo.mate1_find_prob);
                return SkillProfile {
                    elo: clamped,
                    depth_limit: depth_limit.max(1),
                    node_limit,
                    multi_pv_k,
                    noise_cp,
                    softmax_temp_cp,
                    mate1_find_prob,
                };
            }
        }

        ELO_TABLE[ELO_TABLE.len() - 1]
    }

    /// Applies depth, node, and Multi-PV limits onto `limits` when strength limiting is active.
    pub fn apply_to_limits(self, limits: &mut SearchLimits) {
        if self.elo >= UNTHROTTLED_ELO {
            return;
        }
        limits.max_depth = limits.max_depth.min(self.depth_limit);
        if let Some(node_cap) = self.node_limit {
            limits.max_nodes = Some(match limits.max_nodes {
                Some(existing) => existing.min(node_cap),
                None => node_cap,
            });
        }
        if self.multi_pv_k > 1 {
            limits.multi_pv = limits.multi_pv.max(self.multi_pv_k);
        }
    }
}

/// Deterministic XorShift64 PRNG for repeatable stochastic move selection in skill throttling.
#[derive(Debug, Clone)]
pub struct SkillRng(u64);

impl SkillRng {
    pub fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Returns a uniform float in `[0.0, 1.0)`.
    #[inline]
    pub fn next_unit(&mut self) -> f64 {
        let top53 = self.next_u64() >> 11;
        (top53 as f64) / ((1u64 << 53) as f64)
    }
}

/// Selects a move from `result`'s Multi-PV lines according to `profile` and `rng`.
///
/// Returns `(selected_move, ponder_move)`.
pub fn select_move(
    profile: SkillProfile,
    result: &SearchResult,
    rng: &mut SkillRng,
) -> (Move, Move) {
    if result.pv_count <= 1 || profile.elo >= UNTHROTTLED_ELO || profile.noise_cp == 0 {
        return (result.best_move, result.ponder_move);
    }

    let count = result.pv_count;
    let best_line = &result.pv_lines[0];

    // Probabilistic mate-in-1 detection / oversight.
    if Score::from_raw(best_line.score) == Score::Mate(1)
        && rng.next_unit() <= profile.mate1_find_prob
    {
        return (result.best_move, result.ponder_move);
    }

    let mut noisy_scores = [0.0f64; crate::search::MAX_MULTI_PV];
    let mut max_noisy = f64::NEG_INFINITY;

    for (idx, line) in result.pv_lines.iter().take(count).enumerate() {
        let base_score = if Score::from_raw(line.score) == Score::Mate(1) && idx == 0 {
            // Mate-in-1 was stochastically overlooked: cap effective score to avoid forcing it.
            300.0
        } else {
            line.score.clamp(-2000, 2000) as f64
        };

        let noise = (rng.next_unit() * 2.0 - 1.0) * (profile.noise_cp as f64);
        let noisy = base_score + noise;
        noisy_scores[idx] = noisy;
        if noisy > max_noisy {
            max_noisy = noisy;
        }
    }

    if profile.softmax_temp_cp <= 1.0 {
        let mut best_idx = 0usize;
        for (idx, &val) in noisy_scores.iter().enumerate().take(count) {
            if val > noisy_scores[best_idx] {
                best_idx = idx;
            }
        }
        let chosen = &result.pv_lines[best_idx];
        return (
            chosen.moves[0],
            if chosen.len >= 2 {
                chosen.moves[1]
            } else {
                Move::NONE
            },
        );
    }

    let temp = profile.softmax_temp_cp.max(1.0);
    let mut weights = [0.0f64; crate::search::MAX_MULTI_PV];
    let mut sum_w = 0.0f64;

    for (idx, &val) in noisy_scores.iter().enumerate().take(count) {
        let w = ((val - max_noisy) / temp).exp();
        weights[idx] = w;
        sum_w += w;
    }

    let mut pick = rng.next_unit() * sum_w;
    for (idx, &w) in weights.iter().enumerate().take(count) {
        pick -= w;
        if pick <= 0.0 {
            let chosen = &result.pv_lines[idx];
            return (
                chosen.moves[0],
                if chosen.len >= 2 {
                    chosen.moves[1]
                } else {
                    Move::NONE
                },
            );
        }
    }

    (result.best_move, result.ponder_move)
}

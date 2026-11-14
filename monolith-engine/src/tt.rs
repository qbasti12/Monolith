//! Lock-free atomic transposition table with multiplicative indexing, age replacement,
//! and mate-score normalization (safe for concurrent Lazy SMP threads without `unsafe`).

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};

use monolith_core::types::Move;

pub const DEFAULT_TT_MB: usize = 64;
pub const MIN_TT_MB: usize = 1;
pub const MAX_TT_MB: usize = 2048;

pub const MATE_SCORE: i32 = 30_000;
pub const MATE_BOUND: i32 = 29_000;

/// Transposition table entry bound type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Bound {
    None = 0,
    Exact = 1,
    Lower = 2,
    Upper = 3,
}

impl Bound {
    #[inline]
    const fn from_u8(v: u8) -> Self {
        match v & 0b11 {
            1 => Bound::Exact,
            2 => Bound::Lower,
            3 => Bound::Upper,
            _ => Bound::None,
        }
    }
}

/// Compact 16-byte lock-free TT slot using XOR-checksum verification.
#[repr(C)]
struct AtomicTtSlot {
    key_xor: AtomicU64,
    data: AtomicU64,
}

impl Default for AtomicTtSlot {
    fn default() -> Self {
        Self {
            key_xor: AtomicU64::new(0),
            data: AtomicU64::new(0),
        }
    }
}

#[inline]
fn pack_data(best_move: Move, score: i16, depth: u8, bound: Bound, age: u8) -> u64 {
    let meta = ((age & 0x3F) << 2) | (bound as u8 & 0b11);
    (best_move.0 as u64)
        | (((score as u16) as u64) << 16)
        | ((depth as u64) << 32)
        | ((meta as u64) << 40)
}

#[inline]
fn unpack_data(data: u64) -> (Move, i16, u8, Bound, u8) {
    let best_move = Move((data & 0xFFFF) as u16);
    let score = ((data >> 16) & 0xFFFF) as u16 as i16;
    let depth = ((data >> 32) & 0xFF) as u8;
    let meta = ((data >> 40) & 0xFF) as u8;
    let bound = Bound::from_u8(meta & 0b11);
    let age = meta >> 2;
    (best_move, score, depth, bound, age)
}

/// Result returned from a successful [`TranspositionTable::probe`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TtHit {
    pub best_move: Move,
    pub score: i32,
    pub depth: u8,
    pub bound: Bound,
}

/// Fixed-allocation, thread-safe (`Sync`) hash table for search caching.
pub struct TranspositionTable {
    entries: Vec<AtomicTtSlot>,
    age: AtomicU8,
}

impl TranspositionTable {
    /// Allocates a transposition table of `size_mb` megabytes (clamped to `1..=2048`).
    pub fn new(size_mb: usize) -> Self {
        let count = entry_count_for_mb(size_mb);
        let mut entries = Vec::with_capacity(count);
        entries.resize_with(count, AtomicTtSlot::default);
        Self {
            entries,
            age: AtomicU8::new(0),
        }
    }

    /// Resizes and clears the table to `size_mb` megabytes (`1..=2048`).
    pub fn resize(&mut self, size_mb: usize) {
        let count = entry_count_for_mb(size_mb);
        let mut entries = Vec::with_capacity(count);
        entries.resize_with(count, AtomicTtSlot::default);
        self.entries = entries;
        self.age.store(0, Ordering::Relaxed);
    }

    /// Clears all entries and resets age.
    pub fn clear(&self) {
        for slot in &self.entries {
            slot.key_xor.store(0, Ordering::Relaxed);
            slot.data.store(0, Ordering::Relaxed);
        }
        self.age.store(0, Ordering::Relaxed);
    }

    /// Advances the search generation age (used for aging replacement).
    #[inline]
    pub fn new_search(&self) {
        let prev = self.age.load(Ordering::Relaxed);
        self.age.store((prev + 1) & 0x3F, Ordering::Relaxed);
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Maps a 64-bit Zobrist key into `0..self.entries.len()` via 128-bit multiplication.
    #[inline]
    fn index(&self, key: u64) -> usize {
        ((key as u128).wrapping_mul(self.entries.len() as u128) >> 64) as usize
    }

    /// Probes the table for `key`, adjusting stored mate scores back to root-relative plies.
    #[inline]
    pub fn probe(&self, key: u64, ply: usize) -> Option<TtHit> {
        let idx = self.index(key);
        let slot = &self.entries[idx];
        let data = slot.data.load(Ordering::Relaxed);
        let key_xor = slot.key_xor.load(Ordering::Relaxed);
        if (key_xor ^ data) != key {
            return None;
        }
        let (best_move, score, depth, bound, _) = unpack_data(data);
        if bound == Bound::None {
            return None;
        }
        Some(TtHit {
            best_move,
            score: score_from_tt(score as i32, ply),
            depth,
            bound,
        })
    }

    /// Stores a search result with age-aware replacement and ply-normalized mate scores.
    #[inline]
    pub fn store(
        &self,
        key: u64,
        depth: u8,
        score: i32,
        bound: Bound,
        best_move: Move,
        ply: usize,
    ) {
        let idx = self.index(key);
        let slot = &self.entries[idx];
        let cur_age = self.age.load(Ordering::Relaxed);

        let old_data = slot.data.load(Ordering::Relaxed);
        let old_kx = slot.key_xor.load(Ordering::Relaxed);
        let old_key = old_kx ^ old_data;
        let (old_move, _, old_depth, _, old_age) = unpack_data(old_data);

        let replace = old_key == 0
            || old_key == key
            || old_age != cur_age
            || depth.saturating_add(2) >= old_depth
            || bound == Bound::Exact;

        if !replace {
            return;
        }

        let preserved_move = if best_move.is_none() && old_key == key {
            old_move
        } else {
            best_move
        };

        let tt_score = score_to_tt(score, ply).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        let new_data = pack_data(preserved_move, tt_score, depth, bound, cur_age);
        slot.data.store(new_data, Ordering::Relaxed);
        slot.key_xor.store(key ^ new_data, Ordering::Relaxed);
    }

    /// Returns occupancy in permille (`0..=1000`) by sampling the first 1000 slots.
    pub fn hashfull(&self) -> u32 {
        let sample = self.entries.len().min(1000);
        if sample == 0 {
            return 0;
        }
        let cur_age = self.age.load(Ordering::Relaxed);
        let used = self.entries[..sample]
            .iter()
            .filter(|slot| {
                let data = slot.data.load(Ordering::Relaxed);
                let (_, _, _, bound, age) = unpack_data(data);
                bound != Bound::None && age == cur_age
            })
            .count();
        ((used * 1000) / sample) as u32
    }
}

impl Default for TranspositionTable {
    fn default() -> Self {
        Self::new(DEFAULT_TT_MB)
    }
}

fn entry_count_for_mb(size_mb: usize) -> usize {
    let clamped = size_mb.clamp(MIN_TT_MB, MAX_TT_MB);
    let bytes = clamped * 1024 * 1024;
    (bytes / std::mem::size_of::<AtomicTtSlot>()).max(1)
}

#[inline]
pub fn score_to_tt(score: i32, ply: usize) -> i32 {
    if score >= MATE_BOUND {
        score + ply as i32
    } else if score <= -MATE_BOUND {
        score - ply as i32
    } else {
        score
    }
}

#[inline]
pub fn score_from_tt(score: i32, ply: usize) -> i32 {
    if score >= MATE_BOUND {
        score - ply as i32
    } else if score <= -MATE_BOUND {
        score + ply as i32
    } else {
        score
    }
}

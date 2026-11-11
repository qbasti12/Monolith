//! Transposition table with multiplicative indexing, age replacement, and mate-score normalization.

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

/// Compact 16-byte TT entry.
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct TtEntry {
    pub key: u64,
    pub best_move: Move,
    pub score: i16,
    pub depth: u8,
    meta: u8, // bits 0..2: Bound, bits 2..8: age (0..63)
}

impl TtEntry {
    #[inline]
    pub fn bound(self) -> Bound {
        Bound::from_u8(self.meta & 0b11)
    }

    #[inline]
    pub fn age(self) -> u8 {
        self.meta >> 2
    }

    #[inline]
    fn pack_meta(bound: Bound, age: u8) -> u8 {
        ((age & 0x3F) << 2) | (bound as u8 & 0b11)
    }
}

/// Result returned from a successful [`TranspositionTable::probe`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TtHit {
    pub best_move: Move,
    pub score: i32,
    pub depth: u8,
    pub bound: Bound,
}

/// Fixed-allocation hash table for search caching.
pub struct TranspositionTable {
    entries: Vec<TtEntry>,
    age: u8,
}

impl TranspositionTable {
    /// Allocates a transposition table of `size_mb` megabytes (clamped to `1..=2048`).
    pub fn new(size_mb: usize) -> Self {
        let count = entry_count_for_mb(size_mb);
        Self {
            entries: vec![TtEntry::default(); count],
            age: 0,
        }
    }

    /// Resizes and clears the table to `size_mb` megabytes (`1..=2048`).
    pub fn resize(&mut self, size_mb: usize) {
        let count = entry_count_for_mb(size_mb);
        self.entries = vec![TtEntry::default(); count];
        self.age = 0;
    }

    /// Clears all entries and resets age.
    pub fn clear(&mut self) {
        self.entries.fill(TtEntry::default());
        self.age = 0;
    }

    /// Advances the search generation age (used for aging replacement).
    #[inline]
    pub fn new_search(&mut self) {
        self.age = (self.age + 1) & 0x3F;
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
        let entry = self.entries[idx];
        if entry.key != key || entry.bound() == Bound::None {
            return None;
        }
        Some(TtHit {
            best_move: entry.best_move,
            score: score_from_tt(entry.score as i32, ply),
            depth: entry.depth,
            bound: entry.bound(),
        })
    }

    /// Stores a search result with age-aware replacement and ply-normalized mate scores.
    #[inline]
    pub fn store(
        &mut self,
        key: u64,
        depth: u8,
        score: i32,
        bound: Bound,
        best_move: Move,
        ply: usize,
    ) {
        let idx = self.index(key);
        let existing = self.entries[idx];

        let replace = existing.key == 0
            || existing.key == key
            || existing.age() != self.age
            || depth.saturating_add(2) >= existing.depth
            || bound == Bound::Exact;

        if !replace {
            return;
        }

        let preserved_move = if best_move.is_none() && existing.key == key {
            existing.best_move
        } else {
            best_move
        };

        let tt_score = score_to_tt(score, ply).clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        self.entries[idx] = TtEntry {
            key,
            best_move: preserved_move,
            score: tt_score,
            depth,
            meta: TtEntry::pack_meta(bound, self.age),
        };
    }

    /// Returns occupancy in permille (`0..=1000`) by sampling the first 1000 slots.
    pub fn hashfull(&self) -> u32 {
        let sample = self.entries.len().min(1000);
        if sample == 0 {
            return 0;
        }
        let used = self.entries[..sample]
            .iter()
            .filter(|e| e.bound() != Bound::None && e.age() == self.age)
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
    (bytes / std::mem::size_of::<TtEntry>()).max(1)
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

//! Minimal, crisp Chess.com-style wooden/tactile sound synthesis and non-blocking playback.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::thread;

/// Types of chess audio events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundKind {
    Move,
    Capture,
    Check,
    Castle,
    Promote,
    GameEnd,
}

impl SoundKind {
    fn filename(self) -> &'static str {
        match self {
            Self::Move => "monolith_move.wav",
            Self::Capture => "monolith_capture.wav",
            Self::Check => "monolith_check.wav",
            Self::Castle => "monolith_castle.wav",
            Self::Promote => "monolith_promote.wav",
            Self::GameEnd => "monolith_end.wav",
        }
    }
}

/// Non-blocking desktop audio player with synthesized dry wooden Chess.com-style sounds.
#[derive(Debug, Clone)]
pub struct SoundPlayer {
    pub enabled: bool,
}

impl Default for SoundPlayer {
    fn default() -> Self {
        Self { enabled: true }
    }
}

impl SoundPlayer {
    pub fn play(&self, kind: SoundKind) {
        if !self.enabled {
            return;
        }
        if let Some(path) = cached_sound_path(kind) {
            thread::spawn(move || {
                #[cfg(target_os = "macos")]
                {
                    let _ = Command::new("afplay").arg(&path).status();
                }
                #[cfg(target_os = "linux")]
                {
                    if Command::new("aplay").arg("-q").arg(&path).status().is_err() {
                        let _ = Command::new("paplay").arg(&path).status();
                    }
                }
            });
        }
    }
}

static SOUND_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();

fn cached_sound_path(kind: SoundKind) -> Option<PathBuf> {
    let dir = SOUND_DIR
        .get_or_init(|| {
            let dir = std::env::temp_dir().join("monolith_chess_sounds_v1");
            if fs::create_dir_all(&dir).is_err() {
                return None;
            }
            for k in [
                SoundKind::Move,
                SoundKind::Capture,
                SoundKind::Check,
                SoundKind::Castle,
                SoundKind::Promote,
                SoundKind::GameEnd,
            ] {
                let p = dir.join(k.filename());
                if !p.exists() {
                    let wav = synthesize_wav(k);
                    let _ = fs::write(&p, wav);
                }
            }
            Some(dir)
        })
        .as_ref()?;

    let path = dir.join(kind.filename());
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Synthesizes a 16-bit mono 44.1 kHz PCM WAV buffer for the given chess event.
pub fn synthesize_wav(kind: SoundKind) -> Vec<u8> {
    const SAMPLE_RATE: u32 = 44_100;
    let duration_ms: u32 = match kind {
        SoundKind::Move => 65,
        SoundKind::Capture => 90,
        SoundKind::Check => 130,
        SoundKind::Castle => 150,
        SoundKind::Promote => 150,
        SoundKind::GameEnd => 260,
    };
    let num_samples = (SAMPLE_RATE * duration_ms / 1000) as usize;
    let mut pcm: Vec<i16> = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let t = i as f32 / SAMPLE_RATE as f32;
        let sample = match kind {
            SoundKind::Move => wood_thock(t, 195.0, 115.0, 0.0) * 0.72,
            SoundKind::Capture => {
                let s1 = wood_thock(t, 250.0, 120.0, 0.0) * 0.80;
                let s2 = if t > 0.014 {
                    wood_thock(t - 0.014, 340.0, 150.0, 0.0) * 0.45
                } else {
                    0.0
                };
                s1 + s2
            }
            SoundKind::Castle => {
                let s1 = wood_thock(t, 190.0, 110.0, 0.0) * 0.65;
                let s2 = if t > 0.068 {
                    wood_thock(t - 0.068, 215.0, 125.0, 0.0) * 0.70
                } else {
                    0.0
                };
                s1 + s2
            }
            SoundKind::Check => {
                let knock = wood_thock(t, 220.0, 130.0, 0.0) * 0.55;
                let ping =
                    (t * 2.0 * std::f32::consts::PI * 880.0).sin() * (-t * 32.0).exp() * 0.30;
                knock + ping
            }
            SoundKind::Promote => {
                let s1 = wood_thock(t, 240.0, 140.0, 0.0) * 0.50;
                let chime1 =
                    (t * 2.0 * std::f32::consts::PI * 523.25).sin() * (-t * 26.0).exp() * 0.28;
                let chime2 = if t > 0.045 {
                    ((t - 0.045) * 2.0 * std::f32::consts::PI * 659.25).sin()
                        * (-(t - 0.045) * 22.0).exp()
                        * 0.30
                } else {
                    0.0
                };
                s1 + chime1 + chime2
            }
            SoundKind::GameEnd => {
                let c1 = (t * 2.0 * std::f32::consts::PI * 440.0).sin() * (-t * 14.0).exp() * 0.25;
                let c2 = if t > 0.06 {
                    ((t - 0.06) * 2.0 * std::f32::consts::PI * 554.37).sin()
                        * (-(t - 0.06) * 12.0).exp()
                        * 0.25
                } else {
                    0.0
                };
                c1 + c2
            }
        };
        let clamped = sample.clamp(-0.95, 0.95);
        pcm.push((clamped * i16::MAX as f32) as i16);
    }

    encode_wav(SAMPLE_RATE, &pcm)
}

fn wood_thock(t: f32, f_start: f32, f_end: f32, _phase: f32) -> f32 {
    if t < 0.0 {
        return 0.0;
    }
    let freq = f_end + (f_start - f_end) * (-t * 120.0).exp();
    let body = (t * 2.0 * std::f32::consts::PI * freq).sin() * (-t * 85.0).exp();
    let click = (t * 2.0 * std::f32::consts::PI * 1150.0).sin() * (-t * 320.0).exp() * 0.40;
    body + click
}

fn encode_wav(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
    let data_bytes = (samples.len() * 2) as u32;
    let riff_size = 36 + data_bytes;
    let mut out = Vec::with_capacity(44 + data_bytes as usize);

    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    out.extend_from_slice(&1u16.to_le_bytes()); // Mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // Byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // Block align
    out.extend_from_slice(&16u16.to_le_bytes()); // Bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
    for &s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

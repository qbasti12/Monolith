//! Multi-engine UCI slot configuration, process lifecycle, and live telemetry state.

use std::time::Duration;

use monolith_core::movegen::parse_uci_move;
use monolith_core::pgn::move_to_san;
use monolith_core::{Board, Color, Move, Square, STARTPOS_FEN};
use monolith_engine::uci::{UciEvent, UciProcess};

use crate::ipc::{now_timestamp_ms, IpcMessage, IpcSender};

pub const MAX_ENGINES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayRole {
    Analyze,
    PlayBlack,
    PlayWhite,
}

impl PlayRole {
    pub fn next(self) -> Self {
        match self {
            Self::Analyze => Self::PlayBlack,
            Self::PlayBlack => Self::PlayWhite,
            Self::PlayWhite => Self::Analyze,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Analyze => "Analyse",
            Self::PlayBlack => "Engine spielt Schwarz",
            Self::PlayWhite => "Engine spielt Weiß",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EngineSlotConfig {
    pub enabled: bool,
    pub label: String,
    pub binary_path: String,
    pub elo: u32,
    pub limit_strength: bool,
    pub depth: u8,
}

#[derive(Debug, Clone)]
pub struct EngineTelemetry {
    pub connected: bool,
    pub searching: bool,
    pub error: Option<String>,
    pub reported_name: String,
    pub depth: u8,
    pub seldepth: u8,
    pub white_score_cp: Option<i32>,
    pub white_score_mate: Option<i32>,
    pub nodes: u64,
    pub nps: u64,
    pub hashfull: u32,
    pub best_move_uci: Option<String>,
    pub best_move_san: Option<String>,
    pub expected_reply_san: Option<String>,
    pub pv_san: String,
    pub best_move_squares: Option<(Square, Square)>,
}

impl EngineTelemetry {
    pub fn cleared_for_position(&mut self) {
        self.depth = 0;
        self.seldepth = 0;
        self.white_score_cp = None;
        self.white_score_mate = None;
        self.nodes = 0;
        self.nps = 0;
        self.best_move_uci = None;
        self.best_move_san = None;
        self.expected_reply_san = None;
        self.pv_san.clear();
        self.best_move_squares = None;
    }
}

impl Default for EngineTelemetry {
    fn default() -> Self {
        Self {
            connected: false,
            searching: false,
            error: None,
            reported_name: "Monolith".to_string(),
            depth: 0,
            seldepth: 0,
            white_score_cp: None,
            white_score_mate: None,
            nodes: 0,
            nps: 0,
            hashfull: 0,
            best_move_uci: None,
            best_move_san: None,
            expected_reply_san: None,
            pv_san: String::new(),
            best_move_squares: None,
        }
    }
}

pub struct EngineSlot {
    pub config: EngineSlotConfig,
    pub telemetry: EngineTelemetry,
    pub process: Option<UciProcess>,
    pub last_position_seq: u64,
}

/// Resolves the default `monolith-engine` binary next to `monolith-tui`, falling back to `PATH`.
pub fn default_engine_binary() -> String {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(if cfg!(windows) {
                "monolith-engine.exe"
            } else {
                "monolith-engine"
            });
            if candidate.exists() {
                return candidate.to_string_lossy().into_owned();
            }
        }
    }
    "monolith-engine".to_string()
}

pub fn default_slots(engine_override: Option<String>) -> Vec<EngineSlot> {
    let bin = engine_override.unwrap_or_else(default_engine_binary);
    vec![
        EngineSlot {
            config: EngineSlotConfig {
                enabled: true,
                label: "Engine 1".to_string(),
                binary_path: bin.clone(),
                elo: 2400,
                limit_strength: false,
                depth: 14,
            },
            telemetry: EngineTelemetry::default(),
            process: None,
            last_position_seq: 0,
        },
        EngineSlot {
            config: EngineSlotConfig {
                enabled: false,
                label: "Engine 2".to_string(),
                binary_path: bin.clone(),
                elo: 1800,
                limit_strength: true,
                depth: 12,
            },
            telemetry: EngineTelemetry::default(),
            process: None,
            last_position_seq: 0,
        },
        EngineSlot {
            config: EngineSlotConfig {
                enabled: false,
                label: "Engine 3".to_string(),
                binary_path: bin,
                elo: 1200,
                limit_strength: true,
                depth: 10,
            },
            telemetry: EngineTelemetry::default(),
            process: None,
            last_position_seq: 0,
        },
    ]
}

/// Reconciles running engine child processes with slot configuration.
pub fn sync_slots(slots: &mut [EngineSlot]) {
    for slot in slots {
        if !slot.config.enabled {
            if let Some(mut proc) = slot.process.take() {
                proc.shutdown();
            }
            slot.telemetry.connected = false;
            slot.telemetry.searching = false;
            continue;
        }

        if slot.process.is_none() {
            match UciProcess::spawn(&slot.config.binary_path, &[]) {
                Ok(mut proc) => match proc.init_uci(Duration::from_secs(2)) {
                    Ok(id) => {
                        slot.telemetry.reported_name = id;
                        slot.telemetry.connected = true;
                        slot.telemetry.error = None;
                        slot.process = Some(proc);
                    }
                    Err(err) => {
                        slot.telemetry.connected = false;
                        slot.telemetry.error = Some(err.to_string());
                    }
                },
                Err(err) => {
                    slot.telemetry.connected = false;
                    slot.telemetry.error = Some(err.to_string());
                }
            }
        }

        if let Some(proc) = &mut slot.process {
            let _ = proc.set_option(
                "UCI_LimitStrength",
                if slot.config.limit_strength {
                    "true"
                } else {
                    "false"
                },
            );
            let _ = proc.set_option("UCI_Elo", &slot.config.elo.to_string());
            let _ = proc.is_ready(Duration::from_secs(2));
        }
    }
}

pub fn start_searches(
    slots: &mut [EngineSlot],
    start_fen: &str,
    moves: &[String],
    outcome_over: bool,
    seq: u64,
) {
    let fen_opt = if start_fen == STARTPOS_FEN {
        None
    } else {
        Some(start_fen)
    };

    for slot in slots {
        if !slot.config.enabled {
            continue;
        }
        let Some(proc) = &mut slot.process else {
            continue;
        };
        let _ = proc.stop();
        if outcome_over {
            slot.telemetry.searching = false;
            continue;
        }
        if proc.set_position(fen_opt, moves).is_ok()
            && proc.go(&format!("depth {}", slot.config.depth)).is_ok()
        {
            slot.telemetry.searching = true;
            slot.last_position_seq = seq;
        } else {
            slot.telemetry.connected = false;
        }
    }
}

pub fn poll_slots(
    slots: &mut [EngineSlot],
    board: &Board,
    current_seq: u64,
    play_role: PlayRole,
    at_latest_ply: bool,
    ipc: Option<&IpcSender>,
) -> Option<Move> {
    let stm = board.side_to_move();
    let mut engine_auto_move: Option<Move> = None;

    for (slot_idx, slot) in slots.iter_mut().enumerate() {
        let mut batch = Vec::new();
        if let Some(proc) = &slot.process {
            for _ in 0..64 {
                match proc.try_recv() {
                    Some(ev) => batch.push(ev),
                    None => break,
                }
            }
        }

        let is_current = slot.last_position_seq == current_seq;
        for ev in batch {
            match ev {
                UciEvent::RawLine(raw) => {
                    if let Some(tx) = ipc {
                        let _ = tx.send(&IpcMessage::UciLine {
                            slot: slot_idx,
                            engine: slot.telemetry.reported_name.clone(),
                            line: raw,
                            timestamp_ms: now_timestamp_ms(),
                        });
                    }
                }
                UciEvent::Info(info) if is_current => {
                    if info.multipv.unwrap_or(1) != 1 {
                        continue;
                    }
                    if let Some(d) = info.depth {
                        slot.telemetry.depth = d;
                    }
                    if let Some(sd) = info.seldepth {
                        slot.telemetry.seldepth = sd;
                    }
                    if let Some(cp) = info.score_cp {
                        slot.telemetry.white_score_cp =
                            Some(if stm == Color::White { cp } else { -cp });
                        slot.telemetry.white_score_mate = None;
                    }
                    if let Some(mate) = info.score_mate {
                        slot.telemetry.white_score_mate =
                            Some(if stm == Color::White { mate } else { -mate });
                        slot.telemetry.white_score_cp = None;
                    }
                    if let Some(n) = info.nodes {
                        slot.telemetry.nodes = n;
                    }
                    if let Some(nps) = info.nps {
                        slot.telemetry.nps = nps;
                    }
                    if let Some(hf) = info.hashfull {
                        slot.telemetry.hashfull = hf;
                    }
                    if !info.pv.is_empty() {
                        update_pv_display(board, &info.pv, &mut slot.telemetry);
                    }
                    if let Some(tx) = ipc {
                        let _ = tx.send(&IpcMessage::EngineInfo {
                            slot: slot_idx,
                            name: slot.telemetry.reported_name.clone(),
                            elo: slot.config.elo,
                            limit_strength: slot.config.limit_strength,
                            depth: slot.telemetry.depth,
                            seldepth: slot.telemetry.seldepth,
                            score_cp: slot.telemetry.white_score_cp,
                            score_mate: slot.telemetry.white_score_mate,
                            nodes: slot.telemetry.nodes,
                            nps: slot.telemetry.nps,
                            hashfull: slot.telemetry.hashfull,
                            pv: info.pv.clone(),
                            timestamp_ms: now_timestamp_ms(),
                        });
                    }
                }
                UciEvent::BestMove { best_move, .. } => {
                    slot.telemetry.searching = false;
                    if is_current && best_move != "0000" {
                        if let Ok(mv) = parse_uci_move(board, &best_move) {
                            slot.telemetry.best_move_uci = Some(best_move.clone());
                            slot.telemetry.best_move_squares = Some((mv.from_sq(), mv.to_sq()));
                            if slot_idx == 0 && at_latest_ply {
                                let should_play = matches!(
                                    (play_role, stm),
                                    (PlayRole::PlayBlack, Color::Black)
                                        | (PlayRole::PlayWhite, Color::White)
                                );
                                if should_play {
                                    engine_auto_move = Some(mv);
                                }
                            }
                        }
                    }
                }
                UciEvent::Exited(_) => {
                    slot.telemetry.connected = false;
                    slot.telemetry.searching = false;
                }
                _ => {}
            }
        }
    }

    engine_auto_move
}

pub fn shutdown_slots(slots: &mut [EngineSlot]) {
    for slot in slots {
        if let Some(mut proc) = slot.process.take() {
            proc.shutdown();
        }
    }
}

fn update_pv_display(board: &Board, uci_pv: &[String], telemetry: &mut EngineTelemetry) {
    let mut b = board.clone();
    let mut san_list = Vec::with_capacity(uci_pv.len().min(8));
    for (idx, tok) in uci_pv.iter().take(8).enumerate() {
        if let Ok(mv) = parse_uci_move(&b, tok) {
            let san = move_to_san(&b, mv).unwrap_or_else(|_| tok.clone());
            if idx == 0 {
                telemetry.best_move_uci = Some(tok.clone());
                telemetry.best_move_san = Some(san.clone());
                telemetry.best_move_squares = Some((mv.from_sq(), mv.to_sq()));
            } else if idx == 1 {
                telemetry.expected_reply_san = Some(san.clone());
            }
            san_list.push(san);
            b.make_move(mv);
        } else {
            san_list.push(tok.clone());
            break;
        }
    }
    telemetry.pv_san = san_list.join(" ");
}

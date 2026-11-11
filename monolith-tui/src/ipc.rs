//! UDP JSON-Lines IPC protocol for client-to-logs telemetry (`127.0.0.1:42424`).

use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const DEFAULT_IPC_HOST: &str = "127.0.0.1";
pub const DEFAULT_IPC_PORT: u16 = 42424;
pub const MAX_PACKET_BYTES: usize = 1400;

#[derive(Debug, Error)]
pub enum IpcError {
    #[error("IPC socket I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("invalid UTF-8 in telemetry datagram: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("malformed telemetry JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("telemetry packet exceeded {MAX_PACKET_BYTES} byte MTU even after truncation")]
    PacketTooLarge,
}

/// Returns current UNIX epoch milliseconds for telemetry timestamps.
pub fn now_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcMessage {
    Hello {
        client_version: String,
        timestamp_ms: u64,
    },
    Bye {
        timestamp_ms: u64,
    },
    Position {
        fen: String,
        last_move: Option<String>,
        ply: usize,
        timestamp_ms: u64,
    },
    EngineInfo {
        slot: usize,
        name: String,
        elo: u32,
        limit_strength: bool,
        depth: u8,
        seldepth: u8,
        score_cp: Option<i32>,
        score_mate: Option<i32>,
        nodes: u64,
        nps: u64,
        hashfull: u32,
        pv: Vec<String>,
        timestamp_ms: u64,
    },
    UciLine {
        slot: usize,
        engine: String,
        line: String,
        timestamp_ms: u64,
    },
    EvalBreakdown {
        fen: String,
        white_pov_cp: i32,
        stm_pov_cp: i32,
        material_cp: i32,
        pst_cp: i32,
        pawn_structure_cp: i32,
        king_safety_cp: i32,
        mobility_cp: i32,
        bishop_pair_cp: i32,
        phase: i32,
        timestamp_ms: u64,
    },
}

/// Serializes `msg` to a single JSON-Line strictly smaller than [`MAX_PACKET_BYTES`] bytes.
///
/// Oversized `pv` vectors or `UciLine` payloads are deterministically truncated so UDP
/// transmission stays fragmentation-free.
pub fn encode_packet(msg: &IpcMessage) -> Result<Vec<u8>, IpcError> {
    let mut candidate = msg.clone();

    loop {
        let mut bytes = serde_json::to_vec(&candidate)?;
        bytes.push(b'\n');
        if bytes.len() < MAX_PACKET_BYTES {
            return Ok(bytes);
        }

        match &mut candidate {
            IpcMessage::EngineInfo { pv, name, .. } => {
                if !pv.is_empty() {
                    let half = pv.len() / 2;
                    pv.truncate(half);
                    continue;
                }
                if name.len() > 32 {
                    name.truncate(32);
                    continue;
                }
                return Err(IpcError::PacketTooLarge);
            }
            IpcMessage::UciLine { line, engine, .. } => {
                if line.len() > 64 {
                    let target = (line.len() / 2).max(64);
                    truncate_utf8_with_ellipsis(line, target);
                    continue;
                }
                if engine.len() > 32 {
                    engine.truncate(32);
                    continue;
                }
                return Err(IpcError::PacketTooLarge);
            }
            _ => return Err(IpcError::PacketTooLarge),
        }
    }
}

/// Decodes one UDP JSON-Lines payload without panicking on invalid UTF-8 or malformed JSON.
pub fn decode_packet(bytes: &[u8]) -> Result<IpcMessage, IpcError> {
    let text = std::str::from_utf8(bytes)?.trim();
    if text.is_empty() {
        return Err(serde_json::from_str::<IpcMessage>("").unwrap_err().into());
    }
    Ok(serde_json::from_str(text)?)
}

fn truncate_utf8_with_ellipsis(s: &mut String, max_bytes: usize) {
    if s.len() <= max_bytes {
        return;
    }
    let mut cut = max_bytes.saturating_sub(3);
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    s.truncate(cut);
    s.push_str("...");
}

/// Non-blocking fire-and-forget UDP JSON-Lines sender bound to an ephemeral local port.
#[derive(Debug)]
pub struct IpcSender {
    socket: UdpSocket,
    target: SocketAddr,
}

impl IpcSender {
    /// Creates an unconnected non-blocking UDP sender targeting `127.0.0.1:{port}`.
    pub fn new(port: u16) -> Result<Self, IpcError> {
        let socket = UdpSocket::bind(("127.0.0.1", 0))?;
        socket.set_nonblocking(true)?;
        let target: SocketAddr = format!("{DEFAULT_IPC_HOST}:{port}")
            .parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        Ok(Self { socket, target })
    }

    pub fn target(&self) -> SocketAddr {
        self.target
    }

    /// Encodes and sends `msg` cleanly. Absent/closed receivers never block or fail the sender.
    pub fn send(&self, msg: &IpcMessage) -> Result<(), IpcError> {
        let payload = encode_packet(msg)?;
        match self.socket.send_to(&payload, self.target) {
            Ok(_) => Ok(()),
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::ConnectionRefused
                ) =>
            {
                Ok(())
            }
            Err(err) => Err(IpcError::Io(err)),
        }
    }
}

/// Non-blocking UDP JSON-Lines receiver listening on `127.0.0.1:{port}`.
#[derive(Debug)]
pub struct IpcReceiver {
    socket: UdpSocket,
    buf: [u8; 2048],
}

impl IpcReceiver {
    /// Binds to `127.0.0.1:{port}` (`port == 0` chooses an ephemeral port).
    pub fn bind(port: u16) -> Result<Self, IpcError> {
        let socket = UdpSocket::bind((DEFAULT_IPC_HOST, port))?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            buf: [0u8; 2048],
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, IpcError> {
        Ok(self.socket.local_addr()?)
    }

    /// Receives up to `max_packets` datagrams without blocking, decoding each individually.
    pub fn poll(&mut self, max_packets: usize) -> Vec<Result<IpcMessage, IpcError>> {
        let mut out = Vec::new();
        for _ in 0..max_packets {
            match self.socket.recv_from(&mut self.buf) {
                Ok((len, _addr)) => {
                    let packet = &self.buf[..len];
                    for raw_line in packet.split(|&b| b == b'\n') {
                        if raw_line.iter().all(|b| b.is_ascii_whitespace()) {
                            continue;
                        }
                        out.push(decode_packet(raw_line));
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
                Err(err) => {
                    out.push(Err(IpcError::Io(err)));
                    break;
                }
            }
        }
        out
    }
}

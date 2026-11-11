//! UCI protocol server loop and external UCI child-process runner (`UciProcess`).

use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use monolith_core::movegen::parse_uci_move;
use monolith_core::types::Move;
use monolith_core::Board;
use thiserror::Error;

use crate::search::{Score, SearchLimits, Searcher};
use crate::skill::{select_move, SkillProfile, SkillRng, MAX_ELO, MIN_ELO};
use crate::tt::{DEFAULT_TT_MB, MAX_TT_MB, MIN_TT_MB};

/// Errors produced by [`UciProcess`] when spawning or communicating with a UCI engine.
#[derive(Debug, Error)]
pub enum UciError {
    #[error("failed to spawn or communicate with UCI engine: {0}")]
    Io(#[from] io::Error),
    #[error("UCI engine timed out waiting for '{0}'")]
    Timeout(String),
    #[error("UCI engine process exited unexpectedly")]
    Disconnected,
}

#[derive(Debug, Clone)]
struct EngineOptions {
    hash_mb: usize,
    multi_pv: usize,
    limit_strength: bool,
    uci_elo: u32,
    skill_depth: u8,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            hash_mb: DEFAULT_TT_MB,
            multi_pv: 1,
            limit_strength: false,
            uci_elo: 2400,
            skill_depth: 64,
        }
    }
}

enum WorkerCmd {
    NewGame,
    SetPosition(Box<Board>),
    SetOption { name: String, value: String },
    Go(Box<SearchLimits>),
    Sync(Sender<()>),
    Quit,
}

/// Runs the Monolith UCI protocol loop reading from `stdin` and writing to `stdout`.
pub fn run_uci_stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_uci_loop(stdin.lock(), stdout)
}

/// Runs the UCI protocol loop over arbitrary input/output streams.
pub fn run_uci_loop<R: BufRead, W: Write + Send + 'static>(reader: R, writer: W) -> io::Result<()> {
    let out = Arc::new(Mutex::new(BufWriter::new(writer)));
    let stop_flag = Arc::new(AtomicBool::new(false));
    let searching = Arc::new(AtomicBool::new(false));
    let (cmd_tx, cmd_rx) = mpsc::channel::<WorkerCmd>();

    let worker_handle = spawn_worker(
        cmd_rx,
        Arc::clone(&out),
        Arc::clone(&stop_flag),
        Arc::clone(&searching),
    );

    for line_res in reader.lines() {
        let Ok(line) = line_res else {
            break;
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut tokens = trimmed.split_whitespace();
        let Some(cmd) = tokens.next() else {
            continue;
        };

        match cmd {
            "uci" => {
                write_line(
                    &out,
                    &format!("id name Monolith {}", env!("CARGO_PKG_VERSION")),
                );
                write_line(&out, "id author Monolith Contributors");
                write_line(
                    &out,
                    &format!(
                        "option name Hash type spin default {} min {} max {}",
                        DEFAULT_TT_MB, MIN_TT_MB, MAX_TT_MB
                    ),
                );
                write_line(&out, "option name MultiPV type spin default 1 min 1 max 16");
                write_line(
                    &out,
                    "option name UCI_LimitStrength type check default false",
                );
                write_line(
                    &out,
                    &format!(
                        "option name UCI_Elo type spin default 2400 min {} max {}",
                        MIN_ELO, MAX_ELO
                    ),
                );
                write_line(
                    &out,
                    "option name Skill_Depth type spin default 64 min 1 max 64",
                );
                write_line(&out, "uciok");
            }
            "isready" => {
                if searching.load(Ordering::Acquire) {
                    write_line(&out, "readyok");
                } else {
                    let (ack_tx, ack_rx) = mpsc::channel();
                    if cmd_tx.send(WorkerCmd::Sync(ack_tx)).is_ok() {
                        let _ = ack_rx.recv();
                    }
                    write_line(&out, "readyok");
                }
            }
            "ucinewgame" => {
                stop_flag.store(true, Ordering::Relaxed);
                let _ = cmd_tx.send(WorkerCmd::NewGame);
            }
            "setoption" => {
                if let Some((name, value)) = parse_setoption(trimmed) {
                    stop_flag.store(true, Ordering::Relaxed);
                    let _ = cmd_tx.send(WorkerCmd::SetOption { name, value });
                }
            }
            "position" => {
                if let Some(board) = parse_position(trimmed) {
                    stop_flag.store(true, Ordering::Relaxed);
                    let _ = cmd_tx.send(WorkerCmd::SetPosition(Box::new(board)));
                }
            }
            "go" => {
                stop_flag.store(true, Ordering::Relaxed);
                let limits = parse_go(trimmed);
                let _ = cmd_tx.send(WorkerCmd::Go(Box::new(limits)));
            }
            "stop" => {
                stop_flag.store(true, Ordering::Relaxed);
            }
            "quit" => {
                stop_flag.store(true, Ordering::Relaxed);
                let _ = cmd_tx.send(WorkerCmd::Quit);
                break;
            }
            _ => {
                // Unknown UCI commands are safely ignored per specification.
            }
        }
    }

    stop_flag.store(true, Ordering::Relaxed);
    let _ = cmd_tx.send(WorkerCmd::Quit);
    let _ = worker_handle.join();
    Ok(())
}

fn spawn_worker<W: Write + Send + 'static>(
    cmd_rx: Receiver<WorkerCmd>,
    out: Arc<Mutex<BufWriter<W>>>,
    stop_flag: Arc<AtomicBool>,
    searching: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut searcher = Searcher::new(DEFAULT_TT_MB);
        searcher.set_stop_flag(Arc::clone(&stop_flag));
        let mut board = Board::startpos();
        let mut opts = EngineOptions::default();
        let mut rng = SkillRng::new(0x517C_C1B7_2722_0A95);

        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                WorkerCmd::NewGame => {
                    searcher.clear_state();
                    board = Board::startpos();
                }
                WorkerCmd::SetPosition(new_board) => {
                    board = *new_board;
                }
                WorkerCmd::SetOption { name, value } => {
                    apply_option(&mut opts, &mut searcher, &name, &value);
                }
                WorkerCmd::Sync(ack) => {
                    let _ = ack.send(());
                }
                WorkerCmd::Go(mut limits) => {
                    stop_flag.store(false, Ordering::Release);
                    searching.store(true, Ordering::Release);

                    limits.multi_pv = limits.multi_pv.max(opts.multi_pv);
                    limits.max_depth = limits.max_depth.min(opts.skill_depth);

                    let profile = SkillProfile::for_elo(opts.uci_elo);
                    if opts.limit_strength {
                        profile.apply_to_limits(&mut limits);
                    }

                    let mut board_copy = board.clone();
                    let result = searcher.search(&mut board_copy, &limits, |info| {
                        let score_str = match Score::from_raw(info.score) {
                            Score::Cp(cp) => format!("cp {}", cp),
                            Score::Mate(m) => format!("mate {}", m),
                        };
                        let pv_str = info
                            .pv
                            .as_slice()
                            .iter()
                            .map(|m| m.to_uci())
                            .collect::<Vec<_>>()
                            .join(" ");
                        write_line(
                            &out,
                            &format!(
                                "info depth {} seldepth {} multipv {} score {} nodes {} nps {} hashfull {} time {} pv {}",
                                info.depth,
                                info.seldepth,
                                info.multipv,
                                score_str,
                                info.nodes,
                                info.nps,
                                info.hashfull,
                                info.time_ms,
                                pv_str
                            ),
                        );
                    });

                    let (best_mv, ponder_mv) = if opts.limit_strength {
                        select_move(profile, &result, &mut rng)
                    } else {
                        (result.best_move, result.ponder_move)
                    };

                    if best_mv.is_none() {
                        write_line(&out, "bestmove 0000");
                    } else if !ponder_mv.is_none() && !opts.limit_strength {
                        write_line(
                            &out,
                            &format!(
                                "bestmove {} ponder {}",
                                best_mv.to_uci(),
                                ponder_mv.to_uci()
                            ),
                        );
                    } else {
                        write_line(&out, &format!("bestmove {}", best_mv.to_uci()));
                    }

                    searching.store(false, Ordering::Release);
                }
                WorkerCmd::Quit => break,
            }
        }
    })
}

fn apply_option(opts: &mut EngineOptions, searcher: &mut Searcher, name: &str, value: &str) {
    match name.to_ascii_lowercase().as_str() {
        "hash" => {
            if let Ok(mb) = value.trim().parse::<usize>() {
                let clamped = mb.clamp(MIN_TT_MB, MAX_TT_MB);
                opts.hash_mb = clamped;
                searcher.tt_mut().resize(clamped);
            }
        }
        "multipv" => {
            if let Ok(k) = value.trim().parse::<usize>() {
                opts.multi_pv = k.clamp(1, crate::search::MAX_MULTI_PV);
            }
        }
        "uci_limitstrength" => {
            if let Ok(flag) = value.trim().to_ascii_lowercase().parse::<bool>() {
                opts.limit_strength = flag;
            }
        }
        "uci_elo" => {
            if let Ok(elo) = value.trim().parse::<u32>() {
                opts.uci_elo = elo.clamp(MIN_ELO, MAX_ELO);
            }
        }
        "skill_depth" => {
            if let Ok(d) = value.trim().parse::<u8>() {
                opts.skill_depth = d.clamp(1, 64);
            }
        }
        _ => {}
    }
}

fn write_line<W: Write>(out: &Arc<Mutex<BufWriter<W>>>, line: &str) {
    if let Ok(mut guard) = out.lock() {
        let _ = writeln!(guard, "{}", line);
        let _ = guard.flush();
    }
}

fn parse_setoption(line: &str) -> Option<(String, String)> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let name_pos = tokens
        .iter()
        .position(|&t| t.eq_ignore_ascii_case("name"))?;
    let val_pos = tokens.iter().position(|&t| t.eq_ignore_ascii_case("value"));

    let name = match val_pos {
        Some(vp) if vp > name_pos + 1 => tokens[name_pos + 1..vp].join(" "),
        Some(_) => return None,
        None => tokens[name_pos + 1..].join(" "),
    };
    if name.is_empty() {
        return None;
    }
    let value = match val_pos {
        Some(vp) if vp + 1 < tokens.len() => tokens[vp + 1..].join(" "),
        _ => String::new(),
    };
    Some((name, value))
}

fn parse_position(line: &str) -> Option<Board> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 2 {
        return None;
    }

    let moves_pos = tokens.iter().position(|&t| t == "moves");
    let pos_end = moves_pos.unwrap_or(tokens.len());

    let mut board = if tokens[1] == "startpos" {
        Board::startpos()
    } else if tokens[1] == "fen" {
        if pos_end < 8 {
            return None;
        }
        let fen_str = tokens[2..pos_end].join(" ");
        Board::from_fen(&fen_str).ok()?
    } else {
        return None;
    };

    if let Some(mp) = moves_pos {
        for mv_str in &tokens[mp + 1..] {
            if let Ok(mv) = parse_uci_move(&board, mv_str) {
                board.make_move(mv);
            } else {
                return None;
            }
        }
    }

    Some(board)
}

fn parse_go(line: &str) -> SearchLimits {
    let mut limits = SearchLimits::default();
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let mut i = 1usize;

    while i < tokens.len() {
        match tokens[i] {
            "depth" => {
                if let Some(v) = tokens.get(i + 1).and_then(|s| s.parse::<u8>().ok()) {
                    limits.max_depth = v.max(1);
                }
                i += 2;
            }
            "movetime" => {
                if let Some(ms) = tokens.get(i + 1).and_then(|s| s.parse::<u64>().ok()) {
                    limits.movetime = Some(Duration::from_millis(ms));
                }
                i += 2;
            }
            "nodes" => {
                if let Some(n) = tokens.get(i + 1).and_then(|s| s.parse::<u64>().ok()) {
                    limits.max_nodes = Some(n.max(1));
                }
                i += 2;
            }
            "wtime" => {
                if let Some(ms) = tokens.get(i + 1).and_then(|s| s.parse::<u64>().ok()) {
                    limits.wtime = Some(Duration::from_millis(ms));
                }
                i += 2;
            }
            "btime" => {
                if let Some(ms) = tokens.get(i + 1).and_then(|s| s.parse::<u64>().ok()) {
                    limits.btime = Some(Duration::from_millis(ms));
                }
                i += 2;
            }
            "winc" => {
                if let Some(ms) = tokens.get(i + 1).and_then(|s| s.parse::<u64>().ok()) {
                    limits.winc = Some(Duration::from_millis(ms));
                }
                i += 2;
            }
            "binc" => {
                if let Some(ms) = tokens.get(i + 1).and_then(|s| s.parse::<u64>().ok()) {
                    limits.binc = Some(Duration::from_millis(ms));
                }
                i += 2;
            }
            "movestogo" => {
                if let Some(n) = tokens.get(i + 1).and_then(|s| s.parse::<u32>().ok()) {
                    limits.movestogo = Some(n.max(1));
                }
                i += 2;
            }
            "infinite" => {
                limits.infinite = true;
                i += 1;
            }
            "searchmoves" => {
                i += 1;
                while i < tokens.len()
                    && !matches!(
                        tokens[i],
                        "depth"
                            | "movetime"
                            | "nodes"
                            | "wtime"
                            | "btime"
                            | "winc"
                            | "binc"
                            | "movestogo"
                            | "infinite"
                            | "ponder"
                    )
                {
                    if let Ok(mv) = parse_uci_move_unvalidated(tokens[i]) {
                        limits.search_moves.push(mv);
                    }
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }

    limits
}

fn parse_uci_move_unvalidated(s: &str) -> Result<Move, ()> {
    if s.len() != 4 && s.len() != 5 {
        return Err(());
    }
    let from = s[0..2].parse().map_err(|_| ())?;
    let to = s[2..4].parse().map_err(|_| ())?;
    if s.len() == 5 {
        let pt = match s.as_bytes()[4].to_ascii_lowercase() {
            b'n' => monolith_core::types::PieceType::Knight,
            b'b' => monolith_core::types::PieceType::Bishop,
            b'r' => monolith_core::types::PieceType::Rook,
            b'q' => monolith_core::types::PieceType::Queen,
            _ => return Err(()),
        };
        Ok(Move::promotion(from, to, pt, false))
    } else {
        Ok(Move::new(from, to, Move::FLAG_QUIET))
    }
}

/// Parsed UCI `info` line payload.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UciInfo {
    pub depth: Option<u8>,
    pub seldepth: Option<u8>,
    pub multipv: Option<usize>,
    pub score_cp: Option<i32>,
    pub score_mate: Option<i32>,
    pub nodes: Option<u64>,
    pub nps: Option<u64>,
    pub hashfull: Option<u32>,
    pub time_ms: Option<u64>,
    pub pv: Vec<String>,
}

/// Structured event emitted by an external UCI engine via [`UciProcess`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UciEvent {
    RawLine(String),
    IdName(String),
    UciOk,
    ReadyOk,
    Info(UciInfo),
    BestMove {
        best_move: String,
        ponder: Option<String>,
    },
    Exited(Option<i32>),
}

/// Parses a single UCI `info ...` line into a [`UciInfo`] struct.
pub fn parse_uci_info_line(line: &str) -> Option<UciInfo> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.first().copied() != Some("info") {
        return None;
    }
    if tokens.get(1).copied() == Some("string") {
        return None;
    }

    let mut info = UciInfo::default();
    let mut i = 1usize;
    while i < tokens.len() {
        match tokens[i] {
            "depth" => {
                info.depth = tokens.get(i + 1).and_then(|s| s.parse().ok());
                i += 2;
            }
            "seldepth" => {
                info.seldepth = tokens.get(i + 1).and_then(|s| s.parse().ok());
                i += 2;
            }
            "multipv" => {
                info.multipv = tokens.get(i + 1).and_then(|s| s.parse().ok());
                i += 2;
            }
            "score" => match tokens.get(i + 1).copied() {
                Some("cp") => {
                    info.score_cp = tokens.get(i + 2).and_then(|s| s.parse().ok());
                    i += 3;
                }
                Some("mate") => {
                    info.score_mate = tokens.get(i + 2).and_then(|s| s.parse().ok());
                    i += 3;
                }
                _ => i += 1,
            },
            "nodes" => {
                info.nodes = tokens.get(i + 1).and_then(|s| s.parse().ok());
                i += 2;
            }
            "nps" => {
                info.nps = tokens.get(i + 1).and_then(|s| s.parse().ok());
                i += 2;
            }
            "hashfull" => {
                info.hashfull = tokens.get(i + 1).and_then(|s| s.parse().ok());
                i += 2;
            }
            "time" => {
                info.time_ms = tokens.get(i + 1).and_then(|s| s.parse().ok());
                i += 2;
            }
            "pv" => {
                info.pv = tokens[i + 1..].iter().map(|s| (*s).to_string()).collect();
                break;
            }
            _ => i += 1,
        }
    }

    Some(info)
}

/// Manages an external UCI engine child process and streams parsed events over `mpsc`.
pub struct UciProcess {
    child: Child,
    stdin: ChildStdin,
    events: Receiver<UciEvent>,
    reader_thread: Option<JoinHandle<()>>,
    name: String,
}

impl UciProcess {
    /// Spawns a UCI binary (`program`) and starts its background stdout reader thread.
    pub fn spawn(program: &str, args: &[&str]) -> Result<Self, UciError> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = child.stdin.take().ok_or(UciError::Disconnected)?;
        let stdout = child.stdout.take().ok_or(UciError::Disconnected)?;

        let (tx, rx) = mpsc::channel();
        let reader_thread = thread::spawn(move || {
            let buf = BufReader::new(stdout);
            for line_res in buf.lines() {
                let Ok(line) = line_res else {
                    break;
                };
                let trimmed = line.trim().to_string();
                if trimmed.is_empty() {
                    continue;
                }
                let _ = tx.send(UciEvent::RawLine(trimmed.clone()));
                if trimmed == "uciok" {
                    let _ = tx.send(UciEvent::UciOk);
                } else if trimmed == "readyok" {
                    let _ = tx.send(UciEvent::ReadyOk);
                } else if let Some(rest) = trimmed.strip_prefix("id name ") {
                    let _ = tx.send(UciEvent::IdName(rest.trim().to_string()));
                } else if let Some(info) = parse_uci_info_line(&trimmed) {
                    let _ = tx.send(UciEvent::Info(info));
                } else if trimmed.starts_with("bestmove ") {
                    let mut parts = trimmed.split_whitespace();
                    let _ = parts.next();
                    if let Some(bm) = parts.next() {
                        let ponder = match (parts.next(), parts.next()) {
                            (Some("ponder"), Some(p)) => Some(p.to_string()),
                            _ => None,
                        };
                        let _ = tx.send(UciEvent::BestMove {
                            best_move: bm.to_string(),
                            ponder,
                        });
                    }
                }
            }
            let _ = tx.send(UciEvent::Exited(None));
        });

        Ok(Self {
            child,
            stdin,
            events: rx,
            reader_thread: Some(reader_thread),
            name: program.to_string(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Sends a single raw UCI line to the engine.
    pub fn send_command(&mut self, cmd: &str) -> Result<(), UciError> {
        writeln!(self.stdin, "{}", cmd)?;
        self.stdin.flush()?;
        Ok(())
    }

    /// Performs the initial `uci` handshake and records the reported `id name`.
    pub fn init_uci(&mut self, timeout: Duration) -> Result<String, UciError> {
        self.send_command("uci")?;
        let deadline = Instant::now() + timeout;
        while let Some(rem) = deadline.checked_duration_since(Instant::now()) {
            match self.events.recv_timeout(rem) {
                Ok(UciEvent::IdName(n)) => self.name = n,
                Ok(UciEvent::UciOk) => return Ok(self.name.clone()),
                Ok(UciEvent::Exited(_)) => return Err(UciError::Disconnected),
                Ok(_) => {}
                Err(_) => break,
            }
        }
        Err(UciError::Timeout("uciok".to_string()))
    }

    /// Sends `isready` and waits up to `timeout` for `readyok`.
    pub fn is_ready(&mut self, timeout: Duration) -> Result<(), UciError> {
        self.send_command("isready")?;
        let deadline = Instant::now() + timeout;
        while let Some(rem) = deadline.checked_duration_since(Instant::now()) {
            match self.events.recv_timeout(rem) {
                Ok(UciEvent::ReadyOk) => return Ok(()),
                Ok(UciEvent::Exited(_)) => return Err(UciError::Disconnected),
                Ok(_) => {}
                Err(_) => break,
            }
        }
        Err(UciError::Timeout("readyok".to_string()))
    }

    pub fn set_option(&mut self, name: &str, value: &str) -> Result<(), UciError> {
        self.send_command(&format!("setoption name {} value {}", name, value))
    }

    pub fn set_position(&mut self, fen: Option<&str>, moves: &[String]) -> Result<(), UciError> {
        let mut cmd = match fen {
            Some(f) => format!("position fen {}", f),
            None => "position startpos".to_string(),
        };
        if !moves.is_empty() {
            cmd.push_str(" moves ");
            cmd.push_str(&moves.join(" "));
        }
        self.send_command(&cmd)
    }

    pub fn go(&mut self, params: &str) -> Result<(), UciError> {
        if params.trim().is_empty() {
            self.send_command("go")
        } else {
            self.send_command(&format!("go {}", params.trim()))
        }
    }

    pub fn stop(&mut self) -> Result<(), UciError> {
        self.send_command("stop")
    }

    pub fn try_recv(&self) -> Option<UciEvent> {
        self.events.try_recv().ok()
    }

    pub fn recv_timeout(&self, timeout: Duration) -> Option<UciEvent> {
        self.events.recv_timeout(timeout).ok()
    }

    /// Gracefully terminates the child engine process and joins the reader thread.
    pub fn shutdown(&mut self) {
        let _ = self.send_command("quit");
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(150) {
            if let Ok(Some(_)) = self.child.try_wait() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(handle) = self.reader_thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for UciProcess {
    fn drop(&mut self) {
        self.shutdown();
    }
}

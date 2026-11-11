//! Telemetry log viewer application (`--mode logs`) with 5000-line ring buffer and live metrics.

use std::collections::VecDeque;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::ipc::{decode_packet, IpcError, IpcMessage, IpcReceiver};
use crate::theme::{format_eval_short, Theme};

pub const LOG_RING_CAPACITY: usize = 5000;

#[derive(Debug, Clone, Default)]
pub struct EngineMetricRow {
    pub active: bool,
    pub name: String,
    pub elo: u32,
    pub limit_strength: bool,
    pub depth: u8,
    pub seldepth: u8,
    pub score_cp: Option<i32>,
    pub score_mate: Option<i32>,
    pub nodes: u64,
    pub nps: u64,
    pub hashfull: u32,
    pub pv: String,
}

#[derive(Debug, Clone, Default)]
pub struct EvalSummary {
    pub fen: String,
    pub last_move: Option<String>,
    pub ply: usize,
    pub white_pov_cp: i32,
    pub stm_pov_cp: i32,
    pub material_cp: i32,
    pub pst_cp: i32,
    pub pawn_structure_cp: i32,
    pub king_safety_cp: i32,
    pub mobility_cp: i32,
    pub bishop_pair_cp: i32,
    pub phase: i32,
}

#[derive(Debug, Clone)]
pub enum LogKind {
    Uci,
    Info,
    Position,
    Session,
    Error,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub slot: Option<usize>,
    pub kind: LogKind,
    pub text: String,
}

pub struct LogsApp {
    pub theme: Theme,
    pub port: u16,
    pub should_quit: bool,
    pub auto_follow: bool,
    pub scroll_offset: usize,
    pub filter_slot: Option<usize>,
    pub packet_count: u64,
    pub malformed_count: u64,
    pub engine_metrics: [EngineMetricRow; 3],
    pub eval_summary: EvalSummary,
    pub log_buffer: VecDeque<LogEntry>,
    receiver: Option<IpcReceiver>,
}

impl LogsApp {
    /// Creates a live telemetry log viewer bound to UDP `127.0.0.1:{port}`.
    pub fn bind(port: u16) -> Result<Self, IpcError> {
        let receiver = IpcReceiver::bind(port)?;
        let actual_port = receiver.local_addr().map(|a| a.port()).unwrap_or(port);
        Ok(Self::from_receiver(actual_port, Some(receiver)))
    }

    /// Creates a headless telemetry viewer without binding a network socket (for tests).
    pub fn new_headless(port: u16) -> Self {
        Self::from_receiver(port, None)
    }

    fn from_receiver(port: u16, receiver: Option<IpcReceiver>) -> Self {
        Self {
            theme: Theme::detect(),
            port,
            should_quit: false,
            auto_follow: true,
            scroll_offset: 0,
            filter_slot: None,
            packet_count: 0,
            malformed_count: 0,
            engine_metrics: Default::default(),
            eval_summary: EvalSummary::default(),
            log_buffer: VecDeque::with_capacity(LOG_RING_CAPACITY),
            receiver,
        }
    }

    pub fn local_port(&self) -> u16 {
        self.port
    }

    /// Drains pending UDP telemetry datagrams without blocking.
    pub fn tick(&mut self) {
        let packets = match &mut self.receiver {
            Some(rx) => rx.poll(128),
            None => return,
        };
        for item in packets {
            match item {
                Ok(msg) => self.apply_message(msg),
                Err(err) => self.record_malformed(&err),
            }
        }
    }

    /// Decodes raw bytes as a JSON-Line telemetry message (used in tests and direct feeds).
    pub fn process_packet_bytes(&mut self, bytes: &[u8]) {
        match decode_packet(bytes) {
            Ok(msg) => self.apply_message(msg),
            Err(err) => self.record_malformed(&err),
        }
    }

    fn record_malformed(&mut self, err: &IpcError) {
        self.malformed_count += 1;
        self.push_log(LogEntry {
            slot: None,
            kind: LogKind::Error,
            text: format!("Fehlerhaftes Paket ignoriert: {err}"),
        });
    }

    pub fn apply_message(&mut self, msg: IpcMessage) {
        self.packet_count += 1;
        match msg {
            IpcMessage::Hello {
                client_version,
                timestamp_ms,
            } => {
                self.push_log(LogEntry {
                    slot: None,
                    kind: LogKind::Session,
                    text: format!(
                        "[{ts}] Client verbunden (Monolith v{client_version})",
                        ts = format_ts(timestamp_ms)
                    ),
                });
            }
            IpcMessage::Bye { timestamp_ms } => {
                self.push_log(LogEntry {
                    slot: None,
                    kind: LogKind::Session,
                    text: format!("[{}] Client beendet.", format_ts(timestamp_ms)),
                });
            }
            IpcMessage::Position {
                fen,
                last_move,
                ply,
                timestamp_ms,
            } => {
                self.eval_summary.fen = fen.clone();
                self.eval_summary.last_move = last_move.clone();
                self.eval_summary.ply = ply;
                let mv_txt = last_move.as_deref().unwrap_or("Start");
                self.push_log(LogEntry {
                    slot: None,
                    kind: LogKind::Position,
                    text: format!(
                        "[{}] Pos ply={ply} ({mv_txt}): {fen}",
                        format_ts(timestamp_ms)
                    ),
                });
            }
            IpcMessage::EngineInfo {
                slot,
                name,
                elo,
                limit_strength,
                depth,
                seldepth,
                score_cp,
                score_mate,
                nodes,
                nps,
                hashfull,
                pv,
                timestamp_ms,
            } => {
                if let Some(row) = self.engine_metrics.get_mut(slot) {
                    row.active = true;
                    row.name = name.clone();
                    row.elo = elo;
                    row.limit_strength = limit_strength;
                    row.depth = depth;
                    row.seldepth = seldepth;
                    row.score_cp = score_cp;
                    row.score_mate = score_mate;
                    row.nodes = nodes;
                    row.nps = nps;
                    row.hashfull = hashfull;
                    row.pv = pv.join(" ");
                }
                let eval_str = format_eval_short(score_cp, score_mate);
                self.push_log(LogEntry {
                    slot: Some(slot),
                    kind: LogKind::Info,
                    text: format!(
                        "[{}] [E{}] {name} d{depth}/{seldepth} {eval_str} n={nodes} ({}) pv {}",
                        format_ts(timestamp_ms),
                        slot + 1,
                        format_nps(nps),
                        pv.join(" ")
                    ),
                });
            }
            IpcMessage::UciLine {
                slot,
                engine,
                line,
                timestamp_ms,
            } => {
                self.push_log(LogEntry {
                    slot: Some(slot),
                    kind: LogKind::Uci,
                    text: format!(
                        "[{}] [E{} {}] {line}",
                        format_ts(timestamp_ms),
                        slot + 1,
                        engine
                    ),
                });
            }
            IpcMessage::EvalBreakdown {
                fen,
                white_pov_cp,
                stm_pov_cp,
                material_cp,
                pst_cp,
                pawn_structure_cp,
                king_safety_cp,
                mobility_cp,
                bishop_pair_cp,
                phase,
                ..
            } => {
                self.eval_summary.fen = fen;
                self.eval_summary.white_pov_cp = white_pov_cp;
                self.eval_summary.stm_pov_cp = stm_pov_cp;
                self.eval_summary.material_cp = material_cp;
                self.eval_summary.pst_cp = pst_cp;
                self.eval_summary.pawn_structure_cp = pawn_structure_cp;
                self.eval_summary.king_safety_cp = king_safety_cp;
                self.eval_summary.mobility_cp = mobility_cp;
                self.eval_summary.bishop_pair_cp = bishop_pair_cp;
                self.eval_summary.phase = phase;
            }
        }
    }

    fn push_log(&mut self, entry: LogEntry) {
        if self.log_buffer.len() >= LOG_RING_CAPACITY {
            self.log_buffer.pop_front();
            if !self.auto_follow {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
        }
        self.log_buffer.push_back(entry);
    }

    pub fn filtered_len(&self) -> usize {
        self.log_buffer
            .iter()
            .filter(|e| self.filter_slot.is_none_or(|s| e.slot == Some(s)))
            .count()
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'q'))
        {
            self.should_quit = true;
            return;
        }

        let total = self.filtered_len();
        match key.code {
            KeyCode::Char('q' | 'Q') => self.should_quit = true,
            KeyCode::Char('f' | 'F' | ' ') => {
                self.auto_follow = !self.auto_follow;
                if self.auto_follow {
                    self.scroll_offset = total;
                }
            }
            KeyCode::Char('c' | 'C') => {
                self.log_buffer.clear();
                self.scroll_offset = 0;
            }
            KeyCode::Char('0') => self.filter_slot = None,
            KeyCode::Char('1') => self.filter_slot = Some(0),
            KeyCode::Char('2') => self.filter_slot = Some(1),
            KeyCode::Char('3') => self.filter_slot = Some(2),
            KeyCode::Up => {
                self.auto_follow = false;
                self.scroll_offset = self.scroll_offset.min(total).saturating_sub(1);
            }
            KeyCode::Down => {
                self.scroll_offset = (self.scroll_offset + 1).min(total);
                if self.scroll_offset >= total {
                    self.auto_follow = true;
                }
            }
            KeyCode::PageUp => {
                self.auto_follow = false;
                self.scroll_offset = self.scroll_offset.min(total).saturating_sub(15);
            }
            KeyCode::PageDown => {
                self.scroll_offset = (self.scroll_offset + 15).min(total);
                if self.scroll_offset >= total {
                    self.auto_follow = true;
                }
            }
            KeyCode::Home => {
                self.auto_follow = false;
                self.scroll_offset = 0;
            }
            KeyCode::End => {
                self.auto_follow = true;
                self.scroll_offset = total;
            }
            _ => {}
        }
    }
}

/// Renders the `--mode logs` live telemetry dashboard onto `frame`.
pub fn render_logs(frame: &mut Frame, app: &LogsApp) {
    let t = &app.theme;
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(t.base).fg(t.text)),
        area,
    );

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(9),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(area);

    let follow_badge = if app.auto_follow {
        Span::styled(
            " AUTO-FOLLOW AN ",
            Style::default().bg(t.green).fg(t.crust).bold(),
        )
    } else {
        Span::styled(
            " SCROLL PAUSIERT ",
            Style::default().bg(t.yellow).fg(t.crust).bold(),
        )
    };
    let header = Line::from(vec![
        Span::styled(
            format!(" MONOLITH TELEMETRY LOGS (UDP 127.0.0.1:{}) ", app.port),
            Style::default().bg(t.sapphire).fg(t.crust).bold(),
        ),
        Span::raw(" "),
        follow_badge,
        Span::styled(
            format!(
                "  Pakete: {}  Fehlerhaft: {}  Buffer: {}/{}",
                app.packet_count,
                app.malformed_count,
                app.log_buffer.len(),
                LOG_RING_CAPACITY
            ),
            Style::default().fg(t.subtext0),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(header).style(Style::default().bg(t.mantle)),
        root[0],
    );

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(root[1]);

    render_engine_metrics(frame, app, top[0]);
    render_eval_breakdown(frame, app, top[1]);
    render_log_stream(frame, app, root[2]);

    let filter_txt = match app.filter_slot {
        None => "Alle",
        Some(0) => "E1",
        Some(1) => "E2",
        Some(2) => "E3",
        _ => "?",
    };
    let footer = Paragraph::new(Line::from(Span::styled(
        format!(
            "[↑/↓/PgUp/PgDn/Home/End] Scrollen  [F] Auto-Follow  [0/1/2/3] Filter ({filter_txt})  [C] Leeren  [Q] Quit"
        ),
        Style::default().fg(t.subtext0),
    )))
    .style(Style::default().bg(t.mantle));
    frame.render_widget(footer, root[3]);
}

fn render_engine_metrics(frame: &mut Frame, app: &LogsApp, area: Rect) {
    let t = &app.theme;
    let block = Block::default()
        .title(" Live-Engine-Metriken ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.blue))
        .style(Style::default().bg(t.mantle));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![Line::from(Span::styled(
        "Slot Name              Depth   Eval     Nodes       NPS        Hash   PV",
        Style::default().fg(t.subtext0).bold(),
    ))];

    for (idx, row) in app.engine_metrics.iter().enumerate() {
        if !row.active {
            lines.push(Line::from(Span::styled(
                format!(" E{}  (inaktiv)", idx + 1),
                Style::default().fg(t.overlay1),
            )));
            continue;
        }
        let eval_str = format_eval_short(row.score_cp, row.score_mate);
        let name_short: String = row.name.chars().take(16).collect();
        let line = format!(
            " E{}  {:<16} {:>2}/{:<2}  {:>6}  {:>9}  {:>9}  {:>4.1}%  {}",
            idx + 1,
            name_short,
            row.depth,
            row.seldepth,
            eval_str,
            row.nodes,
            format_nps(row.nps),
            row.hashfull as f64 / 10.0,
            row.pv
        );
        lines.push(Line::from(Span::styled(line, Style::default().fg(t.text))));
    }

    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_eval_breakdown(frame: &mut Frame, app: &LogsApp, area: Rect) {
    let t = &app.theme;
    let block = Block::default()
        .title(" Eval-Breakdown (Weiß-POV) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.peach))
        .style(Style::default().bg(t.mantle));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let ev = &app.eval_summary;
    let lines = vec![
        Line::from(vec![
            Span::styled("Gesamt Weiß: ", Style::default().fg(t.subtext0)),
            Span::styled(
                format!("{:+.2} cp", ev.white_pov_cp as f64 / 100.0),
                Style::default().fg(t.yellow).bold(),
            ),
            Span::styled(
                format!("  Phase: {}/24", ev.phase),
                Style::default().fg(t.teal),
            ),
        ]),
        Line::from(Span::styled(
            format!(
                "Material: {:+}  PST: {:+}  Läuferpaar: {:+}",
                ev.material_cp, ev.pst_cp, ev.bishop_pair_cp
            ),
            Style::default().fg(t.text),
        )),
        Line::from(Span::styled(
            format!(
                "Bauern: {:+}  König: {:+}  Mobilität: {:+}",
                ev.pawn_structure_cp, ev.king_safety_cp, ev.mobility_cp
            ),
            Style::default().fg(t.text),
        )),
        Line::from(Span::styled(
            format!("FEN: {}", if ev.fen.is_empty() { "-" } else { &ev.fen }),
            Style::default().fg(t.subtext0),
        )),
    ];

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn render_log_stream(frame: &mut Frame, app: &LogsApp, area: Rect) {
    let t = &app.theme;
    let block = Block::default()
        .title(" UCI & Telemetrie-Log (Ringpuffer) ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.surface1))
        .style(Style::default().bg(t.mantle));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let visible: Vec<&LogEntry> = app
        .log_buffer
        .iter()
        .filter(|e| app.filter_slot.is_none_or(|s| e.slot == Some(s)))
        .collect();

    let height = inner.height as usize;
    let max_start = visible.len().saturating_sub(height);
    let start = if app.auto_follow {
        max_start
    } else {
        app.scroll_offset.min(max_start)
    };

    let lines: Vec<Line> = visible
        .into_iter()
        .skip(start)
        .take(height)
        .map(|e| {
            let color = match e.kind {
                LogKind::Uci => t.subtext1,
                LogKind::Info => t.green,
                LogKind::Position => t.sapphire,
                LogKind::Session => t.mauve,
                LogKind::Error => t.red,
            };
            Line::from(Span::styled(&e.text, Style::default().fg(color)))
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), inner);
}

fn format_ts(ts_ms: u64) -> String {
    let secs = (ts_ms / 1000) % 86400;
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    let ms = ts_ms % 1000;
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}

fn format_nps(nps: u64) -> String {
    if nps >= 1_000_000 {
        format!("{:.2} MN/s", nps as f64 / 1_000_000.0)
    } else if nps >= 1_000 {
        format!("{} kN/s", nps / 1_000)
    } else {
        format!("{nps} N/s")
    }
}

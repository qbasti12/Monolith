//! Application CLI entrypoint for GUI and TUI clients, panic guards, and terminal cleanup.

use std::env;
use std::io::{self, Stdout, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use thiserror::Error;

use crate::app::App;
use crate::gui;
use crate::logs::{render_logs, LogsApp};
use crate::ui;

pub const DEFAULT_IPC_PORT: u16 = 42424;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("invalid CLI arguments: {0}")]
    Args(String),
    #[error("{0}")]
    Runtime(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiKind {
    Gui,
    Tui,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Client,
    Logs,
}

#[derive(Debug, Clone)]
pub struct CliConfig {
    pub mode: RunMode,
    pub ui: UiKind,
    pub port: u16,
    pub fen: Option<String>,
    pub pgn_path: Option<String>,
    pub engine_path: Option<String>,
    pub show_help: bool,
    pub show_version: bool,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            mode: RunMode::Client,
            ui: UiKind::Gui,
            port: DEFAULT_IPC_PORT,
            fen: None,
            pgn_path: None,
            engine_path: None,
            show_help: false,
            show_version: false,
        }
    }
}

pub fn parse_args<I>(args: I) -> Result<CliConfig, CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut cfg = CliConfig::default();
    let mut it = args.into_iter();
    let _prog = it.next();

    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => cfg.show_help = true,
            "-V" | "--version" => cfg.show_version = true,
            "-gui" | "--gui" => cfg.ui = UiKind::Gui,
            "-tui" | "--tui" => cfg.ui = UiKind::Tui,
            "--mode" => {
                let val = it
                    .next()
                    .ok_or_else(|| CliError::Args("--mode requires 'client' or 'logs'".into()))?;
                cfg.mode = match val.as_str() {
                    "client" => RunMode::Client,
                    "logs" => RunMode::Logs,
                    other => {
                        return Err(CliError::Args(format!(
                            "unknown mode '{other}', expected 'client' or 'logs'"
                        )))
                    }
                };
            }
            "--port" => {
                let val = it.next().ok_or_else(|| {
                    CliError::Args("--port requires a TCP/UDP port 1..=65535".into())
                })?;
                let p: u16 = val
                    .parse()
                    .map_err(|_| CliError::Args(format!("invalid port '{val}'")))?;
                if p == 0 {
                    return Err(CliError::Args("port must be >= 1".into()));
                }
                cfg.port = p;
            }
            "--fen" => {
                let val = it
                    .next()
                    .ok_or_else(|| CliError::Args("--fen requires a FEN string".into()))?;
                cfg.fen = Some(val);
            }
            "--pgn" => {
                let val = it
                    .next()
                    .ok_or_else(|| CliError::Args("--pgn requires a file path".into()))?;
                cfg.pgn_path = Some(val);
            }
            "--engine" => {
                let val = it
                    .next()
                    .ok_or_else(|| CliError::Args("--engine requires a binary path".into()))?;
                cfg.engine_path = Some(val);
            }
            other => {
                return Err(CliError::Args(format!(
                    "unknown argument '{other}' (try --help)"
                )))
            }
        }
    }

    Ok(cfg)
}

pub fn print_help() {
    let mut out = io::stdout().lock();
    let _ = writeln!(
        out,
        "Monolith TUI & GUI Chess Studio v{}
Usage: monolith [OPTIONS]

Options:
  -gui, --gui            Launch native graphical GUI client with mouse support (default)
  -tui, --tui            Launch interactive terminal client (TUI)
  --mode <client|logs>   Select interactive chess client or live telemetry viewer (default: client)
  --port <PORT>          UDP IPC telemetry port on 127.0.0.1 (default: {DEFAULT_IPC_PORT})
  --fen <FEN>            Start client with a specific FEN position
  --pgn <PATH>           Load and analyze a PGN file on startup
  --engine <PATH>        Override default Engine 1 UCI binary path
  -h, --help             Print this help message and exit
  -V, --version          Print version information and exit",
        env!("CARGO_PKG_VERSION")
    );
    let _ = out.flush();
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    fn init() -> Result<Self, CliError> {
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore_terminal();
            prev_hook(info);
        }));

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, Hide)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

pub fn run_client(cfg: &CliConfig) -> Result<(), CliError> {
    let mut app = App::new(cfg.engine_path.clone(), true);
    let _ = app.enable_ipc(cfg.port);
    if let Some(fen) = &cfg.fen {
        app.load_fen(fen).map_err(CliError::Runtime)?;
    } else if let Some(pgn_path) = &cfg.pgn_path {
        app.load_pgn_file(pgn_path, true)
            .map_err(CliError::Runtime)?;
    }

    let mut guard = TerminalGuard::init()?;
    let tick_rate = Duration::from_millis(50);
    let mut last_tick = Instant::now();

    while !app.should_quit {
        guard.terminal.draw(|f| ui::render(f, &app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key);
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.tick();
            last_tick = Instant::now();
        }
    }

    app.shutdown();
    Ok(())
}

pub fn run_logs(cfg: &CliConfig) -> Result<(), CliError> {
    let mut logs_app = LogsApp::bind(cfg.port).map_err(|e| CliError::Runtime(e.to_string()))?;
    let mut guard = TerminalGuard::init()?;
    let tick_rate = Duration::from_millis(50);
    let mut last_tick = Instant::now();

    while !logs_app.should_quit {
        guard.terminal.draw(|f| render_logs(f, &logs_app))?;

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    logs_app.handle_key(key);
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            logs_app.tick();
            last_tick = Instant::now();
        }
    }

    Ok(())
}

pub fn run() -> Result<(), CliError> {
    let cfg = parse_args(env::args())?;
    if cfg.show_version {
        let mut out = io::stdout().lock();
        let _ = writeln!(out, "monolith-tui {}", env!("CARGO_PKG_VERSION"));
        let _ = out.flush();
        return Ok(());
    }
    if cfg.show_help {
        print_help();
        return Ok(());
    }

    match cfg.mode {
        RunMode::Logs => run_logs(&cfg),
        RunMode::Client => match cfg.ui {
            UiKind::Gui => {
                match gui::run_gui(
                    cfg.engine_path.clone(),
                    cfg.fen.clone(),
                    cfg.pgn_path.clone(),
                    cfg.port,
                ) {
                    Ok(()) => Ok(()),
                    Err(err) => {
                        eprintln!(
                            "Grafikfenster konnte nicht geöffnet werden ({err}). Wechsle in Terminal-Modus..."
                        );
                        run_client(&cfg)
                    }
                }
            }
            UiKind::Tui => run_client(&cfg),
        },
    }
}

pub fn entrypoint() -> ExitCode {
    if let Err(err) = run() {
        eprintln!("monolith error: {err}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

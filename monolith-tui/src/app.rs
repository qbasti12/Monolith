//! Application state machine, chess interaction, and PGN navigation.

use std::sync::mpsc::TryRecvError;

use monolith_core::movegen::{generate_legal, parse_uci_move};
use monolith_core::pgn::{move_to_san, parse_pgn, parse_san, MoveClassification, PgnGame};
use monolith_core::{Board, Color, Move, MoveList, PieceType, Square, STARTPOS_FEN};
use monolith_engine::eval::{eval_breakdown, evaluate};

use crate::analysis::{start_pgn_analysis, AnalysisEvent, PgnAnalysisHandle};
use crate::engines::{
    default_slots, poll_slots, shutdown_slots, start_searches, sync_slots, EngineSlot, PlayRole,
};
use crate::ipc::{now_timestamp_ms, IpcError, IpcMessage, IpcSender};
use crate::theme::Theme;

pub const FEN_PRESET_STANDARD: &str = STARTPOS_FEN;
pub const FEN_PRESET_INFINITE_QUEENS: &str = "4k3/8/8/8/8/8/QQQQQQQQ/QQQ1KQQQ w - - 0 1";
pub const FEN_PRESET_KIWIPETE: &str =
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";

/// Built-in sample PGN (Morphy Opera Game) demonstrating tactics, sacrifice, and blunder analysis.
pub const SAMPLE_OPERA_PGN: &str = r#"[Event "Paris Opera"]
[White "Paul Morphy"]
[Black "Duke of Brunswick & Count Isouard"]
[Result "1-0"]

1. e4 e5 2. Nf3 d6 3. d4 Bg4 4. dxe5 Bxf3 5. Qxf3 dxe5 6. Bc4 Nf6
7. Qb3 Qe7 8. Nc3 c6 9. Bg5 b5 10. Nxb5 cxb5 11. Bxb5+ Nbd7 12. O-O-O Rd8
13. Rxd7 Rxd7 14. Rd1 Qe6 15. Bxd7+ Nxd7 16. Qb8+ Nxb8 17. Rd8# 1-0"#;

#[derive(Debug, Clone)]
pub struct MoveRecord {
    pub mv: Move,
    pub san: String,
    pub uci: String,
    pub move_number: u16,
    pub side: Color,
    pub classification: Option<MoveClassification>,
    pub best_hint_san: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFocus {
    Board,
    MoveBox,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextModalState {
    pub input: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    None,
    Fen(TextModalState),
    Pgn(TextModalState),
    Engines {
        selected_slot: usize,
        selected_field: usize,
        editing_path: bool,
        path_buffer: String,
    },
    Help,
    Promotion {
        from: Square,
        to: Square,
    },
}

#[derive(Debug, Clone)]
pub struct PgnProgress {
    pub title: String,
    pub running: bool,
    pub done_plies: usize,
    pub total_plies: usize,
    pub review_navigation: bool,
}

pub struct App {
    pub theme: Theme,
    pub should_quit: bool,
    pub auto_spawn_engines: bool,
    pub start_fen: String,
    pub board: Board,
    pub history: Vec<MoveRecord>,
    pub view_ply: usize,
    pub board_flipped: bool,
    pub cursor: Square,
    pub selected_sq: Option<Square>,
    pub legal_targets: Vec<Square>,
    pub input_focus: InputFocus,
    pub move_input: String,
    pub status_message: String,
    pub overlay: Overlay,
    pub play_role: PlayRole,
    pub engines: Vec<EngineSlot>,
    pub pgn_progress: Option<PgnProgress>,
    pub ipc_sender: Option<IpcSender>,
    pgn_handle: Option<PgnAnalysisHandle>,
    position_seq: u64,
    static_eval_white_cp: i32,
}

impl App {
    /// Creates a new TUI client state. Set `auto_spawn_engines = false` in headless tests.
    pub fn new(engine_override: Option<String>, auto_spawn_engines: bool) -> Self {
        let board = Board::startpos();
        let static_eval_white_cp = board_white_eval_cp(&board);

        let mut app = Self {
            theme: Theme::detect(),
            should_quit: false,
            auto_spawn_engines,
            start_fen: STARTPOS_FEN.to_string(),
            board,
            history: Vec::new(),
            view_ply: 0,
            board_flipped: false,
            cursor: Square::E2,
            selected_sq: None,
            legal_targets: Vec::new(),
            input_focus: InputFocus::Board,
            move_input: String::new(),
            status_message: "Bereit. [Pfeile+Enter] Zug  [/] Textzug  [?] Hilfe".to_string(),
            overlay: Overlay::None,
            play_role: PlayRole::Analyze,
            engines: default_slots(engine_override),
            pgn_progress: None,
            ipc_sender: None,
            pgn_handle: None,
            position_seq: 1,
            static_eval_white_cp,
        };

        if auto_spawn_engines {
            app.sync_engines_config();
            app.trigger_engine_searches();
        }
        app
    }

    /// Enables non-blocking UDP JSON-Lines telemetry emission to `127.0.0.1:{port}`.
    pub fn enable_ipc(&mut self, port: u16) -> Result<(), IpcError> {
        let sender = IpcSender::new(port)?;
        let _ = sender.send(&IpcMessage::Hello {
            client_version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp_ms: now_timestamp_ms(),
        });
        self.ipc_sender = Some(sender);
        self.emit_position_telemetry();
        Ok(())
    }

    pub fn new_headless() -> Self {
        Self::new(None, false)
    }

    fn rebuild_board_to_view(&mut self) {
        if let Ok(mut b) = Board::from_fen(&self.start_fen) {
            for rec in self.history.iter().take(self.view_ply) {
                b.make_move(rec.mv);
            }
            self.board = b;
            self.static_eval_white_cp = board_white_eval_cp(&self.board);
        }
        self.clear_selection();
    }

    pub(crate) fn clear_selection(&mut self) {
        self.selected_sq = None;
        self.legal_targets.clear();
    }

    /// Loads any valid FEN string, resetting move history and triggering fresh analysis.
    pub fn load_fen(&mut self, fen: &str) -> Result<(), String> {
        let board = Board::from_fen(fen.trim()).map_err(|e| format!("Ungültige FEN: {e}"))?;
        self.cancel_pgn_analysis();
        self.pgn_progress = None;
        self.start_fen = board.to_fen();
        self.board = board;
        self.history.clear();
        self.view_ply = 0;
        self.static_eval_white_cp = board_white_eval_cp(&self.board);
        self.clear_selection();
        self.status_message = "Position aus FEN geladen.".to_string();
        self.on_position_changed();
        Ok(())
    }

    /// Loads a PGN from a file path on disk (optionally starting background engine classification).
    pub fn load_pgn_file(&mut self, path: &str, start_analysis: bool) -> Result<(), String> {
        let text = std::fs::read_to_string(path.trim())
            .map_err(|e| format!("PGN-Datei konnte nicht gelesen werden: {e}"))?;
        self.load_pgn_str(&text, start_analysis)
    }

    /// Parses PGN text, populates game history at ply 0, and optionally analyzes it in background.
    pub fn load_pgn_str(&mut self, pgn_text: &str, start_analysis: bool) -> Result<(), String> {
        let game = parse_pgn(pgn_text).map_err(|e| format!("PGN-Fehler: {e}"))?;
        self.load_parsed_pgn(game, start_analysis)
    }

    fn load_parsed_pgn(&mut self, game: PgnGame, start_analysis: bool) -> Result<(), String> {
        self.cancel_pgn_analysis();

        let init_fen = if game.initial_fen.is_empty() {
            STARTPOS_FEN.to_string()
        } else {
            game.initial_fen.clone()
        };
        let init_board = Board::from_fen(&init_fen).map_err(|e| format!("PGN FEN-Fehler: {e}"))?;

        let white = game.header("White").unwrap_or("Weiß");
        let black = game.header("Black").unwrap_or("Schwarz");
        let title = format!("{white} vs {black}");
        let total = game.moves.len();

        let records = game
            .moves
            .iter()
            .map(|m| MoveRecord {
                mv: m.mv,
                san: m.san.clone(),
                uci: m.mv.to_uci(),
                move_number: m.move_number,
                side: m.side,
                classification: None,
                best_hint_san: None,
            })
            .collect();

        self.start_fen = init_fen;
        self.board = init_board;
        self.history = records;
        self.view_ply = 0;
        self.static_eval_white_cp = board_white_eval_cp(&self.board);
        self.clear_selection();

        self.pgn_progress = Some(PgnProgress {
            title: title.clone(),
            running: start_analysis && total > 0,
            done_plies: 0,
            total_plies: total,
            review_navigation: true,
        });

        if start_analysis && total > 0 {
            self.pgn_handle = Some(start_pgn_analysis(game, 6));
            self.status_message = format!("PGN geladen ({title}): Analyse läuft...");
        } else {
            self.status_message =
                format!("PGN geladen ({title}, {total} Halbzüge). [←/→] Navigieren");
        }

        self.on_position_changed();
        Ok(())
    }

    pub fn cancel_pgn_analysis(&mut self) {
        if let Some(mut handle) = self.pgn_handle.take() {
            handle.cancel();
        }
        if let Some(pg) = &mut self.pgn_progress {
            if pg.running {
                pg.running = false;
                self.status_message = "PGN-Analyse abgebrochen.".to_string();
            }
        }
    }

    /// Parses and applies a UCI (`e2e4`) or SAN (`Nf3`, `O-O`) move string on the current board.
    pub fn submit_move_text(&mut self, raw: &str) -> Result<(), String> {
        let token = raw.trim();
        if token.is_empty() {
            return Err("Leere Zugeingabe.".to_string());
        }
        let mv = parse_uci_move(&self.board, token)
            .or_else(|_| parse_san(&self.board, token))
            .map_err(|_| format!("Illegaler oder unbekannter Zug: '{token}'"))?;
        self.play_move(mv)
    }

    /// Plays a validated legal move (truncating redo history if stepping from an earlier ply).
    pub fn play_move(&mut self, mv: Move) -> Result<(), String> {
        let mut legal = MoveList::new();
        generate_legal(&self.board, &mut legal);
        if !legal.as_slice().contains(&mv) {
            return Err(format!("Illegaler Zug: {}", mv.to_uci()));
        }

        if self.view_ply < self.history.len() {
            self.history.truncate(self.view_ply);
            if let Some(pg) = &mut self.pgn_progress {
                pg.review_navigation = false;
            }
        }

        let san = move_to_san(&self.board, mv).map_err(|e| e.to_string())?;
        let move_number = self.board.fullmove_number();
        let side = self.board.side_to_move();

        self.board.make_move(mv);
        self.static_eval_white_cp = board_white_eval_cp(&self.board);
        self.history.push(MoveRecord {
            mv,
            san: san.clone(),
            uci: mv.to_uci(),
            move_number,
            side,
            classification: None,
            best_hint_san: None,
        });
        self.view_ply = self.history.len();
        self.cursor = mv.to_sq();
        self.clear_selection();
        self.status_message = format!("Zug gespielt: {san}");
        self.on_position_changed();
        Ok(())
    }

    pub fn undo_move(&mut self) -> bool {
        if self.view_ply == 0 {
            return false;
        }
        if self.view_ply == self.history.len() {
            self.history.pop();
        }
        self.view_ply = self.view_ply.saturating_sub(1);
        self.rebuild_board_to_view();
        self.status_message = "Zug zurückgenommen.".to_string();
        self.on_position_changed();
        true
    }

    pub fn step_backward(&mut self) -> bool {
        if self.view_ply == 0 {
            return false;
        }
        self.view_ply -= 1;
        self.rebuild_board_to_view();
        self.on_position_changed();
        true
    }

    pub fn step_forward(&mut self) -> bool {
        if self.view_ply >= self.history.len() {
            return false;
        }
        self.view_ply += 1;
        self.rebuild_board_to_view();
        self.on_position_changed();
        true
    }

    pub fn jump_start(&mut self) {
        self.view_ply = 0;
        self.rebuild_board_to_view();
        self.on_position_changed();
    }

    pub fn jump_end(&mut self) {
        self.view_ply = self.history.len();
        self.rebuild_board_to_view();
        self.on_position_changed();
    }

    pub fn flip_board(&mut self) {
        self.board_flipped = !self.board_flipped;
    }

    pub fn last_move_squares(&self) -> Option<(Square, Square)> {
        if self.view_ply == 0 {
            return None;
        }
        self.history
            .get(self.view_ply - 1)
            .map(|r| (r.mv.from_sq(), r.mv.to_sq()))
    }

    pub fn current_move_record(&self) -> Option<&MoveRecord> {
        if self.view_ply == 0 {
            None
        } else {
            self.history.get(self.view_ply - 1)
        }
    }

    /// Best White-POV eval across active engines (or static eval fallback).
    pub fn primary_eval_white(&self) -> (Option<i32>, Option<i32>) {
        for slot in &self.engines {
            if slot.config.enabled
                && (slot.telemetry.white_score_cp.is_some()
                    || slot.telemetry.white_score_mate.is_some())
            {
                return (
                    slot.telemetry.white_score_cp,
                    slot.telemetry.white_score_mate,
                );
            }
        }
        (Some(self.static_eval_white_cp), None)
    }

    pub fn game_outcome_label(&self) -> Option<&'static str> {
        let mut legal = MoveList::new();
        generate_legal(&self.board, &mut legal);
        if legal.is_empty() {
            return Some(if self.board.in_check() {
                match self.board.side_to_move() {
                    Color::White => "Schachmatt — Schwarz gewinnt",
                    Color::Black => "Schachmatt — Weiß gewinnt",
                }
            } else {
                "Patt — Remis"
            });
        }
        if self.board.is_threefold_repetition() {
            return Some("Remis durch dreimalige Stellungswiederholung");
        }
        if self.board.is_fifty_move_draw() {
            return Some("Remis durch 50-Züge-Regel");
        }
        if self.board.is_insufficient_material() {
            return Some("Remis durch unzureichendes Material");
        }
        None
    }

    pub(crate) fn matching_moves(&self, from: Square, to: Square) -> Vec<Move> {
        let mut legal = MoveList::new();
        generate_legal(&self.board, &mut legal);
        legal
            .as_slice()
            .iter()
            .copied()
            .filter(|m| m.from_sq() == from && m.to_sq() == to)
            .collect()
    }

    pub(crate) fn find_matching_move(
        &self,
        from: Square,
        to: Square,
        promo: Option<PieceType>,
    ) -> Option<Move> {
        self.matching_moves(from, to)
            .into_iter()
            .find(|m| m.promotion_piece() == promo)
    }

    fn emit_position_telemetry(&self) {
        let Some(tx) = &self.ipc_sender else {
            return;
        };
        let fen = self.board.to_fen();
        let last_move = self.current_move_record().map(|r| r.san.clone());
        let ts = now_timestamp_ms();
        let _ = tx.send(&IpcMessage::Position {
            fen: fen.clone(),
            last_move,
            ply: self.view_ply,
            timestamp_ms: ts,
        });
        let bd = eval_breakdown(&self.board);
        let _ = tx.send(&IpcMessage::EvalBreakdown {
            fen,
            white_pov_cp: bd.white_pov_cp,
            stm_pov_cp: bd.stm_pov_cp,
            material_cp: bd.material_cp,
            pst_cp: bd.pst_cp,
            pawn_structure_cp: bd.pawn_structure_cp,
            king_safety_cp: bd.king_safety_cp,
            mobility_cp: bd.mobility_cp,
            bishop_pair_cp: bd.bishop_pair_cp,
            phase: bd.phase,
            timestamp_ms: ts,
        });
    }

    fn on_position_changed(&mut self) {
        self.position_seq = self.position_seq.wrapping_add(1);
        for slot in &mut self.engines {
            slot.telemetry.cleared_for_position();
        }
        self.emit_position_telemetry();
        if self.auto_spawn_engines {
            self.trigger_engine_searches();
        }
    }

    pub fn sync_engines_config(&mut self) {
        sync_slots(&mut self.engines);
    }

    pub(crate) fn trigger_engine_searches(&mut self) {
        let moves: Vec<String> = self
            .history
            .iter()
            .take(self.view_ply)
            .map(|r| r.uci.clone())
            .collect();
        let over = self.game_outcome_label().is_some();
        start_searches(
            &mut self.engines,
            &self.start_fen,
            &moves,
            over,
            self.position_seq,
        );
    }

    /// Polls UCI engine output queues and background PGN analysis progress.
    pub fn tick(&mut self) {
        if let Some(mv) = poll_slots(
            &mut self.engines,
            &self.board,
            self.position_seq,
            self.play_role,
            self.view_ply == self.history.len(),
            self.ipc_sender.as_ref(),
        ) {
            let _ = self.play_move(mv);
        }
        self.poll_pgn_analysis();
    }

    fn poll_pgn_analysis(&mut self) {
        let mut disconnect = false;
        let mut drained = Vec::new();
        if let Some(handle) = &self.pgn_handle {
            for _ in 0..32 {
                match handle.rx.try_recv() {
                    Ok(ev) => drained.push(ev),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        disconnect = true;
                        break;
                    }
                }
            }
        }

        for ev in drained {
            match ev {
                AnalysisEvent::Started { total_plies } => {
                    if let Some(pg) = &mut self.pgn_progress {
                        pg.running = true;
                        pg.total_plies = total_plies;
                    }
                }
                AnalysisEvent::Step {
                    ply_index,
                    total_plies,
                    classification,
                    best_move_san,
                    ..
                } => {
                    if let Some(rec) = self.history.get_mut(ply_index) {
                        rec.classification = Some(classification);
                        rec.best_hint_san = best_move_san;
                    }
                    if let Some(pg) = &mut self.pgn_progress {
                        pg.done_plies = (ply_index + 1).min(total_plies);
                    }
                }
                AnalysisEvent::Finished => {
                    if let Some(pg) = &mut self.pgn_progress {
                        pg.running = false;
                        pg.done_plies = pg.total_plies;
                    }
                    self.status_message = "PGN-Analyse abgeschlossen.".to_string();
                    disconnect = true;
                }
                AnalysisEvent::Cancelled => {
                    if let Some(pg) = &mut self.pgn_progress {
                        pg.running = false;
                    }
                    disconnect = true;
                }
            }
        }

        if disconnect {
            self.pgn_handle = None;
        }
    }

    pub fn shutdown(&mut self) {
        self.cancel_pgn_analysis();
        shutdown_slots(&mut self.engines);
        if let Some(tx) = self.ipc_sender.take() {
            let _ = tx.send(&IpcMessage::Bye {
                timestamp_ms: now_timestamp_ms(),
            });
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn board_white_eval_cp(board: &Board) -> i32 {
    let stm_cp = evaluate(board);
    if board.side_to_move() == Color::White {
        stm_cp
    } else {
        -stm_cp
    }
}

//! Keyboard routing, board cursor navigation, and modal input handlers for [`App`].

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use monolith_core::movegen::generate_legal;
use monolith_core::{MoveList, PieceType, Square};
use monolith_engine::skill::{MAX_ELO, MIN_ELO};

use crate::app::{
    App, InputFocus, Overlay, TextModalState, FEN_PRESET_INFINITE_QUEENS, FEN_PRESET_KIWIPETE,
    FEN_PRESET_STANDARD, SAMPLE_OPERA_PGN,
};

impl App {
    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'q'))
        {
            self.should_quit = true;
            return;
        }

        match &self.overlay {
            Overlay::Help => {
                if matches!(
                    key.code,
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?') | KeyCode::Char('q')
                ) {
                    self.overlay = Overlay::None;
                }
                return;
            }
            Overlay::Promotion { from, to } => {
                let (from, to) = (*from, *to);
                self.handle_promotion_key(key, from, to);
                return;
            }
            Overlay::Fen(_) => {
                self.handle_fen_modal_key(key);
                return;
            }
            Overlay::Pgn(_) => {
                self.handle_pgn_modal_key(key);
                return;
            }
            Overlay::Engines { .. } => {
                self.handle_engine_modal_key(key);
                return;
            }
            Overlay::None => {}
        }

        if self.input_focus == InputFocus::MoveBox {
            match key.code {
                KeyCode::Esc | KeyCode::Tab => self.input_focus = InputFocus::Board,
                KeyCode::Backspace => {
                    self.move_input.pop();
                }
                KeyCode::Enter => {
                    let entered = std::mem::take(&mut self.move_input);
                    if let Err(err) = self.submit_move_text(&entered) {
                        self.set_error(err);
                    }
                }
                KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.move_input.push(c);
                }
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char('q' | 'Q') => self.should_quit = true,
            KeyCode::Char('?') => self.overlay = Overlay::Help,
            KeyCode::Char('f' | 'F') => {
                self.overlay = Overlay::Fen(TextModalState {
                    input: String::new(),
                    error: None,
                });
            }
            KeyCode::Char('p' | 'P') => {
                self.overlay = Overlay::Pgn(TextModalState {
                    input: String::new(),
                    error: None,
                });
            }
            KeyCode::Char('e' | 'E') => {
                self.overlay = Overlay::Engines {
                    selected_slot: 0,
                    selected_field: 0,
                    editing_path: false,
                    path_buffer: self.engines[0].config.binary_path.clone(),
                };
            }
            KeyCode::Char('/' | ':' | 'm' | 'M') | KeyCode::Tab => {
                self.input_focus = InputFocus::MoveBox;
            }
            KeyCode::Char('u' | 'U') => {
                self.undo_move();
            }
            KeyCode::Char('x' | 'X') => self.flip_board(),
            KeyCode::Char('c' | 'C') => {
                if self.pgn_progress.as_ref().is_some_and(|p| p.running) {
                    self.cancel_pgn_analysis();
                } else if let Some(pg) = &mut self.pgn_progress {
                    pg.review_navigation = !pg.review_navigation;
                }
            }
            KeyCode::Char('[') | KeyCode::PageUp => {
                self.step_backward();
            }
            KeyCode::Char(']') | KeyCode::PageDown => {
                self.step_forward();
            }
            KeyCode::Home => self.jump_start(),
            KeyCode::End => self.jump_end(),
            KeyCode::Left => {
                if self
                    .pgn_progress
                    .as_ref()
                    .is_some_and(|p| p.review_navigation)
                {
                    self.step_backward();
                } else {
                    self.move_cursor(-1, 0);
                }
            }
            KeyCode::Right => {
                if self
                    .pgn_progress
                    .as_ref()
                    .is_some_and(|p| p.review_navigation)
                {
                    self.step_forward();
                } else {
                    self.move_cursor(1, 0);
                }
            }
            KeyCode::Up => self.move_cursor(0, 1),
            KeyCode::Down => self.move_cursor(0, -1),
            KeyCode::Esc => self.clear_selection(),
            KeyCode::Enter | KeyCode::Char(' ') => self.activate_cursor_square(),
            _ => {}
        }
    }

    fn handle_promotion_key(&mut self, key: KeyEvent, from: Square, to: Square) {
        let promo = match key.code {
            KeyCode::Char('q' | 'Q' | '1') | KeyCode::Enter => Some(PieceType::Queen),
            KeyCode::Char('r' | 'R' | '2') => Some(PieceType::Rook),
            KeyCode::Char('b' | 'B' | '3') => Some(PieceType::Bishop),
            KeyCode::Char('n' | 'N' | '4') => Some(PieceType::Knight),
            KeyCode::Esc => {
                self.overlay = Overlay::None;
                return;
            }
            _ => None,
        };
        if let Some(pt) = promo {
            self.overlay = Overlay::None;
            if let Some(mv) = self.find_matching_move(from, to, Some(pt)) {
                if let Err(err) = self.play_move(mv) {
                    self.set_error(err);
                }
            }
        }
    }

    fn handle_fen_modal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.overlay = Overlay::None,
            KeyCode::F(1) => {
                self.overlay = Overlay::None;
                let _ = self.load_fen(FEN_PRESET_STANDARD);
            }
            KeyCode::F(2) => {
                self.overlay = Overlay::None;
                if let Err(err) = self.load_fen(FEN_PRESET_INFINITE_QUEENS) {
                    self.set_error(err);
                }
            }
            KeyCode::F(3) => {
                self.overlay = Overlay::None;
                if let Err(err) = self.load_fen(FEN_PRESET_KIWIPETE) {
                    self.set_error(err);
                }
            }
            KeyCode::Backspace => {
                if let Overlay::Fen(state) = &mut self.overlay {
                    state.input.pop();
                    state.error = None;
                }
            }
            KeyCode::Enter => {
                let candidate = match &self.overlay {
                    Overlay::Fen(state) => state.input.clone(),
                    _ => return,
                };
                match self.load_fen(&candidate) {
                    Ok(()) => self.overlay = Overlay::None,
                    Err(err) => {
                        if let Overlay::Fen(state) = &mut self.overlay {
                            state.error = Some(err);
                        }
                    }
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Overlay::Fen(state) = &mut self.overlay {
                    state.input.push(c);
                    state.error = None;
                }
            }
            _ => {}
        }
    }

    fn handle_pgn_modal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.overlay = Overlay::None,
            KeyCode::F(1) => {
                self.overlay = Overlay::None;
                if let Err(err) = self.load_pgn_str(SAMPLE_OPERA_PGN, true) {
                    self.set_error(err);
                }
            }
            KeyCode::Backspace => {
                if let Overlay::Pgn(state) = &mut self.overlay {
                    state.input.pop();
                    state.error = None;
                }
            }
            KeyCode::Enter => {
                let entered = match &self.overlay {
                    Overlay::Pgn(state) => state.input.clone(),
                    _ => return,
                };
                let trimmed = entered.trim();
                let res = if !trimmed.is_empty() && std::path::Path::new(trimmed).exists() {
                    self.load_pgn_file(trimmed, true)
                } else {
                    self.load_pgn_str(trimmed, true)
                };
                match res {
                    Ok(()) => self.overlay = Overlay::None,
                    Err(err) => {
                        if let Overlay::Pgn(state) = &mut self.overlay {
                            state.error = Some(err);
                        }
                    }
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Overlay::Pgn(state) = &mut self.overlay {
                    state.input.push(c);
                    state.error = None;
                }
            }
            _ => {}
        }
    }

    fn handle_engine_modal_key(&mut self, key: KeyEvent) {
        let mut close_and_apply = false;
        if let Overlay::Engines {
            selected_slot,
            selected_field,
            editing_path,
            path_buffer,
        } = &mut self.overlay
        {
            if *editing_path {
                match key.code {
                    KeyCode::Esc => *editing_path = false,
                    KeyCode::Backspace => {
                        path_buffer.pop();
                    }
                    KeyCode::Enter => {
                        let trimmed = path_buffer.trim();
                        if !trimmed.is_empty() {
                            self.engines[*selected_slot].config.binary_path = trimmed.to_string();
                        }
                        *editing_path = false;
                    }
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        path_buffer.push(c);
                    }
                    _ => {}
                }
                return;
            }

            match key.code {
                KeyCode::Esc => close_and_apply = true,
                KeyCode::Char('1') => {
                    *selected_slot = 0;
                    *path_buffer = self.engines[0].config.binary_path.clone();
                }
                KeyCode::Char('2') => {
                    *selected_slot = 1;
                    *path_buffer = self.engines[1].config.binary_path.clone();
                }
                KeyCode::Char('3') => {
                    *selected_slot = 2;
                    *path_buffer = self.engines[2].config.binary_path.clone();
                }
                KeyCode::Up => *selected_field = (*selected_field + 5) % 6,
                KeyCode::Down => *selected_field = (*selected_field + 1) % 6,
                KeyCode::Left => match *selected_field {
                    0 if *selected_slot > 0 => {
                        self.engines[*selected_slot].config.enabled =
                            !self.engines[*selected_slot].config.enabled;
                    }
                    1 => {
                        self.engines[*selected_slot].config.limit_strength =
                            !self.engines[*selected_slot].config.limit_strength;
                    }
                    2 => {
                        let s = &mut self.engines[*selected_slot];
                        s.config.elo = s.config.elo.saturating_sub(100).max(MIN_ELO);
                    }
                    3 => {
                        let s = &mut self.engines[*selected_slot];
                        s.config.depth = s.config.depth.saturating_sub(1).max(1);
                    }
                    5 => self.play_role = self.play_role.next().next(),
                    _ => {}
                },
                KeyCode::Right | KeyCode::Char(' ') => match *selected_field {
                    0 if *selected_slot > 0 => {
                        self.engines[*selected_slot].config.enabled =
                            !self.engines[*selected_slot].config.enabled;
                    }
                    1 => {
                        self.engines[*selected_slot].config.limit_strength =
                            !self.engines[*selected_slot].config.limit_strength;
                    }
                    2 => {
                        let s = &mut self.engines[*selected_slot];
                        s.config.elo = (s.config.elo + 100).min(MAX_ELO);
                    }
                    3 => {
                        let s = &mut self.engines[*selected_slot];
                        s.config.depth = (s.config.depth + 1).min(30);
                    }
                    4 => {
                        *path_buffer = self.engines[*selected_slot].config.binary_path.clone();
                        *editing_path = true;
                    }
                    5 => self.play_role = self.play_role.next(),
                    _ => {}
                },
                KeyCode::Enter => {
                    if *selected_field == 4 {
                        *path_buffer = self.engines[*selected_slot].config.binary_path.clone();
                        *editing_path = true;
                    } else {
                        close_and_apply = true;
                    }
                }
                _ => {}
            }
        }

        if close_and_apply {
            self.overlay = Overlay::None;
            if self.auto_spawn_engines {
                self.sync_engines_config();
                self.trigger_engine_searches();
            }
        }
    }

    fn move_cursor(&mut self, df_visual: i32, dr_visual: i32) {
        let df = if self.board_flipped {
            -df_visual
        } else {
            df_visual
        };
        let dr = if self.board_flipped {
            -dr_visual
        } else {
            dr_visual
        };
        let file = (self.cursor.file() as i32 + df).clamp(0, 7) as u8;
        let rank = (self.cursor.rank() as i32 + dr).clamp(0, 7) as u8;
        self.cursor = Square::from_coords(file, rank);
    }

    pub fn click_square(&mut self, sq: Square) {
        self.cursor = sq;
        self.activate_cursor_square();
    }

    fn activate_cursor_square(&mut self) {
        let sq = self.cursor;
        if let Some(from) = self.selected_sq {
            if from == sq {
                self.clear_selection();
                return;
            }
            if self.legal_targets.contains(&sq) {
                let matching = self.matching_moves(from, sq);
                if matching.len() > 1 {
                    self.overlay = Overlay::Promotion { from, to: sq };
                    return;
                }
                if let Some(&mv) = matching.first() {
                    if let Err(err) = self.play_move(mv) {
                        self.set_error(err);
                    }
                    return;
                }
            }
        }

        if let Some((color, _)) = self.board.piece_on(sq) {
            if color == self.board.side_to_move() {
                let mut legal = MoveList::new();
                generate_legal(&self.board, &mut legal);
                let targets: Vec<Square> = legal
                    .as_slice()
                    .iter()
                    .filter(|m| m.from_sq() == sq)
                    .map(|m| m.to_sq())
                    .collect();
                if !targets.is_empty() {
                    self.selected_sq = Some(sq);
                    self.legal_targets = targets;
                    return;
                }
            }
        }
        self.clear_selection();
    }
}

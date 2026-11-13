//! Graphical user interface (GUI) application implementation powered by `eframe` / `egui`.

use std::time::Duration;

use egui::{Align, CentralPanel, Color32, Context, Layout, SidePanel, TopBottomPanel, Visuals};

use crate::app::App;
use crate::gui_board::{render_chess_board, GuiBoardState};
use crate::gui_panels::{
    render_analyze_panel, render_engine_slots, render_eval_bar, render_games_panel, render_modals,
    render_play_panel, GuiPanelState, GuiTab,
};
use crate::gui_settings::{render_settings_panel, FontScale, GuiSettingsState};
use crate::gui_sounds::{SoundKind, SoundPlayer};

/// Main GUI client state wrapping the Monolith chess app engine.
pub struct MonolithGui {
    pub app: App,
    pub board_state: GuiBoardState,
    pub panel_state: GuiPanelState,
    pub settings_state: GuiSettingsState,
    pub sound_player: SoundPlayer,
    last_history_len: usize,
}

impl MonolithGui {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        engine_path: Option<String>,
        fen: Option<String>,
        pgn_path: Option<String>,
        ipc_port: u16,
    ) -> Self {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let mut app = App::new(engine_path, true);
        let _ = app.enable_ipc(ipc_port);

        if let Some(f) = fen {
            let _ = app.load_fen(&f);
        } else if let Some(p) = pgn_path {
            let _ = app.load_pgn_file(&p, true);
        }

        let last_history_len = app.history.len();

        Self {
            app,
            board_state: GuiBoardState::default(),
            panel_state: GuiPanelState::new(),
            settings_state: GuiSettingsState::default(),
            sound_player: SoundPlayer::default(),
            last_history_len,
        }
    }

    /// Configures the flat Catppuccin Mocha theme with customizable typography.
    fn setup_theme(ctx: &Context, settings: &GuiSettingsState) {
        let mut visuals = Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(30, 30, 46); // Mocha Base (#1e1e2e)
        visuals.window_fill = Color32::from_rgb(24, 24, 37); // Mocha Mantle (#181825)
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(30, 30, 46);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(49, 50, 68); // Surface0 (#313244)
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(69, 71, 90); // Surface1 (#45475a)
        visuals.widgets.active.bg_fill = Color32::from_rgb(88, 91, 112); // Surface2 (#585b70)
        visuals.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(205, 214, 244); // Text (#cdd6f4)
        ctx.set_visuals(visuals);

        // Dynamic typography scaling based on user settings
        let base_size = match settings.font_scale {
            FontScale::Compact => 12.0,
            FontScale::Normal => 14.0,
            FontScale::Large => 16.0,
        };

        let mut style = (*ctx.style()).clone();
        style.text_styles = [
            (
                egui::TextStyle::Small,
                if settings.monospace_ui {
                    egui::FontId::monospace(base_size * 0.8)
                } else {
                    egui::FontId::proportional(base_size * 0.8)
                },
            ),
            (
                egui::TextStyle::Body,
                if settings.monospace_ui {
                    egui::FontId::monospace(base_size)
                } else {
                    egui::FontId::proportional(base_size)
                },
            ),
            (
                egui::TextStyle::Button,
                if settings.monospace_ui {
                    egui::FontId::monospace(base_size)
                } else {
                    egui::FontId::proportional(base_size)
                },
            ),
            (
                egui::TextStyle::Heading,
                if settings.monospace_ui {
                    egui::FontId::monospace(base_size * 1.3)
                } else {
                    egui::FontId::proportional(base_size * 1.3)
                },
            ),
            (
                egui::TextStyle::Monospace,
                egui::FontId::monospace(base_size),
            ),
        ]
        .into();
        ctx.set_style(style);
    }
}

impl eframe::App for MonolithGui {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        Self::setup_theme(ctx, &self.settings_state);

        // Tick engine and analysis processes.
        self.app.tick();

        // Repaint continuously if engines or review is calculating.
        let is_calculating = self.app.engines.iter().any(|e| e.telemetry.searching)
            || self.app.pgn_progress.as_ref().is_some_and(|p| p.running);
        if is_calculating {
            ctx.request_repaint_after(Duration::from_millis(40));
        }

        // Sync settings flags to board state.
        self.board_state.highlight_last_move = self.settings_state.highlight_last_move;
        self.board_state.highlight_legal_moves = self.settings_state.highlight_legal_moves;

        // Configure board assistance: hide hints during Play (unless toggled) or active Trainer puzzle.
        self.board_state.hide_assistance = (self.panel_state.active_tab == GuiTab::Play
            && !self.panel_state.show_hints_in_play)
            || self.panel_state.trainer.active;

        // Left Navigation Sidebar in flat Catppuccin Mocha aesthetic.
        SidePanel::left("left_nav_sidebar")
            .resizable(false)
            .exact_width(140.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("[M]")
                            .monospace()
                            .strong()
                            .size(16.0)
                            .color(Color32::from_rgb(203, 166, 247)), // Mauve
                    );
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("MONOLITH")
                                .strong()
                                .size(13.0)
                                .color(Color32::from_rgb(205, 214, 244)), // Text
                        );
                        ui.label(
                            egui::RichText::new("CHESS STUDIO")
                                .size(9.0)
                                .color(Color32::from_rgb(166, 173, 200)), // Subtext0
                        );
                    });
                });

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                // Clean navigation items without emojis.
                for tab in GuiTab::ALL {
                    let label = tab.label();
                    let is_active = self.panel_state.active_tab == tab;
                    let btn =
                        egui::Button::new(egui::RichText::new(label).size(13.0).strong().color(
                            if is_active {
                                Color32::from_rgb(203, 166, 247) // Mauve
                            } else {
                                Color32::from_rgb(166, 173, 200) // Subtext0
                            },
                        ))
                        .fill(if is_active {
                            Color32::from_rgb(49, 50, 68) // Surface0
                        } else {
                            Color32::TRANSPARENT
                        })
                        .min_size(egui::vec2(120.0, 32.0));

                    if ui.add(btn).clicked() {
                        self.panel_state.active_tab = tab;
                        if tab == GuiTab::Play {
                            // Ensure an engine role is selected when entering play.
                            if self.app.play_role == crate::engines::PlayRole::Analyze {
                                self.app.play_role = crate::engines::PlayRole::PlayBlack;
                                self.app.sync_engines_config();
                                self.app.trigger_engine_searches();
                            }
                        }
                    }
                    ui.add_space(4.0);
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    if self.app.ipc_sender.is_some() {
                        ui.label(
                            egui::RichText::new("[IPC LIVE]")
                                .small()
                                .monospace()
                                .color(Color32::from_rgb(166, 227, 161)), // Green
                        );
                    }
                    ui.add_space(8.0);
                    ui.separator();
                });
            });

        // Left Eval Bar (only when in Analyze mode or when hints are enabled in Play mode, and not in Trainer puzzle).
        let show_eval_bar = !self.panel_state.trainer.active
            && (self.panel_state.active_tab == GuiTab::Analyze
                || (self.panel_state.active_tab == GuiTab::Play
                    && self.panel_state.show_hints_in_play));

        if show_eval_bar {
            SidePanel::left("left_eval_panel")
                .resizable(false)
                .exact_width(38.0)
                .show(ctx, |ui| {
                    ui.add_space(8.0);
                    render_eval_bar(ui, &self.app, false);
                });
        }

        // Right Panel: Contextual to the selected sidebar tab.
        SidePanel::right("right_dock_panel")
            .resizable(true)
            .default_width(380.0)
            .width_range(300.0..=520.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                match self.panel_state.active_tab {
                    GuiTab::Play => render_play_panel(ui, &mut self.app, &mut self.panel_state),
                    GuiTab::Analyze => {
                        render_analyze_panel(ui, &mut self.app, &mut self.panel_state)
                    }
                    GuiTab::Games => render_games_panel(ui, &mut self.app, &mut self.panel_state),
                    GuiTab::Engines => {
                        render_engine_slots(ui, &mut self.app, &mut self.panel_state)
                    }
                    GuiTab::Settings => render_settings_panel(
                        ui,
                        &mut self.app,
                        &mut self.board_state,
                        &mut self.panel_state,
                        &mut self.settings_state,
                        &mut self.sound_player,
                    ),
                }
            });

        // Keyboard shortcut for Help overlay when not editing text.
        if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.panel_state.show_help_modal = !self.panel_state.show_help_modal;
        }

        // Bottom Status Footer Bar: Error/Status display + Help & Quit controls.
        TopBottomPanel::bottom("gui_status_footer")
            .exact_height(28.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let status_color = if self.app.status_is_error {
                        Color32::from_rgb(243, 139, 168) // Mocha Red
                    } else {
                        Color32::from_rgb(148, 226, 213) // Mocha Teal
                    };
                    let status_label = egui::RichText::new(&self.app.status_message)
                        .small()
                        .color(status_color);
                    ui.label(if self.app.status_is_error {
                        status_label.strong()
                    } else {
                        status_label
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("[Q] Beenden").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if ui.small_button("[?] Hilfe").clicked() {
                            self.panel_state.show_help_modal = !self.panel_state.show_help_modal;
                        }
                    });
                });
            });

        // Central Panel: Scalable Chess Board in flat Catppuccin Mocha.
        CentralPanel::default().show(ctx, |ui| {
            render_chess_board(ui, &mut self.app, &mut self.board_state);
        });

        // Play Chess.com-style woody sound when a new move is added at the live tip.
        if self.app.history.len() > self.last_history_len
            && self.app.view_ply == self.app.history.len()
        {
            if let Some(last) = self.app.history.last() {
                let kind = if self.app.game_outcome_label().is_some() {
                    SoundKind::GameEnd
                } else if self.app.board.in_check() {
                    SoundKind::Check
                } else if last.san.starts_with("O-O") {
                    SoundKind::Castle
                } else if last.san.contains('=') {
                    SoundKind::Promote
                } else if last.san.contains('x') {
                    SoundKind::Capture
                } else {
                    SoundKind::Move
                };
                self.sound_player.play(kind);
            }
        }
        self.last_history_len = self.app.history.len();

        // Modal dialogs (FEN & PGN).
        render_modals(ctx, &mut self.app, &mut self.panel_state);
    }
}

/// Launches the native graphical desktop client window.
pub fn run_gui(
    engine_path: Option<String>,
    fen: Option<String>,
    pgn_path: Option<String>,
    port: u16,
) -> Result<(), String> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Monolith Chess Studio")
            .with_inner_size([1240.0, 840.0])
            .with_min_inner_size([880.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Monolith Chess Studio",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(MonolithGui::new(
                cc,
                engine_path,
                fen,
                pgn_path,
                port,
            )))
        }),
    )
    .map_err(|e| format!("Fehler beim Starten der GUI: {e}"))
}

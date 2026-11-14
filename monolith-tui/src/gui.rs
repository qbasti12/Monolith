//! Clean Chess.com-style desktop GUI shell (`-gui`) powered by `eframe` / `egui` in Catppuccin Mocha.

use std::time::Duration;

use egui::{
    pos2, vec2, Align, CentralPanel, Color32, Context, CornerRadius, Frame, Layout, Margin,
    RichText, Sense, SidePanel, Stroke, TopBottomPanel,
};

use crate::app::App;
use crate::gui_board::{render_board_stage, GuiBoardState};
use crate::gui_panels::{
    render_analyze_panel, render_engine_slots, render_games_panel, render_modals,
    render_play_panel, GuiPanelState, GuiTab,
};
use crate::gui_settings::{render_settings_panel, GuiSettingsState};
use crate::gui_sounds::{SoundKind, SoundPlayer};
use crate::gui_theme::{apply_mocha_theme, Mocha};
use crate::gui_widgets::{draw_icon, ui_button, ButtonVariant, IconKind};

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
}

fn tab_icon(tab: GuiTab) -> IconKind {
    match tab {
        GuiTab::Play => IconKind::Play,
        GuiTab::Analyze => IconKind::BoardAnalyze,
        GuiTab::Games => IconKind::ReviewChart,
        GuiTab::Engines => IconKind::Engines,
        GuiTab::Settings => IconKind::Settings,
    }
}

impl eframe::App for MonolithGui {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        apply_mocha_theme(
            ctx,
            self.settings_state.font_scale,
            self.settings_state.monospace_ui,
        );

        self.app.tick();

        let is_calculating = self.app.engines.iter().any(|e| e.telemetry.searching)
            || self.app.pgn_progress.as_ref().is_some_and(|p| p.running);
        if is_calculating {
            ctx.request_repaint_after(Duration::from_millis(40));
        }

        self.board_state.highlight_last_move = self.settings_state.highlight_last_move;
        self.board_state.highlight_legal_moves = self.settings_state.highlight_legal_moves;

        let hide_assist = (self.panel_state.active_tab == GuiTab::Play
            && !self.panel_state.show_hints_in_play)
            || self.panel_state.trainer.active;
        self.board_state.hide_assistance = hide_assist;

        if !ctx.wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.panel_state.show_help_modal = !self.panel_state.show_help_modal;
        }

        // 1. Sleek Left Navigation Rail with custom minimal vector icons (zero emojis / ASCII brackets)
        SidePanel::left("left_nav_rail")
            .resizable(false)
            .exact_width(156.0)
            .frame(
                Frame::new()
                    .fill(Mocha::MANTLE)
                    .stroke(Stroke::new(1.0, Mocha::SURFACE0))
                    .inner_margin(Margin::symmetric(10, 12)),
            )
            .show(ctx, |ui| {
                // Brand crest + title
                ui.horizontal(|ui| {
                    let (icon_r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
                    ui.painter()
                        .rect_filled(icon_r, CornerRadius::same(6), Mocha::SURFACE0);
                    draw_icon(
                        ui.painter(),
                        icon_r.shrink(4.0),
                        IconKind::BrandRook,
                        Mocha::MAUVE,
                    );
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(
                            RichText::new("MONOLITH")
                                .strong()
                                .size(13.5)
                                .color(Mocha::TEXT),
                        );
                        ui.label(
                            RichText::new("Chess Studio")
                                .size(10.0)
                                .color(Mocha::OVERLAY1),
                        );
                    });
                });

                ui.add_space(16.0);

                for tab in GuiTab::ALL {
                    let is_active = self.panel_state.active_tab == tab;
                    let (row_rect, resp) =
                        ui.allocate_exact_size(vec2(136.0, 36.0), Sense::click());
                    let bg = if is_active {
                        Mocha::SURFACE0
                    } else if resp.hovered() {
                        Color32::from_rgb(38, 39, 56)
                    } else {
                        Color32::TRANSPARENT
                    };

                    let p = ui.painter();
                    p.rect_filled(row_rect, CornerRadius::same(8), bg);
                    if is_active {
                        let accent = egui::Rect::from_min_size(
                            pos2(row_rect.min.x, row_rect.min.y + 7.0),
                            vec2(3.0, row_rect.height() - 14.0),
                        );
                        p.rect_filled(accent, CornerRadius::same(2), Mocha::MAUVE);
                    }

                    let icon_col = if is_active {
                        Mocha::MAUVE
                    } else if resp.hovered() {
                        Mocha::TEXT
                    } else {
                        Mocha::SUBTEXT0
                    };
                    let icon_r = egui::Rect::from_min_size(
                        pos2(row_rect.min.x + 12.0, row_rect.min.y + 9.0),
                        vec2(18.0, 18.0),
                    );
                    draw_icon(p, icon_r, tab_icon(tab), icon_col);

                    p.text(
                        pos2(row_rect.min.x + 38.0, row_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        tab.label(),
                        egui::FontId::proportional(13.5),
                        if is_active { Mocha::TEXT } else { icon_col },
                    );

                    if resp.clicked() {
                        self.panel_state.active_tab = tab;
                        if tab == GuiTab::Play
                            && self.app.play_role == crate::engines::PlayRole::Analyze
                        {
                            self.app.play_role = crate::engines::PlayRole::PlayBlack;
                            self.app.sync_engines_config();
                            self.app.trigger_engine_searches();
                        }
                    }
                    ui.add_space(4.0);
                }

                // Bottom utility controls
                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.add_space(4.0);
                    if ui_button(ui, "Hilfe (F1)", ButtonVariant::Subtle, Some(136.0)).clicked() {
                        self.panel_state.show_help_modal = !self.panel_state.show_help_modal;
                    }
                    ui.add_space(4.0);

                    if self.app.ipc_sender.is_some() {
                        ui.horizontal(|ui| {
                            let (dot_r, _) =
                                ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                            ui.painter()
                                .circle_filled(dot_r.center(), 4.0, Mocha::GREEN);
                            ui.label(
                                RichText::new("Telemetry Live")
                                    .small()
                                    .color(Mocha::SUBTEXT0),
                            );
                        });
                        ui.add_space(4.0);
                    }
                });
            });

        // 2. Slim Bottom Status Bar
        TopBottomPanel::bottom("gui_status_footer")
            .exact_height(30.0)
            .frame(
                Frame::new()
                    .fill(Mocha::MANTLE)
                    .stroke(Stroke::new(1.0, Mocha::SURFACE0))
                    .inner_margin(Margin::symmetric(12, 4)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let dot_color = if self.app.status_is_error {
                        Mocha::RED
                    } else {
                        Mocha::TEAL
                    };
                    let (dot_r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                    ui.painter().circle_filled(dot_r.center(), 4.0, dot_color);

                    let msg_col = if self.app.status_is_error {
                        Mocha::RED
                    } else {
                        Mocha::SUBTEXT1
                    };
                    ui.label(
                        RichText::new(&self.app.status_message)
                            .small()
                            .color(msg_col),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui_button(ui, "Beenden", ButtonVariant::Subtle, Some(74.0)).clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
            });

        // 3. Right Contextual Dock Panel
        SidePanel::right("right_dock_panel")
            .resizable(true)
            .default_width(396.0)
            .width_range(330.0..=520.0)
            .frame(
                Frame::new()
                    .fill(Mocha::BASE)
                    .stroke(Stroke::new(1.0, Mocha::SURFACE0))
                    .inner_margin(Margin::symmetric(12, 10)),
            )
            .show(ctx, |ui| match self.panel_state.active_tab {
                GuiTab::Play => render_play_panel(ui, &mut self.app, &mut self.panel_state),
                GuiTab::Analyze => render_analyze_panel(ui, &mut self.app, &mut self.panel_state),
                GuiTab::Games => render_games_panel(ui, &mut self.app, &mut self.panel_state),
                GuiTab::Engines => render_engine_slots(ui, &mut self.app, &mut self.panel_state),
                GuiTab::Settings => render_settings_panel(
                    ui,
                    &mut self.app,
                    &mut self.board_state,
                    &mut self.panel_state,
                    &mut self.settings_state,
                    &mut self.sound_player,
                ),
            });

        // 4. Center Stage: Integrated Eval-Bar + Top/Bottom Player Bars + Scalable 8x8 Board
        CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(Color32::from_rgb(20, 21, 32))
                    .inner_margin(Margin::same(10)),
            )
            .show(ctx, |ui| {
                let show_eval_bar = !hide_assist;
                render_board_stage(ui, &mut self.app, &mut self.board_state, show_eval_bar);
            });

        // Chess.com-style woody sound triggers on live moves
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
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([920.0, 620.0]),
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

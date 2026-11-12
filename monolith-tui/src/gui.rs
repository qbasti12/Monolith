//! Graphical user interface (GUI) application implementation powered by `eframe` / `egui`.

use std::time::Duration;

use egui::{CentralPanel, Color32, Context, SidePanel, Visuals};

use crate::app::App;
use crate::gui_board::{render_chess_board, GuiBoardState};
use crate::gui_panels::{
    render_analyze_panel, render_engine_slots, render_eval_bar, render_games_panel, render_modals,
    render_play_panel, GuiPanelState, GuiTab,
};

/// Main GUI client state wrapping the Monolith chess app engine.
pub struct MonolithGui {
    pub app: App,
    pub board_state: GuiBoardState,
    pub panel_state: GuiPanelState,
}

impl MonolithGui {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        engine_path: Option<String>,
        fen: Option<String>,
        pgn_path: Option<String>,
        ipc_port: u16,
    ) -> Self {
        let mut app = App::new(engine_path, true);
        let _ = app.enable_ipc(ipc_port);

        if let Some(f) = fen {
            let _ = app.load_fen(&f);
        } else if let Some(p) = pgn_path {
            let _ = app.load_pgn_file(&p, true);
        }

        Self {
            app,
            board_state: GuiBoardState::default(),
            panel_state: GuiPanelState::new(),
        }
    }

    /// Configures the Catppuccin Mocha theme.
    fn setup_theme(ctx: &Context) {
        let mut visuals = Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(30, 30, 46); // Mocha Base (#1e1e2e)
        visuals.window_fill = Color32::from_rgb(24, 24, 37); // Mocha Mantle (#181825)
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(30, 30, 46);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(49, 50, 68); // Surface0 (#313244)
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(69, 71, 90); // Surface1 (#45475a)
        visuals.widgets.active.bg_fill = Color32::from_rgb(88, 91, 112); // Surface2 (#585b70)
        visuals.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(205, 214, 244); // Text (#cdd6f4)
        ctx.set_visuals(visuals);
    }
}

impl eframe::App for MonolithGui {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        Self::setup_theme(ctx);

        // Tick engine and analysis processes.
        self.app.tick();

        // Repaint continuously if engines or review is calculating.
        let is_calculating = self.app.engines.iter().any(|e| e.telemetry.searching)
            || self.app.pgn_progress.as_ref().is_some_and(|p| p.running);
        if is_calculating {
            ctx.request_repaint_after(Duration::from_millis(40));
        }

        // Configure board assistance: when playing a match, hide hints unless requested.
        self.board_state.hide_assistance =
            self.panel_state.active_tab == GuiTab::Play && !self.panel_state.show_hints_in_play;

        // Left Navigation Sidebar in Catppuccin Mocha aesthetic.
        SidePanel::left("left_nav_sidebar")
            .resizable(false)
            .exact_width(140.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("♜")
                            .size(24.0)
                            .color(Color32::from_rgb(203, 166, 247)), // Mauve
                    );
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("MONOLITH")
                                .strong()
                                .size(14.0)
                                .color(Color32::from_rgb(205, 214, 244)), // Text
                        );
                        ui.label(
                            egui::RichText::new("CHESS")
                                .size(10.0)
                                .color(Color32::from_rgb(166, 173, 200)), // Subtext0
                        );
                    });
                });

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                // Navigation buttons.
                let nav_items = [
                    (GuiTab::Play, GuiTab::Play.label()),
                    (GuiTab::Analyze, GuiTab::Analyze.label()),
                    (GuiTab::Games, GuiTab::Games.label()),
                    (GuiTab::Engines, GuiTab::Engines.label()),
                ];

                for (tab, label) in nav_items {
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
                            egui::RichText::new("● IPC Live")
                                .small()
                                .color(Color32::from_rgb(166, 227, 161)), // Green
                        );
                    }
                    ui.add_space(8.0);
                    ui.separator();
                });
            });

        // Left Eval Bar (only when in Analyze mode or when hints are enabled in Play mode).
        let show_eval_bar = self.panel_state.active_tab == GuiTab::Analyze
            || (self.panel_state.active_tab == GuiTab::Play && self.panel_state.show_hints_in_play);

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
                }
            });

        // Central Panel: Scalable Chess Board in Catppuccin Mocha.
        CentralPanel::default().show(ctx, |ui| {
            render_chess_board(ui, &mut self.app, &mut self.board_state);
        });

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

//! Graphical user interface (GUI) application implementation powered by `eframe` / `egui`.

use std::time::Duration;

use egui::{CentralPanel, Color32, Context, SidePanel, TopBottomPanel, Visuals};

use crate::app::App;
use crate::gui_board::{render_chess_board, GuiBoardState};
use crate::gui_panels::{
    render_engine_slots, render_eval_bar, render_game_controls, render_modals, render_move_history,
    GuiPanelState,
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

    /// Configures a clean Catppuccin Mocha-inspired dark palette.
    fn setup_theme(ctx: &Context) {
        let mut visuals = Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(30, 30, 46); // Mocha Base
        visuals.window_fill = Color32::from_rgb(24, 24, 37); // Mocha Mantle
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(30, 30, 46);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(49, 50, 68); // Surface0
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(69, 71, 90); // Surface1
        visuals.widgets.active.bg_fill = Color32::from_rgb(88, 91, 112); // Surface2
        visuals.widgets.noninteractive.fg_stroke.color = Color32::from_rgb(205, 214, 244);
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

        // Header Panel: Brand, Outcome alerts, Status.
        TopBottomPanel::top("top_header").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading(
                    egui::RichText::new("MONOLITH")
                        .strong()
                        .color(Color32::from_rgb(203, 166, 247)),
                );
                ui.label(
                    egui::RichText::new("CHESS STUDIO")
                        .small()
                        .color(Color32::from_rgb(137, 180, 250)),
                );
                ui.separator();

                if let Some(outcome) = self.app.game_outcome_label() {
                    ui.label(
                        egui::RichText::new(outcome)
                            .strong()
                            .color(Color32::from_rgb(243, 139, 168)),
                    );
                    ui.separator();
                }

                ui.label(
                    egui::RichText::new(&self.app.status_message)
                        .color(Color32::from_rgb(166, 173, 200)),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.app.ipc_sender.is_some() {
                        ui.label(
                            egui::RichText::new("● IPC Live")
                                .small()
                                .color(Color32::from_rgb(166, 227, 161)),
                        );
                    }
                });
            });
            ui.add_space(4.0);
        });

        // Left Panel: Eval bar.
        SidePanel::left("left_eval_panel")
            .resizable(false)
            .exact_width(42.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                render_eval_bar(ui, &self.app);
            });

        // Right Panel: Game controls, Multi-Engine telemetry, Move history.
        SidePanel::right("right_dock_panel")
            .resizable(true)
            .default_width(420.0)
            .width_range(340.0..=600.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                render_game_controls(ui, &mut self.app, &mut self.panel_state);
                ui.add_space(6.0);
                render_engine_slots(ui, &mut self.app, &mut self.panel_state);
                ui.add_space(6.0);
                render_move_history(ui, &mut self.app);
            });

        // Central Panel: Graphical vector chess board.
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

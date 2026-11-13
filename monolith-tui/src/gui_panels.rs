//! GUI dock panels: Chess.com-style Play match controls, deep analysis, move history, and modals.

use egui::{
    vec2, Align, Align2, Color32, CornerRadius, Layout, RichText, ScrollArea, Stroke, StrokeKind,
    Ui,
};
use monolith_core::pgn::{MoveClass, MoveClassification};
use monolith_core::STARTPOS_FEN;

use crate::app::{App, SAMPLE_OPERA_PGN};
use crate::engines::PlayRole;
use crate::gui_analysis_widgets::{
    render_eval_graph, render_opening_explorer, render_trainer_panel, TrainerState,
};
use crate::gui_board::{engine_color, ENGINE_COLORS, ENGINE_COLOR_NAMES};

/// Active functional view tab in the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GuiTab {
    #[default]
    Play,
    Analyze,
    Games,
    Engines,
    Settings,
}

impl GuiTab {
    pub const ALL: [GuiTab; 5] = [
        GuiTab::Play,
        GuiTab::Analyze,
        GuiTab::Games,
        GuiTab::Engines,
        GuiTab::Settings,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Play => "Spielen",
            Self::Analyze => "Analyse",
            Self::Games => "Partien",
            Self::Engines => "Engines",
            Self::Settings => "Einstellungen",
        }
    }
}

/// State for GUI panels and dialog modals.
#[derive(Debug, Clone)]
pub struct GuiPanelState {
    pub active_tab: GuiTab,
    pub show_fen_modal: bool,
    pub fen_input: String,
    pub fen_error: Option<String>,
    pub show_pgn_modal: bool,
    pub pgn_input: String,
    pub pgn_error: Option<String>,
    pub selected_engine_tab: usize,
    pub show_hints_in_play: bool,
    pub move_text_input: String,
    pub move_text_error: Option<String>,
    pub show_help_modal: bool,
    pub trainer: TrainerState,
}

impl Default for GuiPanelState {
    fn default() -> Self {
        Self {
            active_tab: GuiTab::Play,
            show_fen_modal: false,
            fen_input: STARTPOS_FEN.to_string(),
            fen_error: None,
            show_pgn_modal: false,
            pgn_input: SAMPLE_OPERA_PGN.to_string(),
            pgn_error: None,
            selected_engine_tab: 0,
            show_hints_in_play: false, // Default: Play without assistance
            move_text_input: String::new(),
            move_text_error: None,
            show_help_modal: false,
            trainer: TrainerState::default(),
        }
    }
}

impl GuiPanelState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Renders a vertical evaluation bar indicating White's winning probability and score.
pub fn render_eval_bar(ui: &mut Ui, app: &App, is_hidden: bool) {
    let avail_h = ui.available_height().max(240.0);
    let bar_width = 30.0;
    let (rect, _) = ui.allocate_exact_size(vec2(bar_width, avail_h), egui::Sense::hover());
    let painter = ui.painter_at(rect);

    if is_hidden {
        // Neutral clean focus bar when playing without assistance.
        painter.rect_filled(rect, CornerRadius::same(4), Color32::from_rgb(30, 30, 46));
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(1.0, Color32::from_rgb(49, 50, 68)),
            StrokeKind::Inside,
        );
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "Match",
            egui::FontId::proportional(10.0),
            Color32::from_rgb(166, 173, 200),
        );
        return;
    }

    let (score_cp, score_mate) = app.primary_eval_white();
    let eval_layout =
        crate::theme::compute_eval_bar_layout(100, score_cp, score_mate, app.board_flipped);
    let white_ratio = eval_layout.white_prob as f32;

    let (top_ratio, top_color, bot_color) = if eval_layout.top_is_white {
        (
            white_ratio,
            Color32::from_rgb(240, 240, 245),
            Color32::from_rgb(40, 42, 54),
        )
    } else {
        (
            1.0 - white_ratio,
            Color32::from_rgb(40, 42, 54),
            Color32::from_rgb(240, 240, 245),
        )
    };

    painter.rect_filled(rect, CornerRadius::same(4), top_color);

    let split_y = rect.min.y + rect.height() * top_ratio;
    let bot_rect = egui::Rect::from_min_max(egui::pos2(rect.min.x, split_y), rect.max);
    painter.rect_filled(bot_rect, CornerRadius::same(4), bot_color);
    painter.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0, Color32::from_rgb(100, 105, 130)),
        StrokeKind::Inside,
    );

    // Place evaluation text on the winning/dominant side (top or bottom, following board flip).
    let white_ahead = white_ratio >= 0.5;
    let label_at_top = if eval_layout.top_is_white {
        white_ahead
    } else {
        !white_ahead
    };
    let text_y = if label_at_top {
        rect.min.y + 12.0
    } else {
        rect.max.y - 12.0
    };
    let text_color = if white_ahead {
        Color32::from_rgb(30, 30, 46)
    } else {
        Color32::from_rgb(205, 214, 244)
    };
    painter.text(
        egui::pos2(rect.center().x, text_y),
        Align2::CENTER_CENTER,
        eval_layout.label,
        egui::FontId::monospace(10.0),
        text_color,
    );
}

/// Renders the Chess.com-style Match / Playing panel without assistance.
pub fn render_play_panel(ui: &mut Ui, app: &mut App, panel_state: &mut GuiPanelState) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Partie gegen Computer")
                    .strong()
                    .color(Color32::from_rgb(203, 166, 247)),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let elo = app.engines.first().map_or(2000, |e| e.config.elo);
                ui.label(
                    RichText::new(format!("Elo {elo}"))
                        .small()
                        .color(Color32::from_rgb(166, 173, 200)),
                );
            });
        });

        ui.separator();

        // Turn & Engine Status badge.
        let is_engine_thinking = app.engines.iter().any(|e| e.telemetry.searching);
        let human_turn = match app.play_role {
            PlayRole::PlayBlack => app.board.side_to_move() == monolith_core::Color::White,
            PlayRole::PlayWhite => app.board.side_to_move() == monolith_core::Color::Black,
            PlayRole::Analyze => true,
        };

        if let Some(outcome) = app.game_outcome_label() {
            ui.label(
                RichText::new(format!("[ENDE] {outcome}"))
                    .strong()
                    .color(Color32::from_rgb(243, 139, 168)),
            );
        } else if is_engine_thinking {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(
                    RichText::new("Monolith überlegt...")
                        .color(Color32::from_rgb(166, 227, 161))
                        .strong(),
                );
            });
        } else if human_turn {
            ui.label(
                RichText::new("Du bist am Zug!")
                    .color(Color32::from_rgb(205, 214, 244))
                    .strong(),
            );
        } else {
            ui.label(RichText::new("Warte auf Zug...").weak());
        }

        ui.add_space(6.0);

        // Match difficulty / Role configuration.
        ui.horizontal(|ui| {
            ui.label("Farbe:");
            let prev_role = app.play_role;
            if ui
                .selectable_label(app.play_role == PlayRole::PlayBlack, "Weiß (Ich)")
                .clicked()
            {
                app.play_role = PlayRole::PlayBlack;
            }
            if ui
                .selectable_label(app.play_role == PlayRole::PlayWhite, "Schwarz (Ich)")
                .clicked()
            {
                app.play_role = PlayRole::PlayWhite;
            }
            if app.play_role != prev_role {
                app.sync_engines_config();
                app.trigger_engine_searches();
            }
        });

        ui.add_space(4.0);

        // Assistance toggle: The user requested playing without hints by default!
        ui.horizontal(|ui| {
            ui.checkbox(
                &mut panel_state.show_hints_in_play,
                "Hilfen & Bewertung anzeigen",
            );
        });

        ui.add_space(6.0);

        // Match control buttons.
        ui.horizontal(|ui| {
            if ui.button("Neues Spiel").clicked() {
                let _ = app.load_fen(STARTPOS_FEN);
            }
            if ui.button("Drehen").clicked() {
                app.flip_board();
            }
            if ui.button("Zug zurück").clicked() {
                app.undo_move();
                if !app.history.is_empty() {
                    // In single player match, undoing takes back both human and engine ply.
                    app.undo_move();
                }
            }
        });

        ui.add_space(4.0);
        render_move_text_input(ui, app, panel_state);
    });

    ui.add_space(6.0);
    render_move_history(ui, app, true);
}

fn render_move_text_input(ui: &mut Ui, app: &mut App, panel_state: &mut GuiPanelState) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Zug:")
                .small()
                .color(Color32::from_rgb(166, 173, 200)),
        );
        let resp = ui.add(
            egui::TextEdit::singleline(&mut panel_state.move_text_input)
                .desired_width(96.0)
                .hint_text("e2e4 / Nf3"),
        );
        let enter_hit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.small_button("Ziehen").clicked() || enter_hit {
            let txt = panel_state.move_text_input.trim().to_string();
            match app.submit_move_text(&txt) {
                Ok(()) => {
                    panel_state.move_text_input.clear();
                    panel_state.move_text_error = None;
                }
                Err(err) => {
                    panel_state.move_text_error = Some(err);
                }
            }
        }
    });
    if let Some(err) = &panel_state.move_text_error {
        ui.label(
            RichText::new(err)
                .small()
                .color(Color32::from_rgb(243, 139, 168)),
        );
    }
}

/// Renders the Deep Analysis panel with multi-engine telemetry, PV lines, and FEN/PGN presets.
pub fn render_analyze_panel(ui: &mut Ui, app: &mut App, panel_state: &mut GuiPanelState) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Analyse-Modus")
                    .strong()
                    .color(Color32::from_rgb(137, 180, 250)),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("FEN laden...").clicked() {
                    panel_state.fen_input = app.board.to_fen();
                    panel_state.fen_error = None;
                    panel_state.show_fen_modal = true;
                }
            });
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if ui.button("Start").clicked() {
                let _ = app.load_fen(STARTPOS_FEN);
            }
            if ui.button("Drehen").clicked() {
                app.flip_board();
            }
            if ui.button("Zurück").clicked() {
                app.undo_move();
            }
            ui.separator();
            if ui.button("|<<").clicked() {
                app.jump_start();
            }
            if ui.button("<").clicked() {
                app.step_backward();
            }
            if ui.button(">").clicked() {
                app.step_forward();
            }
            if ui.button(">>|").clicked() {
                app.jump_end();
            }
        });

        ui.add_space(4.0);
        render_move_text_input(ui, app, panel_state);
    });

    ui.add_space(6.0);
    render_opening_explorer(ui, app);

    ui.add_space(6.0);
    render_eval_graph(ui, app);

    ui.add_space(6.0);
    render_engine_slots(ui, app, panel_state);

    ui.add_space(6.0);
    render_move_history(ui, app, false);
}

/// Renders the multi-engine telemetry inspector supporting up to 3 engines concurrently.
pub fn render_engine_slots(ui: &mut Ui, app: &mut App, panel_state: &mut GuiPanelState) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Engines (Multi-UCI):")
                    .strong()
                    .color(Color32::from_rgb(203, 166, 247)),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                for i in (0..app.engines.len()).rev() {
                    let bot_col = engine_color(app.engines[i].config.color_idx);
                    let name = format!("Engine {}", i + 1);
                    let label = RichText::new(name).color(if app.engines[i].config.enabled {
                        bot_col
                    } else {
                        Color32::from_rgb(166, 173, 200)
                    });
                    if ui
                        .selectable_label(panel_state.selected_engine_tab == i, label)
                        .clicked()
                    {
                        panel_state.selected_engine_tab = i;
                    }
                }
            });
        });

        ui.separator();

        let tab_idx = panel_state
            .selected_engine_tab
            .min(app.engines.len().saturating_sub(1));
        let mut cfg_changed = false;
        if let Some(slot) = app.engines.get_mut(tab_idx) {
            let bot_col = engine_color(slot.config.color_idx);
            ui.horizontal(|ui| {
                if ui.checkbox(&mut slot.config.enabled, "Aktiviert").changed() {
                    cfg_changed = true;
                }
                ui.label(RichText::new(&slot.config.label).color(bot_col).strong());

                let status_text = if !slot.config.enabled {
                    RichText::new("[DEAKTIVIERT]").color(Color32::from_rgb(108, 112, 134))
                } else if slot.telemetry.searching {
                    RichText::new("[BERECHNET]").color(Color32::from_rgb(166, 227, 161))
                } else if slot.telemetry.connected {
                    RichText::new("[BEREIT]").color(Color32::from_rgb(148, 226, 213))
                } else {
                    RichText::new("[OFFLINE]").color(Color32::from_rgb(243, 139, 168))
                };
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(status_text);
                });
            });

            ui.horizontal(|ui| {
                ui.label("Pfeil-Farbe:");
                for (c_idx, &c) in ENGINE_COLORS.iter().enumerate() {
                    let selected = slot.config.color_idx == c_idx;
                    let lbl =
                        RichText::new(ENGINE_COLOR_NAMES[c_idx])
                            .size(11.0)
                            .color(if selected {
                                c
                            } else {
                                Color32::from_rgb(166, 173, 200)
                            });
                    if ui.selectable_label(selected, lbl).clicked() {
                        slot.config.color_idx = c_idx;
                    }
                }
            });

            ui.add_space(2.0);

            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut slot.config.limit_strength, "Elo-Limit")
                    .changed()
                {
                    cfg_changed = true;
                }
                if slot.config.limit_strength {
                    ui.label("Elo:");
                    let elo_slider = egui::Slider::new(
                        &mut slot.config.elo,
                        monolith_engine::skill::MIN_ELO..=monolith_engine::skill::MAX_ELO,
                    )
                    .step_by(50.0);
                    if ui.add(elo_slider).changed() {
                        cfg_changed = true;
                    }
                }
                ui.label("Tiefe:");
                let depth_slider = egui::Slider::new(&mut slot.config.depth, 1..=30);
                if ui.add(depth_slider).changed() {
                    cfg_changed = true;
                }
            });

            ui.add_space(4.0);

            // Telemetry metrics grid.
            egui::Grid::new(format!("engine_metrics_{tab_idx}"))
                .num_columns(4)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Bewertung:").color(Color32::from_rgb(166, 173, 200)));
                    let score_str = if let Some(m) = slot.telemetry.white_score_mate {
                        format!("Matt in {m}")
                    } else if let Some(cp) = slot.telemetry.white_score_cp {
                        format!("{:+.2} cp", cp as f32 / 100.0)
                    } else {
                        "-".to_string()
                    };
                    ui.label(
                        RichText::new(score_str)
                            .strong()
                            .color(Color32::from_rgb(249, 226, 175)),
                    );

                    ui.label(RichText::new("Tiefe:").color(Color32::from_rgb(166, 173, 200)));
                    ui.label(format!(
                        "{}/{}",
                        slot.telemetry.depth, slot.telemetry.seldepth
                    ));
                    ui.end_row();

                    ui.label(
                        RichText::new("Knoten / NPS:").color(Color32::from_rgb(166, 173, 200)),
                    );
                    ui.label(format!(
                        "{} ({} k/s)",
                        slot.telemetry.nodes,
                        slot.telemetry.nps / 1000
                    ));

                    ui.label(RichText::new("Hashfull:").color(Color32::from_rgb(166, 173, 200)));
                    ui.label(format!("{}‰", slot.telemetry.hashfull));
                    ui.end_row();

                    ui.label(RichText::new("Bester Zug:").color(Color32::from_rgb(166, 173, 200)));
                    let best = slot.telemetry.best_move_san.as_deref().unwrap_or("-");
                    ui.label(
                        RichText::new(best)
                            .color(Color32::from_rgb(166, 227, 161))
                            .strong(),
                    );

                    ui.label(
                        RichText::new("Erwartete Antwort:").color(Color32::from_rgb(166, 173, 200)),
                    );
                    let reply = slot.telemetry.expected_reply_san.as_deref().unwrap_or("-");
                    ui.label(reply);
                    ui.end_row();
                });

            ui.add_space(2.0);
            ui.label(
                RichText::new("Hauptvariante (PV):")
                    .small()
                    .color(Color32::from_rgb(166, 173, 200)),
            );
            let pv = if slot.telemetry.pv_san.is_empty() {
                "..."
            } else {
                &slot.telemetry.pv_san
            };
            ui.label(
                RichText::new(pv)
                    .monospace()
                    .color(Color32::from_rgb(186, 194, 222)),
            );
        }

        if cfg_changed {
            app.sync_engines_config();
            app.trigger_engine_searches();
        }
    });
}

/// Renders the move history table with SAN notation, classifications, and clickable ply navigation.
pub fn render_move_history(ui: &mut Ui, app: &mut App, compact: bool) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Zugverlauf:")
                    .strong()
                    .color(Color32::from_rgb(166, 173, 200)),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("Halbzug: {} / {}", app.view_ply, app.history.len()))
                        .small(),
                );
            });
        });

        ui.separator();

        let max_h = if compact { 180.0 } else { 240.0 };
        ScrollArea::vertical()
            .max_height(max_h)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if app.history.is_empty() {
                    ui.label(RichText::new("Keine Züge gespielt.").weak());
                    return;
                }

                let mut jump_target: Option<usize> = None;
                let num_moves = app.history.len().div_ceil(2);
                egui::Grid::new("move_history_grid")
                    .striped(true)
                    .spacing([8.0, 4.0])
                    .num_columns(3)
                    .show(ui, |ui| {
                        for move_idx in 0..num_moves {
                            let w_ply = move_idx * 2;
                            let b_ply = move_idx * 2 + 1;

                            ui.label(RichText::new(format!("{}.", move_idx + 1)).weak());

                            // White move column.
                            if let Some(w_rec) = app.history.get(w_ply) {
                                let is_active = app.view_ply == w_ply + 1;
                                ui.horizontal(|ui| {
                                    let btn = egui::Button::new(
                                        RichText::new(&w_rec.san).strong().color(if is_active {
                                            Color32::from_rgb(249, 226, 175)
                                        } else {
                                            Color32::from_rgb(205, 214, 244)
                                        }),
                                    );
                                    if ui.add(btn).clicked() {
                                        jump_target = Some(w_ply + 1);
                                    }
                                    if !compact {
                                        if let Some(class) = &w_rec.classification {
                                            render_classification_badge(ui, class);
                                        }
                                    }
                                });
                            } else {
                                ui.label("");
                            }

                            // Black move column.
                            if let Some(b_rec) = app.history.get(b_ply) {
                                let is_active = app.view_ply == b_ply + 1;
                                ui.horizontal(|ui| {
                                    let btn = egui::Button::new(
                                        RichText::new(&b_rec.san).strong().color(if is_active {
                                            Color32::from_rgb(249, 226, 175)
                                        } else {
                                            Color32::from_rgb(205, 214, 244)
                                        }),
                                    );
                                    if ui.add(btn).clicked() {
                                        jump_target = Some(b_ply + 1);
                                    }
                                    if !compact {
                                        if let Some(class) = &b_rec.classification {
                                            render_classification_badge(ui, class);
                                        }
                                    }
                                });
                            } else {
                                ui.label("");
                            }

                            ui.end_row();
                        }
                    });

                if let Some(ply) = jump_target {
                    app.jump_to_ply(ply);
                }
            });
    });
}

fn render_classification_badge(ui: &mut Ui, class: &MoveClassification) {
    let (badge_text, bg_color) = match class.class {
        MoveClass::Brilliant => ("!!", Color32::from_rgb(203, 166, 247)),
        MoveClass::Great => ("!", Color32::from_rgb(137, 180, 250)),
        MoveClass::Best => ("BEST", Color32::from_rgb(166, 227, 161)),
        MoveClass::Good => ("GOOD", Color32::from_rgb(148, 226, 213)),
        MoveClass::Inaccuracy => ("?!", Color32::from_rgb(249, 226, 175)),
        MoveClass::Mistake => ("?", Color32::from_rgb(250, 179, 135)),
        MoveClass::Blunder => ("??", Color32::from_rgb(243, 139, 168)),
    };
    ui.label(
        RichText::new(badge_text)
            .strong()
            .size(11.0)
            .color(bg_color),
    );
}

/// Renders the PGN and Tactical Games review panel.
pub fn render_games_panel(ui: &mut Ui, app: &mut App, panel_state: &mut GuiPanelState) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Partien & Taktisches Review")
                    .strong()
                    .color(Color32::from_rgb(166, 227, 161)),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("PGN-Text laden...").clicked() {
                    panel_state.pgn_error = None;
                    panel_state.show_pgn_modal = true;
                }
            });
        });

        ui.separator();

        let mut cancel_analysis = false;
        if let Some(pg) = &app.pgn_progress {
            ui.label(RichText::new(&pg.title).strong());
            if pg.running {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        RichText::new(format!(
                            "Analysiere Züge: {} / {}",
                            pg.done_plies, pg.total_plies
                        ))
                        .color(Color32::from_rgb(166, 227, 161)),
                    );
                    if ui.small_button("Stop").clicked() {
                        cancel_analysis = true;
                    }
                });
            } else {
                ui.label(
                    RichText::new("Analyse abgeschlossen. Klicke auf Züge zur Inspektion.")
                        .color(Color32::from_rgb(148, 226, 213)),
                );
            }
        } else {
            ui.label("Lade ein PGN-Spiel, um automatische taktische Fehleranalyse auszuführen:");
            if ui.button("Sample Opera Game laden (Paul Morphy)").clicked() {
                let _ = app.load_pgn_str(SAMPLE_OPERA_PGN, true);
            }
        }

        if cancel_analysis {
            app.cancel_pgn_analysis();
        }
    });

    ui.add_space(6.0);
    render_eval_graph(ui, app);

    ui.add_space(6.0);
    render_trainer_panel(ui, app, &mut panel_state.trainer);

    ui.add_space(6.0);
    render_move_history(ui, app, false);
}

pub use crate::gui_modals::render_modals;

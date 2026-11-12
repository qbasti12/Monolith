//! GUI sidebar panels: evaluation bar, multi-engine telemetry, move history, and modals.

use egui::{
    vec2, Align, Align2, Color32, CornerRadius, Layout, RichText, ScrollArea, Stroke, StrokeKind,
    TextStyle, Ui, Window,
};
use monolith_core::pgn::{MoveClass, MoveClassification};
use monolith_core::STARTPOS_FEN;

use crate::app::{
    App, FEN_PRESET_INFINITE_QUEENS, FEN_PRESET_KIWIPETE, FEN_PRESET_STANDARD, SAMPLE_OPERA_PGN,
};
use crate::engines::PlayRole;

/// State for GUI panels and dialog modals.
#[derive(Debug, Default, Clone)]
pub struct GuiPanelState {
    pub show_fen_modal: bool,
    pub fen_input: String,
    pub fen_error: Option<String>,
    pub show_pgn_modal: bool,
    pub pgn_input: String,
    pub pgn_error: Option<String>,
    pub selected_engine_tab: usize,
}

impl GuiPanelState {
    pub fn new() -> Self {
        Self {
            show_fen_modal: false,
            fen_input: STARTPOS_FEN.to_string(),
            fen_error: None,
            show_pgn_modal: false,
            pgn_input: SAMPLE_OPERA_PGN.to_string(),
            pgn_error: None,
            selected_engine_tab: 0,
        }
    }
}

/// Renders a vertical evaluation bar indicating White's winning probability and score.
pub fn render_eval_bar(ui: &mut Ui, app: &App) {
    let (score_cp, score_mate) = app.primary_eval_white();
    let avail_h = ui.available_height().max(240.0);
    let bar_width = 30.0;

    let (rect, _) = ui.allocate_exact_size(vec2(bar_width, avail_h), egui::Sense::hover());
    let painter = ui.painter_at(rect);

    // Compute White's winning probability (0.0 to 1.0) via standard logistic curve.
    let white_ratio = if let Some(m) = score_mate {
        if m > 0 {
            1.0
        } else {
            0.0
        }
    } else if let Some(cp) = score_cp {
        1.0 / (1.0 + 10.0_f32.powf(-cp as f32 / 400.0))
    } else {
        0.5
    };

    let (top_ratio, top_color, bot_color) = if app.board_flipped {
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

    // Score text badge.
    let score_text = if let Some(m) = score_mate {
        format!("M{m}")
    } else if let Some(cp) = score_cp {
        format!("{:+.2}", cp as f32 / 100.0)
    } else {
        "0.0".to_string()
    };

    let text_pos = egui::pos2(rect.center().x, rect.min.y + 14.0);
    painter.text(
        text_pos,
        Align2::CENTER_CENTER,
        score_text,
        egui::FontId::proportional(11.0),
        Color32::from_rgb(180, 190, 254),
    );
}

/// Renders main game controls (Game Mode, Navigation, Presets, Undo).
pub fn render_game_controls(ui: &mut Ui, app: &mut App, panel_state: &mut GuiPanelState) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Modus:").strong());
            let prev_role = app.play_role;
            egui::ComboBox::from_id_salt("game_mode_combo")
                .selected_text(match app.play_role {
                    PlayRole::Analyze => "Analyse",
                    PlayRole::PlayBlack => "Mensch (Weiß) vs Engine (Schwarz)",
                    PlayRole::PlayWhite => "Engine (Weiß) vs Mensch (Schwarz)",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.play_role, PlayRole::Analyze, "Analyse");
                    ui.selectable_value(
                        &mut app.play_role,
                        PlayRole::PlayBlack,
                        "Mensch (Weiß) vs Engine",
                    );
                    ui.selectable_value(
                        &mut app.play_role,
                        PlayRole::PlayWhite,
                        "Engine vs Mensch (Schwarz)",
                    );
                });

            if app.play_role != prev_role {
                app.sync_engines_config();
                app.trigger_engine_searches();
            }
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            if ui.button("⟲ Neu").clicked() {
                let _ = app.load_fen(STARTPOS_FEN);
            }
            if ui.button("⇄ Drehen").clicked() {
                app.flip_board();
            }
            if ui.button("⮌ Rückgängig").clicked() {
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

        ui.horizontal(|ui| {
            if ui.button("FEN laden...").clicked() {
                panel_state.fen_input = app.board.to_fen();
                panel_state.fen_error = None;
                panel_state.show_fen_modal = true;
            }
            if ui.button("PGN laden / Review...").clicked() {
                panel_state.pgn_error = None;
                panel_state.show_pgn_modal = true;
            }
            if let Some(pg) = &app.pgn_progress {
                if pg.running {
                    ui.spinner();
                    ui.label(
                        RichText::new(format!("Review: {}/{}", pg.done_plies, pg.total_plies))
                            .color(Color32::from_rgb(166, 227, 161)),
                    );
                    if ui.small_button("Stop").clicked() {
                        app.cancel_pgn_analysis();
                    }
                }
            }
        });
    });
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
                    let name = format!("Engine {}", i + 1);
                    if ui
                        .selectable_label(panel_state.selected_engine_tab == i, name)
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
            ui.horizontal(|ui| {
                if ui.checkbox(&mut slot.config.enabled, "Aktiviert").changed() {
                    cfg_changed = true;
                }
                ui.label(
                    RichText::new(&slot.config.label)
                        .color(Color32::from_rgb(137, 180, 250))
                        .strong(),
                );

                let status_text = if !slot.config.enabled {
                    RichText::new("Deaktiviert").color(Color32::from_rgb(108, 112, 134))
                } else if slot.telemetry.searching {
                    RichText::new("● Berechnet...").color(Color32::from_rgb(166, 227, 161))
                } else if slot.telemetry.connected {
                    RichText::new("● Bereit").color(Color32::from_rgb(148, 226, 213))
                } else {
                    RichText::new("○ Offline").color(Color32::from_rgb(243, 139, 168))
                };
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(status_text);
                });
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
                    let elo_slider =
                        egui::Slider::new(&mut slot.config.elo, 1000..=2400).step_by(50.0);
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

                    ui.label(RichText::new("Erwartet:").color(Color32::from_rgb(166, 173, 200)));
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
pub fn render_move_history(ui: &mut Ui, app: &mut App) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Zugverlauf & Klassifizierung:")
                    .strong()
                    .color(Color32::from_rgb(137, 180, 250)),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("Halbzug: {} / {}", app.view_ply, app.history.len()))
                        .small(),
                );
            });
        });

        ui.separator();

        ScrollArea::vertical()
            .max_height(220.0)
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
                                    if let Some(class) = &w_rec.classification {
                                        render_classification_badge(ui, class);
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
                                    if let Some(class) = &b_rec.classification {
                                        render_classification_badge(ui, class);
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
        MoveClass::Best => ("★", Color32::from_rgb(166, 227, 161)),
        MoveClass::Good => ("✓", Color32::from_rgb(148, 226, 213)),
        MoveClass::Inaccuracy => ("?!", Color32::from_rgb(249, 226, 175)),
        MoveClass::Mistake => ("?", Color32::from_rgb(250, 179, 135)),
        MoveClass::Blunder => ("??", Color32::from_rgb(243, 139, 168)),
    };
    ui.label(RichText::new(badge_text).strong().color(bg_color));
}

/// Renders modal dialog windows (FEN preset / input, PGN review / analysis).
pub fn render_modals(ctx: &egui::Context, app: &mut App, panel_state: &mut GuiPanelState) {
    // FEN Dialog
    if panel_state.show_fen_modal {
        Window::new("FEN laden")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui: &mut Ui| {
                ui.label("Wähle ein Preset oder gib eine eigene FEN ein:");
                ui.horizontal(|ui: &mut Ui| {
                    if ui.button("Standard").clicked() {
                        panel_state.fen_input = FEN_PRESET_STANDARD.to_string();
                        panel_state.fen_error = None;
                    }
                    if ui.button("Kiwipete").clicked() {
                        panel_state.fen_input = FEN_PRESET_KIWIPETE.to_string();
                        panel_state.fen_error = None;
                    }
                    if ui.button("15 Damen").clicked() {
                        panel_state.fen_input = FEN_PRESET_INFINITE_QUEENS.to_string();
                        panel_state.fen_error = None;
                    }
                });

                ui.add_space(4.0);
                ui.add(egui::TextEdit::singleline(&mut panel_state.fen_input).desired_width(450.0));

                if let Some(err) = &panel_state.fen_error {
                    ui.label(RichText::new(err).color(Color32::from_rgb(243, 139, 168)));
                }

                ui.add_space(6.0);
                ui.horizontal(|ui: &mut Ui| {
                    if ui.button("Laden").clicked() {
                        match app.load_fen(&panel_state.fen_input) {
                            Ok(()) => {
                                panel_state.show_fen_modal = false;
                                panel_state.fen_error = None;
                            }
                            Err(e) => {
                                panel_state.fen_error = Some(e);
                            }
                        }
                    }
                    if ui.button("Abbrechen").clicked() {
                        panel_state.show_fen_modal = false;
                        panel_state.fen_error = None;
                    }
                });
            });
    }

    // PGN Dialog
    if panel_state.show_pgn_modal {
        Window::new("PGN laden / Taktisches Review")
            .collapsible(false)
            .default_width(500.0)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui: &mut Ui| {
                ui.label("PGN-Text eingeben oder Morphy Opera Game laden:");
                if ui.button("Sample Opera Game (Paul Morphy)").clicked() {
                    panel_state.pgn_input = SAMPLE_OPERA_PGN.to_string();
                    panel_state.pgn_error = None;
                }

                ui.add_space(4.0);
                ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui: &mut Ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut panel_state.pgn_input)
                                .desired_width(480.0)
                                .desired_rows(10)
                                .font(TextStyle::Monospace),
                        );
                    });

                if let Some(err) = &panel_state.pgn_error {
                    ui.label(RichText::new(err).color(Color32::from_rgb(243, 139, 168)));
                }

                ui.add_space(6.0);
                ui.horizontal(|ui: &mut Ui| {
                    if ui.button("Laden & Analysieren").clicked() {
                        match app.load_pgn_str(&panel_state.pgn_input, true) {
                            Ok(()) => {
                                panel_state.show_pgn_modal = false;
                                panel_state.pgn_error = None;
                            }
                            Err(e) => {
                                panel_state.pgn_error = Some(e);
                            }
                        }
                    }
                    if ui.button("Nur Laden (ohne Engine)").clicked() {
                        match app.load_pgn_str(&panel_state.pgn_input, false) {
                            Ok(()) => {
                                panel_state.show_pgn_modal = false;
                                panel_state.pgn_error = None;
                            }
                            Err(e) => {
                                panel_state.pgn_error = Some(e);
                            }
                        }
                    }
                    if ui.button("Schließen").clicked() {
                        panel_state.show_pgn_modal = false;
                        panel_state.pgn_error = None;
                    }
                });
            });
    }
}

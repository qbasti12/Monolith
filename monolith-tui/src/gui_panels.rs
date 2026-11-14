//! Unified Chess.com-style GUI panels: Play, Analyze, Game Review, and Move List.

use egui::{
    vec2, Align, Color32, CornerRadius, Frame, Margin, RichText, ScrollArea, Sense, Stroke, Ui,
};
use monolith_core::STARTPOS_FEN;
use monolith_engine::skill::{MAX_ELO, MIN_ELO};

use crate::app::{App, SAMPLE_OPERA_PGN};
use crate::engines::PlayRole;
use crate::gui_analysis_widgets::{
    render_eval_graph, render_opening_explorer, render_review_summary, render_trainer_panel,
    TrainerState,
};
use crate::gui_board::engine_color;
use crate::gui_stage::paint_eval_bar;
use crate::gui_theme::Mocha;
use crate::gui_widgets::{
    class_visuals, draw_class_pill, ui_button, ui_card, ui_icon_button, ui_pill, ui_section_header,
    ui_segmented_tab, ButtonVariant, IconKind,
};
use crate::theme::format_eval_short;

pub use crate::gui_engines::render_engine_slots;

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
            Self::Games => "Review",
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
            show_hints_in_play: false,
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

/// Standalone vertical Eval Bar renderer (also used in headless panel tests).
pub fn render_eval_bar(ui: &mut Ui, app: &App, is_hidden: bool) {
    let avail_h = ui.available_height().max(240.0);
    paint_eval_bar(ui, app, 28.0, avail_h, is_hidden);
}

/// Renders the Chess.com-style Match / Play panel without engine spoilers by default.
pub fn render_play_panel(ui: &mut Ui, app: &mut App, panel_state: &mut GuiPanelState) {
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui_card(ui, |ui| {
                let elo = app.engines.first().map_or(2400, |e| e.config.elo);
                ui_section_header(ui, "Partie gegen Monolith", Mocha::MAUVE, |ui| {
                    ui_pill(ui, &format!("Elo {elo}"), Mocha::SUBTEXT1, Mocha::SURFACE0);
                });
                ui.add_space(6.0);

                // Match status banner
                let is_thinking = app.engines.iter().any(|e| e.telemetry.searching);
                let human_turn = match app.play_role {
                    PlayRole::PlayBlack => app.board.side_to_move() == monolith_core::Color::White,
                    PlayRole::PlayWhite => app.board.side_to_move() == monolith_core::Color::Black,
                    PlayRole::Analyze => true,
                };

                Frame::new()
                    .fill(Mocha::CRUST)
                    .stroke(Stroke::new(1.0, Mocha::SURFACE0))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if let Some(outcome) = app.game_outcome_label() {
                                ui.label(RichText::new(outcome).strong().color(Mocha::YELLOW));
                            } else if is_thinking {
                                ui.spinner();
                                ui.label(
                                    RichText::new("Monolith berechnet Zug...")
                                        .strong()
                                        .color(Mocha::GREEN),
                                );
                            } else if human_turn {
                                ui.label(
                                    RichText::new("Du bist am Zug").strong().color(Mocha::TEXT),
                                );
                            } else {
                                ui.label(
                                    RichText::new("Warte auf Engine...").color(Mocha::SUBTEXT0),
                                );
                            }
                        });
                    });

                ui.add_space(10.0);

                // Side selector (uniform segmented pills)
                ui.label(RichText::new("Deine Farbe:").small().color(Mocha::SUBTEXT0));
                ui.add_space(2.0);
                let prev_role = app.play_role;
                ui.columns(2, |cols| {
                    let w0 = cols[0].available_width();
                    if ui_segmented_tab(
                        &mut cols[0],
                        "Weiß spielen",
                        app.play_role == PlayRole::PlayBlack,
                        w0,
                    )
                    .clicked()
                    {
                        app.play_role = PlayRole::PlayBlack;
                    }
                    let w1 = cols[1].available_width();
                    if ui_segmented_tab(
                        &mut cols[1],
                        "Schwarz spielen",
                        app.play_role == PlayRole::PlayWhite,
                        w1,
                    )
                    .clicked()
                    {
                        app.play_role = PlayRole::PlayWhite;
                    }
                });
                if app.play_role != prev_role {
                    app.sync_engines_config();
                    app.trigger_engine_searches();
                }

                ui.add_space(10.0);

                // Quick Bot Elo Presets + smooth slider
                ui.label(
                    RichText::new("Spielstärke (Elo):")
                        .small()
                        .color(Mocha::SUBTEXT0),
                );
                ui.add_space(2.0);
                let mut changed = false;
                if let Some(slot) = app.engines.first_mut() {
                    ui.horizontal_wrapped(|ui| {
                        for &(preset_elo, label) in &[
                            (800u32, "800"),
                            (1400, "1400"),
                            (1800, "1800"),
                            (2400, "2400"),
                            (3200, "Max"),
                        ] {
                            let active = slot.config.elo == preset_elo;
                            if ui_segmented_tab(ui, label, active, 52.0).clicked() {
                                slot.config.elo = preset_elo;
                                slot.config.limit_strength = preset_elo < 3000;
                                slot.config.enabled = true;
                                changed = true;
                            }
                        }
                    });
                    ui.add_space(4.0);
                    if ui
                        .add(egui::Slider::new(&mut slot.config.elo, MIN_ELO..=MAX_ELO).text("Elo"))
                        .changed()
                    {
                        slot.config.limit_strength = slot.config.elo < 3000;
                        slot.config.enabled = true;
                        changed = true;
                    }
                }
                if changed {
                    app.sync_engines_config();
                    app.trigger_engine_searches();
                }

                ui.add_space(6.0);
                ui.checkbox(
                    &mut panel_state.show_hints_in_play,
                    "Bewertungsbalken & Pfeile einblenden",
                );

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui_button(ui, "Neue Partie", ButtonVariant::Primary, Some(130.0)).clicked() {
                        let _ = app.load_fen(STARTPOS_FEN);
                    }
                    if ui_button(ui, "Drehen", ButtonVariant::Secondary, None).clicked() {
                        app.flip_board();
                    }
                    if ui_button(ui, "Zurück", ButtonVariant::Secondary, None).clicked() {
                        app.undo_move();
                        if !app.history.is_empty() {
                            app.undo_move();
                        }
                    }
                });

                ui.add_space(8.0);
                render_move_text_input(ui, app, panel_state);
            });

            ui.add_space(8.0);
            render_move_history(ui, app, true);
            ui.add_space(6.0);
            render_nav_strip(ui, app);
        });
}

/// Renders the Chess.com-style Deep Analysis panel (Engine Lines, Opening Banner, Move List & Nav).
pub fn render_analyze_panel(ui: &mut Ui, app: &mut App, panel_state: &mut GuiPanelState) {
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Top Board / Position Quick Tools
            ui_card(ui, |ui| {
                ui_section_header(ui, "Analyse-Werkzeuge", Mocha::BLUE, |_| {});
                ui.add_space(6.0);

                ui.horizontal_wrapped(|ui| {
                    if ui_button(ui, "Startstellung", ButtonVariant::Secondary, None).clicked() {
                        let _ = app.load_fen(STARTPOS_FEN);
                    }
                    if ui_button(ui, "FEN laden...", ButtonVariant::Secondary, None).clicked() {
                        panel_state.fen_input = app.board.to_fen();
                        panel_state.fen_error = None;
                        panel_state.show_fen_modal = true;
                    }
                    if ui_button(ui, "PGN laden...", ButtonVariant::Secondary, None).clicked() {
                        panel_state.pgn_error = None;
                        panel_state.show_pgn_modal = true;
                    }
                });

                ui.add_space(8.0);
                render_move_text_input(ui, app, panel_state);
            });

            ui.add_space(8.0);

            // Compact Player-Facing Engine Lines (no internal telemetry: no Nodes/NPS/Hashfull/SelDepth)
            ui_card(ui, |ui| {
                let any_enabled = app.engines.iter().any(|s| s.config.enabled);
                ui_section_header(ui, "Engine-Bewertung", Mocha::MAUVE, |ui| {
                    if !any_enabled {
                        if ui_button(ui, "Engine aktivieren", ButtonVariant::Primary, None)
                            .clicked()
                        {
                            if let Some(s) = app.engines.first_mut() {
                                s.config.enabled = true;
                            }
                            app.sync_engines_config();
                            app.trigger_engine_searches();
                        }
                    } else {
                        ui_pill(ui, "Analyse aktiv", Mocha::CRUST, Mocha::GREEN);
                    }
                });
                ui.add_space(6.0);

                if !any_enabled {
                    ui.label(
                        RichText::new("Alle Engine-Slots sind pausiert.")
                            .small()
                            .color(Mocha::SUBTEXT0),
                    );
                } else {
                    for (idx, slot) in app.engines.iter().enumerate() {
                        if !slot.config.enabled {
                            continue;
                        }
                        let bot_col = engine_color(slot.config.color_idx);
                        let eval_s = format_eval_short(
                            slot.telemetry.white_score_cp,
                            slot.telemetry.white_score_mate,
                        );
                        let best_s = slot
                            .telemetry
                            .best_move_san
                            .as_deref()
                            .or(slot.telemetry.best_move_uci.as_deref())
                            .unwrap_or("-");
                        let reply_s = slot.telemetry.expected_reply_san.as_deref().unwrap_or("-");

                        Frame::new()
                            .fill(Mocha::CRUST)
                            .stroke(Stroke::new(1.0, Mocha::SURFACE0))
                            .corner_radius(CornerRadius::same(8))
                            .inner_margin(Margin::symmetric(10, 8))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (dot_r, _) =
                                        ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                                    ui.painter().circle_filled(dot_r.center(), 4.5, bot_col);
                                    ui.label(
                                        RichText::new(format!(
                                            "{} ({})",
                                            slot.telemetry.reported_name,
                                            if slot.config.limit_strength {
                                                format!("Elo {}", slot.config.elo)
                                            } else {
                                                "Max".to_string()
                                            }
                                        ))
                                        .strong()
                                        .size(12.5)
                                        .color(Mocha::TEXT),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(Align::Center),
                                        |ui| {
                                            ui_pill(
                                                ui,
                                                &format!("Tiefe {}", slot.telemetry.depth),
                                                Mocha::SUBTEXT0,
                                                Mocha::SURFACE0,
                                            );
                                            ui_pill(ui, &eval_s, Mocha::CRUST, Mocha::YELLOW);
                                        },
                                    );
                                });
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Bester Zug:").small().color(Mocha::SUBTEXT0),
                                    );
                                    ui.label(RichText::new(best_s).strong().color(Mocha::GREEN));
                                    ui.add_space(10.0);
                                    ui.label(
                                        RichText::new("Erwartete Antwort:")
                                            .small()
                                            .color(Mocha::SUBTEXT0),
                                    );
                                    ui.label(
                                        RichText::new(reply_s).strong().color(Mocha::SAPPHIRE),
                                    );
                                });
                                if !slot.telemetry.pv_san.is_empty() {
                                    ui.label(
                                        RichText::new(&slot.telemetry.pv_san)
                                            .monospace()
                                            .small()
                                            .color(Mocha::SUBTEXT1),
                                    );
                                } else if slot.telemetry.searching {
                                    ui.label(
                                        RichText::new("Berechne Hauptvariante...")
                                            .small()
                                            .color(Mocha::OVERLAY1),
                                    );
                                } else if idx == 0 {
                                    ui.label(RichText::new("-").small().color(Mocha::OVERLAY1));
                                }
                            });
                        ui.add_space(4.0);
                    }
                }
            });

            ui.add_space(8.0);
            render_opening_explorer(ui, app);
            ui.add_space(8.0);
            render_eval_graph(ui, app);
            ui.add_space(8.0);
            render_move_history(ui, app, false);
            ui.add_space(6.0);
            render_nav_strip(ui, app);
        });
}

/// Renders the Chess.com-style Game Review panel (Accuracy %, Coach Card, Eval-Graph, Trainer, PGN).
pub fn render_games_panel(ui: &mut Ui, app: &mut App, panel_state: &mut GuiPanelState) {
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui_card(ui, |ui| {
                ui_section_header(ui, "Game Review", Mocha::GREEN, |ui| {
                    if ui_button(ui, "PGN laden...", ButtonVariant::Secondary, None).clicked() {
                        panel_state.pgn_error = None;
                        panel_state.show_pgn_modal = true;
                    }
                });
                ui.add_space(6.0);

                let mut cancel_analysis = false;
                if let Some(pg) = &app.pgn_progress {
                    ui.label(RichText::new(&pg.title).strong().color(Mocha::TEXT));
                    ui.add_space(4.0);
                    if pg.running {
                        let frac = if pg.total_plies > 0 {
                            pg.done_plies as f32 / pg.total_plies as f32
                        } else {
                            0.0
                        };
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::ProgressBar::new(frac)
                                    .desired_width((ui.available_width() - 80.0).max(120.0))
                                    .text(format!("{} / {}", pg.done_plies, pg.total_plies)),
                            );
                            if ui_button(ui, "Stop", ButtonVariant::Danger, None).clicked() {
                                cancel_analysis = true;
                            }
                        });
                    } else {
                        ui.label(
                            RichText::new("Analyse abgeschlossen. Klicke auf Züge zur Inspektion.")
                                .small()
                                .color(Mocha::TEAL),
                        );
                    }
                } else {
                    ui.label(
                        RichText::new(
                            "Lade eine PGN-Partie für Zug-für-Zug-Klassifizierung, Genauigkeit und Coach-Hinweise.",
                        )
                        .small()
                        .color(Mocha::SUBTEXT0),
                    );
                    ui.add_space(6.0);
                    if ui_button(
                        ui,
                        "Beispielpartie: Morphy Opera Game laden",
                        ButtonVariant::Primary,
                        None,
                    )
                    .clicked()
                    {
                        let _ = app.load_pgn_str(SAMPLE_OPERA_PGN, true);
                    }
                }

                if cancel_analysis {
                    app.cancel_pgn_analysis();
                }

                // Coach Card for the selected move
                if let Some(rec) = app.current_move_record().cloned() {
                    ui.add_space(8.0);
                    Frame::new()
                        .fill(Mocha::CRUST)
                        .stroke(Stroke::new(1.0, Mocha::SURFACE0))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("{}. {}", rec.move_number, rec.san))
                                        .strong()
                                        .size(14.5)
                                        .color(Mocha::TEXT),
                                );
                                if let Some(cls) = &rec.classification {
                                    let (_, name, fg, bg) = class_visuals(cls.class);
                                    draw_class_pill(ui, cls);
                                    ui_pill(ui, name, fg, bg);
                                }
                            });
                            ui.add_space(4.0);
                            if let Some(cls) = &rec.classification {
                                ui.label(RichText::new(&cls.explanation).color(Mocha::SUBTEXT1));
                            } else {
                                ui.label(
                                    RichText::new("Noch nicht klassifiziert.")
                                        .small()
                                        .color(Mocha::OVERLAY1),
                                );
                            }
                            if let Some(best) = &rec.best_hint_san {
                                if best != &rec.san {
                                    ui.add_space(4.0);
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new("Bester Zug war:")
                                                .small()
                                                .color(Mocha::SUBTEXT0),
                                        );
                                        ui.label(
                                            RichText::new(best).strong().color(Mocha::GREEN),
                                        );
                                    });
                                }
                            }
                        });
                }
            });

            ui.add_space(8.0);
            render_review_summary(ui, app);
            ui.add_space(8.0);
            render_eval_graph(ui, app);
            ui.add_space(8.0);
            render_trainer_panel(ui, app, &mut panel_state.trainer);
            ui.add_space(8.0);
            render_move_history(ui, app, false);
            ui.add_space(6.0);
            render_nav_strip(ui, app);
        });
}

/// Renders the two-column Chess.com-style move history list with clickable SAN cells.
pub fn render_move_history(ui: &mut Ui, app: &mut App, compact: bool) {
    ui_card(ui, |ui| {
        ui_section_header(ui, "Zugliste", Mocha::TEXT, |ui| {
            ui_pill(
                ui,
                &format!("{} / {}", app.view_ply, app.history.len()),
                Mocha::SUBTEXT0,
                Mocha::SURFACE0,
            );
        });
        ui.add_space(6.0);

        let max_h = if compact { 165.0 } else { 215.0 };
        ScrollArea::vertical()
            .max_height(max_h)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if app.history.is_empty() {
                    ui.label(
                        RichText::new("Noch keine Züge gespielt.")
                            .small()
                            .color(Mocha::OVERLAY1),
                    );
                    return;
                }

                let mut jump_target: Option<usize> = None;
                let num_moves = app.history.len().div_ceil(2);
                egui::Grid::new("chesscom_move_history_grid")
                    .striped(true)
                    .spacing([8.0, 3.0])
                    .num_columns(3)
                    .min_col_width(40.0)
                    .show(ui, |ui| {
                        for move_idx in 0..num_moves {
                            let w_ply = move_idx * 2;
                            let b_ply = move_idx * 2 + 1;

                            ui.label(
                                RichText::new(format!("{}.", move_idx + 1))
                                    .monospace()
                                    .small()
                                    .color(Mocha::OVERLAY1),
                            );

                            for ply_opt in [Some(w_ply), Some(b_ply)] {
                                let ply = ply_opt.unwrap_or(0);
                                if let Some(rec) = app.history.get(ply) {
                                    let is_active = app.view_ply == ply + 1;
                                    ui.horizontal(|ui| {
                                        let bg = if is_active {
                                            Mocha::SURFACE1
                                        } else {
                                            Color32::TRANSPARENT
                                        };
                                        let stroke = if is_active {
                                            Stroke::new(1.0, Mocha::BLUE)
                                        } else {
                                            Stroke::NONE
                                        };
                                        let resp = ui.add(
                                            egui::Button::new(
                                                RichText::new(&rec.san).strong().size(12.5).color(
                                                    if is_active {
                                                        Mocha::YELLOW
                                                    } else {
                                                        Mocha::TEXT
                                                    },
                                                ),
                                            )
                                            .fill(bg)
                                            .stroke(stroke)
                                            .corner_radius(CornerRadius::same(5))
                                            .min_size(vec2(52.0, 22.0)),
                                        );
                                        if resp.clicked() {
                                            jump_target = Some(ply + 1);
                                        }
                                        if !compact {
                                            if let Some(cls) = &rec.classification {
                                                draw_class_pill(ui, cls);
                                            }
                                        }
                                    });
                                } else {
                                    ui.label("");
                                }
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

fn render_nav_strip(ui: &mut Ui, app: &mut App) {
    ui_card(ui, |ui| {
        let count = 6.0;
        let spacing = 6.0;
        let btn_w = ((ui.available_width() - spacing * (count - 1.0)) / count).max(34.0);
        let sz = vec2(btn_w, 32.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;
            if ui_icon_button(ui, IconKind::NavFirst, "An den Anfang", false, sz).clicked() {
                app.jump_start();
            }
            if ui_icon_button(ui, IconKind::NavPrev, "Einen Zug zurück", false, sz).clicked() {
                app.step_backward();
            }
            if ui_icon_button(ui, IconKind::NavNext, "Einen Zug vor", false, sz).clicked() {
                app.step_forward();
            }
            if ui_icon_button(ui, IconKind::NavLast, "Ans Ende", false, sz).clicked() {
                app.jump_end();
            }
            if ui_icon_button(ui, IconKind::Flip, "Brett drehen", app.board_flipped, sz).clicked() {
                app.flip_board();
            }
            if ui_icon_button(ui, IconKind::Undo, "Letzten Zug zurücknehmen", false, sz).clicked()
            {
                app.undo_move();
            }
        });
    });
}

fn render_move_text_input(ui: &mut Ui, app: &mut App, panel_state: &mut GuiPanelState) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Zugeingabe:").small().color(Mocha::SUBTEXT0));
        let resp = ui.add(
            egui::TextEdit::singleline(&mut panel_state.move_text_input)
                .desired_width(110.0)
                .hint_text("e2e4 / Nf3"),
        );
        let enter_hit = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui_button(ui, "Ausführen", ButtonVariant::Secondary, None).clicked() || enter_hit {
            let txt = panel_state.move_text_input.trim().to_string();
            match app.submit_move_text(&txt) {
                Ok(()) => {
                    panel_state.move_text_input.clear();
                    panel_state.move_text_error = None;
                }
                Err(err) => {
                    app.set_error(err.clone());
                    panel_state.move_text_error = Some(err);
                }
            }
        }
    });
    if let Some(err) = &panel_state.move_text_error {
        ui.add_space(2.0);
        ui.label(RichText::new(err).small().color(Mocha::RED));
    }
}

pub use crate::gui_modals::render_modals;

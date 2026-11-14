//! Chess.com-style GUI analysis & review widgets: Accuracy %, Classification Summary,
//! Interactive Eval-Graph, ECO Opening Banner, and "Finde den besseren Zug" Trainer.

use egui::{
    pos2, vec2, Align, Color32, CornerRadius, Frame, Margin, Rect, RichText, Sense, Stroke,
    StrokeKind, Ui,
};
use monolith_core::pgn::MoveClass;
use monolith_core::Color;

use crate::app::{App, MoveRecord};
use crate::gui_theme::Mocha;
use crate::gui_widgets::{
    class_visuals, ui_button, ui_card, ui_pill, ui_section_header, ButtonVariant,
};

/// Computes estimated Chess.com-style accuracy percentage (0.0..=100.0) per side from centipawn loss.
pub fn compute_side_accuracies(history: &[MoveRecord]) -> (Option<f32>, Option<f32>) {
    let calc = |side: Color| -> Option<f32> {
        let mut sum_cpl = 0.0_f64;
        let mut count = 0usize;
        for rec in history {
            if rec.side == side {
                if let Some(cls) = &rec.classification {
                    sum_cpl += cls.cp_loss.max(0) as f64;
                    count += 1;
                }
            }
        }
        if count == 0 {
            return None;
        }
        let avg_cpl = (sum_cpl / count as f64).min(500.0);
        let acc = (103.1668 * (-0.004_354 * avg_cpl).exp() - 3.1669).clamp(0.0, 100.0);
        Some(acc as f32)
    };
    (calc(Color::White), calc(Color::Black))
}

/// Counts classified moves per [`MoveClass`] split into `(white_count, black_count)`.
pub fn classification_summary(history: &[MoveRecord]) -> [(MoveClass, usize, usize); 7] {
    let order = [
        MoveClass::Brilliant,
        MoveClass::Great,
        MoveClass::Best,
        MoveClass::Good,
        MoveClass::Inaccuracy,
        MoveClass::Mistake,
        MoveClass::Blunder,
    ];
    let mut counts = [(0usize, 0usize); 7];
    for rec in history {
        if let Some(cls) = &rec.classification {
            if let Some(idx) = order.iter().position(|c| *c == cls.class) {
                if rec.side == Color::White {
                    counts[idx].0 += 1;
                } else {
                    counts[idx].1 += 1;
                }
            }
        }
    }
    [
        (order[0], counts[0].0, counts[0].1),
        (order[1], counts[1].0, counts[1].1),
        (order[2], counts[2].0, counts[2].1),
        (order[3], counts[3].0, counts[3].1),
        (order[4], counts[4].0, counts[4].1),
        (order[5], counts[5].0, counts[5].1),
        (order[6], counts[6].0, counts[6].1),
    ]
}

/// State for the "Finde den besseren Zug" tactical/blunder trainer.
#[derive(Debug, Clone, Default)]
pub struct TrainerState {
    pub active: bool,
    pub puzzle_ply: Option<usize>,
    pub expected_san: Option<String>,
    pub last_checked_history_len: usize,
    pub feedback: Option<(bool, String)>,
    pub solved_count: usize,
    pub attempts: usize,
}

impl TrainerState {
    /// Finds all plies in `app.history` that are Inaccuracy, Mistake, or Blunder with a known best hint.
    pub fn puzzle_plies(app: &App) -> Vec<usize> {
        app.history
            .iter()
            .enumerate()
            .filter_map(|(idx, rec)| {
                let is_error = rec.classification.as_ref().is_some_and(|c| {
                    matches!(
                        c.class,
                        MoveClass::Inaccuracy | MoveClass::Mistake | MoveClass::Blunder
                    )
                });
                if is_error && rec.best_hint_san.is_some() {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Starts or advances to the next puzzle ply after `after_ply`.
    pub fn next_puzzle(&mut self, app: &mut App, after_ply: Option<usize>) {
        let puzzles = Self::puzzle_plies(app);
        let next = match after_ply {
            Some(prev) => puzzles
                .iter()
                .copied()
                .find(|&p| p > prev)
                .or_else(|| puzzles.first().copied()),
            None => puzzles.first().copied(),
        };

        if let Some(ply_idx) = next {
            self.active = true;
            self.puzzle_ply = Some(ply_idx);
            self.expected_san = app.history[ply_idx].best_hint_san.clone();
            self.feedback = None;
            app.jump_to_ply(ply_idx);
            self.last_checked_history_len = app.history.len();
        } else {
            self.active = false;
            self.puzzle_ply = None;
            self.expected_san = None;
            self.feedback = Some((
                false,
                "Keine Fehler/Patzer mit Analyse-Daten gefunden. Erst Partie analysieren."
                    .to_string(),
            ));
        }
    }

    pub fn stop(&mut self) {
        self.active = false;
        self.puzzle_ply = None;
        self.expected_san = None;
        self.feedback = None;
    }

    /// Evaluates if the user just played a move on the board while in Trainer mode.
    pub fn poll_user_move(&mut self, app: &mut App) {
        if !self.active {
            return;
        }
        let Some(target_ply) = self.puzzle_ply else {
            return;
        };
        let Some(expected) = self.expected_san.clone() else {
            return;
        };

        if app.view_ply == target_ply + 1 && app.history.len() == target_ply + 1 {
            if let Some(played) = app.history.last() {
                if played.san != expected || app.history.len() != self.last_checked_history_len {
                    self.attempts += 1;
                    self.last_checked_history_len = app.history.len();
                    if played.san == expected {
                        self.solved_count += 1;
                        self.feedback =
                            Some((true, format!("Richtig! {expected} ist der beste Zug.")));
                    } else {
                        self.feedback = Some((
                            false,
                            format!(
                                "{} ist nicht optimal. Versuche es erneut oder zeige die Lösung.",
                                played.san
                            ),
                        ));
                    }
                }
            }
        }
    }
}

/// Renders Chess.com-style Accuracy % cards and move classification counts.
pub fn render_review_summary(ui: &mut Ui, app: &App) {
    let (w_acc, b_acc) = compute_side_accuracies(&app.history);
    if w_acc.is_none() && b_acc.is_none() {
        return;
    }

    ui_card(ui, |ui| {
        ui_section_header(ui, "Genauigkeit & Zugqualität", Mocha::GREEN, |_| {});
        ui.add_space(6.0);

        ui.columns(2, |cols| {
            let draw_acc_box = |ui: &mut Ui, side_label: &str, acc: Option<f32>, light: bool| {
                let (bg, txt_col) = if light {
                    (Color32::from_rgb(220, 226, 240), Mocha::CRUST)
                } else {
                    (Mocha::SURFACE0, Mocha::TEXT)
                };
                Frame::new()
                    .fill(bg)
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(side_label).small().color(txt_col));
                            ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                                let s = acc.map_or_else(|| "-".into(), |v| format!("{v:.1}%"));
                                ui.label(
                                    RichText::new(s)
                                        .monospace()
                                        .strong()
                                        .size(15.0)
                                        .color(txt_col),
                                );
                            });
                        });
                    });
            };
            draw_acc_box(&mut cols[0], "Weiß", w_acc, true);
            draw_acc_box(&mut cols[1], "Schwarz", b_acc, false);
        });

        ui.add_space(8.0);
        let summary = classification_summary(&app.history);
        egui::Grid::new("review_summary_grid")
            .num_columns(3)
            .spacing([10.0, 4.0])
            .show(ui, |ui| {
                for (class, w_cnt, b_cnt) in summary {
                    if w_cnt == 0 && b_cnt == 0 {
                        continue;
                    }
                    let (glyph, name, fg, bg) = class_visuals(class);
                    ui.horizontal(|ui| {
                        ui_pill(ui, glyph, fg, bg);
                        ui.label(RichText::new(name).small().color(Mocha::SUBTEXT1));
                    });
                    ui.label(
                        RichText::new(format!("{w_cnt}"))
                            .monospace()
                            .color(Mocha::TEXT),
                    );
                    ui.label(
                        RichText::new(format!("{b_cnt}"))
                            .monospace()
                            .color(Mocha::SUBTEXT0),
                    );
                    ui.end_row();
                }
            });
    });
}

/// Renders the interactive Evaluation Graph across all plies of the current game.
pub fn render_eval_graph(ui: &mut Ui, app: &mut App) {
    ui_card(ui, |ui| {
        ui_section_header(ui, "Bewertungsverlauf", Mocha::BLUE, |ui| {
            ui.label(
                RichText::new("Klick = Zug anspringen")
                    .small()
                    .color(Mocha::OVERLAY1),
            );
        });
        ui.add_space(6.0);

        let graph_h = 78.0;
        let avail_w = ui.available_width().max(120.0);
        let (rect, response) = ui.allocate_exact_size(vec2(avail_w, graph_h), Sense::click());
        let painter = ui.painter_at(rect);

        painter.rect_filled(rect, CornerRadius::same(6), Mocha::CRUST);
        let mid_y = rect.center().y;
        painter.line_segment(
            [pos2(rect.min.x, mid_y), pos2(rect.max.x, mid_y)],
            Stroke::new(1.0, Mocha::SURFACE1),
        );

        let total = app.history.len();
        if total == 0 {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Noch keine Züge gespielt",
                egui::FontId::proportional(12.0),
                Mocha::OVERLAY1,
            );
            return;
        }

        let mut points = Vec::with_capacity(total + 1);
        let max_cp = 600.0_f32;
        for ply in 0..=total {
            let cp = if ply == 0 {
                0.0
            } else {
                app.history[ply - 1].eval_white_cp.unwrap_or(0) as f32
            };
            let clamped = (cp / max_cp).clamp(-1.0, 1.0);
            let x = rect.min.x + (ply as f32 / total as f32) * rect.width();
            let y = mid_y - clamped * (graph_h * 0.44);
            points.push((pos2(x, y), cp));
        }

        let bar_w = (rect.width() / (total as f32 + 1.0)).max(2.0);
        for (ply, &(pt, cp)) in points.iter().enumerate().skip(1) {
            let color = if cp >= 0.0 {
                Color32::from_rgba_unmultiplied(205, 214, 244, 150)
            } else {
                Color32::from_rgba_unmultiplied(108, 112, 134, 175)
            };
            let top = pt.y.min(mid_y);
            let bot = pt.y.max(mid_y);
            let bar_rect = Rect::from_min_max(
                pos2((pt.x - bar_w * 0.45).max(rect.min.x), top),
                pos2((pt.x + bar_w * 0.45).min(rect.max.x), bot.max(top + 1.5)),
            );
            painter.rect_filled(bar_rect, CornerRadius::ZERO, color);

            if let Some(cls) = &app.history[ply - 1].classification {
                if matches!(cls.class, MoveClass::Blunder | MoveClass::Mistake) {
                    let dot_c = if cls.class == MoveClass::Blunder {
                        Mocha::RED
                    } else {
                        Mocha::PEACH
                    };
                    painter.circle_filled(pt, 3.0, dot_c);
                }
            }
        }

        let line_pts: Vec<egui::Pos2> = points.iter().map(|(p, _)| *p).collect();
        painter.add(egui::Shape::line(line_pts, Stroke::new(1.5, Mocha::BLUE)));

        let cur_x = rect.min.x + (app.view_ply.min(total) as f32 / total as f32) * rect.width();
        painter.line_segment(
            [pos2(cur_x, rect.min.y), pos2(cur_x, rect.max.y)],
            Stroke::new(2.0, Mocha::MAUVE),
        );

        painter.rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(1.0, Mocha::SURFACE0),
            StrokeKind::Inside,
        );

        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let frac = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                let target_ply = (frac * total as f32).round() as usize;
                app.jump_to_ply(target_ply);
            }
        }
    });
}

/// Detects the current ECO opening name from the move sequence prefix.
pub fn detect_opening_label(app: &App) -> (&'static str, &'static str) {
    let seq: Vec<&str> = app
        .history
        .iter()
        .take(app.view_ply.min(8))
        .map(|r| r.san.as_str())
        .collect();

    if seq.starts_with(&["e4", "e5", "Nf3", "Nc6", "Bb5"]) {
        ("C60", "Spanische Partie (Ruy Lopez)")
    } else if seq.starts_with(&["e4", "e5", "Nf3", "Nc6", "Bc4"]) {
        ("C50", "Italienische Partie")
    } else if seq.starts_with(&["e4", "e5", "Nf3", "d6"]) {
        ("C41", "Philidor-Verteidigung")
    } else if seq.starts_with(&["e4", "c5"]) {
        ("B20", "Sizilianische Verteidigung")
    } else if seq.starts_with(&["e4", "e6"]) {
        ("C00", "Französische Verteidigung")
    } else if seq.starts_with(&["e4", "c6"]) {
        ("B10", "Caro-Kann Verteidigung")
    } else if seq.starts_with(&["e4", "e5"]) {
        ("C20", "Offenes Spiel (1. e4 e5)")
    } else if seq.starts_with(&["d4", "d5", "c4"]) {
        ("D06", "Damengambit")
    } else if seq.starts_with(&["d4", "Nf6", "c4", "g6"]) {
        ("E60", "Königsindische Verteidigung")
    } else if seq.starts_with(&["d4", "Nf6"]) {
        ("A45", "Indische Verteidigung")
    } else if seq.starts_with(&["c4"]) {
        ("A10", "Englische Eröffnung")
    } else if seq.is_empty() {
        ("A00", "Grundstellung")
    } else {
        ("A00", "Freie Eröffnungsfolge")
    }
}

/// Renders the compact ECO Opening Banner card with subtle planned Polyglot info.
pub fn render_opening_explorer(ui: &mut Ui, app: &App) {
    let (eco, name) = detect_opening_label(app);
    ui_card(ui, |ui| {
        ui.horizontal(|ui| {
            ui_pill(ui, eco, Mocha::CRUST, Mocha::MAUVE);
            ui.label(RichText::new(name).strong().size(13.0).color(Mocha::TEXT));
        });
        ui.add_space(2.0);
        ui.label(
            RichText::new("Eröffnungs-Explorer: Polyglot-Buch (.bin) & W/D/L-Statistik geplant.")
                .small()
                .color(Mocha::OVERLAY1),
        );
    });
}

/// Renders the "Finde den besseren Zug" tactical trainer card using uniform GUI buttons.
pub fn render_trainer_panel(ui: &mut Ui, app: &mut App, trainer: &mut TrainerState) {
    trainer.poll_user_move(app);
    let puzzles = TrainerState::puzzle_plies(app);

    ui_card(ui, |ui| {
        ui_section_header(ui, "Trainer: Finde den besseren Zug", Mocha::YELLOW, |ui| {
            ui_pill(
                ui,
                &format!(
                    "{}/{} gelöst · {} Fehler",
                    trainer.solved_count,
                    trainer.attempts,
                    puzzles.len()
                ),
                Mocha::SUBTEXT1,
                Mocha::SURFACE0,
            );
        });
        ui.add_space(6.0);

        if trainer.active {
            if let Some(ply) = trainer.puzzle_ply {
                let side_str = if ply % 2 == 0 { "Weiß" } else { "Schwarz" };
                ui.label(
                    RichText::new(format!(
                        "Zug {}: Finde eine bessere Fortsetzung für {side_str} direkt auf dem Brett.",
                        (ply / 2) + 1
                    ))
                    .color(Mocha::TEXT),
                );
            }

            if let Some((ok, msg)) = &trainer.feedback {
                ui.add_space(4.0);
                ui.label(RichText::new(msg).strong().color(if *ok {
                    Mocha::GREEN
                } else {
                    Mocha::RED
                }));
            }

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if ui_button(ui, "Erneut", ButtonVariant::Secondary, None).clicked() {
                    if let Some(ply) = trainer.puzzle_ply {
                        app.jump_to_ply(ply);
                        trainer.feedback = None;
                    }
                }
                if ui_button(ui, "Lösung", ButtonVariant::Secondary, None).clicked() {
                    if let Some(exp) = &trainer.expected_san {
                        trainer.feedback = Some((true, format!("Bester Zug laut Engine: {exp}")));
                    }
                }
                if ui_button(ui, "Nächste", ButtonVariant::Primary, None).clicked() {
                    let cur = trainer.puzzle_ply;
                    trainer.next_puzzle(app, cur);
                }
                if ui_button(ui, "Beenden", ButtonVariant::Subtle, None).clicked() {
                    trainer.stop();
                }
            });
        } else {
            if let Some((_, msg)) = &trainer.feedback {
                ui.label(RichText::new(msg).small().color(Mocha::PEACH));
                ui.add_space(4.0);
            }
            if ui_button(
                ui,
                "Fehler der Partie üben",
                ButtonVariant::Primary,
                Some(180.0),
            )
            .clicked()
            {
                trainer.next_puzzle(app, None);
            }
        }
    });
}

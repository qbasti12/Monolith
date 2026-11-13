//! GUI analysis widgets: Interactive Eval-Graph, Opening Explorer card, and "Finde den besseren Zug" Trainer.

use egui::{pos2, vec2, Color32, CornerRadius, Rect, RichText, Sense, Stroke, StrokeKind, Ui};
use monolith_core::pgn::MoveClass;

use crate::app::App;

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
                "Keine Fehler/Einsteller mit Analyse-Daten gefunden. Erst Partie analysieren."
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

        // When user plays a move from `view_ply == target_ply`, `app.play_move` truncates history
        // to `target_ply` and pushes the new move, so `app.view_ply == target_ply + 1`.
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

/// Renders the interactive Evaluation Graph across all plies of the current game.
pub fn render_eval_graph(ui: &mut Ui, app: &mut App) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Bewertungsverlauf (Eval-Graph)")
                    .strong()
                    .size(12.0)
                    .color(Color32::from_rgb(137, 180, 250)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new("Klick = Zug anspringen")
                        .size(10.0)
                        .color(Color32::from_rgb(166, 173, 200)),
                );
            });
        });
        ui.add_space(4.0);

        let graph_h = 74.0;
        let avail_w = ui.available_width().max(120.0);
        let (rect, response) = ui.allocate_exact_size(vec2(avail_w, graph_h), Sense::click());
        let painter = ui.painter_at(rect);

        // Background (dark half for Black advantage, light-tinted upper half for White).
        painter.rect_filled(rect, CornerRadius::same(4), Color32::from_rgb(24, 24, 37));
        let mid_y = rect.center().y;

        // Center 0.00 line.
        painter.line_segment(
            [pos2(rect.min.x, mid_y), pos2(rect.max.x, mid_y)],
            Stroke::new(1.0, Color32::from_rgb(88, 91, 112)),
        );

        let total = app.history.len();
        if total == 0 {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Noch keine Züge gespielt",
                egui::FontId::monospace(10.0),
                Color32::from_rgb(166, 173, 200),
            );
            return;
        }

        // Build points (ply 0 = 0 cp, then each move's white_cp).
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

        // Draw filled bars/columns per ply for clean readability.
        let bar_w = (rect.width() / (total as f32 + 1.0)).max(2.0);
        for (ply, &(pt, cp)) in points.iter().enumerate().skip(1) {
            let color = if cp >= 0.0 {
                Color32::from_rgba_unmultiplied(205, 214, 244, 150) // White advantage
            } else {
                Color32::from_rgba_unmultiplied(108, 112, 134, 170) // Black advantage
            };
            let top = pt.y.min(mid_y);
            let bot = pt.y.max(mid_y);
            let bar_rect = Rect::from_min_max(
                pos2((pt.x - bar_w * 0.45).max(rect.min.x), top),
                pos2((pt.x + bar_w * 0.45).min(rect.max.x), bot.max(top + 1.5)),
            );
            painter.rect_filled(bar_rect, CornerRadius::ZERO, color);

            // Highlight mistake/blunder markers at bottom/top.
            if let Some(cls) = &app.history[ply - 1].classification {
                if matches!(cls.class, MoveClass::Blunder | MoveClass::Mistake) {
                    let dot_c = if cls.class == MoveClass::Blunder {
                        Color32::from_rgb(243, 139, 168)
                    } else {
                        Color32::from_rgb(250, 179, 135)
                    };
                    painter.circle_filled(pt, 2.5, dot_c);
                }
            }
        }

        // Connect evaluation curve.
        let line_pts: Vec<egui::Pos2> = points.iter().map(|(p, _)| *p).collect();
        painter.add(egui::Shape::line(
            line_pts,
            Stroke::new(1.5, Color32::from_rgb(137, 180, 250)),
        ));

        // Current view_ply marker.
        let cur_x = rect.min.x + (app.view_ply.min(total) as f32 / total as f32) * rect.width();
        painter.line_segment(
            [pos2(cur_x, rect.min.y), pos2(cur_x, rect.max.y)],
            Stroke::new(2.0, Color32::from_rgb(203, 166, 247)),
        );

        // Border.
        painter.rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(1.0, Color32::from_rgb(69, 71, 90)),
            StrokeKind::Inside,
        );

        // Click to jump to ply.
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
        ("A00", "Freie / Unklassifizierte Eröffnung")
    }
}

/// Renders the Opening Explorer card with ECO detection and planned module info.
pub fn render_opening_explorer(ui: &mut Ui, app: &App) {
    let (eco, name) = detect_opening_label(app);
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Eröffnungs-Explorer")
                    .strong()
                    .size(12.0)
                    .color(Color32::from_rgb(166, 227, 161)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(eco)
                        .monospace()
                        .strong()
                        .size(11.0)
                        .color(Color32::from_rgb(203, 166, 247)),
                );
            });
        });
        ui.label(
            RichText::new(name)
                .strong()
                .size(12.0)
                .color(Color32::from_rgb(205, 214, 244)),
        );
        ui.label(
            RichText::new(
                "Geplant: Polyglot-Eröffnungsbuch (.bin), Meister-Statistiken (W/D/L) & Zug-Häufigkeiten.",
            )
            .size(10.0)
            .color(Color32::from_rgb(166, 173, 200)),
        );
    });
}

/// Renders the "Finde den besseren Zug" tactical trainer panel.
pub fn render_trainer_panel(ui: &mut Ui, app: &mut App, trainer: &mut TrainerState) {
    trainer.poll_user_move(app);
    let puzzles = TrainerState::puzzle_plies(app);

    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Trainer: Finde den besseren Zug")
                    .strong()
                    .size(12.0)
                    .color(Color32::from_rgb(249, 226, 175)),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!(
                        "Gelöst: {}/{} | Fehler in Partie: {}",
                        trainer.solved_count,
                        trainer.attempts,
                        puzzles.len()
                    ))
                    .size(10.0)
                    .color(Color32::from_rgb(166, 173, 200)),
                );
            });
        });

        ui.add_space(4.0);

        if trainer.active {
            if let Some(ply) = trainer.puzzle_ply {
                let side_str = if ply % 2 == 0 { "Weiß" } else { "Schwarz" };
                ui.label(
                    RichText::new(format!(
                        "Zug {}: Finde eine bessere Fortsetzung für {side_str} direkt auf dem Brett!",
                        (ply / 2) + 1
                    ))
                    .size(11.0)
                    .color(Color32::from_rgb(205, 214, 244)),
                );
            }

            if let Some((ok, msg)) = &trainer.feedback {
                let color = if *ok {
                    Color32::from_rgb(166, 227, 161)
                } else {
                    Color32::from_rgb(243, 139, 168)
                };
                ui.label(RichText::new(msg).strong().size(11.0).color(color));
            }

            ui.horizontal(|ui| {
                if ui.button("Erneut versuchen").clicked() {
                    if let Some(ply) = trainer.puzzle_ply {
                        app.jump_to_ply(ply);
                        trainer.feedback = None;
                    }
                }
                if ui.button("Lösung zeigen").clicked() {
                    if let Some(exp) = &trainer.expected_san {
                        trainer.feedback = Some((true, format!("Bester Zug laut Engine: {exp}")));
                    }
                }
                if ui.button("Nächste Aufgabe").clicked() {
                    let cur = trainer.puzzle_ply;
                    trainer.next_puzzle(app, cur);
                }
                if ui.button("Beenden").clicked() {
                    trainer.stop();
                }
            });
        } else {
            if let Some((_, msg)) = &trainer.feedback {
                ui.label(
                    RichText::new(msg)
                        .size(11.0)
                        .color(Color32::from_rgb(250, 179, 135)),
                );
            }
            if ui.button("Trainer starten (Fehler üben)").clicked() {
                trainer.next_puzzle(app, None);
            }
        }
    });
}

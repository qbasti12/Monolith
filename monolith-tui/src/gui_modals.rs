//! Clean Catppuccin Mocha modal dialog windows (FEN presets/input, PGN import, and Help).

use egui::{Align2, RichText, ScrollArea, TextStyle, Ui, Window};

use crate::app::{
    App, FEN_PRESET_INFINITE_QUEENS, FEN_PRESET_KIWIPETE, FEN_PRESET_STANDARD, SAMPLE_OPERA_PGN,
};
use crate::gui_panels::GuiPanelState;
use crate::gui_theme::Mocha;
use crate::gui_widgets::{ui_button, ButtonVariant};

/// Renders modal dialog windows (FEN preset / input, PGN review / analysis, and Help overlay).
pub fn render_modals(ctx: &egui::Context, app: &mut App, panel_state: &mut GuiPanelState) {
    if panel_state.show_fen_modal {
        Window::new("Stellung laden (FEN)")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui: &mut Ui| {
                ui.label(
                    RichText::new("Preset auswählen oder eigene FEN eingeben:")
                        .color(Mocha::SUBTEXT1),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui: &mut Ui| {
                    if ui_button(ui, "Startstellung", ButtonVariant::Secondary, None).clicked() {
                        panel_state.fen_input = FEN_PRESET_STANDARD.to_string();
                        panel_state.fen_error = None;
                    }
                    if ui_button(ui, "Kiwipete", ButtonVariant::Secondary, None).clicked() {
                        panel_state.fen_input = FEN_PRESET_KIWIPETE.to_string();
                        panel_state.fen_error = None;
                    }
                    if ui_button(ui, "15 Damen", ButtonVariant::Secondary, None).clicked() {
                        panel_state.fen_input = FEN_PRESET_INFINITE_QUEENS.to_string();
                        panel_state.fen_error = None;
                    }
                });

                ui.add_space(6.0);
                ui.add(
                    egui::TextEdit::singleline(&mut panel_state.fen_input)
                        .desired_width(460.0)
                        .font(TextStyle::Monospace),
                );

                if let Some(err) = &panel_state.fen_error {
                    ui.add_space(4.0);
                    ui.label(RichText::new(err).color(Mocha::RED));
                }

                ui.add_space(10.0);
                ui.horizontal(|ui: &mut Ui| {
                    if ui_button(ui, "Laden", ButtonVariant::Primary, Some(96.0)).clicked() {
                        match app.load_fen(&panel_state.fen_input) {
                            Ok(()) => {
                                panel_state.show_fen_modal = false;
                                panel_state.fen_error = None;
                            }
                            Err(e) => {
                                app.set_error(e.clone());
                                panel_state.fen_error = Some(e);
                            }
                        }
                    }
                    if ui_button(ui, "Abbrechen", ButtonVariant::Subtle, None).clicked() {
                        panel_state.show_fen_modal = false;
                        panel_state.fen_error = None;
                    }
                });
            });
    }

    if panel_state.show_pgn_modal {
        Window::new("PGN importieren & analysieren")
            .collapsible(false)
            .default_width(510.0)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui: &mut Ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("PGN-Text einfügen oder Beispielpartie laden:")
                            .color(Mocha::SUBTEXT1),
                    );
                    if ui_button(ui, "Morphy Opera Game", ButtonVariant::Secondary, None).clicked()
                    {
                        panel_state.pgn_input = SAMPLE_OPERA_PGN.to_string();
                        panel_state.pgn_error = None;
                    }
                });

                ui.add_space(6.0);
                ScrollArea::vertical().max_height(210.0).show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut panel_state.pgn_input)
                            .desired_width(488.0)
                            .desired_rows(10)
                            .font(TextStyle::Monospace),
                    );
                });

                if let Some(err) = &panel_state.pgn_error {
                    ui.add_space(4.0);
                    ui.label(RichText::new(err).color(Mocha::RED));
                }

                ui.add_space(10.0);
                ui.horizontal(|ui: &mut Ui| {
                    if ui_button(
                        ui,
                        "Laden & Analysieren",
                        ButtonVariant::Primary,
                        Some(160.0),
                    )
                    .clicked()
                    {
                        match app.load_pgn_str(&panel_state.pgn_input, true) {
                            Ok(()) => {
                                panel_state.show_pgn_modal = false;
                                panel_state.pgn_error = None;
                            }
                            Err(e) => {
                                app.set_error(e.clone());
                                panel_state.pgn_error = Some(e);
                            }
                        }
                    }
                    if ui_button(ui, "Nur laden", ButtonVariant::Secondary, None).clicked() {
                        match app.load_pgn_str(&panel_state.pgn_input, false) {
                            Ok(()) => {
                                panel_state.show_pgn_modal = false;
                                panel_state.pgn_error = None;
                            }
                            Err(e) => {
                                app.set_error(e.clone());
                                panel_state.pgn_error = Some(e);
                            }
                        }
                    }
                    if ui_button(ui, "Schließen", ButtonVariant::Subtle, None).clicked() {
                        panel_state.show_pgn_modal = false;
                        panel_state.pgn_error = None;
                    }
                });
            });
    }

    if panel_state.show_help_modal {
        Window::new("Hilfe & Steuerung")
            .collapsible(false)
            .resizable(false)
            .default_width(500.0)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui: &mut Ui| {
                ui.label(
                    RichText::new("Monolith Chess Studio – Kurzanleitung")
                        .strong()
                        .size(15.0)
                        .color(Mocha::MAUVE),
                );
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                egui::Grid::new("gui_help_grid")
                    .num_columns(2)
                    .spacing([14.0, 6.0])
                    .show(ui, |ui| {
                        let rows = [
                            ("Linksklick / Drag & Drop", "Figur auswählen und ziehen"),
                            (
                                "Rechtsklick (Feld / Ziehen)",
                                "Feld markieren oder Pfeil zeichnen",
                            ),
                            (
                                "Texteingabe (e2e4 / Nf3)",
                                "Zug direkt als UCI oder SAN spielen",
                            ),
                            (
                                "Zug-Navigation",
                                "Durch die Zughistorie springen oder klicken",
                            ),
                            ("FEN / PGN", "Presets, eigene Stellung oder Review laden"),
                            (
                                "Engines (Slots 1–3)",
                                "Elo 800–3200, Tiefe, Binary & Pfeilfarbe",
                            ),
                            (
                                "Terminal 2 (--mode logs)",
                                "Nodes, NPS, Hashfull, SelDepth & UCI-Log",
                            ),
                            ("F1", "Hilfefenster öffnen oder schließen"),
                        ];
                        for (k, desc) in rows {
                            ui.label(RichText::new(k).strong().color(Mocha::BLUE));
                            ui.label(RichText::new(desc).color(Mocha::TEXT));
                            ui.end_row();
                        }
                    });
                ui.add_space(12.0);
                if ui_button(ui, "Schließen", ButtonVariant::Secondary, Some(100.0)).clicked() {
                    panel_state.show_help_modal = false;
                }
            });
    }
}

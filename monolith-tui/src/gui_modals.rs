//! Modal dialog windows (FEN preset / input and PGN review / analysis) for the GUI.

use egui::{Align2, Color32, RichText, ScrollArea, TextStyle, Ui, Window};

use crate::app::{
    App, FEN_PRESET_INFINITE_QUEENS, FEN_PRESET_KIWIPETE, FEN_PRESET_STANDARD, SAMPLE_OPERA_PGN,
};
use crate::gui_panels::GuiPanelState;

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
                                app.set_error(e.clone());
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
                                app.set_error(e.clone());
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
                                app.set_error(e.clone());
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

    // Help Dialog
    if panel_state.show_help_modal {
        Window::new("Hilfe & Steuerung")
            .collapsible(false)
            .resizable(false)
            .default_width(480.0)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui: &mut Ui| {
                ui.label(
                    RichText::new("Monolith Chess Studio – Kurzanleitung")
                        .strong()
                        .color(Color32::from_rgb(203, 166, 247)),
                );
                ui.separator();
                egui::Grid::new("gui_help_grid")
                    .num_columns(2)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        let rows = [
                            (
                                "Linksklick / Drag & Drop",
                                "Figur auswählen und auf Zielfeld ziehen",
                            ),
                            (
                                "Rechtsklick (Feld / Ziehen)",
                                "Feld markieren oder Pfeil zeichnen",
                            ),
                            (
                                "Texteingabe (e2e4 / Nf3)",
                                "Zug direkt als UCI oder SAN eingeben",
                            ),
                            ("Navigation |<< < > >>|", "Durch die Zughistorie blättern"),
                            (
                                "FEN / PGN Dialoge",
                                "Presets, eigene FEN oder PGN-Analyse laden",
                            ),
                            (
                                "Engines (Slots 1-3)",
                                "Elo (800-3200), Tiefe, Binary & Pfeilfarbe",
                            ),
                            (
                                "Terminal 2 (--mode logs)",
                                "Nodes, NPS, Hashfull, SelDepth & UCI-Log",
                            ),
                            ("[?] / F1", "Dieses Hilfefenster umschalten"),
                        ];
                        for (k, desc) in rows {
                            ui.label(
                                RichText::new(k)
                                    .monospace()
                                    .strong()
                                    .color(Color32::from_rgb(137, 180, 250)),
                            );
                            ui.label(desc);
                            ui.end_row();
                        }
                    });
                ui.add_space(8.0);
                if ui.button("Schließen").clicked() {
                    panel_state.show_help_modal = false;
                }
            });
    }
}

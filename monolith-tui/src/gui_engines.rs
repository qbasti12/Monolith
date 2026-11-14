//! Multi-Engine slot configurator (Slots 1–3) with player-safe UCI output.

use egui::{vec2, Align, CornerRadius, Frame, Margin, RichText, ScrollArea, Sense, Stroke, Ui};
use monolith_engine::skill::{MAX_ELO, MIN_ELO};

use crate::app::App;
use crate::gui_board::{engine_color, ENGINE_COLORS, ENGINE_COLOR_NAMES};
use crate::gui_panels::GuiPanelState;
use crate::gui_theme::Mocha;
use crate::gui_widgets::{
    ui_button, ui_card, ui_pill, ui_section_header, ui_segmented_tab, ButtonVariant,
};

/// Renders the Multi-Engine configurator & player-facing output inspector (1–3 engines).
pub fn render_engine_slots(ui: &mut Ui, app: &mut App, panel_state: &mut GuiPanelState) {
    ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui_card(ui, |ui| {
                ui_section_header(ui, "Multi-Engine Konfiguration", Mocha::MAUVE, |ui| {
                    ui.horizontal(|ui| {
                        for i in 0..app.engines.len() {
                            let sel = panel_state.selected_engine_tab == i;
                            if ui_segmented_tab(ui, &format!("Slot {}", i + 1), sel, 56.0)
                                .clicked()
                            {
                                panel_state.selected_engine_tab = i;
                            }
                        }
                    });
                });
                ui.add_space(8.0);

                let tab_idx = panel_state
                    .selected_engine_tab
                    .min(app.engines.len().saturating_sub(1));
                let mut cfg_changed = false;

                if let Some(slot) = app.engines.get_mut(tab_idx) {
                    let bot_col = engine_color(slot.config.color_idx);
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut slot.config.enabled, "Slot aktiviert").changed() {
                            cfg_changed = true;
                        }
                        ui.label(RichText::new(&slot.config.label).strong().color(bot_col));

                        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                            let (status_txt, fg, bg) = if !slot.config.enabled {
                                ("Deaktiviert", Mocha::SUBTEXT0, Mocha::SURFACE0)
                            } else if slot.telemetry.searching {
                                ("Berechnet", Mocha::CRUST, Mocha::GREEN)
                            } else if slot.telemetry.connected {
                                ("Bereit", Mocha::CRUST, Mocha::TEAL)
                            } else {
                                ("Offline", Mocha::CRUST, Mocha::RED)
                            };
                            ui_pill(ui, status_txt, fg, bg);
                        });
                    });

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Pfeil-Farbe:").small().color(Mocha::SUBTEXT0));
                        for (c_idx, &c) in ENGINE_COLORS.iter().enumerate() {
                            let selected = slot.config.color_idx == c_idx;
                            let (swatch_r, resp) =
                                ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
                            let p = ui.painter();
                            p.circle_filled(swatch_r.center(), 7.5, c);
                            if selected {
                                p.circle_stroke(
                                    swatch_r.center(),
                                    9.5,
                                    Stroke::new(1.5, Mocha::TEXT),
                                );
                            }
                            if resp.on_hover_text(ENGINE_COLOR_NAMES[c_idx]).clicked() {
                                slot.config.color_idx = c_idx;
                            }
                        }
                    });

                    ui.add_space(6.0);
                    if ui
                        .checkbox(&mut slot.config.limit_strength, "Elo-Drosselung aktivieren")
                        .changed()
                    {
                        cfg_changed = true;
                    }
                    if ui
                        .add_enabled(
                            slot.config.limit_strength,
                            egui::Slider::new(&mut slot.config.elo, MIN_ELO..=MAX_ELO)
                                .text("Ziel-Elo"),
                        )
                        .changed()
                    {
                        cfg_changed = true;
                    }
                    let mut depth_u32 = slot.config.depth as u32;
                    if ui
                        .add(egui::Slider::new(&mut depth_u32, 1..=30).text("Suchtiefe"))
                        .changed()
                    {
                        slot.config.depth = depth_u32 as u8;
                        cfg_changed = true;
                    }

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Binary:").small().color(Mocha::SUBTEXT0));
                        ui.add(
                            egui::TextEdit::singleline(&mut slot.config.binary_path)
                                .desired_width(180.0)
                                .font(egui::TextStyle::Monospace),
                        );
                        if ui_button(ui, "Anwenden", ButtonVariant::Secondary, None).clicked() {
                            cfg_changed = true;
                        }
                    });

                    ui.add_space(8.0);
                    Frame::new()
                        .fill(Mocha::CRUST)
                        .stroke(Stroke::new(1.0, Mocha::SURFACE0))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(Margin::same(10))
                        .show(ui, |ui| {
                            let score_str = if let Some(m) = slot.telemetry.white_score_mate {
                                format!("Matt in {m}")
                            } else if let Some(cp) = slot.telemetry.white_score_cp {
                                format!("{:+.2}", cp as f32 / 100.0)
                            } else {
                                "-".to_string()
                            };
                            let best = slot.telemetry.best_move_san.as_deref().unwrap_or("-");
                            let reply =
                                slot.telemetry.expected_reply_san.as_deref().unwrap_or("-");

                            egui::Grid::new("engine_player_summary")
                                .num_columns(2)
                                .spacing([14.0, 4.0])
                                .show(ui, |ui| {
                                    ui.label(RichText::new("Bewertung:").color(Mocha::SUBTEXT0));
                                    ui.label(
                                        RichText::new(score_str).strong().color(Mocha::YELLOW),
                                    );
                                    ui.end_row();

                                    ui.label(RichText::new("Tiefe:").color(Mocha::SUBTEXT0));
                                    ui.label(format!("{}", slot.telemetry.depth));
                                    ui.end_row();

                                    ui.label(RichText::new("Bester Zug:").color(Mocha::SUBTEXT0));
                                    ui.label(RichText::new(best).strong().color(Mocha::GREEN));
                                    ui.end_row();

                                    ui.label(
                                        RichText::new("Erwartete Antwort:")
                                            .color(Mocha::SUBTEXT0),
                                    );
                                    ui.label(
                                        RichText::new(reply).strong().color(Mocha::SAPPHIRE),
                                    );
                                    ui.end_row();
                                });

                            ui.add_space(4.0);
                            let pv = if slot.telemetry.pv_san.is_empty() {
                                "..."
                            } else {
                                &slot.telemetry.pv_san
                            };
                            ui.label(
                                RichText::new(format!("PV: {pv}"))
                                    .monospace()
                                    .small()
                                    .color(Mocha::SUBTEXT1),
                            );
                        });

                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(
                            "Detail-Telemetrie (Nodes/NPS/Hashfull/SelDepth/UCI) nur in Terminal 2 (--mode logs).",
                        )
                        .small()
                        .color(Mocha::OVERLAY1),
                    );
                }

                if cfg_changed {
                    app.sync_engines_config();
                    app.trigger_engine_searches();
                }
            });
        });
}

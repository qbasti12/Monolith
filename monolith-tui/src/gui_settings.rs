//! Clean standard-user Settings panel: piece set previews, board theme, typography, gameplay, and IPC.

use egui::{vec2, Align, Color32, CornerRadius, Rect, RichText, Stroke, StrokeKind, Ui};
use monolith_core::{Color, PieceType};

use crate::app::App;
use crate::gui_board::GuiBoardState;
use crate::gui_panels::GuiPanelState;
use crate::gui_pieces::{render_piece_styled, PieceSet};
use crate::gui_sounds::SoundPlayer;
use crate::gui_theme::{BoardTheme, Mocha};
use crate::gui_widgets::{
    ui_button, ui_card, ui_pill, ui_section_header, ui_segmented_tab, ButtonVariant,
};

/// Typography scaling options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FontScale {
    Compact,
    #[default]
    Normal,
    Large,
}

impl FontScale {
    pub const ALL: [FontScale; 3] = [FontScale::Compact, FontScale::Normal, FontScale::Large];

    pub fn label(self) -> &'static str {
        match self {
            Self::Compact => "Kompakt (12.5 pt)",
            Self::Normal => "Standard (14 pt)",
            Self::Large => "Groß (15.5 pt)",
        }
    }
}

/// Settings panel state and preferences (proportional UI by default for standard users).
#[derive(Debug, Clone)]
pub struct GuiSettingsState {
    pub font_scale: FontScale,
    pub monospace_ui: bool,
    pub highlight_last_move: bool,
    pub highlight_legal_moves: bool,
}

impl Default for GuiSettingsState {
    fn default() -> Self {
        Self {
            font_scale: FontScale::Normal,
            monospace_ui: false,
            highlight_last_move: true,
            highlight_legal_moves: true,
        }
    }
}

/// Renders the Settings view using unified Catppuccin Mocha cards, controls, and live SVG piece previews.
pub fn render_settings_panel(
    ui: &mut Ui,
    app: &mut App,
    board_state: &mut GuiBoardState,
    panel_state: &mut GuiPanelState,
    settings_state: &mut GuiSettingsState,
    sound_player: &mut SoundPlayer,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Section 1: Piece Set with live SVG preview strip
            ui_card(ui, |ui| {
                ui_section_header(ui, "Figurensatz", Mocha::BLUE, |ui| {
                    ui_pill(ui, "Vektor SVG", Mocha::CRUST, Mocha::BLUE);
                });
                ui.add_space(6.0);

                for set in PieceSet::ALL {
                    let selected = board_state.piece_set == set;
                    let fill = if selected {
                        Mocha::SURFACE0
                    } else {
                        Mocha::CRUST
                    };
                    let stroke = if selected {
                        Mocha::BLUE
                    } else {
                        Mocha::SURFACE0
                    };

                    egui::Frame::new()
                        .fill(fill)
                        .stroke(Stroke::new(1.0, stroke))
                        .corner_radius(CornerRadius::same(8))
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if ui
                                    .radio(
                                        selected,
                                        RichText::new(set.display_name()).color(if selected {
                                            Mocha::TEXT
                                        } else {
                                            Mocha::SUBTEXT0
                                        }),
                                    )
                                    .clicked()
                                {
                                    board_state.piece_set = set;
                                }

                                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                                    let (preview_rect, resp) = ui.allocate_exact_size(
                                        vec2(84.0, 22.0),
                                        egui::Sense::click(),
                                    );
                                    if resp.clicked() {
                                        board_state.piece_set = set;
                                    }
                                    let painter = ui.painter_at(preview_rect);
                                    let sample = [
                                        (Color::White, PieceType::King),
                                        (Color::White, PieceType::Queen),
                                        (Color::Black, PieceType::Knight),
                                        (Color::Black, PieceType::Pawn),
                                    ];
                                    for (i, (col, pt)) in sample.into_iter().enumerate() {
                                        let slot = Rect::from_min_size(
                                            egui::pos2(
                                                preview_rect.min.x + i as f32 * 21.0,
                                                preview_rect.min.y,
                                            ),
                                            vec2(20.0, 22.0),
                                        );
                                        painter.rect_filled(
                                            slot,
                                            CornerRadius::same(4),
                                            if i % 2 == 0 {
                                                Color32::from_rgb(220, 226, 240)
                                            } else {
                                                Color32::from_rgb(106, 114, 142)
                                            },
                                        );
                                        render_piece_styled(ui, &painter, slot, pt, col, set);
                                    }
                                });
                            });
                        });
                    ui.add_space(4.0);
                }
            });

            ui.add_space(8.0);

            // Section 2: Board Theme & Presentation
            ui_card(ui, |ui| {
                ui_section_header(ui, "Brett & Darstellung", Mocha::GREEN, |_| {});
                ui.add_space(6.0);

                ui.label(RichText::new("Brett-Farben:").small().color(Mocha::SUBTEXT0));
                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    for theme in BoardTheme::ALL {
                        let selected = board_state.board_theme == theme;
                        let (light, dark, _) = theme.colors();
                        let (swatch_r, _) =
                            ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::hover());
                        let p = ui.painter();
                        p.rect_filled(
                            Rect::from_min_size(swatch_r.min, vec2(9.0, 18.0)),
                            CornerRadius::same(2),
                            light,
                        );
                        p.rect_filled(
                            Rect::from_min_size(
                                egui::pos2(swatch_r.min.x + 9.0, swatch_r.min.y),
                                vec2(9.0, 18.0),
                            ),
                            CornerRadius::same(2),
                            dark,
                        );
                        p.rect_stroke(
                            swatch_r,
                            CornerRadius::same(3),
                            Stroke::new(1.0, Mocha::SURFACE1),
                            StrokeKind::Inside,
                        );
                        if ui_segmented_tab(ui, theme.label(), selected, 0.0).clicked() {
                            board_state.board_theme = theme;
                        }
                    }
                });

                ui.add_space(6.0);
                ui.checkbox(&mut board_state.show_coords, "Koordinaten (a-h / 1-8) anzeigen");
                ui.checkbox(
                    &mut settings_state.highlight_last_move,
                    "Letzten Zug farblich markieren",
                );
                ui.checkbox(
                    &mut settings_state.highlight_legal_moves,
                    "Legale Zugziele markieren (Punkte & Schlagringe)",
                );
                ui.checkbox(&mut app.board_flipped, "Brett drehen (Schwarz unten)");
                ui.checkbox(
                    &mut board_state.show_engine_arrows,
                    "Multi-Engine Best-Move-Pfeile anzeigen",
                );

                ui.add_space(6.0);
                if ui_button(
                    ui,
                    "Pfeile & Markierungen zurücksetzen",
                    ButtonVariant::Secondary,
                    None,
                )
                .clicked()
                {
                    board_state.clear_annotations();
                }
            });

            ui.add_space(8.0);

            // Section 3: Typography
            ui_card(ui, |ui| {
                ui_section_header(ui, "Typografie", Mocha::YELLOW, |_| {});
                ui.add_space(6.0);

                ui.label(RichText::new("Schriftgröße:").small().color(Mocha::SUBTEXT0));
                ui.horizontal_wrapped(|ui| {
                    for scale in FontScale::ALL {
                        if ui_segmented_tab(ui, scale.label(), settings_state.font_scale == scale, 0.0)
                            .clicked()
                        {
                            settings_state.font_scale = scale;
                        }
                    }
                });

                ui.add_space(6.0);
                ui.label(RichText::new("Schriftstil:").small().color(Mocha::SUBTEXT0));
                ui.horizontal(|ui| {
                    if ui_segmented_tab(
                        ui,
                        "Clean Sans (Standard)",
                        !settings_state.monospace_ui,
                        130.0,
                    )
                    .clicked()
                    {
                        settings_state.monospace_ui = false;
                    }
                    if ui_segmented_tab(
                        ui,
                        "Monospace (Technisch)",
                        settings_state.monospace_ui,
                        130.0,
                    )
                    .clicked()
                    {
                        settings_state.monospace_ui = true;
                    }
                });
            });

            ui.add_space(8.0);

            // Section 4: Gameplay & Audio
            ui_card(ui, |ui| {
                ui_section_header(ui, "Partie & Audio", Mocha::PEACH, |_| {});
                ui.add_space(6.0);
                ui.checkbox(
                    &mut panel_state.show_hints_in_play,
                    "Bewertungsbalken & Pfeile auch in Partien einblenden",
                );
                ui.checkbox(&mut sound_player.enabled, "Sound-Effekte (Chess.com Stil)");
            });

            ui.add_space(8.0);

            // Section 5: Telemetry & IPC (strictly pointing detailed telemetry to Terminal 2)
            ui_card(ui, |ui| {
                let ipc_active = app.ipc_sender.is_some();
                ui_section_header(ui, "Telemetrie & IPC", Mocha::TEAL, |ui| {
                    ui_pill(
                        ui,
                        if ipc_active { "Aktiv" } else { "Inaktiv" },
                        Mocha::CRUST,
                        if ipc_active {
                            Mocha::GREEN
                        } else {
                            Mocha::YELLOW
                        },
                    );
                });
                ui.add_space(4.0);

                let ipc_target = app.ipc_sender.as_ref().map_or_else(
                    || format!("UDP 127.0.0.1:{}", crate::ipc::DEFAULT_IPC_PORT),
                    |s| format!("UDP {}", s.target()),
                );
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Ziel:").small().color(Mocha::SUBTEXT0));
                    ui.label(RichText::new(ipc_target).monospace().color(Mocha::TEXT));
                });
                ui.label(
                    RichText::new(
                        "Nodes, NPS, Hashfull, SelDepth, UCI-Rohzeilen & Eval-Breakdown erscheinen ausschließlich in Terminal 2 (--mode logs).",
                    )
                    .small()
                    .color(Mocha::OVERLAY1),
                );
            });
        });
}

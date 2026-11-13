//! GUI Settings panel: piece set selection, board options, typography, engine and telemetry.

use egui::{Color32, RichText, Ui};

use crate::app::App;
use crate::gui_board::GuiBoardState;
use crate::gui_panels::GuiPanelState;
use crate::gui_pieces::PieceSet;
use crate::gui_sounds::SoundPlayer;

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
            Self::Compact => "Kompakt (12 pt)",
            Self::Normal => "Standard (14 pt)",
            Self::Large => "Groß (16 pt)",
        }
    }
}

/// Settings panel state and preferences.
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
            monospace_ui: true,
            highlight_last_move: true,
            highlight_legal_moves: true,
        }
    }
}

/// Renders the complete, clean Settings view (no emojis, flat Catppuccin Mocha colors).
pub fn render_settings_panel(
    ui: &mut Ui,
    app: &mut App,
    board_state: &mut GuiBoardState,
    panel_state: &mut GuiPanelState,
    settings_state: &mut GuiSettingsState,
    sound_player: &mut SoundPlayer,
) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Einstellungen")
                    .strong()
                    .size(15.0)
                    .color(Color32::from_rgb(203, 166, 247)),
            );
        });
        ui.add_space(8.0);

        // Section 1: Piece Set Theme
        ui.group(|ui| {
            ui.label(
                RichText::new("Figurensatz")
                    .strong()
                    .color(Color32::from_rgb(137, 180, 250)),
            );
            ui.label(
                RichText::new("Wähle das Vektor-Design für alle Schachfiguren:")
                    .size(11.0)
                    .color(Color32::from_rgb(166, 173, 200)),
            );
            ui.add_space(4.0);

            for set in PieceSet::ALL {
                let is_selected = board_state.piece_set == set;
                if ui
                    .radio(
                        is_selected,
                        RichText::new(set.display_name()).color(if is_selected {
                            Color32::from_rgb(203, 166, 247)
                        } else {
                            Color32::from_rgb(205, 214, 244)
                        }),
                    )
                    .clicked()
                {
                    board_state.piece_set = set;
                }
            }
        });

        ui.add_space(8.0);

        // Section 2: Board Presentation
        ui.group(|ui| {
            ui.label(
                RichText::new("Brett & Anzeige")
                    .strong()
                    .color(Color32::from_rgb(166, 227, 161)),
            );
            ui.add_space(4.0);

            ui.checkbox(
                &mut board_state.show_coords,
                "Koordinaten (1..8 und a..h) anzeigen",
            );
            ui.checkbox(
                &mut settings_state.highlight_last_move,
                "Letzten Zug farblich hervorheben",
            );
            ui.checkbox(
                &mut settings_state.highlight_legal_moves,
                "Legale Zugziele markieren (Punkte & Schlagringe)",
            );

            if ui
                .checkbox(&mut app.board_flipped, "Brett drehen (Schwarz unten)")
                .changed()
            {
                // App state updated automatically
            }

            ui.checkbox(
                &mut board_state.show_engine_arrows,
                "Multi-Engine Pfeile auf dem Brett anzeigen",
            );

            ui.add_space(4.0);
            if ui.button("Brett-Markierungen löschen").clicked() {
                board_state.clear_annotations();
            }
        });

        ui.add_space(8.0);

        // Section 3: Typography & Fonts
        ui.group(|ui| {
            ui.label(
                RichText::new("Typografie")
                    .strong()
                    .color(Color32::from_rgb(249, 226, 175)),
            );
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("Schriftgröße:");
                for scale in FontScale::ALL {
                    if ui
                        .selectable_label(settings_state.font_scale == scale, scale.label())
                        .clicked()
                    {
                        settings_state.font_scale = scale;
                    }
                }
            });

            ui.checkbox(
                &mut settings_state.monospace_ui,
                "Monospace-Schriftart (JetBrains Mono Stil) priorisieren",
            );
        });

        ui.add_space(8.0);

        // Section 4: Match & Assistance Settings
        ui.group(|ui| {
            ui.label(
                RichText::new("Spielmodus & Hilfen")
                    .strong()
                    .color(Color32::from_rgb(245, 194, 231)),
            );
            ui.add_space(4.0);

            ui.checkbox(
                &mut panel_state.show_hints_in_play,
                "Bewertungsbalken und Zugvorschläge während der Partie einblenden",
            );
            ui.checkbox(&mut sound_player.enabled, "Sound-Effekte (Chess.com Stil)");
        });

        ui.add_space(8.0);

        // Section 5: Telemetry & Network
        ui.group(|ui| {
            ui.label(
                RichText::new("Telemetrie & IPC")
                    .strong()
                    .color(Color32::from_rgb(148, 226, 213)),
            );
            ui.add_space(4.0);

            let ipc_target = app.ipc_sender.as_ref().map_or_else(
                || format!("UDP 127.0.0.1:{}", crate::ipc::DEFAULT_IPC_PORT),
                |s| format!("UDP {}", s.target()),
            );
            let ipc_active = app.ipc_sender.is_some();

            ui.horizontal(|ui| {
                ui.label("IPC-Socket:");
                ui.label(
                    RichText::new(ipc_target)
                        .monospace()
                        .color(Color32::from_rgb(205, 214, 244)),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Status:");
                ui.label(
                    RichText::new(if ipc_active {
                        "Aktiv (--mode logs)"
                    } else {
                        "Inaktiv (--no-ipc)"
                    })
                    .strong()
                    .color(if ipc_active {
                        Color32::from_rgb(166, 227, 161)
                    } else {
                        Color32::from_rgb(249, 226, 175)
                    }),
                );
            });
            ui.label(
                RichText::new(
                    "Nodes, NPS, Hashfull, SelDepth, UCI-Log & Eval-Breakdown nur in Terminal 2.",
                )
                .small()
                .color(Color32::from_rgb(166, 173, 200)),
            );
        });
    });
}

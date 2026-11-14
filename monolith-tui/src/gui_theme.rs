//! Catppuccin Mocha design tokens, board color themes, and global `egui` styling for Monolith GUI.

use egui::epaint::Shadow;
use egui::{vec2, Color32, Context, CornerRadius, FontId, Margin, Stroke, TextStyle, Visuals};

use crate::gui_settings::FontScale;

/// Flat Catppuccin Mocha color tokens (no blue-purple gradients).
pub struct Mocha;

impl Mocha {
    pub const CRUST: Color32 = Color32::from_rgb(17, 17, 27);
    pub const MANTLE: Color32 = Color32::from_rgb(24, 24, 37);
    pub const BASE: Color32 = Color32::from_rgb(30, 30, 46);
    pub const SURFACE0: Color32 = Color32::from_rgb(49, 50, 68);
    pub const SURFACE1: Color32 = Color32::from_rgb(69, 71, 90);
    pub const SURFACE2: Color32 = Color32::from_rgb(88, 91, 112);
    pub const OVERLAY0: Color32 = Color32::from_rgb(108, 112, 134);
    pub const OVERLAY1: Color32 = Color32::from_rgb(127, 132, 156);
    pub const SUBTEXT0: Color32 = Color32::from_rgb(166, 173, 200);
    pub const SUBTEXT1: Color32 = Color32::from_rgb(186, 194, 222);
    pub const TEXT: Color32 = Color32::from_rgb(205, 214, 244);

    pub const MAUVE: Color32 = Color32::from_rgb(203, 166, 247);
    pub const BLUE: Color32 = Color32::from_rgb(137, 180, 250);
    pub const SAPPHIRE: Color32 = Color32::from_rgb(116, 199, 236);
    pub const TEAL: Color32 = Color32::from_rgb(148, 226, 213);
    pub const GREEN: Color32 = Color32::from_rgb(166, 227, 161);
    pub const YELLOW: Color32 = Color32::from_rgb(249, 226, 175);
    pub const PEACH: Color32 = Color32::from_rgb(250, 179, 135);
    pub const RED: Color32 = Color32::from_rgb(243, 139, 168);
}

/// Chessboard color themes harmonious with the dark Catppuccin Mocha shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BoardTheme {
    #[default]
    MochaSlate,
    ChessSage,
    MochaWalnut,
}

impl BoardTheme {
    pub const ALL: [BoardTheme; 3] = [
        BoardTheme::MochaSlate,
        BoardTheme::ChessSage,
        BoardTheme::MochaWalnut,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::MochaSlate => "Catppuccin Slate",
            Self::ChessSage => "Turnier-Grün (Sage)",
            Self::MochaWalnut => "Matt-Walnuss",
        }
    }

    pub fn colors(self) -> (Color32, Color32, Color32) {
        match self {
            // (light square, dark square, highlighted last-move tint)
            Self::MochaSlate => (
                Color32::from_rgb(220, 226, 240),
                Color32::from_rgb(106, 114, 142),
                Color32::from_rgba_unmultiplied(137, 180, 250, 105),
            ),
            Self::ChessSage => (
                Color32::from_rgb(235, 236, 208),
                Color32::from_rgb(115, 149, 82),
                Color32::from_rgba_unmultiplied(245, 246, 130, 125),
            ),
            Self::MochaWalnut => (
                Color32::from_rgb(232, 214, 188),
                Color32::from_rgb(144, 106, 76),
                Color32::from_rgba_unmultiplied(249, 226, 175, 115),
            ),
        }
    }
}

/// Applies the unified flat Catppuccin Mocha design system and clean typography across `ctx`.
pub fn apply_mocha_theme(ctx: &Context, scale: FontScale, monospace_ui: bool) {
    let mut visuals = Visuals::dark();
    visuals.dark_mode = true;
    visuals.panel_fill = Mocha::BASE;
    visuals.window_fill = Mocha::MANTLE;
    visuals.faint_bg_color = Color32::from_rgb(36, 37, 54);
    visuals.extreme_bg_color = Mocha::CRUST;

    visuals.window_corner_radius = CornerRadius::same(10);
    visuals.menu_corner_radius = CornerRadius::same(8);
    visuals.window_stroke = Stroke::new(1.0, Mocha::SURFACE1);
    visuals.window_shadow = Shadow {
        offset: [0, 6],
        blur: 16,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 110),
    };
    visuals.popup_shadow = visuals.window_shadow;

    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(137, 180, 250, 60);
    visuals.selection.stroke = Stroke::new(1.0, Mocha::BLUE);

    let radius = CornerRadius::same(6);
    visuals.widgets.noninteractive.bg_fill = Mocha::BASE;
    visuals.widgets.noninteractive.weak_bg_fill = Mocha::MANTLE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Mocha::SURFACE0);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Mocha::TEXT);
    visuals.widgets.noninteractive.corner_radius = radius;

    visuals.widgets.inactive.bg_fill = Mocha::SURFACE0;
    visuals.widgets.inactive.weak_bg_fill = Mocha::SURFACE0;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Mocha::SURFACE1);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Mocha::TEXT);
    visuals.widgets.inactive.corner_radius = radius;

    visuals.widgets.hovered.bg_fill = Mocha::SURFACE1;
    visuals.widgets.hovered.weak_bg_fill = Mocha::SURFACE1;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Mocha::OVERLAY0);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.2, Mocha::TEXT);
    visuals.widgets.hovered.corner_radius = radius;
    visuals.widgets.hovered.expansion = 0.0;

    visuals.widgets.active.bg_fill = Mocha::SURFACE2;
    visuals.widgets.active.weak_bg_fill = Mocha::SURFACE2;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, Mocha::BLUE);
    visuals.widgets.active.fg_stroke = Stroke::new(1.2, Mocha::TEXT);
    visuals.widgets.active.corner_radius = radius;
    visuals.widgets.active.expansion = 0.0;

    visuals.widgets.open.bg_fill = Mocha::SURFACE1;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, Mocha::BLUE);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, Mocha::TEXT);
    visuals.widgets.open.corner_radius = radius;

    ctx.set_visuals(visuals);

    let base_size = match scale {
        FontScale::Compact => 12.5,
        FontScale::Normal => 14.0,
        FontScale::Large => 15.5,
    };

    let make_ui_font = |size: f32| -> FontId {
        if monospace_ui {
            FontId::monospace(size)
        } else {
            FontId::proportional(size)
        }
    };

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = vec2(8.0, 6.0);
    style.spacing.button_padding = vec2(12.0, 6.0);
    style.spacing.window_margin = Margin::same(14);
    style.spacing.interact_size = vec2(36.0, 28.0);
    style.spacing.slider_width = 160.0;
    style.text_styles = [
        (TextStyle::Small, make_ui_font(base_size * 0.84)),
        (TextStyle::Body, make_ui_font(base_size)),
        (TextStyle::Button, make_ui_font(base_size * 0.94)),
        (TextStyle::Heading, make_ui_font(base_size * 1.24)),
        (TextStyle::Monospace, FontId::monospace(base_size * 0.92)),
    ]
    .into();
    ctx.set_style(style);
}

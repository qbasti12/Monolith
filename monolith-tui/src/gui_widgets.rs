//! Uniform GUI primitives and custom geometric vector icons (no emojis, consistent styling).

use egui::epaint::PathShape;
use egui::{
    pos2, vec2, Align, Align2, Color32, CornerRadius, Frame, Margin, Painter, Pos2, Rect, Response,
    RichText, Sense, Stroke, StrokeKind, Ui,
};
use monolith_core::pgn::{MoveClass, MoveClassification};

use crate::gui_theme::Mocha;

/// Minimal custom geometric icons rendered via `egui::Painter` (zero emojis or ASCII brackets).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    BrandRook,
    Play,
    BoardAnalyze,
    ReviewChart,
    Engines,
    Settings,
    NavFirst,
    NavPrev,
    NavNext,
    NavLast,
    Flip,
    Undo,
    Clear,
    Help,
    Close,
    User,
    Bot,
}

/// Draws `icon` inside `rect` using crisp geometric vector primitives.
pub fn draw_icon(painter: &Painter, rect: Rect, icon: IconKind, color: Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.42;
    let stroke = Stroke::new((r * 0.22).clamp(1.4, 2.2), color);

    match icon {
        IconKind::BrandRook => {
            let bw = r * 1.45;
            let base = Rect::from_center_size(pos2(c.x, c.y + r * 0.68), vec2(bw, r * 0.34));
            let body = Rect::from_center_size(pos2(c.x, c.y + r * 0.08), vec2(bw * 0.66, r * 0.96));
            let top = Rect::from_center_size(pos2(c.x, c.y - r * 0.56), vec2(bw * 0.9, r * 0.34));
            painter.rect_filled(base, CornerRadius::same(2), color);
            painter.rect_filled(body, CornerRadius::same(1), color);
            painter.rect_filled(top, CornerRadius::same(1), color);
            for dx in [-0.34_f32, 0.0, 0.34] {
                let merlon = Rect::from_center_size(
                    pos2(c.x + dx * bw * 0.85, c.y - r * 0.80),
                    vec2(bw * 0.2, r * 0.24),
                );
                painter.rect_filled(merlon, CornerRadius::same(1), color);
            }
        }
        IconKind::Play => {
            let tri = vec![
                pos2(c.x - r * 0.5, c.y - r * 0.72),
                pos2(c.x + r * 0.78, c.y),
                pos2(c.x - r * 0.5, c.y + r * 0.72),
            ];
            painter.add(PathShape::convex_polygon(tri, color, Stroke::NONE));
        }
        IconKind::BoardAnalyze => {
            let box_r = Rect::from_center_size(c, vec2(r * 1.65, r * 1.65));
            let q = box_r.width() * 0.5;
            painter.rect_filled(
                Rect::from_min_size(box_r.min, vec2(q, q)),
                CornerRadius::ZERO,
                color,
            );
            painter.rect_filled(
                Rect::from_min_size(pos2(box_r.min.x + q, box_r.min.y + q), vec2(q, q)),
                CornerRadius::ZERO,
                color,
            );
            painter.rect_stroke(box_r, CornerRadius::same(2), stroke, StrokeKind::Inside);
        }
        IconKind::ReviewChart => {
            let b_w = r * 0.38;
            for (idx, h_fac) in [0.55_f32, 1.05, 0.78].iter().enumerate() {
                let x = c.x + (idx as f32 - 1.0) * r * 0.58;
                let h = r * 1.3 * h_fac;
                let bar = Rect::from_min_max(
                    pos2(x - b_w * 0.5, c.y + r * 0.72 - h),
                    pos2(x + b_w * 0.5, c.y + r * 0.72),
                );
                painter.rect_filled(bar, CornerRadius::same(1), color);
            }
        }
        IconKind::Engines => {
            for (dy, knob_x) in [(-0.42_f32, 0.35_f32), (0.0, -0.3), (0.42, 0.25)] {
                let y = c.y + dy * r * 1.25;
                painter.line_segment([pos2(c.x - r * 0.85, y), pos2(c.x + r * 0.85, y)], stroke);
                painter.circle_filled(pos2(c.x + knob_x * r, y), r * 0.24, color);
            }
        }
        IconKind::Settings => {
            painter.circle_stroke(c, r * 0.45, stroke);
            for k in 0..6 {
                let a = (k as f32) * std::f32::consts::FRAC_PI_3;
                let p0 = pos2(c.x + a.cos() * r * 0.56, c.y + a.sin() * r * 0.56);
                let p1 = pos2(c.x + a.cos() * r * 0.88, c.y + a.sin() * r * 0.88);
                painter.line_segment([p0, p1], stroke);
            }
        }
        IconKind::NavFirst => {
            let x_bar = c.x - r * 0.55;
            painter.line_segment(
                [pos2(x_bar, c.y - r * 0.62), pos2(x_bar, c.y + r * 0.62)],
                stroke,
            );
            let tri = vec![
                pos2(c.x - r * 0.28, c.y),
                pos2(c.x + r * 0.65, c.y - r * 0.62),
                pos2(c.x + r * 0.65, c.y + r * 0.62),
            ];
            painter.add(PathShape::convex_polygon(tri, color, Stroke::NONE));
        }
        IconKind::NavPrev => {
            painter.line_segment(
                [
                    pos2(c.x + r * 0.32, c.y - r * 0.62),
                    pos2(c.x - r * 0.38, c.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(c.x - r * 0.38, c.y),
                    pos2(c.x + r * 0.32, c.y + r * 0.62),
                ],
                stroke,
            );
        }
        IconKind::NavNext => {
            painter.line_segment(
                [
                    pos2(c.x - r * 0.32, c.y - r * 0.62),
                    pos2(c.x + r * 0.38, c.y),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(c.x + r * 0.38, c.y),
                    pos2(c.x - r * 0.32, c.y + r * 0.62),
                ],
                stroke,
            );
        }
        IconKind::NavLast => {
            let x_bar = c.x + r * 0.55;
            painter.line_segment(
                [pos2(x_bar, c.y - r * 0.62), pos2(x_bar, c.y + r * 0.62)],
                stroke,
            );
            let tri = vec![
                pos2(c.x + r * 0.28, c.y),
                pos2(c.x - r * 0.65, c.y - r * 0.62),
                pos2(c.x - r * 0.65, c.y + r * 0.62),
            ];
            painter.add(PathShape::convex_polygon(tri, color, Stroke::NONE));
        }
        IconKind::Flip => {
            let x0 = c.x - r * 0.42;
            let x1 = c.x + r * 0.42;
            painter.line_segment([pos2(x0, c.y + r * 0.55), pos2(x0, c.y - r * 0.55)], stroke);
            painter.line_segment(
                [pos2(x0 - r * 0.3, c.y - r * 0.22), pos2(x0, c.y - r * 0.58)],
                stroke,
            );
            painter.line_segment(
                [pos2(x0 + r * 0.3, c.y - r * 0.22), pos2(x0, c.y - r * 0.58)],
                stroke,
            );
            painter.line_segment([pos2(x1, c.y - r * 0.55), pos2(x1, c.y + r * 0.55)], stroke);
            painter.line_segment(
                [pos2(x1 - r * 0.3, c.y + r * 0.22), pos2(x1, c.y + r * 0.58)],
                stroke,
            );
            painter.line_segment(
                [pos2(x1 + r * 0.3, c.y + r * 0.22), pos2(x1, c.y + r * 0.58)],
                stroke,
            );
        }
        IconKind::Undo => {
            painter.line_segment(
                [
                    pos2(c.x - r * 0.65, c.y - r * 0.15),
                    pos2(c.x + r * 0.45, c.y - r * 0.15),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(c.x + r * 0.45, c.y - r * 0.15),
                    pos2(c.x + r * 0.45, c.y + r * 0.52),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(c.x - r * 0.15, c.y + r * 0.52),
                    pos2(c.x + r * 0.45, c.y + r * 0.52),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(c.x - r * 0.65, c.y - r * 0.15),
                    pos2(c.x - r * 0.25, c.y - r * 0.55),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    pos2(c.x - r * 0.65, c.y - r * 0.15),
                    pos2(c.x - r * 0.25, c.y + r * 0.25),
                ],
                stroke,
            );
        }
        IconKind::Clear | IconKind::Close => {
            let d = r * 0.58;
            painter.line_segment([pos2(c.x - d, c.y - d), pos2(c.x + d, c.y + d)], stroke);
            painter.line_segment([pos2(c.x + d, c.y - d), pos2(c.x - d, c.y + d)], stroke);
        }
        IconKind::Help => {
            painter.circle_stroke(c, r * 0.78, stroke);
            painter.text(
                c,
                Align2::CENTER_CENTER,
                "?",
                egui::FontId::proportional((r * 1.25).clamp(10.0, 16.0)),
                color,
            );
        }
        IconKind::User => {
            painter.circle_filled(pos2(c.x, c.y - r * 0.34), r * 0.36, color);
            let shoulders =
                Rect::from_center_size(pos2(c.x, c.y + r * 0.42), vec2(r * 1.25, r * 0.66));
            painter.rect_filled(shoulders, CornerRadius::same(4), color);
        }
        IconKind::Bot => {
            let head = Rect::from_center_size(pos2(c.x, c.y + r * 0.1), vec2(r * 1.35, r * 1.05));
            painter.rect_stroke(head, CornerRadius::same(3), stroke, StrokeKind::Inside);
            painter.circle_filled(pos2(c.x - r * 0.3, c.y + r * 0.08), r * 0.14, color);
            painter.circle_filled(pos2(c.x + r * 0.3, c.y + r * 0.08), r * 0.14, color);
            painter.line_segment(
                [pos2(c.x, c.y - r * 0.45), pos2(c.x, c.y - r * 0.78)],
                stroke,
            );
        }
    }
}

/// Visual button variants enforcing uniform height, corners, and hover states across the GUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Subtle,
    Danger,
}

/// Uniform styled action button with consistent height, padding, corner radius, and Catppuccin colors.
pub fn ui_button(
    ui: &mut Ui,
    label: &str,
    variant: ButtonVariant,
    min_width: Option<f32>,
) -> Response {
    let (bg, fg, border) = match variant {
        ButtonVariant::Primary => (Mocha::BLUE, Mocha::CRUST, Mocha::BLUE),
        ButtonVariant::Secondary => (Mocha::SURFACE0, Mocha::TEXT, Mocha::SURFACE1),
        ButtonVariant::Subtle => (Color32::TRANSPARENT, Mocha::SUBTEXT1, Mocha::SURFACE0),
        ButtonVariant::Danger => (
            Color32::from_rgba_unmultiplied(243, 139, 168, 36),
            Mocha::RED,
            Mocha::RED,
        ),
    };
    let mut text = RichText::new(label).color(fg).size(13.0);
    if variant == ButtonVariant::Primary {
        text = text.strong();
    }
    let btn = egui::Button::new(text)
        .fill(bg)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(CornerRadius::same(6))
        .min_size(vec2(min_width.unwrap_or(0.0), 32.0));
    ui.add(btn)
}

/// Uniform compact icon button used for board navigation (`|< < > >|`), Flip, Undo, and utilities.
pub fn ui_icon_button(
    ui: &mut Ui,
    icon: IconKind,
    tooltip: &str,
    active: bool,
    size: egui::Vec2,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let bg = if active {
        Mocha::SURFACE1
    } else if response.is_pointer_button_down_on() {
        Mocha::SURFACE2
    } else if response.hovered() {
        Mocha::SURFACE1
    } else {
        Mocha::SURFACE0
    };
    let stroke_col = if active {
        Mocha::BLUE
    } else if response.hovered() {
        Mocha::OVERLAY0
    } else {
        Mocha::SURFACE1
    };
    let icon_col = if active {
        Mocha::BLUE
    } else if response.hovered() {
        Mocha::TEXT
    } else {
        Mocha::SUBTEXT1
    };

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(6), bg);
    painter.rect_stroke(
        rect,
        CornerRadius::same(6),
        Stroke::new(1.0, stroke_col),
        StrokeKind::Inside,
    );
    draw_icon(painter, rect.shrink(6.0), icon, icon_col);

    response.on_hover_text(tooltip)
}

/// Uniform segmented pill/tab button for exclusive option groups (`Weiß | Schwarz`, `E1 | E2 | E3`, etc.).
pub fn ui_segmented_tab(ui: &mut Ui, label: &str, selected: bool, min_width: f32) -> Response {
    let (bg, fg, border) = if selected {
        (Mocha::SURFACE1, Mocha::TEXT, Mocha::BLUE)
    } else {
        (Mocha::MANTLE, Mocha::SUBTEXT0, Mocha::SURFACE0)
    };
    let text = RichText::new(label).size(12.5).color(fg);
    let text = if selected { text.strong() } else { text };
    let btn = egui::Button::new(text)
        .fill(bg)
        .stroke(Stroke::new(1.0, border))
        .corner_radius(CornerRadius::same(6))
        .min_size(vec2(min_width, 30.0));
    ui.add(btn)
}

/// Rounded status pill badge (`Elo 2400`, `Tiefe 14`, `ECO C60`).
pub fn ui_pill(ui: &mut Ui, text: &str, fg: Color32, bg: Color32) {
    Frame::new()
        .fill(bg)
        .corner_radius(CornerRadius::same(99))
        .inner_margin(Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.5).strong().color(fg));
        });
}

/// Unified Catppuccin `MANTLE` surface card replacing plain `ui.group`.
pub fn ui_card<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(Mocha::MANTLE)
        .stroke(Stroke::new(1.0, Mocha::SURFACE0))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(12))
        .show(ui, add_contents)
        .inner
}

/// Standard section card title row with optional right-aligned action/meta content.
pub fn ui_section_header(
    ui: &mut Ui,
    title: &str,
    accent: Color32,
    right_ui: impl FnOnce(&mut Ui),
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(14.0).strong().color(accent));
        ui.with_layout(egui::Layout::right_to_left(Align::Center), right_ui);
    });
}

/// Maps a [`MoveClass`] to crisp display glyph, German label, foreground, and tint background.
pub fn class_visuals(class: MoveClass) -> (&'static str, &'static str, Color32, Color32) {
    match class {
        MoveClass::Brilliant => ("!!", "Brillant", Mocha::CRUST, Mocha::MAUVE),
        MoveClass::Great => ("!", "Sehr gut", Mocha::CRUST, Mocha::BLUE),
        MoveClass::Best => ("*", "Bester Zug", Mocha::CRUST, Mocha::GREEN),
        MoveClass::Good => ("+", "Gut", Mocha::CRUST, Mocha::TEAL),
        MoveClass::Inaccuracy => ("?!", "Ungenauigkeit", Mocha::CRUST, Mocha::YELLOW),
        MoveClass::Mistake => ("?", "Fehler", Mocha::CRUST, Mocha::PEACH),
        MoveClass::Blunder => ("??", "Patzer", Mocha::CRUST, Mocha::RED),
    }
}

/// Renders a compact uniform move classification pill inline in move lists and review cards.
pub fn draw_class_pill(ui: &mut Ui, cls: &MoveClassification) {
    let (glyph, _, fg, bg) = class_visuals(cls.class);
    Frame::new()
        .fill(bg)
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::symmetric(5, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(glyph).size(10.5).strong().color(fg));
        });
}

/// Helper computing pixel center for placing small inline badges.
pub fn badge_center(rect: Rect, radius: f32) -> Pos2 {
    pos2(rect.max.x - radius * 0.85, rect.min.y + radius * 0.85)
}

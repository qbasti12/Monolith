//! Chess piece set themes and SVG image rendering for Monolith GUI.

use egui::epaint::PathShape;
use egui::{pos2, vec2, Color32, CornerRadius, Image, Painter, Rect, Stroke, StrokeKind, Ui};
use monolith_core::{Color, PieceType};

/// Available chess piece vector sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PieceSet {
    #[default]
    LichessClassic,
    LichessStaunty,
    MonolithMinimal,
    VectorProcedural,
}

impl PieceSet {
    pub const ALL: [PieceSet; 4] = [
        PieceSet::LichessClassic,
        PieceSet::LichessStaunty,
        PieceSet::MonolithMinimal,
        PieceSet::VectorProcedural,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            PieceSet::LichessClassic => "Lichess Classic (cburnett)",
            PieceSet::LichessStaunty => "Lichess Staunty",
            PieceSet::MonolithMinimal => "Monolith Minimal",
            PieceSet::VectorProcedural => "Vektor-Staunton (Prozedural)",
        }
    }
}

// Embedded Lichess Classic (cburnett) SVGs
const LICHESS_WP: &[u8] = include_bytes!("../assets/pieces/lichess/wP.svg");
const LICHESS_WN: &[u8] = include_bytes!("../assets/pieces/lichess/wN.svg");
const LICHESS_WB: &[u8] = include_bytes!("../assets/pieces/lichess/wB.svg");
const LICHESS_WR: &[u8] = include_bytes!("../assets/pieces/lichess/wR.svg");
const LICHESS_WQ: &[u8] = include_bytes!("../assets/pieces/lichess/wQ.svg");
const LICHESS_WK: &[u8] = include_bytes!("../assets/pieces/lichess/wK.svg");
const LICHESS_BP: &[u8] = include_bytes!("../assets/pieces/lichess/bP.svg");
const LICHESS_BN: &[u8] = include_bytes!("../assets/pieces/lichess/bN.svg");
const LICHESS_BB: &[u8] = include_bytes!("../assets/pieces/lichess/bB.svg");
const LICHESS_BR: &[u8] = include_bytes!("../assets/pieces/lichess/bR.svg");
const LICHESS_BQ: &[u8] = include_bytes!("../assets/pieces/lichess/bQ.svg");
const LICHESS_BK: &[u8] = include_bytes!("../assets/pieces/lichess/bK.svg");

// Embedded Lichess Staunty SVGs
const STAUNTY_WP: &[u8] = include_bytes!("../assets/pieces/staunty/wP.svg");
const STAUNTY_WN: &[u8] = include_bytes!("../assets/pieces/staunty/wN.svg");
const STAUNTY_WB: &[u8] = include_bytes!("../assets/pieces/staunty/wB.svg");
const STAUNTY_WR: &[u8] = include_bytes!("../assets/pieces/staunty/wR.svg");
const STAUNTY_WQ: &[u8] = include_bytes!("../assets/pieces/staunty/wQ.svg");
const STAUNTY_WK: &[u8] = include_bytes!("../assets/pieces/staunty/wK.svg");
const STAUNTY_BP: &[u8] = include_bytes!("../assets/pieces/staunty/bP.svg");
const STAUNTY_BN: &[u8] = include_bytes!("../assets/pieces/staunty/bN.svg");
const STAUNTY_BB: &[u8] = include_bytes!("../assets/pieces/staunty/bB.svg");
const STAUNTY_BR: &[u8] = include_bytes!("../assets/pieces/staunty/bR.svg");
const STAUNTY_BQ: &[u8] = include_bytes!("../assets/pieces/staunty/bQ.svg");
const STAUNTY_BK: &[u8] = include_bytes!("../assets/pieces/staunty/bK.svg");

// Embedded Monolith Minimal SVGs
const MINIMAL_WP: &[u8] = include_bytes!("../assets/pieces/minimal/wP.svg");
const MINIMAL_WN: &[u8] = include_bytes!("../assets/pieces/minimal/wN.svg");
const MINIMAL_WB: &[u8] = include_bytes!("../assets/pieces/minimal/wB.svg");
const MINIMAL_WR: &[u8] = include_bytes!("../assets/pieces/minimal/wR.svg");
const MINIMAL_WQ: &[u8] = include_bytes!("../assets/pieces/minimal/wQ.svg");
const MINIMAL_WK: &[u8] = include_bytes!("../assets/pieces/minimal/wK.svg");
const MINIMAL_BP: &[u8] = include_bytes!("../assets/pieces/minimal/bP.svg");
const MINIMAL_BN: &[u8] = include_bytes!("../assets/pieces/minimal/bN.svg");
const MINIMAL_BB: &[u8] = include_bytes!("../assets/pieces/minimal/bB.svg");
const MINIMAL_BR: &[u8] = include_bytes!("../assets/pieces/minimal/bR.svg");
const MINIMAL_BQ: &[u8] = include_bytes!("../assets/pieces/minimal/bQ.svg");
const MINIMAL_BK: &[u8] = include_bytes!("../assets/pieces/minimal/bK.svg");

/// Resolves piece SVG bytes and URI for the chosen piece set.
fn piece_svg_bytes(
    set: PieceSet,
    color: Color,
    piece_type: PieceType,
) -> (&'static str, &'static [u8]) {
    match set {
        PieceSet::LichessClassic => match (color, piece_type) {
            (Color::White, PieceType::Pawn) => ("bytes://lic_wp.svg", LICHESS_WP),
            (Color::White, PieceType::Knight) => ("bytes://lic_wn.svg", LICHESS_WN),
            (Color::White, PieceType::Bishop) => ("bytes://lic_wb.svg", LICHESS_WB),
            (Color::White, PieceType::Rook) => ("bytes://lic_wr.svg", LICHESS_WR),
            (Color::White, PieceType::Queen) => ("bytes://lic_wq.svg", LICHESS_WQ),
            (Color::White, PieceType::King) => ("bytes://lic_wk.svg", LICHESS_WK),
            (Color::Black, PieceType::Pawn) => ("bytes://lic_bp.svg", LICHESS_BP),
            (Color::Black, PieceType::Knight) => ("bytes://lic_bn.svg", LICHESS_BN),
            (Color::Black, PieceType::Bishop) => ("bytes://lic_bb.svg", LICHESS_BB),
            (Color::Black, PieceType::Rook) => ("bytes://lic_br.svg", LICHESS_BR),
            (Color::Black, PieceType::Queen) => ("bytes://lic_bq.svg", LICHESS_BQ),
            (Color::Black, PieceType::King) => ("bytes://lic_bk.svg", LICHESS_BK),
        },
        PieceSet::LichessStaunty => match (color, piece_type) {
            (Color::White, PieceType::Pawn) => ("bytes://sty_wp.svg", STAUNTY_WP),
            (Color::White, PieceType::Knight) => ("bytes://sty_wn.svg", STAUNTY_WN),
            (Color::White, PieceType::Bishop) => ("bytes://sty_wb.svg", STAUNTY_WB),
            (Color::White, PieceType::Rook) => ("bytes://sty_wr.svg", STAUNTY_WR),
            (Color::White, PieceType::Queen) => ("bytes://sty_wq.svg", STAUNTY_WQ),
            (Color::White, PieceType::King) => ("bytes://sty_wk.svg", STAUNTY_WK),
            (Color::Black, PieceType::Pawn) => ("bytes://sty_bp.svg", STAUNTY_BP),
            (Color::Black, PieceType::Knight) => ("bytes://sty_bn.svg", STAUNTY_BN),
            (Color::Black, PieceType::Bishop) => ("bytes://sty_bb.svg", STAUNTY_BB),
            (Color::Black, PieceType::Rook) => ("bytes://sty_br.svg", STAUNTY_BR),
            (Color::Black, PieceType::Queen) => ("bytes://sty_bq.svg", STAUNTY_BQ),
            (Color::Black, PieceType::King) => ("bytes://sty_bk.svg", STAUNTY_BK),
        },
        PieceSet::MonolithMinimal => match (color, piece_type) {
            (Color::White, PieceType::Pawn) => ("bytes://min_wp.svg", MINIMAL_WP),
            (Color::White, PieceType::Knight) => ("bytes://min_wn.svg", MINIMAL_WN),
            (Color::White, PieceType::Bishop) => ("bytes://min_wb.svg", MINIMAL_WB),
            (Color::White, PieceType::Rook) => ("bytes://min_wr.svg", MINIMAL_WR),
            (Color::White, PieceType::Queen) => ("bytes://min_wq.svg", MINIMAL_WQ),
            (Color::White, PieceType::King) => ("bytes://min_wk.svg", MINIMAL_WK),
            (Color::Black, PieceType::Pawn) => ("bytes://min_bp.svg", MINIMAL_BP),
            (Color::Black, PieceType::Knight) => ("bytes://min_bn.svg", MINIMAL_BN),
            (Color::Black, PieceType::Bishop) => ("bytes://min_bb.svg", MINIMAL_BB),
            (Color::Black, PieceType::Rook) => ("bytes://min_br.svg", MINIMAL_BR),
            (Color::Black, PieceType::Queen) => ("bytes://min_bq.svg", MINIMAL_BQ),
            (Color::Black, PieceType::King) => ("bytes://min_bk.svg", MINIMAL_BK),
        },
        PieceSet::VectorProcedural => ("", &[]),
    }
}

/// Renders a chess piece in the given rectangle using the configured piece set.
pub fn render_piece_styled(
    ui: &mut Ui,
    painter: &Painter,
    rect: Rect,
    piece_type: PieceType,
    color: Color,
    set: PieceSet,
) {
    if set == PieceSet::VectorProcedural {
        let padding = rect.width() * 0.06;
        let piece_rect = rect.shrink(padding);
        draw_scalable_piece(painter, piece_rect, color, piece_type);
        return;
    }

    let (uri, bytes) = piece_svg_bytes(set, color, piece_type);
    if bytes.is_empty() {
        let padding = rect.width() * 0.06;
        let piece_rect = rect.shrink(padding);
        draw_scalable_piece(painter, piece_rect, color, piece_type);
        return;
    }

    // Shrink slightly so pieces have a balanced margin inside squares.
    let padding = rect.width() * 0.07;
    let piece_rect = rect.shrink(padding);

    // Subtle drop shadow under piece.
    let shadow_rect = piece_rect.translate(egui::vec2(0.0, 1.5));
    painter.circle_filled(
        shadow_rect.center_bottom() - egui::vec2(0.0, 4.0),
        piece_rect.width() * 0.28,
        Color32::from_rgba_unmultiplied(0, 0, 0, 35),
    );

    let img = Image::from_bytes(uri, bytes);
    img.paint_at(ui, piece_rect);
}

/// Draws an elegant, scalable vector chess piece inspired by Staunton/Neo shapes with Catppuccin Mocha colors.
pub fn draw_scalable_piece(
    painter: &egui::Painter,
    rect: Rect,
    color: Color,
    piece_type: PieceType,
) {
    let c = rect.center();
    let s = rect.width().min(rect.height());
    let r = s * 0.44;

    let (fill, stroke_color, detail_color, eye_color) = if color == Color::White {
        (
            Color32::from_rgb(248, 245, 240), // warm ivory
            Color32::from_rgb(24, 24, 37),    // dark Mocha outline
            Color32::from_rgb(108, 112, 134), // subtle inner detail
            Color32::from_rgb(24, 24, 37),
        )
    } else {
        (
            Color32::from_rgb(30, 32, 48),    // dark Mocha slate
            Color32::from_rgb(205, 214, 244), // bright Mocha outline
            Color32::from_rgb(147, 153, 178), // subtle inner detail
            Color32::from_rgb(205, 214, 244),
        )
    };

    let stroke = Stroke::new((s * 0.038).clamp(2.0, 3.8), stroke_color);
    let thin_stroke = Stroke::new((s * 0.024).clamp(1.5, 2.4), detail_color);

    let base_w = match piece_type {
        PieceType::Pawn => r * 0.74,
        PieceType::Knight | PieceType::Bishop => r * 0.84,
        PieceType::Rook => r * 0.88,
        PieceType::Queen | PieceType::King => r * 0.94,
    };

    // Tier 1: Wide bottom base plate.
    let base_plate = Rect::from_min_max(
        pos2(c.x - base_w * 0.52, c.y + r * 0.68),
        pos2(c.x + base_w * 0.52, c.y + r * 0.84),
    );
    painter.rect(
        base_plate,
        CornerRadius::same(3),
        fill,
        stroke,
        StrokeKind::Inside,
    );

    // Tier 2: Sculpted riser rim.
    let base_riser = Rect::from_min_max(
        pos2(c.x - base_w * 0.40, c.y + r * 0.54),
        pos2(c.x + base_w * 0.40, c.y + r * 0.69),
    );
    painter.rect(
        base_riser,
        CornerRadius::same(2),
        fill,
        stroke,
        StrokeKind::Inside,
    );

    let torso_bottom_y = c.y + r * 0.55;

    match piece_type {
        PieceType::Pawn => {
            // Tapered torso.
            let body_pts = vec![
                pos2(c.x - base_w * 0.34, torso_bottom_y),
                pos2(c.x + base_w * 0.34, torso_bottom_y),
                pos2(c.x + r * 0.16, c.y - r * 0.05),
                pos2(c.x - r * 0.16, c.y - r * 0.05),
            ];
            painter.add(PathShape::convex_polygon(body_pts, fill, stroke));

            // Sculpted collar ring.
            painter.rect(
                Rect::from_center_size(pos2(c.x, c.y - r * 0.08), vec2(r * 0.46, r * 0.12)),
                CornerRadius::same(3),
                fill,
                stroke,
                StrokeKind::Inside,
            );

            // Spherical head.
            painter.circle(pos2(c.x, c.y - r * 0.38), r * 0.30, fill, stroke);

            // Subtle gloss highlight glint.
            painter.circle_filled(pos2(c.x - r * 0.10, c.y - r * 0.48), r * 0.06, detail_color);
        }

        PieceType::Knight => {
            // Sculpted Staunton/Neo knight body and arched mane.
            let neck_pts = vec![
                pos2(c.x - base_w * 0.36, torso_bottom_y),
                pos2(c.x + base_w * 0.36, torso_bottom_y),
                pos2(c.x + r * 0.38, c.y + r * 0.12),
                pos2(c.x + r * 0.26, c.y - r * 0.28),
                pos2(c.x - r * 0.12, c.y + r * 0.10),
                pos2(c.x - r * 0.32, c.y + r * 0.36),
            ];
            painter.add(PathShape::convex_polygon(neck_pts, fill, stroke));

            // Head and muzzle polygon.
            let head_pts = vec![
                pos2(c.x + r * 0.26, c.y - r * 0.28),
                pos2(c.x + r * 0.32, c.y - r * 0.40),
                pos2(c.x + r * 0.18, c.y - r * 0.66), // ear rear
                pos2(c.x + r * 0.10, c.y - r * 0.84), // ear tip
                pos2(c.x - r * 0.06, c.y - r * 0.60), // brow
                pos2(c.x - r * 0.44, c.y - r * 0.34), // bridge
                pos2(c.x - r * 0.54, c.y - r * 0.16), // nose muzzle
                pos2(c.x - r * 0.34, c.y - r * 0.04), // jaw chin
                pos2(c.x - r * 0.12, c.y + r * 0.10), // throat
            ];
            painter.add(PathShape::convex_polygon(head_pts, fill, stroke));

            // Eye.
            painter.circle_filled(
                pos2(c.x - r * 0.14, c.y - r * 0.38),
                (r * 0.055).max(2.0),
                eye_color,
            );

            // Nostril dot.
            painter.circle_filled(
                pos2(c.x - r * 0.44, c.y - r * 0.18),
                (r * 0.035).max(1.5),
                detail_color,
            );

            // Mouth slit.
            painter.line_segment(
                [
                    pos2(c.x - r * 0.50, c.y - r * 0.14),
                    pos2(c.x - r * 0.36, c.y - r * 0.10),
                ],
                thin_stroke,
            );

            // Mane accent ridges.
            painter.line_segment(
                [
                    pos2(c.x + r * 0.12, c.y - r * 0.50),
                    pos2(c.x + r * 0.22, c.y - r * 0.24),
                ],
                thin_stroke,
            );
            painter.line_segment(
                [
                    pos2(c.x + r * 0.22, c.y - r * 0.22),
                    pos2(c.x + r * 0.32, c.y + r * 0.02),
                ],
                thin_stroke,
            );
        }

        PieceType::Bishop => {
            // Tapered torso up to shoulder.
            let bishop_pts = vec![
                pos2(c.x - base_w * 0.34, torso_bottom_y),
                pos2(c.x + base_w * 0.34, torso_bottom_y),
                pos2(c.x + r * 0.32, c.y + r * 0.12),
                pos2(c.x + r * 0.26, c.y - r * 0.30),
                pos2(c.x, c.y - r * 0.65), // mitre peak
                pos2(c.x - r * 0.26, c.y - r * 0.30),
                pos2(c.x - r * 0.32, c.y + r * 0.12),
            ];
            painter.add(PathShape::convex_polygon(bishop_pts, fill, stroke));

            // Top finial ball.
            painter.circle(pos2(c.x, c.y - r * 0.74), r * 0.09, fill, stroke);

            // Distinctive diagonal mitre slit.
            painter.line_segment(
                [
                    pos2(c.x - r * 0.12, c.y - r * 0.44),
                    pos2(c.x + r * 0.16, c.y - r * 0.22),
                ],
                thin_stroke,
            );

            // Waist ring line.
            painter.line_segment(
                [
                    pos2(c.x - r * 0.26, c.y + r * 0.12),
                    pos2(c.x + r * 0.26, c.y + r * 0.12),
                ],
                thin_stroke,
            );
        }

        PieceType::Rook => {
            // Castle tower body.
            let rook_pts = vec![
                pos2(c.x - base_w * 0.36, torso_bottom_y),
                pos2(c.x + base_w * 0.36, torso_bottom_y),
                pos2(c.x + r * 0.30, c.y - r * 0.28),
                pos2(c.x - r * 0.30, c.y - r * 0.28),
            ];
            painter.add(PathShape::convex_polygon(rook_pts, fill, stroke));

            // Cornice ledge.
            painter.rect(
                Rect::from_center_size(pos2(c.x, c.y - r * 0.30), vec2(r * 0.76, r * 0.10)),
                CornerRadius::same(2),
                fill,
                stroke,
                StrokeKind::Inside,
            );

            // 3 crenellations (merlons).
            let crenel_y_top = c.y - r * 0.62;
            let crenel_y_bot = c.y - r * 0.35;
            let teeth = [
                (c.x - r * 0.38, c.x - r * 0.18),
                (c.x - r * 0.10, c.x + r * 0.10),
                (c.x + r * 0.18, c.x + r * 0.38),
            ];

            for (x1, x2) in teeth {
                painter.rect(
                    Rect::from_min_max(pos2(x1, crenel_y_top), pos2(x2, crenel_y_bot)),
                    CornerRadius::ZERO,
                    fill,
                    stroke,
                    StrokeKind::Inside,
                );
            }

            // Arrow slit window in center.
            painter.line_segment(
                [pos2(c.x, c.y - r * 0.16), pos2(c.x, c.y + r * 0.16)],
                thin_stroke,
            );
        }

        PieceType::Queen => {
            // Flared royal torso.
            let queen_pts = vec![
                pos2(c.x - base_w * 0.38, torso_bottom_y),
                pos2(c.x + base_w * 0.38, torso_bottom_y),
                pos2(c.x + r * 0.26, c.y - r * 0.16),
                pos2(c.x - r * 0.26, c.y - r * 0.16),
            ];
            painter.add(PathShape::convex_polygon(queen_pts, fill, stroke));

            // 5-point flared coronet.
            let crown_pts = vec![
                pos2(c.x - r * 0.28, c.y - r * 0.16),
                pos2(c.x + r * 0.28, c.y - r * 0.16),
                pos2(c.x + r * 0.44, c.y - r * 0.46), // right wing
                pos2(c.x + r * 0.22, c.y - r * 0.60),
                pos2(c.x, c.y - r * 0.70), // center spire
                pos2(c.x - r * 0.22, c.y - r * 0.60),
                pos2(c.x - r * 0.44, c.y - r * 0.46), // left wing
            ];
            painter.add(PathShape::convex_polygon(crown_pts, fill, stroke));

            // 5 crown pearl gems atop spires.
            let pearls = [
                pos2(c.x - r * 0.44, c.y - r * 0.48),
                pos2(c.x - r * 0.22, c.y - r * 0.62),
                pos2(c.x, c.y - r * 0.72),
                pos2(c.x + r * 0.22, c.y - r * 0.62),
                pos2(c.x + r * 0.44, c.y - r * 0.48),
            ];
            for p in pearls {
                painter.circle(p, (r * 0.065).max(2.0), fill, stroke);
            }

            // Waistband contour line.
            painter.line_segment(
                [
                    pos2(c.x - r * 0.26, c.y - r * 0.16),
                    pos2(c.x + r * 0.26, c.y - r * 0.16),
                ],
                thin_stroke,
            );
        }

        PieceType::King => {
            // Stately mantle torso.
            let king_pts = vec![
                pos2(c.x - base_w * 0.38, torso_bottom_y),
                pos2(c.x + base_w * 0.38, torso_bottom_y),
                pos2(c.x + r * 0.32, c.y - r * 0.20),
                pos2(c.x - r * 0.32, c.y - r * 0.20),
            ];
            painter.add(PathShape::convex_polygon(king_pts, fill, stroke));

            // Crown rim arch.
            painter.rect(
                Rect::from_center_size(pos2(c.x, c.y - r * 0.34), vec2(r * 0.70, r * 0.22)),
                CornerRadius::same(4),
                fill,
                stroke,
                StrokeKind::Inside,
            );

            // Latin Cross: vertical shaft.
            painter.rect(
                Rect::from_center_size(pos2(c.x, c.y - r * 0.66), vec2(r * 0.12, r * 0.30)),
                CornerRadius::ZERO,
                fill,
                stroke,
                StrokeKind::Inside,
            );
            // Latin Cross: horizontal arms.
            painter.rect(
                Rect::from_center_size(pos2(c.x, c.y - r * 0.70), vec2(r * 0.30, r * 0.12)),
                CornerRadius::ZERO,
                fill,
                stroke,
                StrokeKind::Inside,
            );
            // Central jewel at cross center.
            painter.circle_filled(
                pos2(c.x, c.y - r * 0.70),
                (r * 0.045).max(1.5),
                detail_color,
            );
        }
    }
}

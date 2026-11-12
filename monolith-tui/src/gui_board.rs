//! High-resolution vector-rendered graphical chess board with interactive mouse support,
//! Catppuccin Mocha palette, and elegant Staunton/Neo piece silhouettes.

use egui::epaint::PathShape;
use egui::{pos2, vec2, Color32, CornerRadius, Rect, Sense, Stroke, StrokeKind, Ui};
use monolith_core::movegen::generate_legal;
use monolith_core::types::MoveList;
use monolith_core::{Color, PieceType, Square};

use crate::app::App;
use crate::gui_pieces::{render_piece_styled, PieceSet};

/// State for the GUI board interaction (selection, promotion popup, hover, assistance toggle).
#[derive(Debug, Clone)]
pub struct GuiBoardState {
    pub pending_promotion: Option<(Square, Square)>,
    pub hovered_sq: Option<Square>,
    pub hide_assistance: bool,
    pub piece_set: PieceSet,
    pub show_coords: bool,
}

impl Default for GuiBoardState {
    fn default() -> Self {
        Self {
            pending_promotion: None,
            hovered_sq: None,
            hide_assistance: false,
            piece_set: PieceSet::LichessClassic,
            show_coords: true,
        }
    }
}

/// Renders the 8x8 chess board with scalable vector pieces and mouse interactions.
pub fn render_chess_board(ui: &mut Ui, app: &mut App, state: &mut GuiBoardState) {
    let avail = ui.available_size();
    let board_size = (avail.x.min(avail.y) - 20.0).clamp(320.0, 960.0);
    let (board_rect, _) = ui.allocate_exact_size(vec2(board_size, board_size), Sense::hover());
    let sq_size = board_size / 8.0;

    let painter = ui.painter_at(board_rect);
    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let pointer_clicked = ui.input(|i| i.pointer.primary_clicked());

    state.hovered_sq = None;
    let mut clicked_sq: Option<Square> = None;

    // Determine square hover / click from pointer coordinates.
    if let Some(pos) = pointer_pos {
        if board_rect.contains(pos) {
            let rel_x = pos.x - board_rect.min.x;
            let rel_y = pos.y - board_rect.min.y;
            let col = (rel_x / sq_size).floor().clamp(0.0, 7.0) as u8;
            let row = (rel_y / sq_size).floor().clamp(0.0, 7.0) as u8;

            let file = if app.board_flipped { 7 - col } else { col };
            let rank = if app.board_flipped { row } else { 7 - row };
            let sq = Square::from_coords(file, rank);
            state.hovered_sq = Some(sq);

            if pointer_clicked && state.pending_promotion.is_none() {
                clicked_sq = Some(sq);
            }
        }
    }

    // Process user click on a square.
    if let Some(sq) = clicked_sq {
        handle_square_click(app, state, sq);
    }

    let last_move = app.last_move_squares();
    let best_move = if state.hide_assistance {
        None
    } else {
        app.engines.first().and_then(|e| {
            if e.config.enabled {
                e.telemetry.best_move_squares
            } else {
                None
            }
        })
    };

    let checked_king_sq = if app.board.in_check() {
        Some(app.board.king_square(app.board.side_to_move()))
    } else {
        None
    };

    // Pure Catppuccin Mocha board palette.
    let light_color = Color32::from_rgb(222, 227, 238); // Catppuccin light square
    let dark_color = Color32::from_rgb(108, 112, 134); // Catppuccin Overlay0 dark square
    let last_mv_color = Color32::from_rgba_unmultiplied(137, 180, 250, 140); // Blue
    let sel_color = Color32::from_rgba_unmultiplied(249, 226, 175, 200); // Yellow

    // Draw 64 squares.
    for col in 0..8u8 {
        for row in 0..8u8 {
            let file = if app.board_flipped { 7 - col } else { col };
            let rank = if app.board_flipped { row } else { 7 - row };
            let sq = Square::from_coords(file, rank);

            let sq_min = pos2(
                board_rect.min.x + col as f32 * sq_size,
                board_rect.min.y + row as f32 * sq_size,
            );
            let sq_rect = Rect::from_min_size(sq_min, vec2(sq_size, sq_size));

            let is_light = (file + rank) % 2 == 1;
            let base_color = if is_light { light_color } else { dark_color };

            // Background fill.
            painter.rect_filled(sq_rect, CornerRadius::ZERO, base_color);

            // Last move highlight.
            if let Some((from, to)) = last_move {
                if sq == from || sq == to {
                    painter.rect_filled(sq_rect, CornerRadius::ZERO, last_mv_color);
                }
            }

            // Best move engine highlight (only when assistance is enabled).
            if let Some((from, to)) = best_move {
                if sq == from || sq == to {
                    painter.rect_filled(
                        sq_rect,
                        CornerRadius::ZERO,
                        Color32::from_rgba_unmultiplied(148, 226, 213, 120),
                    );
                }
            }

            // In-check King highlight.
            if checked_king_sq == Some(sq) {
                painter.rect_filled(
                    sq_rect,
                    CornerRadius::ZERO,
                    Color32::from_rgba_unmultiplied(243, 139, 168, 220),
                );
            }

            // Selected square highlight.
            if app.selected_sq == Some(sq) {
                painter.rect_filled(sq_rect, CornerRadius::ZERO, sel_color);
            }

            // Legal target indicators.
            if app.legal_targets.contains(&sq) {
                let center = sq_rect.center();
                if app.board.piece_on(sq).is_some() {
                    // Capture target: outer red ring.
                    painter.circle_stroke(
                        center,
                        sq_size * 0.44,
                        Stroke::new(
                            (sq_size * 0.08).clamp(3.0, 5.0),
                            Color32::from_rgba_unmultiplied(243, 139, 168, 210),
                        ),
                    );
                } else {
                    // Quiet move target: green dot.
                    painter.circle_filled(
                        center,
                        sq_size * 0.16,
                        Color32::from_rgba_unmultiplied(166, 227, 161, 210),
                    );
                }
            }

            // Piece rendering.
            if let Some((color, piece_type)) = app.board.piece_on(sq) {
                render_piece_styled(ui, &painter, sq_rect, piece_type, color, state.piece_set);
            }

            // Hover indicator.
            if state.hovered_sq == Some(sq) && state.pending_promotion.is_none() {
                painter.rect_stroke(
                    sq_rect.shrink(1.5),
                    CornerRadius::same(2),
                    Stroke::new(2.0, Color32::from_rgba_unmultiplied(245, 224, 220, 180)),
                    StrokeKind::Inside,
                );
            }

            // Board coordinate annotations (Rank 1..8, File a..h).
            if state.show_coords {
                let label_color = if is_light {
                    Color32::from_rgb(120, 125, 145)
                } else {
                    Color32::from_rgb(205, 214, 244)
                };
                let font_id = egui::FontId::proportional((sq_size * 0.17).clamp(9.0, 16.0));

                if col == 0 {
                    let rank_str = format!("{}", rank + 1);
                    painter.text(
                        sq_rect.min + vec2(3.0, 2.0),
                        egui::Align2::LEFT_TOP,
                        rank_str,
                        font_id.clone(),
                        label_color,
                    );
                }

                if row == 7 {
                    let file_char = (b'a' + file) as char;
                    let file_str = file_char.to_string();
                    painter.text(
                        sq_rect.max - vec2(3.0, 3.0),
                        egui::Align2::RIGHT_BOTTOM,
                        file_str,
                        font_id,
                        label_color,
                    );
                }
            }
        }
    }

    // Outer board border.
    painter.rect_stroke(
        board_rect,
        CornerRadius::same(4),
        Stroke::new(2.5, Color32::from_rgb(137, 180, 250)),
        StrokeKind::Inside,
    );

    // Promotion dialog overlay.
    if let Some((from, to)) = state.pending_promotion {
        render_promotion_overlay(ui, app, state, board_rect, from, to);
    }
}

fn handle_square_click(app: &mut App, state: &mut GuiBoardState, sq: Square) {
    if let Some(from) = app.selected_sq {
        if from == sq {
            app.clear_selection();
            return;
        }

        if app.legal_targets.contains(&sq) {
            let matching = app.matching_moves(from, sq);
            if matching.len() > 1 {
                // Pawn promotion: prompt user choice.
                state.pending_promotion = Some((from, sq));
                return;
            }
            if let Some(&mv) = matching.first() {
                let _ = app.play_move(mv);
                return;
            }
        }

        // If clicking another friendly piece, switch selection.
        if let Some((color, _)) = app.board.piece_on(sq) {
            if color == app.board.side_to_move() {
                select_piece(app, sq);
                return;
            }
        }
        app.clear_selection();
    } else if let Some((color, _)) = app.board.piece_on(sq) {
        if color == app.board.side_to_move() {
            select_piece(app, sq);
        }
    }
}

fn select_piece(app: &mut App, sq: Square) {
    let mut legal = MoveList::new();
    generate_legal(&app.board, &mut legal);
    let targets: Vec<Square> = legal
        .as_slice()
        .iter()
        .filter(|m| m.from_sq() == sq)
        .map(|m| m.to_sq())
        .collect();
    if !targets.is_empty() {
        app.selected_sq = Some(sq);
        app.legal_targets = targets;
    }
}

fn render_promotion_overlay(
    ui: &mut Ui,
    app: &mut App,
    state: &mut GuiBoardState,
    board_rect: Rect,
    from: Square,
    to: Square,
) {
    let overlay_w = 320.0;
    let overlay_h = 100.0;
    let center = board_rect.center();
    let overlay_rect = Rect::from_center_size(center, vec2(overlay_w, overlay_h));

    let painter = ui.painter_at(board_rect);
    painter.rect_filled(
        board_rect,
        CornerRadius::same(4),
        Color32::from_rgba_unmultiplied(17, 17, 27, 190),
    );
    painter.rect_filled(
        overlay_rect,
        CornerRadius::same(8),
        Color32::from_rgb(30, 30, 46),
    );
    painter.rect_stroke(
        overlay_rect,
        CornerRadius::same(8),
        Stroke::new(2.0, Color32::from_rgb(203, 166, 247)),
        StrokeKind::Inside,
    );

    let promo_options = [
        (PieceType::Queen, "Dame (Q)"),
        (PieceType::Rook, "Turm (R)"),
        (PieceType::Bishop, "Läufer (B)"),
        (PieceType::Knight, "Springer (N)"),
    ];

    let btn_w = 70.0;
    let btn_h = 75.0;
    let spacing = 8.0;
    let start_x = overlay_rect.min.x + 12.0;
    let start_y = overlay_rect.min.y + 12.0;

    let pointer_pos = ui.input(|i| i.pointer.hover_pos());
    let pointer_clicked = ui.input(|i| i.pointer.primary_clicked());

    for (idx, (pt, label)) in promo_options.iter().enumerate() {
        let btn_min = pos2(start_x + idx as f32 * (btn_w + spacing), start_y);
        let btn_rect = Rect::from_min_size(btn_min, vec2(btn_w, btn_h));

        let hovered = pointer_pos.is_some_and(|p| btn_rect.contains(p));
        let btn_bg = if hovered {
            Color32::from_rgb(69, 71, 90)
        } else {
            Color32::from_rgb(49, 50, 68)
        };

        painter.rect_filled(btn_rect, CornerRadius::same(6), btn_bg);
        painter.rect_stroke(
            btn_rect,
            CornerRadius::same(6),
            Stroke::new(1.0, Color32::from_rgb(137, 180, 250)),
            StrokeKind::Inside,
        );

        let piece_rect = Rect::from_min_size(
            pos2(btn_rect.min.x + 10.0, btn_rect.min.y + 4.0),
            vec2(btn_w - 20.0, btn_h - 26.0),
        );
        render_piece_styled(
            ui,
            &painter,
            piece_rect,
            *pt,
            app.board.side_to_move(),
            state.piece_set,
        );

        painter.text(
            pos2(btn_rect.center().x, btn_rect.max.y - 12.0),
            egui::Align2::CENTER_CENTER,
            *label,
            egui::FontId::proportional(11.0),
            Color32::from_rgb(205, 214, 244),
        );

        if hovered && pointer_clicked {
            let promo_moves = app.matching_moves(from, to);
            if let Some(mv) = promo_moves
                .into_iter()
                .find(|m| m.promotion_piece() == Some(*pt))
            {
                let _ = app.play_move(mv);
            }
            state.pending_promotion = None;
            return;
        }
    }
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

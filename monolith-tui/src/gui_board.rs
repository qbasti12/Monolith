//! High-resolution vector-rendered graphical chess board with interactive mouse support.

use egui::epaint::PathShape;
use egui::{pos2, vec2, Color32, CornerRadius, Rect, Sense, Stroke, StrokeKind, Ui};
use monolith_core::movegen::generate_legal;
use monolith_core::types::MoveList;
use monolith_core::{Color, PieceType, Square};

use crate::app::App;

/// State for the GUI board interaction (selection, promotion popup, hover).
#[derive(Debug, Default, Clone)]
pub struct GuiBoardState {
    pub pending_promotion: Option<(Square, Square)>,
    pub hovered_sq: Option<Square>,
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
    let best_move = app.engines.first().and_then(|e| {
        if e.config.enabled {
            e.telemetry.best_move_squares
        } else {
            None
        }
    });
    let checked_king_sq = if app.board.in_check() {
        Some(app.board.king_square(app.board.side_to_move()))
    } else {
        None
    };

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
            let base_color = if is_light {
                Color32::from_rgb(222, 227, 238) // Catppuccin light square
            } else {
                Color32::from_rgb(108, 112, 134) // Catppuccin dark square
            };

            // Background fill.
            painter.rect_filled(sq_rect, CornerRadius::ZERO, base_color);

            // Last move highlight.
            if let Some((from, to)) = last_move {
                if sq == from || sq == to {
                    painter.rect_filled(
                        sq_rect,
                        CornerRadius::ZERO,
                        Color32::from_rgba_unmultiplied(137, 180, 250, 140),
                    );
                }
            }

            // Best move engine highlight.
            if let Some((from, to)) = best_move {
                if sq == from || sq == to {
                    painter.rect_filled(
                        sq_rect,
                        CornerRadius::ZERO,
                        Color32::from_rgba_unmultiplied(148, 226, 213, 90),
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
                painter.rect_filled(
                    sq_rect,
                    CornerRadius::ZERO,
                    Color32::from_rgba_unmultiplied(249, 226, 175, 200),
                );
            }

            // Legal target indicators.
            if app.legal_targets.contains(&sq) {
                let center = sq_rect.center();
                if app.board.piece_on(sq).is_some() {
                    // Capture target: outer ring.
                    painter.circle_stroke(
                        center,
                        sq_size * 0.44,
                        Stroke::new(3.5, Color32::from_rgba_unmultiplied(166, 227, 161, 230)),
                    );
                } else {
                    // Quiet move target: solid center dot.
                    painter.circle_filled(
                        center,
                        sq_size * 0.16,
                        Color32::from_rgba_unmultiplied(166, 227, 161, 210),
                    );
                }
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

            // Draw board coordinates (Rank 1..8 on left column, File a..h on bottom row).
            let label_color = if is_light {
                Color32::from_rgb(120, 125, 145)
            } else {
                Color32::from_rgb(205, 214, 244)
            };
            let font_id = egui::FontId::proportional(sq_size * 0.18);

            if col == 0 {
                let rank_str = (rank + 1).to_string();
                painter.text(
                    pos2(sq_rect.min.x + 3.0, sq_rect.min.y + 2.0),
                    egui::Align2::LEFT_TOP,
                    rank_str,
                    font_id.clone(),
                    label_color,
                );
            }
            if row == 7 {
                let file_str = ((b'a' + file) as char).to_string();
                painter.text(
                    pos2(sq_rect.max.x - 3.0, sq_rect.max.y - 2.0),
                    egui::Align2::RIGHT_BOTTOM,
                    file_str,
                    font_id,
                    label_color,
                );
            }

            // Draw piece if present.
            if let Some((color, pt)) = app.board.piece_on(sq) {
                draw_scalable_piece(&painter, sq_rect, color, pt);
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
                // Pawn promotion to 8th/1st rank: show piece choice.
                state.pending_promotion = Some((from, sq));
                return;
            }
            if let Some(&mv) = matching.first() {
                let _ = app.play_move(mv);
                return;
            }
        }

        // If clicking another friendly piece, reselect.
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
    // Dim background.
    painter.rect_filled(
        board_rect,
        CornerRadius::same(4),
        Color32::from_rgba_unmultiplied(17, 17, 27, 180),
    );
    // Modal card.
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
        draw_scalable_piece(&painter, piece_rect, app.board.side_to_move(), *pt);

        painter.text(
            pos2(btn_rect.center().x, btn_rect.max.y - 12.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(11.0),
            Color32::from_rgb(205, 214, 244),
        );

        if hovered && pointer_clicked {
            if let Some(mv) = app.find_matching_move(from, to, Some(*pt)) {
                let _ = app.play_move(mv);
            }
            state.pending_promotion = None;
            return;
        }
    }

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.pending_promotion = None;
    }
}

/// Draws an unmistakable, high-resolution vector chess piece scaled to `rect`.
pub fn draw_scalable_piece(
    painter: &egui::Painter,
    rect: Rect,
    color: Color,
    piece_type: PieceType,
) {
    let c = rect.center();
    let s = rect.width().min(rect.height());
    let r = s * 0.42;

    let (fill, stroke_color, detail_color) = if color == Color::White {
        (
            Color32::from_rgb(248, 245, 240), // warm ivory
            Color32::from_rgb(24, 24, 37),    // dark crisp outline
            Color32::from_rgb(90, 95, 115),   // subtle inner detail
        )
    } else {
        (
            Color32::from_rgb(30, 32, 48),    // dark obsidian slate
            Color32::from_rgb(205, 214, 244), // bright crisp outline
            Color32::from_rgb(147, 153, 178), // subtle inner detail
        )
    };

    let stroke = Stroke::new((s * 0.038).clamp(2.0, 3.8), stroke_color);
    let thin_stroke = Stroke::new((s * 0.026).clamp(1.5, 2.6), detail_color);

    let base_y1 = c.y + r * 0.58;
    let base_y2 = c.y + r * 0.80;
    let base_w = match piece_type {
        PieceType::Pawn => r * 0.72,
        PieceType::Knight | PieceType::Bishop => r * 0.82,
        PieceType::Rook => r * 0.88,
        PieceType::Queen | PieceType::King => r * 0.94,
    };

    // Pedestal base for all pieces.
    painter.rect(
        Rect::from_min_max(
            pos2(c.x - base_w * 0.5, base_y1),
            pos2(c.x + base_w * 0.5, base_y2),
        ),
        CornerRadius::same(3),
        fill,
        stroke,
        StrokeKind::Inside,
    );

    match piece_type {
        PieceType::Pawn => {
            // Body.
            let body_pts = vec![
                pos2(c.x - base_w * 0.36, base_y1),
                pos2(c.x + base_w * 0.36, base_y1),
                pos2(c.x + r * 0.16, c.y - r * 0.05),
                pos2(c.x - r * 0.16, c.y - r * 0.05),
            ];
            painter.add(PathShape::convex_polygon(body_pts, fill, stroke));

            // Collar.
            painter.rect(
                Rect::from_center_size(pos2(c.x, c.y - r * 0.08), vec2(r * 0.46, r * 0.12)),
                CornerRadius::same(2),
                fill,
                stroke,
                StrokeKind::Inside,
            );

            // Head.
            painter.circle(pos2(c.x, c.y - r * 0.38), r * 0.28, fill, stroke);
        }

        PieceType::Knight => {
            // Horse silhouette polygon.
            let knight_pts = vec![
                pos2(c.x - base_w * 0.38, base_y1),
                pos2(c.x + base_w * 0.38, base_y1),
                pos2(c.x + r * 0.32, c.y + r * 0.12),
                pos2(c.x + r * 0.24, c.y - r * 0.28),
                pos2(c.x + r * 0.16, c.y - r * 0.62),
                pos2(c.x + r * 0.09, c.y - r * 0.76), // ear tip
                pos2(c.x - r * 0.04, c.y - r * 0.56),
                pos2(c.x - r * 0.38, c.y - r * 0.32), // snout top
                pos2(c.x - r * 0.48, c.y - r * 0.18), // muzzle tip
                pos2(c.x - r * 0.28, c.y - r * 0.06), // jaw bottom
                pos2(c.x - r * 0.14, c.y + r * 0.12), // throat
                pos2(c.x - r * 0.30, c.y + r * 0.40), // chest
            ];
            painter.add(PathShape::convex_polygon(knight_pts, fill, stroke));

            // Eye dot.
            painter.circle_filled(
                pos2(c.x - r * 0.12, c.y - r * 0.35),
                (r * 0.06).max(1.8),
                stroke_color,
            );

            // Mouth line.
            painter.line_segment(
                [
                    pos2(c.x - r * 0.44, c.y - r * 0.18),
                    pos2(c.x - r * 0.32, c.y - r * 0.14),
                ],
                thin_stroke,
            );
        }

        PieceType::Bishop => {
            // Miter oval body.
            let bishop_pts = vec![
                pos2(c.x - base_w * 0.38, base_y1),
                pos2(c.x + base_w * 0.38, base_y1),
                pos2(c.x + r * 0.36, c.y + r * 0.10),
                pos2(c.x + r * 0.28, c.y - r * 0.34),
                pos2(c.x, c.y - r * 0.62), // miter peak
                pos2(c.x - r * 0.28, c.y - r * 0.34),
                pos2(c.x - r * 0.36, c.y + r * 0.10),
            ];
            painter.add(PathShape::convex_polygon(bishop_pts, fill, stroke));

            // Cross / Ball at peak.
            painter.circle(pos2(c.x, c.y - r * 0.72), r * 0.10, fill, stroke);

            // Miter diagonal slash slit.
            painter.line_segment(
                [
                    pos2(c.x - r * 0.12, c.y - r * 0.44),
                    pos2(c.x + r * 0.16, c.y - r * 0.20),
                ],
                thin_stroke,
            );
        }

        PieceType::Rook => {
            // Main tower body.
            let tower_pts = vec![
                pos2(c.x - base_w * 0.40, base_y1),
                pos2(c.x + base_w * 0.40, base_y1),
                pos2(c.x + r * 0.26, c.y - r * 0.32),
                pos2(c.x - r * 0.26, c.y - r * 0.32),
            ];
            painter.add(PathShape::convex_polygon(tower_pts, fill, stroke));

            // Crenelated battlements.
            let crenel_y_top = c.y - r * 0.66;
            let crenel_y_bot = c.y - r * 0.34;
            let teeth = [
                (c.x - r * 0.40, c.x - r * 0.18),
                (c.x - r * 0.11, c.x + r * 0.11),
                (c.x + r * 0.18, c.x + r * 0.40),
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

            // Arrow slit window in tower center.
            painter.line_segment(
                [pos2(c.x, c.y - r * 0.18), pos2(c.x, c.y + r * 0.18)],
                thin_stroke,
            );
        }

        PieceType::Queen => {
            // Royal gown body.
            let queen_pts = vec![
                pos2(c.x - base_w * 0.42, base_y1),
                pos2(c.x + base_w * 0.42, base_y1),
                pos2(c.x + r * 0.28, c.y - r * 0.20),
                pos2(c.x - r * 0.28, c.y - r * 0.20),
            ];
            painter.add(PathShape::convex_polygon(queen_pts, fill, stroke));

            // 5-point flared crown.
            let crown_pts = vec![
                pos2(c.x - r * 0.30, c.y - r * 0.20),
                pos2(c.x + r * 0.30, c.y - r * 0.20),
                pos2(c.x + r * 0.44, c.y - r * 0.48), // rightmost tip
                pos2(c.x + r * 0.22, c.y - r * 0.60),
                pos2(c.x, c.y - r * 0.68), // center tip
                pos2(c.x - r * 0.22, c.y - r * 0.60),
                pos2(c.x - r * 0.44, c.y - r * 0.48), // leftmost tip
            ];
            painter.add(PathShape::convex_polygon(crown_pts, fill, stroke));

            // 5 crown pearl gems.
            let pearls = [
                pos2(c.x - r * 0.44, c.y - r * 0.50),
                pos2(c.x - r * 0.22, c.y - r * 0.62),
                pos2(c.x, c.y - r * 0.70),
                pos2(c.x + r * 0.22, c.y - r * 0.62),
                pos2(c.x + r * 0.44, c.y - r * 0.50),
            ];
            for p in pearls {
                painter.circle(p, r * 0.065, fill, stroke);
            }
        }

        PieceType::King => {
            // Regal body.
            let king_pts = vec![
                pos2(c.x - base_w * 0.42, base_y1),
                pos2(c.x + base_w * 0.42, base_y1),
                pos2(c.x + r * 0.32, c.y - r * 0.26),
                pos2(c.x - r * 0.32, c.y - r * 0.26),
            ];
            painter.add(PathShape::convex_polygon(king_pts, fill, stroke));

            // Crown arch.
            painter.rect(
                Rect::from_center_size(pos2(c.x, c.y - r * 0.38), vec2(r * 0.68, r * 0.24)),
                CornerRadius::same(4),
                fill,
                stroke,
                StrokeKind::Inside,
            );

            // Latin Cross on top: vertical post.
            painter.rect(
                Rect::from_center_size(pos2(c.x, c.y - r * 0.68), vec2(r * 0.12, r * 0.30)),
                CornerRadius::ZERO,
                fill,
                stroke,
                StrokeKind::Inside,
            );
            // Horizontal crossbar.
            painter.rect(
                Rect::from_center_size(pos2(c.x, c.y - r * 0.72), vec2(r * 0.30, r * 0.11)),
                CornerRadius::ZERO,
                fill,
                stroke,
                StrokeKind::Inside,
            );
        }
    }
}

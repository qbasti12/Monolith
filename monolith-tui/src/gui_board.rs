//! High-resolution vector-rendered graphical chess board with Drag & Drop,
//! Right-click arrows/highlights, Multi-Engine colored arrows, and Catppuccin Mocha palette.

use egui::epaint::PathShape;
use egui::{pos2, vec2, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Ui};
use monolith_core::movegen::generate_legal;
use monolith_core::types::MoveList;
use monolith_core::{PieceType, Square};

use crate::app::App;
use crate::gui_pieces::{render_piece_styled, PieceSet};

/// Catppuccin Mocha accent colors assignable to individual engine bots.
pub const ENGINE_COLORS: [Color32; 6] = [
    Color32::from_rgb(137, 180, 250), // Blue
    Color32::from_rgb(166, 227, 161), // Green
    Color32::from_rgb(250, 179, 135), // Peach
    Color32::from_rgb(203, 166, 247), // Mauve
    Color32::from_rgb(249, 226, 175), // Yellow
    Color32::from_rgb(148, 226, 213), // Teal
];

pub const ENGINE_COLOR_NAMES: [&str; 6] = ["Blau", "Grün", "Pfirsich", "Mauve", "Gelb", "Teal"];

/// Returns the Catppuccin Mocha color for the given engine color index.
pub fn engine_color(idx: usize) -> Color32 {
    ENGINE_COLORS[idx % ENGINE_COLORS.len()]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserArrow {
    pub from: Square,
    pub to: Square,
    pub color: Color32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserHighlight {
    pub sq: Square,
    pub color: Color32,
}

/// State for the GUI board interaction (Drag & Drop, Right-click arrows, Promotion, PieceSet).
#[derive(Debug, Clone)]
pub struct GuiBoardState {
    pub pending_promotion: Option<(Square, Square)>,
    pub hovered_sq: Option<Square>,
    pub hide_assistance: bool,
    pub piece_set: PieceSet,
    pub show_coords: bool,
    pub show_engine_arrows: bool,
    pub highlight_last_move: bool,
    pub highlight_legal_moves: bool,
    // Drag & Drop state
    pub dragging_sq: Option<Square>,
    pub drag_pos: Option<Pos2>,
    pub drag_moved: bool,
    pub drag_was_selected: bool,
    // Right-click annotations
    pub right_drag_start: Option<Square>,
    pub user_arrows: Vec<UserArrow>,
    pub user_highlights: Vec<UserHighlight>,
}

impl Default for GuiBoardState {
    fn default() -> Self {
        Self {
            pending_promotion: None,
            hovered_sq: None,
            hide_assistance: false,
            piece_set: PieceSet::LichessClassic,
            show_coords: true,
            show_engine_arrows: true,
            highlight_last_move: true,
            highlight_legal_moves: true,
            dragging_sq: None,
            drag_pos: None,
            drag_moved: false,
            drag_was_selected: false,
            right_drag_start: None,
            user_arrows: Vec::new(),
            user_highlights: Vec::new(),
        }
    }
}

impl GuiBoardState {
    pub fn clear_annotations(&mut self) {
        self.user_arrows.clear();
        self.user_highlights.clear();
    }
}

fn sq_from_pos(board_rect: Rect, sq_size: f32, flipped: bool, pos: Pos2) -> Option<Square> {
    if !board_rect.contains(pos) {
        return None;
    }
    let rel_x = pos.x - board_rect.min.x;
    let rel_y = pos.y - board_rect.min.y;
    let col = (rel_x / sq_size).floor().clamp(0.0, 7.0) as u8;
    let row = (rel_y / sq_size).floor().clamp(0.0, 7.0) as u8;
    let file = if flipped { 7 - col } else { col };
    let rank = if flipped { row } else { 7 - row };
    Some(Square::from_coords(file, rank))
}

fn square_center(board_rect: Rect, sq_size: f32, flipped: bool, sq: Square) -> Pos2 {
    let file = sq.file();
    let rank = sq.rank();
    let col = if flipped { 7 - file } else { file };
    let row = if flipped { rank } else { 7 - rank };
    pos2(
        board_rect.min.x + (col as f32 + 0.5) * sq_size,
        board_rect.min.y + (row as f32 + 0.5) * sq_size,
    )
}

/// Renders the 8x8 chess board with Drag & Drop, Right-click arrows/highlights, and Multi-Engine arrows.
pub fn render_chess_board(ui: &mut Ui, app: &mut App, state: &mut GuiBoardState) {
    let avail = ui.available_size();
    let board_size = (avail.x.min(avail.y) - 20.0).clamp(320.0, 960.0);
    let (board_rect, _) =
        ui.allocate_exact_size(vec2(board_size, board_size), Sense::click_and_drag());
    let sq_size = board_size / 8.0;

    let painter = ui.painter_at(board_rect);
    let (
        pointer_pos,
        primary_pressed,
        primary_down,
        primary_released,
        secondary_pressed,
        secondary_released,
        modifiers,
        pointer_delta,
    ) = ui.input(|i| {
        (
            i.pointer.hover_pos(),
            i.pointer.primary_pressed(),
            i.pointer.primary_down(),
            i.pointer.primary_released(),
            i.pointer.secondary_pressed(),
            i.pointer.secondary_released(),
            i.modifiers,
            i.pointer.delta(),
        )
    });

    state.hovered_sq =
        pointer_pos.and_then(|p| sq_from_pos(board_rect, sq_size, app.board_flipped, p));

    // Handle Right-Click Arrows & Square Highlights
    if state.pending_promotion.is_none() {
        if secondary_pressed {
            if let Some(sq) = state.hovered_sq {
                state.right_drag_start = Some(sq);
            }
        }
        if secondary_released {
            if let Some(start) = state.right_drag_start.take() {
                if let Some(end) = state.hovered_sq {
                    if start == end {
                        let color = if modifiers.shift {
                            Color32::from_rgba_unmultiplied(166, 227, 161, 150) // Green
                        } else if modifiers.alt {
                            Color32::from_rgba_unmultiplied(249, 226, 175, 150) // Yellow
                        } else {
                            Color32::from_rgba_unmultiplied(243, 139, 168, 150) // Red
                        };
                        if let Some(pos) = state.user_highlights.iter().position(|h| h.sq == end) {
                            state.user_highlights.remove(pos);
                        } else {
                            state.user_highlights.push(UserHighlight { sq: end, color });
                        }
                    } else {
                        let color = if modifiers.shift {
                            Color32::from_rgba_unmultiplied(243, 139, 168, 200) // Red
                        } else if modifiers.alt {
                            Color32::from_rgba_unmultiplied(137, 180, 250, 200) // Blue
                        } else {
                            Color32::from_rgba_unmultiplied(166, 227, 161, 200) // Green
                        };
                        if let Some(pos) = state
                            .user_arrows
                            .iter()
                            .position(|a| a.from == start && a.to == end)
                        {
                            state.user_arrows.remove(pos);
                        } else {
                            state.user_arrows.push(UserArrow {
                                from: start,
                                to: end,
                                color,
                            });
                        }
                    }
                }
            }
        }
    }

    // Handle Left-Click & Drag-and-Drop
    if state.pending_promotion.is_none() {
        if primary_pressed {
            if let Some(sq) = state.hovered_sq {
                state.clear_annotations();
                if let Some((color, _)) = app.board.piece_on(sq) {
                    if color == app.board.side_to_move() {
                        let was_selected = app.selected_sq == Some(sq);
                        select_piece(app, sq);
                        state.dragging_sq = Some(sq);
                        state.drag_pos = pointer_pos;
                        state.drag_moved = false;
                        state.drag_was_selected = was_selected;
                    } else {
                        handle_square_click(app, state, sq);
                    }
                } else {
                    handle_square_click(app, state, sq);
                }
            }
        }

        if primary_down && state.dragging_sq.is_some() {
            if let Some(pos) = pointer_pos {
                state.drag_pos = Some(pos);
            }
            if pointer_delta.length_sq() > 2.0 {
                state.drag_moved = true;
            }
        }

        if primary_released {
            if let Some(from) = state.dragging_sq.take() {
                let moved = state.drag_moved;
                let was_selected = state.drag_was_selected;
                state.drag_pos = None;
                state.drag_moved = false;
                state.drag_was_selected = false;

                if let Some(to) = state.hovered_sq {
                    if to != from && app.legal_targets.contains(&to) {
                        execute_move_or_prompt_promo(app, state, from, to);
                    } else if to == from && !moved && was_selected {
                        app.clear_selection();
                    }
                }
            }
        }
    }

    let last_move = if state.highlight_last_move {
        app.last_move_squares()
    } else {
        None
    };

    let checked_king_sq = if app.board.in_check() {
        Some(app.board.king_square(app.board.side_to_move()))
    } else {
        None
    };

    // Pure Catppuccin Mocha board palette.
    let light_color = Color32::from_rgb(222, 227, 238);
    let dark_color = Color32::from_rgb(108, 112, 134);
    let last_mv_color = Color32::from_rgba_unmultiplied(137, 180, 250, 120);
    let sel_color = Color32::from_rgba_unmultiplied(249, 226, 175, 170);

    // Draw 64 squares & non-dragged pieces.
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

            painter.rect_filled(sq_rect, CornerRadius::ZERO, base_color);

            if let Some((from, to)) = last_move {
                if sq == from || sq == to {
                    painter.rect_filled(sq_rect, CornerRadius::ZERO, last_mv_color);
                }
            }

            if let Some(hl) = state.user_highlights.iter().find(|h| h.sq == sq) {
                painter.rect_filled(sq_rect, CornerRadius::ZERO, hl.color);
            }

            if checked_king_sq == Some(sq) {
                painter.rect_filled(
                    sq_rect,
                    CornerRadius::ZERO,
                    Color32::from_rgba_unmultiplied(243, 139, 168, 210),
                );
            }

            if app.selected_sq == Some(sq) {
                painter.rect_filled(sq_rect, CornerRadius::ZERO, sel_color);
            }

            // Legal target indicators.
            if state.highlight_legal_moves && app.legal_targets.contains(&sq) {
                let center = sq_rect.center();
                if app.board.piece_on(sq).is_some() {
                    painter.circle_stroke(
                        center,
                        sq_size * 0.44,
                        Stroke::new(
                            (sq_size * 0.08).clamp(3.0, 5.0),
                            Color32::from_rgba_unmultiplied(243, 139, 168, 210),
                        ),
                    );
                } else {
                    painter.circle_filled(
                        center,
                        sq_size * 0.16,
                        Color32::from_rgba_unmultiplied(166, 227, 161, 210),
                    );
                }
            }

            // Piece rendering (skip currently dragged piece; drawn on top below).
            if state.dragging_sq != Some(sq) {
                if let Some((color, piece_type)) = app.board.piece_on(sq) {
                    render_piece_styled(ui, &painter, sq_rect, piece_type, color, state.piece_set);
                }
            }

            // Hover indicator when dragging or hovering.
            if state.hovered_sq == Some(sq) && state.pending_promotion.is_none() {
                painter.rect_stroke(
                    sq_rect.shrink(1.5),
                    CornerRadius::same(2),
                    Stroke::new(2.0, Color32::from_rgba_unmultiplied(245, 224, 220, 160)),
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
                let font_id = egui::FontId::monospace((sq_size * 0.16).clamp(9.0, 15.0));

                if col == 0 {
                    painter.text(
                        sq_rect.min + vec2(3.0, 2.0),
                        egui::Align2::LEFT_TOP,
                        format!("{}", rank + 1),
                        font_id.clone(),
                        label_color,
                    );
                }
                if row == 7 {
                    painter.text(
                        sq_rect.max - vec2(3.0, 3.0),
                        egui::Align2::RIGHT_BOTTOM,
                        ((b'a' + file) as char).to_string(),
                        font_id,
                        label_color,
                    );
                }
            }
        }
    }

    // Multi-Engine colored best-move arrows (drawn when assistance is enabled).
    if !state.hide_assistance && state.show_engine_arrows {
        for slot in app.engines.iter().rev() {
            if slot.config.enabled {
                if let Some((from, to)) = slot.telemetry.best_move_squares {
                    let base = engine_color(slot.config.color_idx);
                    let c = Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), 190);
                    draw_board_arrow(
                        &painter,
                        board_rect,
                        sq_size,
                        app.board_flipped,
                        from,
                        to,
                        c,
                        0.9,
                    );
                }
            }
        }
    }

    // User right-click arrows.
    for arrow in &state.user_arrows {
        draw_board_arrow(
            &painter,
            board_rect,
            sq_size,
            app.board_flipped,
            arrow.from,
            arrow.to,
            arrow.color,
            1.0,
        );
    }

    // Live right-click drag preview arrow.
    if let (Some(start), Some(end)) = (state.right_drag_start, state.hovered_sq) {
        if start != end {
            draw_board_arrow(
                &painter,
                board_rect,
                sq_size,
                app.board_flipped,
                start,
                end,
                Color32::from_rgba_unmultiplied(166, 227, 161, 140),
                1.0,
            );
        }
    }

    // Render dragged piece centered under the cursor (above arrows).
    if let (Some(drag_sq), Some(pos)) = (state.dragging_sq, state.drag_pos) {
        if let Some((color, piece_type)) = app.board.piece_on(drag_sq) {
            let drag_rect = Rect::from_center_size(pos, vec2(sq_size * 1.12, sq_size * 1.12));
            render_piece_styled(ui, &painter, drag_rect, piece_type, color, state.piece_set);
        }
    }

    // Outer board border.
    painter.rect_stroke(
        board_rect,
        CornerRadius::same(4),
        Stroke::new(2.0, Color32::from_rgb(69, 71, 90)),
        StrokeKind::Inside,
    );

    // Promotion dialog overlay.
    if let Some((from, to)) = state.pending_promotion {
        render_promotion_overlay(ui, app, state, board_rect, from, to);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_board_arrow(
    painter: &egui::Painter,
    board_rect: Rect,
    sq_size: f32,
    flipped: bool,
    from: Square,
    to: Square,
    color: Color32,
    width_scale: f32,
) {
    let start = square_center(board_rect, sq_size, flipped, from);
    let end = square_center(board_rect, sq_size, flipped, to);
    let diff = end - start;
    let len = diff.length();
    if len < 1.0 {
        return;
    }
    let dir = diff / len;
    let perp = vec2(-dir.y, dir.x);

    let head_len = (sq_size * 0.34).min(len * 0.45);
    let head_half_w = sq_size * 0.22 * width_scale;
    let shaft_w = sq_size * 0.14 * width_scale;

    let shaft_start = start + dir * (sq_size * 0.12);
    let tip = end - dir * (sq_size * 0.06);
    let head_base = tip - dir * head_len;

    painter.line_segment([shaft_start, head_base], Stroke::new(shaft_w, color));
    let tri = vec![
        tip,
        head_base + perp * head_half_w,
        head_base - perp * head_half_w,
    ];
    painter.add(PathShape::convex_polygon(tri, color, Stroke::NONE));
}

fn execute_move_or_prompt_promo(
    app: &mut App,
    state: &mut GuiBoardState,
    from: Square,
    to: Square,
) {
    let matching = app.matching_moves(from, to);
    if matching.len() > 1 {
        state.pending_promotion = Some((from, to));
    } else if let Some(&mv) = matching.first() {
        let _ = app.play_move(mv);
    }
}

fn handle_square_click(app: &mut App, state: &mut GuiBoardState, sq: Square) {
    if let Some(from) = app.selected_sq {
        if from == sq {
            app.clear_selection();
            return;
        }
        if app.legal_targets.contains(&sq) {
            execute_move_or_prompt_promo(app, state, from, sq);
            return;
        }
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
            egui::FontId::monospace(10.0),
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

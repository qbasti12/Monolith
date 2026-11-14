//! Chess.com-style center stage: integrated Eval-Bar, Top/Bottom Player Cards, and Board framing.

use egui::{
    pos2, vec2, Align, Align2, Color32, CornerRadius, Frame, Margin, Rect, RichText, Sense, Stroke,
    StrokeKind, Ui,
};
use monolith_core::{Board, Color, PieceType, Square};

use crate::app::App;
use crate::engines::PlayRole;
use crate::gui_board::{render_board_with_size, GuiBoardState};
use crate::gui_theme::Mocha;
use crate::gui_widgets::{draw_icon, ui_pill, IconKind};
use crate::theme::compute_eval_bar_layout;

/// Material balance and captured piece summary computed from current board occupancy.
#[derive(Debug, Clone, Default)]
pub struct MaterialSummary {
    pub white_advantage: i32,
    pub captured_by_white: String,
    pub captured_by_black: String,
}

/// Computes material advantage and missing pieces relative to standard chess piece counts.
pub fn compute_material_summary(board: &Board) -> MaterialSummary {
    let mut w_counts = [0usize; 6];
    let mut b_counts = [0usize; 6];
    for rank in 0..8u8 {
        for file in 0..8u8 {
            if let Some((color, pt)) = board.piece_on(Square::from_coords(file, rank)) {
                let idx = match pt {
                    PieceType::Pawn => 0,
                    PieceType::Knight => 1,
                    PieceType::Bishop => 2,
                    PieceType::Rook => 3,
                    PieceType::Queen => 4,
                    PieceType::King => 5,
                };
                if color == Color::White {
                    w_counts[idx] += 1;
                } else {
                    b_counts[idx] += 1;
                }
            }
        }
    }

    let values = [1i32, 3, 3, 5, 9];
    let w_mat: i32 = (0..5).map(|i| w_counts[i] as i32 * values[i]).sum();
    let b_mat: i32 = (0..5).map(|i| b_counts[i] as i32 * values[i]).sum();

    let format_captured = |remaining: &[usize; 6]| -> String {
        let base: [(usize, &str, usize); 5] = [
            (4, "Q", 1),
            (3, "R", 2),
            (2, "B", 2),
            (1, "N", 2),
            (0, "P", 8),
        ];
        let mut out = Vec::new();
        for (idx, sym, std_cnt) in base {
            let cap = std_cnt.saturating_sub(remaining[idx]);
            if cap > 0 {
                if cap == 1 {
                    out.push(sym.to_string());
                } else {
                    out.push(format!("{sym}×{cap}"));
                }
            }
        }
        out.join(" ")
    };

    MaterialSummary {
        white_advantage: w_mat - b_mat,
        captured_by_white: format_captured(&b_counts),
        captured_by_black: format_captured(&w_counts),
    }
}

/// Shared vertical Eval-Bar painter used by both the board stage and standalone panel API.
pub fn paint_eval_bar(ui: &mut Ui, app: &App, width: f32, height: f32, is_hidden: bool) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let painter = ui.painter_at(rect);

    if is_hidden {
        painter.rect_filled(rect, CornerRadius::same(5), Mocha::MANTLE);
        painter.rect_stroke(
            rect,
            CornerRadius::same(5),
            Stroke::new(1.0, Mocha::SURFACE0),
            StrokeKind::Inside,
        );
        return;
    }

    let (score_cp, score_mate) = app.primary_eval_white();
    let eval_layout = compute_eval_bar_layout(100, score_cp, score_mate, app.board_flipped);
    let white_ratio = eval_layout.white_prob as f32;

    let white_col = Color32::from_rgb(235, 238, 248);
    let black_col = Mocha::SURFACE0;
    let (top_ratio, top_col, bot_col) = if eval_layout.top_is_white {
        (white_ratio, white_col, black_col)
    } else {
        (1.0 - white_ratio, black_col, white_col)
    };

    let radius = CornerRadius::same(5);
    painter.rect_filled(rect, radius, top_col);
    let split_y = (rect.min.y + rect.height() * top_ratio).clamp(rect.min.y, rect.max.y);
    let bot_rect = Rect::from_min_max(pos2(rect.min.x, split_y), rect.max);
    painter.rect_filled(bot_rect, radius, bot_col);
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, Mocha::SURFACE1),
        StrokeKind::Inside,
    );

    let white_ahead = white_ratio >= 0.5;
    let label_at_top = if eval_layout.top_is_white {
        white_ahead
    } else {
        !white_ahead
    };
    let text_y = if label_at_top {
        rect.min.y + 12.0
    } else {
        rect.max.y - 12.0
    };
    let text_col = if white_ahead {
        Mocha::CRUST
    } else {
        Mocha::TEXT
    };
    painter.text(
        pos2(rect.center().x, text_y),
        Align2::CENTER_CENTER,
        eval_layout.label,
        egui::FontId::monospace(9.5),
        text_col,
    );
}

/// Renders the complete Chess.com center stage: Eval-Bar + Top Player Bar + 8x8 Board + Bottom Player Bar.
pub fn render_board_stage(
    ui: &mut Ui,
    app: &mut App,
    state: &mut GuiBoardState,
    show_eval_bar: bool,
) {
    let avail = ui.available_size();
    let bar_h = 44.0;
    let gap = 8.0;
    let eval_w = 26.0;
    let eval_gap = 10.0;

    let max_board_h = (avail.y - bar_h * 2.0 - gap * 2.0 - 12.0).max(280.0);
    let eval_reserve = if show_eval_bar {
        eval_w + eval_gap
    } else {
        0.0
    };
    let max_board_w = (avail.x - eval_reserve - 16.0).max(280.0);
    let board_size = max_board_w.min(max_board_h).clamp(280.0, 920.0);

    let total_w = board_size + eval_reserve;
    let total_h = board_size + bar_h * 2.0 + gap * 2.0;
    let pad_x = ((avail.x - total_w) * 0.5).max(4.0);
    let pad_y = ((avail.y - total_h) * 0.5).max(4.0);

    let (top_color, bot_color) = if app.board_flipped {
        (Color::White, Color::Black)
    } else {
        (Color::Black, Color::White)
    };
    let mat = compute_material_summary(&app.board);

    ui.add_space(pad_y);
    ui.horizontal(|ui| {
        ui.add_space(pad_x);

        if show_eval_bar {
            ui.vertical(|ui| {
                ui.add_space(bar_h + gap);
                paint_eval_bar(ui, app, eval_w, board_size, false);
            });
            ui.add_space(eval_gap);
        }

        ui.vertical(|ui| {
            render_player_bar(ui, app, top_color, &mat, board_size, bar_h);
            ui.add_space(gap);
            render_board_with_size(ui, app, state, board_size);
            ui.add_space(gap);
            render_player_bar(ui, app, bot_color, &mat, board_size, bar_h);
        });
    });
}

fn player_identity(app: &App, side: Color, is_engine_side: bool) -> (String, Option<u32>) {
    if is_engine_side {
        let slot = &app.engines[0];
        let name = if slot.telemetry.reported_name.is_empty() {
            "Monolith Engine".to_string()
        } else {
            slot.telemetry.reported_name.clone()
        };
        return (name, Some(slot.config.elo));
    }

    if matches!(app.play_role, PlayRole::PlayBlack | PlayRole::PlayWhite) {
        let label = if side == Color::White {
            "Du (Weiß)"
        } else {
            "Du (Schwarz)"
        };
        return (label.to_string(), None);
    }

    if let Some(pg) = &app.pgn_progress {
        let parts: Vec<&str> = pg.title.split(" vs ").collect();
        if parts.len() == 2 {
            let name = if side == Color::White {
                parts[0]
            } else {
                parts[1]
            };
            return (name.to_string(), None);
        }
    }

    let fallback = if side == Color::White {
        "Weiß"
    } else {
        "Schwarz"
    };
    (fallback.to_string(), None)
}

fn render_player_bar(
    ui: &mut Ui,
    app: &App,
    side: Color,
    mat: &MaterialSummary,
    width: f32,
    height: f32,
) {
    let is_turn = app.board.side_to_move() == side && app.game_outcome_label().is_none();
    let is_engine_side = matches!(
        (app.play_role, side),
        (PlayRole::PlayBlack, Color::Black) | (PlayRole::PlayWhite, Color::White)
    );
    let is_thinking = is_engine_side && app.engines.iter().any(|e| e.telemetry.searching);
    let (name, elo_opt) = player_identity(app, side, is_engine_side);

    let (captured, adv) = if side == Color::White {
        (&mat.captured_by_white, mat.white_advantage)
    } else {
        (&mat.captured_by_black, -mat.white_advantage)
    };

    Frame::new()
        .fill(Mocha::MANTLE)
        .stroke(Stroke::new(
            1.0,
            if is_turn {
                Mocha::BLUE
            } else {
                Mocha::SURFACE0
            },
        ))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(width - 20.0);
            ui.set_height(height - 12.0);
            ui.horizontal_centered(|ui| {
                let (av_rect, _) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::hover());
                let p = ui.painter();
                let (av_bg, av_fg) = if side == Color::White {
                    (Color32::from_rgb(220, 226, 240), Mocha::CRUST)
                } else {
                    (Mocha::SURFACE1, Mocha::TEXT)
                };
                p.rect_filled(av_rect, CornerRadius::same(6), av_bg);
                draw_icon(
                    p,
                    av_rect.shrink(4.0),
                    if is_engine_side {
                        IconKind::Bot
                    } else {
                        IconKind::User
                    },
                    av_fg,
                );

                ui.add_space(4.0);
                ui.label(RichText::new(name).size(13.5).strong().color(Mocha::TEXT));

                if let Some(elo) = elo_opt {
                    ui_pill(ui, &format!("{elo}"), Mocha::SUBTEXT1, Mocha::SURFACE0);
                }

                if !captured.is_empty() || adv > 0 {
                    let cap_txt = if adv > 0 {
                        if captured.is_empty() {
                            format!("+{adv}")
                        } else {
                            format!("{captured}  +{adv}")
                        }
                    } else {
                        captured.clone()
                    };
                    ui.label(
                        RichText::new(cap_txt)
                            .monospace()
                            .size(11.5)
                            .color(Mocha::SUBTEXT0),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    if is_thinking {
                        ui.spinner();
                        ui.label(RichText::new("Denkt...").small().color(Mocha::GREEN));
                    } else if is_turn {
                        let (dot_r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                        ui.painter()
                            .circle_filled(dot_r.center(), 4.5, Mocha::GREEN);
                        ui.label(RichText::new("Am Zug").small().color(Mocha::GREEN));
                    }
                });
            });
        });
}

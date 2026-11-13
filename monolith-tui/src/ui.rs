//! Catppuccin Mocha terminal rendering via `ratatui`.

use monolith_core::pgn::MoveClass;
use monolith_core::{Color, PieceType, Square};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Gauge, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, InputFocus};
use crate::engines::EngineSlot;
use crate::modals::render_overlay;
use crate::theme::{compute_eval_bar_layout, format_eval_short, Theme};

/// Renders the entire client UI onto the given `ratatui::Frame`.
pub fn render(frame: &mut Frame, app: &App) {
    let t = &app.theme;
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(t.base).fg(t.text)),
        area,
    );

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(12),
            Constraint::Length(3),
        ])
        .split(area);

    render_header(frame, app, root[0]);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(7),
            Constraint::Min(30),
            Constraint::Percentage(38),
        ])
        .split(root[1]);

    render_eval_bar(frame, app, body[0]);
    render_board_panel(frame, app, body[1]);
    render_side_panel(frame, app, body[2]);
    render_footer(frame, app, root[2]);
    render_overlay(frame, app);
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let turn = match app.board.side_to_move() {
        Color::White => Span::styled(
            " Weiß am Zug ",
            Style::default().bg(t.text).fg(t.crust).bold(),
        ),
        Color::Black => Span::styled(
            " Schwarz am Zug ",
            Style::default().bg(t.surface1).fg(t.text).bold(),
        ),
    };

    let mode_text = if let Some(pg) = &app.pgn_progress {
        if pg.review_navigation {
            format!(
                " | PGN: {} ({}/{})",
                pg.title,
                app.view_ply,
                app.history.len()
            )
        } else {
            format!(" | Modus: {}", app.play_role.label())
        }
    } else {
        format!(" | Modus: {}", app.play_role.label())
    };

    let mut spans = vec![
        Span::styled(
            " ♜ MONOLITH ",
            Style::default().bg(t.mauve).fg(t.crust).bold(),
        ),
        Span::raw(" "),
        turn,
        Span::styled(mode_text, Style::default().fg(t.subtext0)),
    ];

    if let Some(outcome) = app.game_outcome_label() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!(" ★ {outcome} "),
            Style::default().bg(t.yellow).fg(t.crust).bold(),
        ));
    } else if app.board.in_check() {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            " SCHACH! ",
            Style::default().bg(t.red).fg(t.crust).bold(),
        ));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(t.mantle)),
        area,
    );
}

fn render_eval_bar(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let block = Block::default()
        .title(" Eval ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.surface1))
        .style(Style::default().bg(t.mantle));

    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 3 || inner.width == 0 {
        return;
    }

    let (cp, mate) = app.primary_eval_white();
    let bar_rows = inner.height.saturating_sub(1) as usize;
    let layout = compute_eval_bar_layout(bar_rows, cp, mate, app.board_flipped);

    let mut lines = Vec::with_capacity(inner.height as usize);
    let fill_str = "█".repeat(inner.width as usize);

    for row in 0..bar_rows {
        let color = if layout.is_white_row(row) {
            t.text
        } else {
            t.surface0
        };
        lines.push(Line::from(Span::styled(
            fill_str.clone(),
            Style::default().fg(color),
        )));
    }

    let score_color = if layout.white_prob >= 0.55 {
        t.green
    } else if layout.white_prob <= 0.45 {
        t.red
    } else {
        t.yellow
    };
    lines.push(Line::from(Span::styled(
        layout.label,
        Style::default().fg(score_color).bold(),
    )));

    frame.render_widget(Paragraph::new(lines).alignment(Alignment::Center), inner);
}

/// Pure headless-testable geometry for the TUI chessboard panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardLayout {
    pub board_rect: Rect,
    pub cell_w: u16,
    pub cell_h: u16,
    pub flipped: bool,
    pub piece_subrow: u16,
}

impl BoardLayout {
    /// Returns the terminal rectangle for board square `sq` (excluding rank/file margin).
    pub fn square_rect(&self, sq: Square) -> Rect {
        let v_file = if self.flipped {
            7 - sq.file()
        } else {
            sq.file()
        } as u16;
        let v_rank = if self.flipped {
            sq.rank()
        } else {
            7 - sq.rank()
        } as u16;
        Rect::new(
            self.board_rect.x + 2 + v_file * self.cell_w,
            self.board_rect.y + v_rank * self.cell_h,
            self.cell_w,
            self.cell_h,
        )
    }
}

/// Computes responsive TUI board dimensions inside `inner`, returning `None` if too small.
pub fn compute_board_layout(inner: Rect, flipped: bool) -> Option<BoardLayout> {
    if inner.width < 18 || inner.height < 10 {
        return None;
    }
    let avail_w = inner.width.saturating_sub(3);
    let avail_h = inner.height.saturating_sub(2);
    let cell_h = (avail_h / 8).clamp(1, 5);
    let max_cell_w = (avail_w / 8).max(2);
    let cell_w = (cell_h * 2 + 1).min(max_cell_w).max(3).min(max_cell_w);

    let board_w = cell_w * 8 + 3;
    let board_h = cell_h * 8 + 1;
    let x_off = inner.x + inner.width.saturating_sub(board_w) / 2;
    let y_off = inner.y + inner.height.saturating_sub(board_h) / 2;

    Some(BoardLayout {
        board_rect: Rect::new(
            x_off,
            y_off,
            board_w.min(inner.width),
            board_h.min(inner.height),
        ),
        cell_w,
        cell_h,
        flipped,
        piece_subrow: cell_h / 2,
    })
}

fn render_board_panel(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let block = Block::default()
        .title(if app.board_flipped {
            " Brett (Schwarz unten) "
        } else {
            " Brett (Weiß unten) "
        })
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.blue))
        .style(Style::default().bg(t.base));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(layout) = compute_board_layout(inner, app.board_flipped) else {
        frame.render_widget(
            Paragraph::new("Terminal zu klein für Brett").style(Style::default().fg(t.red)),
            inner,
        );
        return;
    };

    let cell_w = layout.cell_w;
    let cell_h = layout.cell_h;
    let piece_subrow = layout.piece_subrow;
    let board_h = cell_h * 8 + 1;

    let last_mv = app.last_move_squares();
    let best_mv = app.engines.first().and_then(|e| {
        if e.config.enabled {
            e.telemetry.best_move_squares
        } else {
            None
        }
    });
    let stm_check = app.board.in_check();
    let stm = app.board.side_to_move();
    let cur_dest_badge = app.current_move_record().and_then(|rec| {
        rec.classification
            .as_ref()
            .map(|cls| (rec.mv.to_sq(), tui_class_badge(cls.class, t)))
    });

    let mut lines = Vec::with_capacity(board_h as usize);

    for v_rank in 0..8u8 {
        let rank = if app.board_flipped {
            v_rank
        } else {
            7 - v_rank
        };

        for sub_r in 0..cell_h {
            let mut spans = Vec::with_capacity(9);
            if sub_r == piece_subrow {
                spans.push(Span::styled(
                    format!("{} ", rank + 1),
                    Style::default().fg(t.subtext0),
                ));
            } else {
                spans.push(Span::raw("  "));
            }

            for v_file in 0..8u8 {
                let file = if app.board_flipped {
                    7 - v_file
                } else {
                    v_file
                };
                let sq = Square::from_coords(file, rank);
                let is_light = (file + rank) % 2 == 1;

                let piece = app.board.piece_on(sq);
                let is_checked_king = stm_check && piece == Some((stm, PieceType::King));
                let is_cursor = app.cursor == sq;
                let is_selected = app.selected_sq == Some(sq);
                let is_last = last_mv.is_some_and(|(f, to)| f == sq || to == sq);
                let is_best = best_mv.is_some_and(|(f, to)| f == sq || to == sq);
                let is_target = app.legal_targets.contains(&sq);

                let bg = if is_checked_king {
                    t.board_check
                } else if is_selected {
                    t.board_selected
                } else if is_cursor {
                    t.board_cursor
                } else if is_last {
                    if is_light {
                        t.board_last_light
                    } else {
                        t.board_last_dark
                    }
                } else if is_light {
                    t.board_light
                } else {
                    t.board_dark
                };

                let dest_badge =
                    cur_dest_badge.and_then(|(d_sq, b)| if d_sq == sq { Some(b) } else { None });

                let (text, fg, mod_flags) = if sub_r == piece_subrow {
                    render_square_center(
                        sq,
                        piece,
                        is_target,
                        is_best,
                        is_cursor,
                        if cell_h == 1 { dest_badge } else { None },
                        cell_w,
                        t,
                    )
                } else if sub_r == 0 && (is_best || dest_badge.is_some()) {
                    let w = cell_w as usize;
                    if let Some((badge, badge_col)) = dest_badge {
                        let b_len = badge.chars().count();
                        let left_mark = if is_best && w > b_len { "★" } else { "" };
                        let l_len = left_mark.chars().count();
                        let pad = w.saturating_sub(b_len + l_len);
                        (
                            format!("{left_mark}{}{badge}", " ".repeat(pad)),
                            badge_col,
                            Modifier::BOLD,
                        )
                    } else {
                        let marker = format!("★{:width$}", "", width = w.saturating_sub(1));
                        (marker, t.green, Modifier::BOLD)
                    }
                } else {
                    (" ".repeat(cell_w as usize), t.text, Modifier::empty())
                };
                let _ = sq;
                spans.push(Span::styled(
                    text,
                    Style::default().bg(bg).fg(fg).add_modifier(mod_flags),
                ));
            }
            lines.push(Line::from(spans));
        }
    }

    // File coordinate footer (`a..h`).
    let mut file_line = String::from("  ");
    for v_file in 0..8u8 {
        let file = if app.board_flipped {
            7 - v_file
        } else {
            v_file
        };
        let ch = (b'a' + file) as char;
        let left_pad = (cell_w as usize).saturating_sub(1) / 2;
        let right_pad = (cell_w as usize).saturating_sub(1 + left_pad);
        for _ in 0..left_pad {
            file_line.push(' ');
        }
        file_line.push(ch);
        for _ in 0..right_pad {
            file_line.push(' ');
        }
    }
    lines.push(Line::from(Span::styled(
        file_line,
        Style::default().fg(t.subtext0),
    )));

    frame.render_widget(Paragraph::new(lines), layout.board_rect);
}

fn tui_class_badge(class: MoveClass, t: &Theme) -> (&'static str, ratatui::style::Color) {
    match class {
        MoveClass::Brilliant => ("!!", t.mauve),
        MoveClass::Great => ("!", t.blue),
        MoveClass::Best => ("★", t.green),
        MoveClass::Good => ("✓", t.teal),
        MoveClass::Inaccuracy => ("?!", t.yellow),
        MoveClass::Mistake => ("?", t.peach),
        MoveClass::Blunder => ("??", t.red),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_square_center(
    _sq: Square,
    piece: Option<(Color, PieceType)>,
    is_target: bool,
    is_best: bool,
    is_cursor: bool,
    inline_badge: Option<(&'static str, ratatui::style::Color)>,
    cell_w: u16,
    t: &Theme,
) -> (String, ratatui::style::Color, Modifier) {
    let w = cell_w as usize;
    let left_pad = w.saturating_sub(1) / 2;
    let right_pad = w.saturating_sub(1 + left_pad);

    let (glyph, fg, mut mods) = match piece {
        Some((color, pt)) => {
            let g = piece_unicode(color, pt);
            let fg_color = if color == Color::White {
                t.piece_white
            } else {
                t.piece_black
            };
            (g, fg_color, Modifier::BOLD)
        }
        None if is_target => ('•', t.green, Modifier::BOLD),
        None => (' ', t.text, Modifier::empty()),
    };

    if is_cursor && piece.is_none() && !is_target {
        mods |= Modifier::UNDERLINED;
    }

    let mut s = String::with_capacity(w + 4);
    for idx in 0..left_pad {
        if idx == 0 && is_best {
            s.push('▸');
        } else {
            s.push(' ');
        }
    }
    s.push(glyph);
    if let Some((badge, _)) = inline_badge {
        let b_chars: Vec<char> = badge.chars().take(right_pad).collect();
        let spaces = right_pad.saturating_sub(b_chars.len());
        for _ in 0..spaces {
            s.push(' ');
        }
        for c in b_chars {
            s.push(c);
        }
    } else {
        for _ in 0..right_pad {
            s.push(' ');
        }
    }
    (s, fg, mods)
}

fn piece_unicode(color: Color, pt: PieceType) -> char {
    match (color, pt) {
        (Color::White, PieceType::King) => '♔',
        (Color::White, PieceType::Queen) => '♕',
        (Color::White, PieceType::Rook) => '♖',
        (Color::White, PieceType::Bishop) => '♗',
        (Color::White, PieceType::Knight) => '♘',
        (Color::White, PieceType::Pawn) => '♙',
        (Color::Black, PieceType::King) => '♚',
        (Color::Black, PieceType::Queen) => '♛',
        (Color::Black, PieceType::Rook) => '♜',
        (Color::Black, PieceType::Bishop) => '♝',
        (Color::Black, PieceType::Knight) => '♞',
        (Color::Black, PieceType::Pawn) => '♟',
    }
}

fn render_side_panel(frame: &mut Frame, app: &App, area: Rect) {
    let enabled_indices: Vec<usize> = app
        .engines
        .iter()
        .enumerate()
        .filter(|(_, e)| e.config.enabled)
        .map(|(i, _)| i)
        .collect();

    let engine_box_h = 5u16;
    let total_engine_h = (enabled_indices.len() as u16) * engine_box_h;
    let pgn_h = if app.pgn_progress.is_some() {
        5u16
    } else {
        0u16
    };

    let mut constraints = Vec::new();
    for _ in &enabled_indices {
        constraints.push(Constraint::Length(engine_box_h));
    }
    if pgn_h > 0 {
        constraints.push(Constraint::Length(pgn_h));
    }
    constraints.push(Constraint::Min(6));

    let _ = total_engine_h;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let mut chunk_idx = 0;
    for &eng_idx in &enabled_indices {
        render_engine_box(
            frame,
            app,
            eng_idx,
            &app.engines[eng_idx],
            chunks[chunk_idx],
        );
        chunk_idx += 1;
    }

    if let Some(pg) = &app.pgn_progress {
        render_pgn_panel(frame, app, pg, chunks[chunk_idx]);
        chunk_idx += 1;
    }

    render_move_history(frame, app, chunks[chunk_idx]);
}

fn render_engine_box(frame: &mut Frame, app: &App, idx: usize, slot: &EngineSlot, area: Rect) {
    let t = &app.theme;
    let strength_tag = if slot.config.limit_strength {
        format!("Elo {}", slot.config.elo)
    } else {
        format!("T{}", slot.config.depth)
    };
    let title = format!(
        " {} : {} [{}] ",
        slot.config.label, slot.telemetry.reported_name, strength_tag
    );

    let border_col = if slot.telemetry.error.is_some() {
        t.red
    } else if slot.telemetry.searching {
        t.green
    } else if idx == 0 {
        t.lavender
    } else {
        t.surface2
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_col))
        .style(Style::default().bg(t.mantle));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(err) = &slot.telemetry.error {
        frame.render_widget(
            Paragraph::new(format!("Fehler: {err}")).style(Style::default().fg(t.red)),
            inner,
        );
        return;
    }

    let eval_s = format_eval_short(
        slot.telemetry.white_score_cp,
        slot.telemetry.white_score_mate,
    );
    let nps_s = format_nps(slot.telemetry.nps);
    let best_s = slot
        .telemetry
        .best_move_san
        .as_deref()
        .or(slot.telemetry.best_move_uci.as_deref())
        .unwrap_or("-");
    let reply_s = slot.telemetry.expected_reply_san.as_deref().unwrap_or("-");

    let line1 = Line::from(vec![
        Span::styled(eval_s, Style::default().fg(t.yellow).bold()),
        Span::styled(
            format!(
                "  D:{}/{}  Kn:{}  {}",
                slot.telemetry.depth,
                slot.telemetry.seldepth,
                format_nodes(slot.telemetry.nodes),
                nps_s
            ),
            Style::default().fg(t.subtext0),
        ),
    ]);
    let line2 = Line::from(vec![
        Span::styled("Bester Zug: ", Style::default().fg(t.subtext0)),
        Span::styled(best_s, Style::default().fg(t.green).bold()),
        Span::styled("  Erwartet: ", Style::default().fg(t.subtext0)),
        Span::styled(reply_s, Style::default().fg(t.sapphire).bold()),
    ]);
    let pv_text = if slot.telemetry.pv_san.is_empty() {
        if slot.telemetry.searching {
            "Suche läuft...".to_string()
        } else {
            "-".to_string()
        }
    } else {
        slot.telemetry.pv_san.clone()
    };
    let line3 = Line::from(Span::styled(pv_text, Style::default().fg(t.text)));

    frame.render_widget(Paragraph::new(vec![line1, line2, line3]), inner);
}

fn render_pgn_panel(frame: &mut Frame, app: &App, pg: &crate::app::PgnProgress, area: Rect) {
    let t = &app.theme;
    let block = Block::default()
        .title(format!(" PGN-Analyse: {} ", pg.title))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.peach))
        .style(Style::default().bg(t.mantle));

    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height < 2 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    let ratio = if pg.total_plies == 0 {
        1.0
    } else {
        (pg.done_plies as f64 / pg.total_plies as f64).clamp(0.0, 1.0)
    };
    let label = if pg.running {
        format!(
            "Analysiere {}/{} ({:.0}%)",
            pg.done_plies,
            pg.total_plies,
            ratio * 100.0
        )
    } else {
        format!(
            "Fertig {}/{} ([←/→] Zugwahl, [C] Modus)",
            pg.done_plies, pg.total_plies
        )
    };

    let gauge = Gauge::default()
        .ratio(ratio)
        .label(label)
        .gauge_style(Style::default().fg(t.peach).bg(t.surface0));
    frame.render_widget(gauge, chunks[0]);

    let info_line = if let Some(rec) = app.current_move_record() {
        if let Some(cls) = &rec.classification {
            Line::from(vec![
                Span::styled(
                    format!("{} [{}] ", rec.san, cls.class.symbol()),
                    Style::default().fg(t.yellow).bold(),
                ),
                Span::styled(&cls.explanation, Style::default().fg(t.text)),
            ])
        } else {
            Line::from(Span::styled(
                format!("Zug {}: {} (wird bewertet...)", rec.move_number, rec.san),
                Style::default().fg(t.subtext0),
            ))
        }
    } else {
        Line::from(Span::styled(
            "Startposition der Partie.",
            Style::default().fg(t.subtext0),
        ))
    };

    frame.render_widget(
        Paragraph::new(info_line).wrap(Wrap { trim: true }),
        chunks[1],
    );
}

fn render_move_history(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let block = Block::default()
        .title(format!(
            " Zugliste ({}/{}) ",
            app.view_ply,
            app.history.len()
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.surface1))
        .style(Style::default().bg(t.mantle));

    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let mut lines = Vec::new();
    let mut i = 0;
    while i < app.history.len() {
        let w = &app.history[i];
        let b = app.history.get(i + 1);

        let w_badge = w
            .classification
            .as_ref()
            .map(|c| format!(" {}", c.class.symbol()))
            .unwrap_or_default();
        let w_active = app.view_ply == i + 1;
        let w_style = if w_active {
            Style::default().bg(t.surface1).fg(t.yellow).bold()
        } else {
            Style::default().fg(t.text)
        };

        let mut spans = vec![
            Span::styled(
                format!("{:>3}. ", w.move_number),
                Style::default().fg(t.subtext0),
            ),
            Span::styled(format!("{:<8}", format!("{}{}", w.san, w_badge)), w_style),
        ];

        if let Some(bm) = b {
            let b_badge = bm
                .classification
                .as_ref()
                .map(|c| format!(" {}", c.class.symbol()))
                .unwrap_or_default();
            let b_active = app.view_ply == i + 2;
            let b_style = if b_active {
                Style::default().bg(t.surface1).fg(t.yellow).bold()
            } else {
                Style::default().fg(t.text)
            };
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                format!("{:<8}", format!("{}{}", bm.san, b_badge)),
                b_style,
            ));
        }

        lines.push(Line::from(spans));
        i += 2;
    }

    let active_row = app.view_ply.saturating_sub(1) / 2;
    let visible = inner.height as usize;
    let scroll = if active_row >= visible {
        (active_row - visible + 1) as u16
    } else {
        0
    };

    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let t = &app.theme;
    let border_col = if app.input_focus == InputFocus::MoveBox {
        t.yellow
    } else {
        t.surface1
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_col))
        .style(Style::default().bg(t.mantle));

    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }

    let prompt_style = if app.input_focus == InputFocus::MoveBox {
        Style::default().fg(t.yellow).bold()
    } else {
        Style::default().fg(t.subtext0)
    };
    let cursor_str = if app.input_focus == InputFocus::MoveBox {
        "_"
    } else {
        ""
    };

    let line1 = Line::from(vec![
        Span::styled("Zug [/]: ", prompt_style),
        Span::styled(
            format!("{}{cursor_str}   ", app.move_input),
            Style::default().fg(t.text).bold(),
        ),
        Span::styled(&app.status_message, Style::default().fg(t.teal)),
    ]);

    frame.render_widget(Paragraph::new(line1), inner);
}

fn format_nodes(nodes: u64) -> String {
    if nodes >= 1_000_000 {
        format!("{:.1}M", nodes as f64 / 1_000_000.0)
    } else if nodes >= 1_000 {
        format!("{}k", nodes / 1_000)
    } else {
        nodes.to_string()
    }
}

fn format_nps(nps: u64) -> String {
    if nps >= 1_000_000 {
        format!("{:.2} MN/s", nps as f64 / 1_000_000.0)
    } else if nps >= 1_000 {
        format!("{} kN/s", nps / 1_000)
    } else {
        format!("{nps} N/s")
    }
}

//! Modal overlay rendering for FEN/PGN inputs, engine setup, promotion, and help.

use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{
    App, Overlay, FEN_PRESET_INFINITE_QUEENS, FEN_PRESET_KIWIPETE, FEN_PRESET_STANDARD,
};
use crate::theme::Theme;

/// Renders the currently active modal popup centered over the main screen, if any.
pub fn render_overlay(frame: &mut Frame, app: &App) {
    let t = &app.theme;
    match &app.overlay {
        Overlay::None => {}
        Overlay::Fen(modal) => render_text_modal(
            frame,
            t,
            "FEN laden [F]",
            "FEN-String eingeben oder Preset wählen:",
            "[F1] Startpos   [F2] Infinite Queens   [F3] Kiwipete   [Enter] Laden   [Esc] Abbruch",
            &modal.input,
            modal.error.as_deref(),
        ),
        Overlay::Pgn(modal) => render_text_modal(
            frame,
            t,
            "PGN laden & analysieren [P]",
            "Dateipfad (.pgn) oder PGN-Text eingeben:",
            "[F1] Beispiel (Morphy Opera Game)   [Enter] Analysieren   [Esc] Abbruch",
            &modal.input,
            modal.error.as_deref(),
        ),
        Overlay::Engines {
            selected_slot,
            selected_field,
            editing_path,
            path_buffer,
        } => render_engines_modal(
            frame,
            app,
            *selected_slot,
            *selected_field,
            *editing_path,
            path_buffer,
        ),
        Overlay::Help => render_help_modal(frame, t),
        Overlay::Promotion { .. } => render_promotion_modal(frame, t),
    }
}

fn render_text_modal(
    frame: &mut Frame,
    t: &Theme,
    title: &str,
    prompt: &str,
    hints: &str,
    input: &str,
    error: Option<&str>,
) {
    let popup = centered_rect(76, 11, frame.area());
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(format!(" {title} "))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.blue))
        .style(Style::default().bg(t.base).fg(t.text));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let mut lines = vec![
        Line::from(Span::styled(prompt, Style::default().fg(t.subtext1))),
        Line::from(""),
        Line::from(Span::styled(
            format!("> {input}_"),
            Style::default().bg(t.surface0).fg(t.yellow).bold(),
        )),
        Line::from(""),
        Line::from(Span::styled(hints, Style::default().fg(t.subtext0))),
    ];
    if let Some(err) = error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            err,
            Style::default().fg(t.red).bold(),
        )));
    }

    let _ = (
        FEN_PRESET_STANDARD,
        FEN_PRESET_INFINITE_QUEENS,
        FEN_PRESET_KIWIPETE,
    );
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_engines_modal(
    frame: &mut Frame,
    app: &App,
    selected_slot: usize,
    selected_field: usize,
    editing_path: bool,
    path_buffer: &str,
) {
    let t = &app.theme;
    let popup = centered_rect(76, 17, frame.area());
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Multi-Engine Konfiguration [E] ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.mauve))
        .style(Style::default().bg(t.base).fg(t.text));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let slot = &app.engines[selected_slot];
    let mut tabs = Vec::new();
    for (i, s) in app.engines.iter().enumerate() {
        let st = if i == selected_slot {
            Style::default().bg(t.mauve).fg(t.crust).bold()
        } else {
            Style::default().bg(t.surface0).fg(t.subtext0)
        };
        let state_ch = if s.config.enabled { "●" } else { "○" };
        tabs.push(Span::styled(format!(" [{state_ch} {}] ", i + 1), st));
        tabs.push(Span::raw(" "));
    }

    let row_style = |idx: usize| -> Style {
        if idx == selected_field {
            Style::default().bg(t.surface1).fg(t.yellow).bold()
        } else {
            Style::default().fg(t.text)
        }
    };

    let path_display = if editing_path {
        format!("{path_buffer}_")
    } else {
        slot.config.binary_path.clone()
    };

    let lines = vec![
        Line::from(tabs),
        Line::from(""),
        Line::from(Span::styled(
            format!(
                "  Aktiviert (Slot 1 immer an): {}",
                if slot.config.enabled { "Ja" } else { "Nein" }
            ),
            row_style(0),
        )),
        Line::from(Span::styled(
            format!(
                "  Stärke begrenzen (UCI_LimitStrength): {}",
                if slot.config.limit_strength {
                    "Ja"
                } else {
                    "Nein"
                }
            ),
            row_style(1),
        )),
        Line::from(Span::styled(
            format!("  Ziel-Elo (UCI_Elo 800..3200): {}", slot.config.elo),
            row_style(2),
        )),
        Line::from(Span::styled(
            format!("  Suchtiefe (1..30): {}", slot.config.depth),
            row_style(3),
        )),
        Line::from(Span::styled(
            format!("  UCI-Binary ([Enter] bearbeiten): {path_display}"),
            row_style(4),
        )),
        Line::from(Span::styled(
            format!("  Spielmodus: {}", app.play_role.label()),
            row_style(5),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "[1/2/3] Slot  [↑/↓] Feld  [←/→/Space] Ändern  [Enter] Übernehmen  [Esc] Schließen",
            Style::default().fg(t.subtext0),
        )),
    ];

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_help_modal(frame: &mut Frame, t: &Theme) {
    let popup = centered_rect(76, 17, frame.area());
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Monolith TUI – Tastenkürzel [?] ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.green))
        .style(Style::default().bg(t.base).fg(t.text));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let rows = vec![
        "Pfeiltasten + Enter/Space : Figur auf dem Brett wählen & ziehen",
        "/  oder  Tab  oder  M     : Texteingabe für Züge (e2e4, Nf3, O-O)",
        "U                         : Letzten Zug zurücknehmen (Undo)",
        "X                         : Brett drehen (Weiß / Schwarz unten)",
        "[ / ] oder PgUp / PgDn    : Halbzüge vor/zurück navigieren",
        "Home / End                : Anfang / Ende der Zugliste",
        "F                         : FEN-Dialog + Presets (Standard/14 Damen/Kiwipete)",
        "P                         : PGN-Datei/Text laden & Hintergrundanalyse starten",
        "E                         : Multi-Engine-Setup (1–3 Engines, Elo, Pfad, Spielmodus)",
        "C                         : PGN-Analyse abbrechen / PGN-Navigationsmodus umschalten",
        "Q  oder  Ctrl+Q / Ctrl+C  : Client sauber beenden",
    ];
    let lines: Vec<Line> = rows
        .into_iter()
        .map(|r| Line::from(Span::styled(r, Style::default().fg(t.text))))
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_promotion_modal(frame: &mut Frame, t: &Theme) {
    let popup = centered_rect(44, 6, frame.area());
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Bauernumwandlung ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.yellow))
        .style(Style::default().bg(t.base).fg(t.text));
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let text = vec![
        Line::from(Span::styled(
            "Figur wählen:",
            Style::default().fg(t.subtext1),
        )),
        Line::from(Span::styled(
            "[Q] Dame   [R] Turm   [B] Läufer   [N] Springer",
            Style::default().fg(t.yellow).bold(),
        )),
    ];
    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), inner);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect::new(x, y, w, h)
}

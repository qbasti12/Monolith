use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use monolith_core::Square;
use monolith_tui::app::{App, InputFocus, Overlay, FEN_PRESET_INFINITE_QUEENS, SAMPLE_OPERA_PGN};
use monolith_tui::ui;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn headless_app_moves_undo_flip_and_fen_presets() {
    let mut app = App::new_headless();

    // Focus move box via '/' and submit UCI + SAN moves.
    app.handle_key(key(KeyCode::Char('/')));
    assert_eq!(app.input_focus, InputFocus::MoveBox);
    for ch in "e2e4".chars() {
        app.handle_key(key(KeyCode::Char(ch)));
    }
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.history[0].san, "e4");

    assert!(app.submit_move_text("e5").is_ok());
    assert!(app.submit_move_text("Nf3").is_ok());
    assert!(app.submit_move_text("not_a_move").is_err());
    assert_eq!(app.history.len(), 3);

    // Undo move via 'u'.
    app.handle_key(key(KeyCode::Esc)); // leave MoveBox
    assert_eq!(app.input_focus, InputFocus::Board);
    // Reject illegal move without corrupting state
    let pre_err_len = app.history.len();
    let pre_err_fen = app.board.to_fen();
    assert!(app.submit_move_text("e2e5").is_err());
    assert_eq!(app.history.len(), pre_err_len);
    assert_eq!(app.board.to_fen(), pre_err_fen);
    app.handle_key(key(KeyCode::Char('u')));
    assert_eq!(app.history.len(), 2);
    assert_eq!(app.view_ply, 2);

    // Flip board via 'x'.
    assert!(!app.board_flipped);
    app.handle_key(key(KeyCode::Char('x')));
    assert!(app.board_flipped);

    // Open FEN modal, enter invalid FEN -> stays open with error and preserves board/history state!
    let pre_fen = app.board.to_fen();
    let pre_start_fen = app.start_fen.clone();
    let pre_hist_len = app.history.len();
    let pre_view_ply = app.view_ply;
    app.handle_key(key(KeyCode::Char('f')));
    assert!(matches!(app.overlay, Overlay::Fen(_)));
    for ch in "bad_fen".chars() {
        app.handle_key(key(KeyCode::Char(ch)));
    }
    app.handle_key(key(KeyCode::Enter));
    match &app.overlay {
        Overlay::Fen(st) => assert!(st.error.is_some()),
        other => panic!("expected FEN modal with error, got {other:?}"),
    }
    assert_eq!(app.board.to_fen(), pre_fen);
    assert_eq!(app.start_fen, pre_start_fen);
    assert_eq!(app.history.len(), pre_hist_len);
    assert_eq!(app.view_ply, pre_view_ply);

    app.handle_key(key(KeyCode::F(2)));
    assert_eq!(app.overlay, Overlay::None);
    assert_eq!(app.board.to_fen(), FEN_PRESET_INFINITE_QUEENS);

    // Preset F3 (Kiwipete / tactical middlegame) & F1 (Standard).
    app.handle_key(key(KeyCode::Char('f')));
    app.handle_key(key(KeyCode::F(3)));
    assert_eq!(app.overlay, Overlay::None);
    assert_eq!(app.board.to_fen(), monolith_tui::app::FEN_PRESET_KIWIPETE);
    app.handle_key(key(KeyCode::Char('f')));
    app.handle_key(key(KeyCode::F(1)));
    assert_eq!(app.overlay, Overlay::None);
    assert_eq!(app.board.to_fen(), monolith_core::STARTPOS_FEN);
}

#[test]
fn cursor_move_with_promotion_and_engine_modal() {
    let mut app = App::new_headless();
    app.load_fen("k7/4P3/8/8/8/8/8/4K3 w - - 0 1")
        .expect("valid promotion FEN");

    // Navigate cursor from E7 to E8 and select promotion.
    app.cursor = Square::E7;
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.selected_sq, Some(Square::E7));
    assert!(app.legal_targets.contains(&Square::E8));

    app.handle_key(key(KeyCode::Up));
    assert_eq!(app.cursor, Square::E8);
    app.handle_key(key(KeyCode::Enter));

    assert!(matches!(
        app.overlay,
        Overlay::Promotion {
            from: Square::E7,
            to: Square::E8
        }
    ));
    app.handle_key(key(KeyCode::Char('q')));
    assert_eq!(app.overlay, Overlay::None);
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.history[0].san, "e8=Q+");

    // Configure Slot 2 in Engines modal.
    app.handle_key(key(KeyCode::Char('e')));
    assert!(matches!(app.overlay, Overlay::Engines { .. }));
    app.handle_key(key(KeyCode::Char('2')));
    app.handle_key(key(KeyCode::Char(' '))); // toggle Slot 2 enabled
    assert!(app.engines[1].config.enabled);
    app.handle_key(key(KeyCode::Esc));
    assert_eq!(app.overlay, Overlay::None);
}

#[test]
fn pgn_background_analysis_completes_and_navigates() {
    let mut app = App::new_headless();
    app.load_pgn_str(SAMPLE_OPERA_PGN, true)
        .expect("load Opera Game PGN");

    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        app.tick();
        if let Some(pg) = &app.pgn_progress {
            if !pg.running {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(15));
    }

    let pg = app.pgn_progress.as_ref().expect("pgn progress present");
    assert!(!pg.running, "analysis should complete within timeout");
    assert_eq!(pg.done_plies, pg.total_plies);
    assert!(
        app.history.iter().all(|m| m.classification.is_some()),
        "every ply in sample game must receive a classification"
    );

    // Step forward and backward through PGN with Right/Left arrows and render target-square badge.
    assert_eq!(app.view_ply, 0);
    app.handle_key(key(KeyCode::Right));
    app.handle_key(key(KeyCode::Right));
    assert_eq!(app.view_ply, 2);
    let backend = TestBackend::new(110, 38);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::render(f, &app)).unwrap();
    app.handle_key(key(KeyCode::Left));
    assert_eq!(app.view_ply, 1);
}

#[test]
fn pgn_analysis_cancellation_leaves_consistent_state() {
    let mut app = App::new_headless();
    app.load_pgn_str(SAMPLE_OPERA_PGN, true)
        .expect("load Opera Game PGN");
    assert!(app.pgn_progress.as_ref().is_some_and(|p| p.running));

    app.tick();
    app.cancel_pgn_analysis();

    let pg = app.pgn_progress.as_ref().expect("pgn progress retained");
    assert!(!pg.running, "cancelled analysis must stop running flag");
    assert!(!app.history.is_empty());

    // Navigation and board consistency must remain intact after cancel.
    app.jump_to_ply(4);
    assert_eq!(app.view_ply, 4);
    assert!(app.step_backward());
    assert_eq!(app.view_ply, 3);
    assert!(app.step_forward());
    assert_eq!(app.view_ply, 4);
    assert_eq!(app.board.fullmove_number(), 3);
}

#[test]
fn ui_renders_cleanly_at_small_and_large_sizes_and_overlays() {
    let mut app = App::new_headless();
    app.load_fen(FEN_PRESET_INFINITE_QUEENS).unwrap();
    app.engines[1].config.enabled = true;
    app.engines[2].config.enabled = true;

    for (w, h) in [(80u16, 24u16), (140u16, 42u16)] {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();

        app.overlay = Overlay::None;
        terminal.draw(|f| ui::render(f, &app)).unwrap();

        app.handle_key(key(KeyCode::Char('f')));
        terminal.draw(|f| ui::render(f, &app)).unwrap();
        app.handle_key(key(KeyCode::Esc));

        app.handle_key(key(KeyCode::Char('p')));
        terminal.draw(|f| ui::render(f, &app)).unwrap();
        app.handle_key(key(KeyCode::Esc));

        app.handle_key(key(KeyCode::Char('e')));
        terminal.draw(|f| ui::render(f, &app)).unwrap();
        app.handle_key(key(KeyCode::Esc));

        app.handle_key(key(KeyCode::Char('?')));
        terminal.draw(|f| ui::render(f, &app)).unwrap();
        app.handle_key(key(KeyCode::Esc));
    }
}

#[test]
fn board_layout_geometry_separates_from_rendering() {
    use ratatui::layout::Rect;
    use ui::compute_board_layout;

    assert!(compute_board_layout(Rect::new(0, 0, 12, 8), false).is_none());

    let normal = compute_board_layout(Rect::new(0, 0, 60, 42), false).expect("valid layout");
    let a1 = normal.square_rect(Square::A1);
    let h8 = normal.square_rect(Square::H8);
    assert!(a1.x < h8.x);
    assert!(h8.y < a1.y);

    let flipped = compute_board_layout(Rect::new(0, 0, 60, 42), true).expect("flipped layout");
    let a1_f = flipped.square_rect(Square::A1);
    let h8_f = flipped.square_rect(Square::H8);
    assert!(h8_f.x < a1_f.x);
    assert!(a1_f.y < h8_f.y);
}

#[test]
fn eval_bar_mapping_and_move_history_navigation() {
    use monolith_tui::theme::{compute_eval_bar_layout, eval_to_win_prob, format_eval_short};

    // cp = 0 -> 50%
    assert!((eval_to_win_prob(Some(0), None) - 0.5).abs() < 1e-9);
    // Mate -> 100% / 0%
    assert_eq!(eval_to_win_prob(None, Some(3)), 1.0);
    assert_eq!(eval_to_win_prob(None, Some(-2)), 0.0);
    assert_eq!(format_eval_short(Some(145), None), "+1.45");
    assert_eq!(format_eval_short(None, Some(3)), "M3");
    assert_eq!(format_eval_short(None, Some(-3)), "-M3");

    // Eval bar layout obeys flip cleanly.
    let eq_layout = compute_eval_bar_layout(20, Some(0), None, false);
    assert_eq!(eq_layout.white_rows, 10);
    assert_eq!(eq_layout.black_rows, 10);
    assert!(!eq_layout.is_white_row(0));
    assert!(eq_layout.is_white_row(19));

    let mate_white_normal = compute_eval_bar_layout(20, None, Some(3), false);
    assert_eq!(mate_white_normal.white_rows, 20);
    assert!(mate_white_normal.is_white_row(0));

    let adv_flipped = compute_eval_bar_layout(20, Some(300), None, true);
    assert!(adv_flipped.top_is_white);
    assert!(adv_flipped.is_white_row(0));
    assert!(!adv_flipped.is_white_row(19));

    // Move list navigation (step backward/forward, Home/End).
    let mut app = App::new_headless();
    for san in ["e4", "e5", "Nf3", "Nc6"] {
        app.submit_move_text(san).unwrap();
    }
    assert_eq!(app.view_ply, 4);

    app.handle_key(key(KeyCode::Char('[')));
    assert_eq!(app.view_ply, 3);
    app.handle_key(key(KeyCode::Home));
    assert_eq!(app.view_ply, 0);
    app.handle_key(key(KeyCode::Char(']')));
    assert_eq!(app.view_ply, 1);
    app.handle_key(key(KeyCode::End));
    assert_eq!(app.view_ply, 4);
}

#[test]
fn client_ui_excludes_internal_telemetry() {
    let mut app = App::new_headless();
    let tm = &mut app.engines[0].telemetry;
    tm.connected = true;
    tm.depth = 15;
    tm.seldepth = 23;
    tm.white_score_cp = Some(42);
    tm.nodes = 1_234_567;
    tm.nps = 4_560_000;
    tm.hashfull = 420;
    tm.best_move_san = Some("e4".to_string());
    tm.expected_reply_san = Some("e5".to_string());
    tm.pv_san = "e4 e5 Nf3".to_string();

    let backend = TestBackend::new(140, 42);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::render(f, &app)).unwrap();

    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(rendered.contains("Tiefe 15"));
    assert!(rendered.contains("Bester Zug:"));
    assert!(!rendered.contains("15/23"));
    assert!(!rendered.contains("Kn:"));
    assert!(!rendered.contains("MN/s"));
    assert!(!rendered.contains("kN/s"));
    assert!(!rendered.contains("1234567"));
    assert!(!rendered.contains("Hashfull"));

    // Invalid move sets status_is_error; valid move resets it; invalid engine binary propagates error.
    app.handle_key(key(KeyCode::Char('/')));
    for ch in "e2e5".chars() {
        app.handle_key(key(KeyCode::Char(ch)));
    }
    app.handle_key(key(KeyCode::Enter));
    assert!(app.status_is_error);

    for ch in "e2e4".chars() {
        app.handle_key(key(KeyCode::Char(ch)));
    }
    app.handle_key(key(KeyCode::Enter));
    assert!(!app.status_is_error);
    app.handle_key(key(KeyCode::Esc));

    app.engines[0].config.enabled = true;
    app.engines[0].config.binary_path = "/nonexistent/monolith-engine-does-not-exist".to_string();
    app.sync_engines_config();
    assert!(app.status_is_error);
    assert!(app.status_message.contains("Engine 1"));
    app.engines[0].config.enabled = false;
    app.sync_engines_config();
}

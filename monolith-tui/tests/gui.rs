//! Headless integration tests for GUI client components and CLI argument parser.

use egui::{Context, RawInput};
use monolith_core::{Color, PieceType, Square};
use monolith_tui::app::{App, FEN_PRESET_KIWIPETE};
use monolith_tui::entrypoint::{parse_args, RunMode, UiKind};
use monolith_tui::gui_board::{render_chess_board, GuiBoardState};
use monolith_tui::gui_panels::{
    render_analyze_panel, render_engine_slots, render_eval_bar, render_games_panel, render_modals,
    render_move_history, render_play_panel, GuiPanelState, GuiTab,
};

#[test]
fn test_cli_parser_gui_and_tui_flags() {
    let args_gui = vec!["monolith".into(), "-gui".into()];
    let cfg_gui = parse_args(args_gui).expect("parsed -gui");
    assert_eq!(cfg_gui.ui, UiKind::Gui);

    let args_tui = vec!["monolith".into(), "--tui".into()];
    let cfg_tui = parse_args(args_tui).expect("parsed --tui");
    assert_eq!(cfg_tui.ui, UiKind::Tui);

    let args_default = vec!["monolith".into()];
    let cfg_default = parse_args(args_default).expect("parsed default");
    assert_eq!(cfg_default.ui, UiKind::Gui);
    assert_eq!(cfg_default.mode, RunMode::Client);

    let args_logs = vec!["monolith".into(), "--mode".into(), "logs".into()];
    let cfg_logs = parse_args(args_logs).expect("parsed logs mode");
    assert_eq!(cfg_logs.mode, RunMode::Logs);
}

#[test]
fn test_gui_board_rendering_and_interaction() {
    let ctx = Context::default();
    let mut app = App::new_headless();
    let mut board_state = GuiBoardState::default();

    let _ = ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            render_chess_board(ui, &mut app, &mut board_state);
        });
    });

    // Selecting e2 should populate legal targets including e3 and e4.
    app.cursor = Square::E2;
    let matching = app.matching_moves(Square::E2, Square::E4);
    assert!(!matching.is_empty(), "e2e4 must be legal");

    // Layout math separation: square_center -> square_at roundtrip (normal & flipped).
    let normal_layout = monolith_tui::gui_board::GuiBoardLayout::from_available(
        egui::pos2(0.0, 0.0),
        600.0,
        600.0,
        false,
    );
    assert_eq!(
        normal_layout.square_at(normal_layout.square_center(Square::E2)),
        Some(Square::E2)
    );
    let flipped_layout = monolith_tui::gui_board::GuiBoardLayout::from_available(
        egui::pos2(0.0, 0.0),
        600.0,
        600.0,
        true,
    );
    assert_eq!(
        flipped_layout.square_at(flipped_layout.square_center(Square::H8)),
        Some(Square::H8)
    );
    assert!(
        flipped_layout.square_center(Square::E2).y < normal_layout.square_center(Square::E2).y,
        "flipped board swaps vertical rank position"
    );

    // Play move e2e4 and verify history.
    app.play_move(matching[0]).expect("e2e4 executed");
    assert_eq!(app.history.len(), 1);
    assert_eq!(app.history[0].san, "e4");

    // Enable hide_assistance and render again.
    board_state.hide_assistance = true;

    let _ = ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            render_chess_board(ui, &mut app, &mut board_state);
        });
    });
}

use monolith_tui::gui_pieces::PieceSet;
use monolith_tui::gui_settings::{render_settings_panel, GuiSettingsState};
use monolith_tui::gui_sounds::{synthesize_wav, SoundKind, SoundPlayer};

#[test]
fn test_gui_settings_and_piece_sets() {
    let ctx = Context::default();
    egui_extras::install_image_loaders(&ctx);
    let mut app = App::new_headless();
    let mut board_state = GuiBoardState::default();
    let mut panel_state = GuiPanelState::new();
    let mut settings_state = GuiSettingsState::default();
    let mut sound_player = SoundPlayer::default();

    for set in PieceSet::ALL {
        board_state.piece_set = set;
        let _ = ctx.run(RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_chess_board(ui, &mut app, &mut board_state);
            });
            egui::SidePanel::right("settings_test").show(ctx, |ui| {
                render_settings_panel(
                    ui,
                    &mut app,
                    &mut board_state,
                    &mut panel_state,
                    &mut settings_state,
                    &mut sound_player,
                );
            });
        });
    }

    assert_eq!(board_state.piece_set, PieceSet::VectorProcedural);
}

#[test]
fn test_gui_panels_and_modals_render_cleanly() {
    let ctx = Context::default();
    let mut app = App::new_headless();
    let mut panel_state = GuiPanelState::new();

    let _ = ctx.run(RawInput::default(), |ctx| {
        egui::SidePanel::left("left_test").show(ctx, |ui| {
            render_eval_bar(ui, &app, false);
        });
        egui::SidePanel::right("right_test").show(ctx, |ui| {
            render_play_panel(ui, &mut app, &mut panel_state);
            render_analyze_panel(ui, &mut app, &mut panel_state);
            render_games_panel(ui, &mut app, &mut panel_state);
            render_engine_slots(ui, &mut app, &mut panel_state);
            render_move_history(ui, &mut app, false);
        });

        // Open FEN modal and test preset loading.
        panel_state.show_fen_modal = true;
        render_modals(ctx, &mut app, &mut panel_state);
    });

    // Load Kiwipete position.
    app.load_fen(FEN_PRESET_KIWIPETE).expect("kiwipete loaded");
    assert_eq!(
        app.board.piece_on(Square::E1),
        Some((Color::White, PieceType::King))
    );

    // Render with Kiwipete position and populated engine telemetry (only player-facing stats).
    app.engines[0].telemetry.depth = 15;
    app.engines[0].telemetry.seldepth = 23;
    app.engines[0].telemetry.nodes = 1_234_567;
    app.engines[0].telemetry.nps = 4_560_000;
    app.engines[0].telemetry.hashfull = 420;
    app.engines[0].telemetry.white_score_cp = Some(42);
    app.engines[0].telemetry.best_move_san = Some("e4".to_string());

    panel_state.active_tab = GuiTab::Games;
    let _ = ctx.run(RawInput::default(), |ctx| {
        egui::SidePanel::right("right_test2").show(ctx, |ui| {
            render_games_panel(ui, &mut app, &mut panel_state);
            render_engine_slots(ui, &mut app, &mut panel_state);
        });
    });
}

#[test]
fn test_gui_new_features_widgets_sounds_and_annotations() {
    use monolith_core::pgn::{MoveClass, MoveClassification};
    use monolith_tui::gui_analysis_widgets::{
        detect_opening_label, render_eval_graph, render_opening_explorer, render_trainer_panel,
        TrainerState,
    };
    use monolith_tui::gui_board::{engine_color, UserArrow, UserHighlight, ENGINE_COLORS};

    // 1. Right-click annotations & engine colors
    let mut board_state = GuiBoardState::default();
    board_state.user_arrows.push(UserArrow {
        from: Square::E2,
        to: Square::E4,
        color: ENGINE_COLORS[0],
    });
    board_state.user_highlights.push(UserHighlight {
        sq: Square::D4,
        color: ENGINE_COLORS[1],
    });
    assert_eq!(board_state.user_arrows.len(), 1);
    assert_eq!(board_state.user_highlights.len(), 1);
    board_state.clear_annotations();
    assert!(board_state.user_arrows.is_empty());
    assert!(board_state.user_highlights.is_empty());
    assert_eq!(engine_color(0), ENGINE_COLORS[0]);
    assert_eq!(engine_color(ENGINE_COLORS.len()), ENGINE_COLORS[0]);

    // 2. Minimal Chess.com-style synthesized WAV sounds
    for kind in [
        SoundKind::Move,
        SoundKind::Capture,
        SoundKind::Check,
        SoundKind::Castle,
        SoundKind::Promote,
        SoundKind::GameEnd,
    ] {
        let wav = synthesize_wav(kind);
        assert!(wav.len() > 44, "WAV must contain header + PCM samples");
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
    }

    // 3. Opening Explorer detection & Trainer puzzle flow
    let mut app = App::new_headless();
    app.load_pgn_str("1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 *", false)
        .expect("Ruy Lopez PGN loaded");
    app.jump_to_ply(app.history.len());
    let (eco, name) = detect_opening_label(&app);
    assert_eq!(eco, "C60");
    assert!(name.contains("Spanisch") || name.contains("Ruy Lopez"));

    // Mark ply 1 (1... e5) as a simulated mistake with best hint c5 and eval
    app.history[1].classification = Some(MoveClassification {
        class: MoveClass::Blunder,
        cp_loss: 220,
        explanation: "Simulierter Patzer für Test".to_string(),
    });
    app.history[1].best_hint_san = Some("c5".to_string());
    app.history[1].eval_white_cp = Some(150);

    let puzzles = TrainerState::puzzle_plies(&app);
    assert_eq!(puzzles, vec![1]);

    let mut trainer = TrainerState::default();
    trainer.next_puzzle(&mut app, None);
    assert!(trainer.active);
    assert_eq!(trainer.puzzle_ply, Some(1));
    assert_eq!(app.view_ply, 1);

    // Also advance to ply 2 (1... e5 with Blunder classification) so target badge renders.
    app.jump_to_ply(2);
    let mut board_state = GuiBoardState::default();
    let mut panel_state = GuiPanelState::new();
    let ctx = Context::default();
    let _ = ctx.run(RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            render_chess_board(ui, &mut app, &mut board_state);
        });
        egui::SidePanel::right("analysis_widgets_test").show(ctx, |ui| {
            render_opening_explorer(ui, &app);
            render_eval_graph(ui, &mut app);
            render_trainer_panel(ui, &mut app, &mut trainer);
            render_games_panel(ui, &mut app, &mut panel_state);
        });
    });
}

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

#[test]
fn test_gui_settings_and_piece_sets() {
    let ctx = Context::default();
    egui_extras::install_image_loaders(&ctx);
    let mut app = App::new_headless();
    let mut board_state = GuiBoardState::default();
    let mut panel_state = GuiPanelState::new();
    let mut settings_state = GuiSettingsState::default();

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

    // Render with Kiwipete position loaded in games view.
    panel_state.active_tab = GuiTab::Games;
    let _ = ctx.run(RawInput::default(), |ctx| {
        egui::SidePanel::right("right_test2").show(ctx, |ui| {
            render_games_panel(ui, &mut app, &mut panel_state);
        });
    });
}

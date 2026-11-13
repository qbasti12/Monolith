//! Integration test exercising multi-engine UCI communication via a scripted fake UCI engine.

use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use monolith_tui::app::App;

#[cfg(unix)]
fn create_fake_uci_script(name: &str, log_path: &std::path::Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let script_path = std::env::temp_dir().join(format!(
        "monolith_fake_uci_{name}_{}.sh",
        std::process::id()
    ));
    let log_str = log_path.to_string_lossy();
    let script = format!(
        r#"#!/bin/sh
ELO="2400"
while IFS= read -r line; do
  printf '%s\n' "$line" >> "{log_str}"
  case "$line" in
    uci)
      echo "id name FakeUciBot"
      echo "option name UCI_LimitStrength type check default false"
      echo "option name UCI_Elo type spin default 2400 min 800 max 3200"
      echo "option name Skill_Depth type spin default 64 min 1 max 64"
      echo "uciok"
      ;;
    isready)
      echo "readyok"
      ;;
    setoption\ name\ UCI_Elo\ value\ *)
      ELO="${{line##*value }}"
      ;;
    go\ *)
      if [ "$ELO" = "1500" ]; then
        echo "info depth 5 seldepth 7 multipv 1 score cp 215 nodes 4000 nps 800000 hashfull 12 time 5 pv d2d4 d7d5 c2c4"
        echo "bestmove d2d4 ponder d7d5"
      else
        echo "info depth 8 seldepth 11 multipv 1 score cp 145 nodes 12000 nps 900000 hashfull 20 time 14 pv e2e4 e7e5 g1f3"
        echo "bestmove e2e4 ponder e7e5"
      fi
      ;;
    quit)
      exit 0
      ;;
  esac
done
"#
    );

    fs::write(&script_path, script).expect("write fake uci script");
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))
        .expect("chmod fake uci script");
    script_path
}

#[cfg(unix)]
#[test]
fn scripted_fake_uci_engine_updates_panel_and_applies_live_config_changes() {
    let log_path =
        std::env::temp_dir().join(format!("monolith_fake_uci_log_{}.txt", std::process::id()));
    let _ = fs::remove_file(&log_path);
    let script_path = create_fake_uci_script("slot0", &log_path);

    let mut app = App::new_headless();
    app.auto_spawn_engines = true;
    app.engines[0].config.binary_path = script_path.to_string_lossy().into_owned();
    app.engines[0].config.enabled = true;
    app.sync_engines_config();
    app.trigger_engine_searches();

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        app.tick();
        if app.engines[0].telemetry.best_move_san.as_deref() == Some("e4")
            && !app.engines[0].telemetry.searching
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(15));
    }

    let tel0 = &app.engines[0].telemetry;
    assert!(tel0.connected, "fake UCI slot must be connected");
    assert_eq!(tel0.reported_name, "FakeUciBot");
    assert_eq!(tel0.depth, 8);
    assert_eq!(tel0.white_score_cp, Some(145));
    assert_eq!(tel0.best_move_san.as_deref(), Some("e4"));
    assert_eq!(tel0.expected_reply_san.as_deref(), Some("e5"));
    assert_eq!(tel0.pv_san, "e4 e5 Nf3");

    // Apply live config change: Elo = 1500, LimitStrength = true, Depth = 5, plus enable Slot 2.
    app.engines[0].config.limit_strength = true;
    app.engines[0].config.elo = 1500;
    app.engines[0].config.depth = 5;
    app.engines[1].config.binary_path = script_path.to_string_lossy().into_owned();
    app.engines[1].config.enabled = true;
    app.sync_engines_config();
    app.trigger_engine_searches();

    let deadline2 = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline2 {
        app.tick();
        if app.engines[0].telemetry.best_move_san.as_deref() == Some("d4")
            && app.engines[1].telemetry.connected
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(15));
    }

    let tel0_after = &app.engines[0].telemetry;
    assert_eq!(tel0_after.depth, 5);
    assert_eq!(tel0_after.white_score_cp, Some(215));
    assert_eq!(tel0_after.best_move_san.as_deref(), Some("d4"));
    assert_eq!(tel0_after.expected_reply_san.as_deref(), Some("d5"));
    assert!(app.engines[1].telemetry.connected, "Slot 2 enabled live");

    app.shutdown();

    let cmd_log = fs::read_to_string(&log_path).expect("fake uci command log exists");
    assert!(cmd_log.contains("setoption name UCI_LimitStrength value true"));
    assert!(cmd_log.contains("setoption name UCI_Elo value 1500"));
    assert!(cmd_log.contains("setoption name Skill_Depth value 5"));
    assert!(cmd_log.contains("go depth 5"));

    let _ = fs::remove_file(&script_path);
    let _ = fs::remove_file(&log_path);
}

use std::net::UdpSocket;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use monolith_tui::app::App;
use monolith_tui::ipc::{
    decode_packet, encode_packet, now_timestamp_ms, IpcMessage, IpcReceiver, IpcSender,
    MAX_PACKET_BYTES,
};
use monolith_tui::logs::{render_logs, LogsApp, LOG_RING_CAPACITY};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn udp_loopback_roundtrip_all_message_variants() {
    let mut rx = IpcReceiver::bind(0).expect("bind ephemeral loopback receiver");
    let port = rx.local_addr().unwrap().port();
    let tx = IpcSender::new(port).expect("create UDP sender");

    let messages = vec![
        IpcMessage::Hello {
            client_version: "0.1.0".to_string(),
            timestamp_ms: now_timestamp_ms(),
        },
        IpcMessage::Position {
            fen: monolith_core::STARTPOS_FEN.to_string(),
            last_move: Some("e4".to_string()),
            ply: 1,
            timestamp_ms: now_timestamp_ms(),
        },
        IpcMessage::EngineInfo {
            slot: 0,
            name: "Monolith 0.1.0".to_string(),
            elo: 2400,
            limit_strength: false,
            depth: 12,
            seldepth: 17,
            score_cp: Some(34),
            score_mate: None,
            nodes: 125_000,
            nps: 2_500_000,
            hashfull: 18,
            pv: vec!["e2e4".into(), "e7e5".into(), "g1f3".into()],
            timestamp_ms: now_timestamp_ms(),
        },
        IpcMessage::UciLine {
            slot: 1,
            engine: "Monolith".to_string(),
            line: "bestmove e2e4 ponder e7e5".to_string(),
            timestamp_ms: now_timestamp_ms(),
        },
        IpcMessage::EvalBreakdown {
            fen: monolith_core::STARTPOS_FEN.to_string(),
            white_pov_cp: 0,
            stm_pov_cp: 0,
            material_cp: 0,
            pst_cp: 0,
            pawn_structure_cp: 0,
            king_safety_cp: 0,
            mobility_cp: 0,
            bishop_pair_cp: 0,
            phase: 24,
            timestamp_ms: now_timestamp_ms(),
        },
        IpcMessage::Bye {
            timestamp_ms: now_timestamp_ms(),
        },
    ];

    for msg in &messages {
        tx.send(msg).expect("send UDP message");
    }

    std::thread::sleep(Duration::from_millis(20));
    let mut received = Vec::new();
    for item in rx.poll(32) {
        received.push(item.expect("valid IPC decode"));
    }
    assert_eq!(received, messages);
}

#[test]
fn oversized_pv_and_uci_line_are_truncated_under_1400_bytes() {
    let huge_pv: Vec<String> = (0..300).map(|_| "g1f3".to_string()).collect();
    let msg1 = IpcMessage::EngineInfo {
        slot: 0,
        name: "Monolith".into(),
        elo: 2600,
        limit_strength: false,
        depth: 20,
        seldepth: 28,
        score_cp: Some(12),
        score_mate: None,
        nodes: 9_999_999,
        nps: 3_000_000,
        hashfull: 450,
        pv: huge_pv,
        timestamp_ms: 123456,
    };
    let pkt1 = encode_packet(&msg1).expect("truncate oversized PV cleanly");
    assert!(
        pkt1.len() < MAX_PACKET_BYTES,
        "packet {} bytes must stay < {MAX_PACKET_BYTES}",
        pkt1.len()
    );
    let dec1 = decode_packet(&pkt1).expect("truncated packet must remain valid JSON");
    match dec1 {
        IpcMessage::EngineInfo { pv, .. } => assert!(pv.len() < 300),
        other => panic!("unexpected decoded variant: {other:?}"),
    }

    let huge_line = "info string ".to_string() + &"a".repeat(3500);
    let msg2 = IpcMessage::UciLine {
        slot: 0,
        engine: "Monolith".into(),
        line: huge_line,
        timestamp_ms: 123456,
    };
    let pkt2 = encode_packet(&msg2).expect("truncate oversized UCI line cleanly");
    assert!(pkt2.len() < MAX_PACKET_BYTES);
    assert!(decode_packet(&pkt2).is_ok());
}

#[test]
fn malformed_udp_packets_are_resiliently_ignored() {
    let mut logs = LogsApp::bind(0).expect("bind LogsApp");
    let port = logs.local_port();

    let raw_sender = UdpSocket::bind(("127.0.0.1", 0)).unwrap();
    let target = ("127.0.0.1", port);

    // Send invalid UTF-8, truncated JSON, empty payload, and unknown tag.
    let _ = raw_sender.send_to(&[0xff, 0xfe, 0xfd, 0x80], target);
    let _ = raw_sender.send_to(br#"{"type":"engine_info","slot":0"#, target);
    let _ = raw_sender.send_to(br#"{"type":"nonexistent_variant","foo":1}"#, target);
    let _ = raw_sender.send_to(b"not json at all\n", target);

    // Follow with a valid client emission.
    let mut client = App::new_headless();
    client.enable_ipc(port).expect("enable IPC on client");
    client.submit_move_text("e2e4").unwrap();
    client.shutdown();

    std::thread::sleep(Duration::from_millis(30));
    logs.tick();

    assert!(
        logs.malformed_count >= 4,
        "expected malformed packets counted cleanly, got {}",
        logs.malformed_count
    );
    assert!(
        logs.packet_count >= 4,
        "valid packets after malformed ones must still arrive cleanly, got {}",
        logs.packet_count
    );
    assert_eq!(logs.eval_summary.last_move.as_deref(), Some("e4"));
}

#[test]
fn logs_ring_buffer_cap_scroll_and_render() {
    let mut logs = LogsApp::new_headless(42424);

    for i in 0..(LOG_RING_CAPACITY + 250) {
        logs.apply_message(IpcMessage::UciLine {
            slot: i % 3,
            engine: "Monolith".into(),
            line: format!("info depth {} nodes {}", (i % 18) + 1, i * 100),
            timestamp_ms: i as u64,
        });
    }
    assert_eq!(logs.log_buffer.len(), LOG_RING_CAPACITY);

    logs.apply_message(IpcMessage::EngineInfo {
        slot: 0,
        name: "Monolith".into(),
        elo: 2400,
        limit_strength: false,
        depth: 12,
        seldepth: 16,
        score_cp: Some(45),
        score_mate: None,
        nodes: 420_000,
        nps: 2_100_000,
        hashfull: 120,
        pv: vec!["e2e4".into(), "e7e5".into()],
        timestamp_ms: 9999,
    });
    assert_eq!(logs.log_buffer.len(), LOG_RING_CAPACITY);

    // Test scroll, Auto-Follow toggle, and slot filters.
    assert!(logs.auto_follow);
    logs.handle_key(key(KeyCode::PageUp));
    assert!(!logs.auto_follow);
    logs.handle_key(key(KeyCode::End));
    assert!(logs.auto_follow);
    logs.handle_key(key(KeyCode::Char('1')));
    assert_eq!(logs.filter_slot, Some(0));
    logs.handle_key(key(KeyCode::Char('0')));
    assert_eq!(logs.filter_slot, None);

    let backend = TestBackend::new(120, 36);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| render_logs(f, &logs)).unwrap();
}

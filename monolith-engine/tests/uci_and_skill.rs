use std::io::{self, Cursor, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use monolith_core::movegen::{generate_legal, parse_uci_move};
use monolith_core::{Board, Color, MoveList};
use monolith_engine::eval::evaluate;
use monolith_engine::search::{Score, SearchLimits, Searcher};
use monolith_engine::skill::{select_move, SkillProfile, SkillRng, MAX_ELO, MIN_ELO};
use monolith_engine::uci::{run_uci_loop, UciError, UciEvent, UciProcess};

#[derive(Clone, Default)]
struct SharedWriter(Arc<Mutex<Vec<u8>>>);

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut guard = self.0.lock().map_err(|_| io::Error::other("poisoned"))?;
        guard.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn uci_loop_handles_standard_and_malformed_commands() {
    let input = "\
uci
isready
setoption name Hash value 16
setoption name MultiPV value 2
setoption name UCI_LimitStrength value true
setoption name UCI_Elo value 1800
setoption name Hash value invalid_number
position fen not_a_valid_fen
garbage unknown command
position startpos moves e2e4 e7e5
isready
go depth 3
isready
quit
";
    let writer = SharedWriter::default();
    let res = run_uci_loop(Cursor::new(input.as_bytes()), writer.clone());
    assert!(res.is_ok(), "UCI loop should exit cleanly on quit: {res:?}");

    let raw = writer.0.lock().unwrap();
    let out = String::from_utf8_lossy(&raw);
    assert!(out.contains("id name Monolith"), "missing id name: {out}");
    assert!(out.contains("uciok"), "missing uciok: {out}");
    assert!(out.contains("readyok"), "missing readyok: {out}");
    assert!(out.contains("info depth "), "missing search info: {out}");
    assert!(out.contains("bestmove "), "missing bestmove: {out}");
}

#[test]
fn uci_process_communicates_with_compiled_binary() {
    let bin = env!("CARGO_BIN_EXE_monolith-engine");

    // Nonexistent binary must return an Err cleanly.
    let err = UciProcess::spawn("/nonexistent/path/to/monolith-engine-missing", &[]);
    assert!(matches!(err, Err(UciError::Io(_))));

    let mut proc = UciProcess::spawn(bin, &[]).expect("spawn compiled monolith-engine");
    let id = proc
        .init_uci(Duration::from_secs(5))
        .expect("uci handshake succeeds");
    assert!(
        id.contains("Monolith"),
        "expected engine id name containing Monolith, got '{id}'"
    );

    proc.set_option("Hash", "16").expect("set Hash");
    proc.is_ready(Duration::from_secs(5))
        .expect("isready succeeds");

    let moves = vec!["e2e4".to_string(), "e7e5".to_string()];
    proc.set_position(None, &moves).expect("position startpos");
    proc.go("depth 4").expect("go depth 4");

    let mut saw_info = false;
    let mut best_move_str = None;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if let Some(ev) = proc.recv_timeout(Duration::from_millis(200)) {
            match ev {
                UciEvent::Info(info) => {
                    if info.depth.is_some() && info.nodes.is_some() {
                        saw_info = true;
                    }
                }
                UciEvent::BestMove { best_move, .. } => {
                    best_move_str = Some(best_move);
                    break;
                }
                _ => {}
            }
        }
    }

    assert!(saw_info, "expected UCI info telemetry before bestmove");
    let bm = best_move_str.expect("expected bestmove within timeout");

    let mut board = Board::startpos();
    for m in &moves {
        let mv = parse_uci_move(&board, m).unwrap();
        board.make_move(mv);
    }
    let parsed_bm = parse_uci_move(&board, &bm);
    assert!(
        parsed_bm.is_ok(),
        "bestmove '{bm}' must be legal after 1.e4 e5"
    );

    // Verify `go infinite` responds to `stop`.
    proc.go("infinite").expect("go infinite");
    std::thread::sleep(Duration::from_millis(80));
    proc.stop().expect("stop");

    let mut stopped_best = None;
    let stop_deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < stop_deadline {
        if let Some(UciEvent::BestMove { best_move, .. }) =
            proc.recv_timeout(Duration::from_millis(100))
        {
            stopped_best = Some(best_move);
            break;
        }
    }
    assert!(
        stopped_best.is_some(),
        "engine should emit bestmove promptly after stop"
    );

    // Verify zero-delay `go infinite` + `isready` + `stop` race condition is resolved.
    proc.go("infinite").expect("go infinite immediate");
    proc.send_command("isready").expect("isready during go");
    proc.stop().expect("immediate stop");

    let mut saw_ready = false;
    let mut immediate_stop_best = None;
    let race_deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < race_deadline && (!saw_ready || immediate_stop_best.is_none()) {
        if let Some(ev) = proc.recv_timeout(Duration::from_millis(100)) {
            match ev {
                UciEvent::ReadyOk => saw_ready = true,
                UciEvent::BestMove { best_move, .. } => immediate_stop_best = Some(best_move),
                _ => {}
            }
        }
    }
    assert!(saw_ready, "expected readyok during active/queued search");
    assert!(
        immediate_stop_best.is_some(),
        "expected bestmove after immediate stop without sleep"
    );

    // Verify negative time limits, small node limits, and searchmoves on double pawn push.
    proc.set_position(None, &[]).expect("reset startpos");
    proc.go("wtime -500 btime -500 nodes 50 searchmoves d2d4")
        .expect("go with negative time and searchmoves");
    let mut restricted_best = None;
    let mut reported_nodes = 0u64;
    let limit_deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < limit_deadline {
        if let Some(ev) = proc.recv_timeout(Duration::from_millis(100)) {
            match ev {
                UciEvent::Info(info) => {
                    if let Some(n) = info.nodes {
                        reported_nodes = n;
                    }
                }
                UciEvent::BestMove { best_move, .. } => {
                    restricted_best = Some(best_move);
                    break;
                }
                _ => {}
            }
        }
    }
    assert_eq!(restricted_best.as_deref(), Some("d2d4"));
    assert!(
        reported_nodes <= 100,
        "expected nodes <= 100 for `go nodes 50`, got {reported_nodes}"
    );

    proc.shutdown();
}

#[test]
fn skill_profile_table_invariants() {
    let p_min = SkillProfile::for_elo(MIN_ELO);
    let p_mid = SkillProfile::for_elo(1600);
    let p_high = SkillProfile::for_elo(2400);
    let p_max = SkillProfile::for_elo(MAX_ELO);

    assert_eq!(p_min.elo, MIN_ELO);
    assert!(p_min.depth_limit < p_mid.depth_limit);
    assert!(p_mid.depth_limit < p_high.depth_limit);
    assert!(p_min.noise_cp > p_mid.noise_cp);
    assert!(p_mid.noise_cp > p_high.noise_cp);
    assert_eq!(p_high.noise_cp, 0);
    assert!(p_max.node_limit.is_none());
    assert_eq!(p_max.multi_pv_k, 1);
    assert!(p_max.depth_limit >= 64);

    // Clamping outside [800, 3200].
    assert_eq!(SkillProfile::for_elo(100), p_min);
    assert_eq!(SkillProfile::for_elo(4000), p_max);
}

#[derive(Default, Debug)]
struct MatchScore {
    wins_high: u32,
    draws: u32,
    wins_low: u32,
}

impl MatchScore {
    fn high_score_ratio(&self) -> f64 {
        let total = (self.wins_high + self.draws + self.wins_low) as f64;
        if total == 0.0 {
            return 0.0;
        }
        (self.wins_high as f64 + 0.5 * self.draws as f64) / total
    }
}

fn play_single_game(
    high_is_white: bool,
    white_searcher: &mut Searcher,
    black_searcher: &mut Searcher,
    rng: &mut SkillRng,
) -> i32 {
    let mut board = Board::startpos();
    let prof_low = SkillProfile::for_elo(1200);
    let prof_high = SkillProfile::for_elo(2400);

    // Up to 40 full moves (80 plies) per game as specified in M6 gate.
    for _ply in 0..80 {
        let mut legal = MoveList::new();
        generate_legal(&board, &mut legal);
        if legal.is_empty() {
            if board.in_check() {
                // Side to move is checkmated.
                return if (board.side_to_move() == Color::White) == high_is_white {
                    -1
                } else {
                    1
                };
            }
            return 0;
        }
        if board.is_fifty_move_draw()
            || board.is_threefold_repetition()
            || board.is_insufficient_material()
        {
            return 0;
        }

        let stm_is_high = (board.side_to_move() == Color::White) == high_is_white;
        let profile = if stm_is_high { prof_high } else { prof_low };

        let mut limits = if stm_is_high {
            SearchLimits {
                max_depth: 10,
                movetime: Some(Duration::from_millis(35)),
                ..SearchLimits::default()
            }
        } else {
            SearchLimits {
                max_depth: 10,
                movetime: Some(Duration::from_millis(25)),
                ..SearchLimits::default()
            }
        };
        profile.apply_to_limits(&mut limits);

        let result = if board.side_to_move() == Color::White {
            white_searcher.search(&mut board, &limits, |_| {})
        } else {
            black_searcher.search(&mut board, &limits, |_| {})
        };

        let (mv, _) = select_move(profile, &result, rng);
        board.make_move(mv);

        // Quick mate-score or overwhelming advantage adjudication is deferred to end of 40 moves,
        // unless one side literally has a forced mate announced <= 2 plies? Keep natural play up to 40 moves.
        let _ = Score::from_raw(result.score);
    }

    // Move-limit adjudication at move 40 based on static eval (>= 200 cp clear winning advantage).
    let stm_eval = evaluate(&board);
    let high_eval = if (board.side_to_move() == Color::White) == high_is_white {
        stm_eval
    } else {
        -stm_eval
    };

    if high_eval >= 200 {
        1
    } else if high_eval <= -200 {
        -1
    } else {
        0
    }
}

#[test]
fn elo_2400_beats_elo_1200_in_20_game_selfplay_match() {
    let mut white_searcher = Searcher::new(8);
    let mut black_searcher = Searcher::new(8);
    let mut rng = SkillRng::new(0x4D4F_4E4F_4C49_5448);

    let mut score = MatchScore::default();
    let t0 = Instant::now();

    for game_idx in 0..20 {
        white_searcher.clear_state();
        black_searcher.clear_state();
        let high_is_white = game_idx % 2 == 0;
        match play_single_game(
            high_is_white,
            &mut white_searcher,
            &mut black_searcher,
            &mut rng,
        ) {
            1 => score.wins_high += 1,
            -1 => score.wins_low += 1,
            _ => score.draws += 1,
        }
    }

    let elapsed = t0.elapsed();
    let ratio = score.high_score_ratio();
    eprintln!(
        "Elo 2400 vs Elo 1200 (20 games, <=40 moves/game) in {:.2}s: W={}, D={}, L={}, Score={:.1}%",
        elapsed.as_secs_f64(),
        score.wins_high,
        score.draws,
        score.wins_low,
        ratio * 100.0
    );

    assert!(
        ratio > 0.65,
        "expected Elo 2400 to score > 65% against Elo 1200, got {:.1}% ({score:?})",
        ratio * 100.0
    );
}

#[test]
fn nnue_and_smp_roundtrip_and_parallel_search() {
    use monolith_engine::nnue::{BulletRecord, NnueNet, NNUE_HL, NNUE_INPUTS};
    use monolith_engine::smp::search_parallel;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;

    let mut board = Board::startpos();
    let rec = BulletRecord::from_board(&board, 25, 2).expect("startpos encodes to BulletRecord");
    let rec2 = BulletRecord::from_bytes(&rec.to_bytes());
    assert_eq!(rec, rec2);
    assert_eq!(rec2.iter_pieces().count(), 32);

    let net = NnueNet {
        ft_weights: Box::new([1i16; NNUE_INPUTS * NNUE_HL]),
        ft_biases: Box::new([10i16; NNUE_HL]),
        out_weights: Box::new([2i16; 2 * NNUE_HL]),
        out_bias: 0,
    };
    let bytes = net.to_bytes();
    let loaded = NnueNet::from_bytes(&bytes).expect("MNUEv001 roundtrip succeeds");
    assert_eq!(net.evaluate_board(&board), loaded.evaluate_board(&board));

    let mut searcher = Searcher::new(8);
    searcher.set_nnue(Some(Arc::new(loaded)));
    let stop = Arc::new(AtomicBool::new(false));
    let limits = SearchLimits {
        max_depth: 4,
        ..SearchLimits::default()
    };
    let res = search_parallel(&mut searcher, &mut board, &limits, 2, &stop, |_| {});
    let mut legal = MoveList::new();
    generate_legal(&board, &mut legal);
    assert!(legal.contains(res.best_move));
}

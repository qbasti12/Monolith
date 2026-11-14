//! Multi-threaded self-play data generator writing 32-byte `bulletformat` training records.

use std::env;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use monolith_core::movegen::generate_legal;
use monolith_core::types::{Color, Move, MoveList};
use monolith_core::Board;
use monolith_engine::nnue::{BulletRecord, NnueNet};
use monolith_engine::search::{Score, SearchLimits, Searcher};
use monolith_engine::tt::MATE_BOUND;
use monolith_engine::SkillRng;

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Bullet,
    Text,
}

struct Config {
    positions: u64,
    nodes_per_move: u64,
    threads: usize,
    out_path: PathBuf,
    eval_file: Option<PathBuf>,
    seed: u64,
    format: OutputFormat,
}

fn parse_args() -> Result<Config, String> {
    let mut cfg = Config {
        positions: 500_000,
        nodes_per_move: 5_000,
        threads: thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4),
        out_path: PathBuf::from("data/monolith_train.bin"),
        eval_file: None,
        seed: 0x4D4F_4E4F_4C49_5448,
        format: OutputFormat::Bullet,
    };

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--positions" | "-n" => {
                let v = args.next().ok_or("missing value for --positions")?;
                cfg.positions = v.parse().map_err(|_| "invalid --positions")?;
            }
            "--nodes" => {
                let v = args.next().ok_or("missing value for --nodes")?;
                cfg.nodes_per_move = v.parse().map_err(|_| "invalid --nodes")?;
            }
            "--threads" | "-t" => {
                let v = args.next().ok_or("missing value for --threads")?;
                cfg.threads = v.parse::<usize>().map_err(|_| "invalid --threads")?.max(1);
            }
            "--out" | "-o" => {
                let v = args.next().ok_or("missing value for --out")?;
                cfg.out_path = PathBuf::from(v);
            }
            "--eval-file" => {
                let v = args.next().ok_or("missing value for --eval-file")?;
                cfg.eval_file = Some(PathBuf::from(v));
            }
            "--seed" => {
                let v = args.next().ok_or("missing value for --seed")?;
                cfg.seed = v.parse().map_err(|_| "invalid --seed")?;
            }
            "--format" => {
                let v = args.next().ok_or("missing value for --format")?;
                cfg.format = match v.as_str() {
                    "bullet" => OutputFormat::Bullet,
                    "text" => OutputFormat::Text,
                    _ => return Err("invalid --format (expected 'bullet' or 'text')".into()),
                };
            }
            "--help" | "-h" => {
                println!(
                    "Usage: monolith-datagen [--positions N] [--nodes N] [--threads T] [--out FILE] [--format bullet|text] [--eval-file MNUE] [--seed S]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(cfg)
}

fn white_relative_score(score_raw: i32, stm: Color) -> i32 {
    if stm == Color::White {
        score_raw
    } else {
        -score_raw
    }
}

fn play_single_game(
    searcher: &mut Searcher,
    rng: &mut SkillRng,
    nodes_per_move: u64,
    buf: &mut Vec<(Board, i16)>,
) -> Option<u8> {
    buf.clear();
    let mut board = Board::startpos();
    let random_plies = 8 + (rng.next_u64() % 3) as usize;
    for _ in 0..random_plies {
        let mut legal = MoveList::new();
        generate_legal(&board, &mut legal);
        if legal.is_empty() {
            return None;
        }
        let idx = (rng.next_u64() as usize) % legal.len();
        let mv = legal.as_slice()[idx];
        board.make_move(mv);
        if board.is_fifty_move_draw() || board.is_repetition() || board.is_insufficient_material() {
            return None;
        }
    }

    let limits = SearchLimits {
        max_depth: 64,
        max_nodes: Some(nodes_per_move),
        ..SearchLimits::default()
    };

    let mut adj_win_streak = 0u8;
    let mut adj_loss_streak = 0u8;
    let mut draw_streak = 0u8;
    searcher.clear_state();

    for ply in 0..240usize {
        let mut legal = MoveList::new();
        generate_legal(&board, &mut legal);
        if legal.is_empty() {
            if board.in_check() {
                return Some(if board.side_to_move() == Color::White {
                    0
                } else {
                    2
                });
            }
            return Some(1);
        }
        if board.is_fifty_move_draw() || board.is_repetition() || board.is_insufficient_material() {
            return Some(1);
        }

        let res = searcher.search(&mut board.clone(), &limits, |_| {});
        let mv = res.best_move;
        if mv == Move::NONE {
            return Some(1);
        }

        let w_cp = match Score::from_raw(res.score) {
            Score::Cp(cp) => white_relative_score(cp, board.side_to_move()),
            Score::Mate(m) => {
                let raw = if m > 0 { 29_000 } else { -29_000 };
                white_relative_score(raw, board.side_to_move())
            }
        };

        if ply == 0 && w_cp.abs() > 1000 {
            return None;
        }

        let is_quiet = !board.in_check() && !mv.is_capture() && mv.promotion_piece().is_none();
        if is_quiet && w_cp.abs() < MATE_BOUND - 1000 {
            let clamped = w_cp.clamp(i16::MIN as i32 + 1, i16::MAX as i32 - 1) as i16;
            buf.push((board.clone(), clamped));
        }

        if w_cp >= 2200 {
            adj_win_streak += 1;
            adj_loss_streak = 0;
            draw_streak = 0;
        } else if w_cp <= -2200 {
            adj_loss_streak += 1;
            adj_win_streak = 0;
            draw_streak = 0;
        } else if ply > 50 && w_cp.abs() <= 15 {
            draw_streak += 1;
            adj_win_streak = 0;
            adj_loss_streak = 0;
        } else {
            adj_win_streak = 0;
            adj_loss_streak = 0;
            draw_streak = 0;
        }

        if adj_win_streak >= 4 {
            return Some(2);
        }
        if adj_loss_streak >= 4 {
            return Some(0);
        }
        if draw_streak >= 10 {
            return Some(1);
        }

        board.make_move(mv);
    }

    Some(1)
}

fn main() {
    let cfg = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let nnue = cfg.eval_file.as_ref().map(|p| {
        Arc::new(NnueNet::load_from_file(p).unwrap_or_else(|e| {
            eprintln!("Failed to load NNUE {}: {e}", p.display());
            std::process::exit(1);
        }))
    });

    if let Some(parent) = cfg.out_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).expect("create output directory");
        }
    }
    let file = File::create(&cfg.out_path).expect("create output file");
    let writer = Arc::new(Mutex::new(BufWriter::with_capacity(1 << 20, file)));
    let total_pos = Arc::new(AtomicU64::new(0));
    let total_games = Arc::new(AtomicU64::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let start = Instant::now();

    println!(
        "Monolith DataGen: target={} positions | nodes/move={} | threads={} | out={}",
        cfg.positions,
        cfg.nodes_per_move,
        cfg.threads,
        cfg.out_path.display()
    );

    thread::scope(|scope| {
        for tid in 0..cfg.threads {
            let writer_ref = Arc::clone(&writer);
            let pos_ref = Arc::clone(&total_pos);
            let games_ref = Arc::clone(&total_games);
            let done_ref = Arc::clone(&done);
            let nnue_ref = nnue.clone();
            let target = cfg.positions;
            let nodes = cfg.nodes_per_move;
            let format = cfg.format;
            let seed = cfg.seed
                ^ ((tid as u64)
                    .wrapping_add(1)
                    .wrapping_mul(0x9E37_79B9_7F4A_7C15));

            scope.spawn(move || {
                let mut searcher = Searcher::new(16);
                searcher.set_nnue(nnue_ref);
                let mut rng = SkillRng::new(seed);
                let mut game_buf = Vec::with_capacity(128);
                let mut byte_buf = Vec::with_capacity(8192);

                while !done_ref.load(Ordering::Relaxed) {
                    let Some(wdl) = play_single_game(&mut searcher, &mut rng, nodes, &mut game_buf)
                    else {
                        continue;
                    };
                    if game_buf.is_empty() {
                        continue;
                    }

                    byte_buf.clear();
                    let mut count = 0u64;
                    let wdl_str = match wdl {
                        2 => "1.0",
                        1 => "0.5",
                        _ => "0.0",
                    };
                    for (b, score) in &game_buf {
                        match format {
                            OutputFormat::Bullet => {
                                if let Some(rec) = BulletRecord::from_board(b, *score, wdl) {
                                    byte_buf.extend_from_slice(&rec.to_bytes());
                                    count += 1;
                                }
                            }
                            OutputFormat::Text => {
                                let _ = writeln!(byte_buf, "{} | {score} | {wdl_str}", b.to_fen());
                                count += 1;
                            }
                        }
                    }

                    if count == 0 {
                        continue;
                    }

                    if let Ok(mut guard) = writer_ref.lock() {
                        let _ = guard.write_all(&byte_buf);
                    }
                    let prev = pos_ref.fetch_add(count, Ordering::Relaxed);
                    let g = games_ref.fetch_add(1, Ordering::Relaxed) + 1;
                    let now_pos = prev + count;
                    if now_pos >= target {
                        done_ref.store(true, Ordering::Relaxed);
                    }
                    if g.is_multiple_of(100) || done_ref.load(Ordering::Relaxed) {
                        let secs = start.elapsed().as_secs_f64().max(0.001);
                        let pps = now_pos as f64 / secs;
                        println!("Progress: {now_pos}/{target} pos ({g} games, {pps:.0} pos/s)");
                    }
                }
            });
        }
    });

    if let Ok(mut guard) = writer.lock() {
        let _ = guard.flush();
    }
    let final_pos = total_pos.load(Ordering::Relaxed);
    let secs = start.elapsed().as_secs_f64().max(0.001);
    println!(
        "Finished: {final_pos} positions in {secs:.2}s ({:.0} pos/s) -> {}",
        final_pos as f64 / secs,
        cfg.out_path.display()
    );
}

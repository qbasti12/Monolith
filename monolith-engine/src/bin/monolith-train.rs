//! Pure-Rust multi-threaded CPU AdamW trainer for `(768 -> 256) x 2 -> 1` SCReLU NNUEs.
//! Reads 32-byte `bulletformat` `.bin` datasets and outputs `.mnue` weights for `monolith-engine`.

use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use std::thread;
use std::time::Instant;

use monolith_core::types::Color;
use monolith_engine::nnue::{
    BulletRecord, NnueNet, NNUE_HL, NNUE_INPUTS, NNUE_QA, NNUE_QB, NNUE_SCALE,
};

struct TrainConfig {
    data_path: PathBuf,
    out_path: PathBuf,
    epochs: usize,
    batch_size: usize,
    lr: f32,
    wdl_lambda: f32,
    threads: usize,
}

fn parse_args() -> Result<TrainConfig, String> {
    let mut cfg = TrainConfig {
        data_path: PathBuf::from("data/monolith_train.bin"),
        out_path: PathBuf::from("nnue/monolith_cpu.mnue"),
        epochs: 15,
        batch_size: 2048,
        lr: 0.001,
        wdl_lambda: 0.5,
        threads: thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4),
    };
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data" | "-d" => {
                cfg.data_path = PathBuf::from(args.next().ok_or("missing --data")?);
            }
            "--out" | "-o" => {
                cfg.out_path = PathBuf::from(args.next().ok_or("missing --out")?);
            }
            "--epochs" | "-e" => {
                cfg.epochs = args
                    .next()
                    .ok_or("missing --epochs")?
                    .parse()
                    .map_err(|_| "invalid --epochs")?;
            }
            "--batch" | "-b" => {
                cfg.batch_size = args
                    .next()
                    .ok_or("missing --batch")?
                    .parse()
                    .map_err(|_| "invalid --batch")?;
            }
            "--lr" => {
                cfg.lr = args
                    .next()
                    .ok_or("missing --lr")?
                    .parse()
                    .map_err(|_| "invalid --lr")?;
            }
            "--wdl" => {
                cfg.wdl_lambda = args
                    .next()
                    .ok_or("missing --wdl")?
                    .parse()
                    .map_err(|_| "invalid --wdl")?;
            }
            "--threads" | "-t" => {
                cfg.threads = args
                    .next()
                    .ok_or("missing --threads")?
                    .parse::<usize>()
                    .map_err(|_| "invalid --threads")?
                    .max(1);
            }
            "--help" | "-h" => {
                println!(
                    "Usage: monolith-train --data FILE [--out MNUE] [--epochs N] [--batch N] [--lr F] [--wdl F] [--threads T]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(cfg)
}

#[inline]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

struct FloatNet {
    ft_w: Vec<f32>,
    ft_b: Vec<f32>,
    out_w: Vec<f32>,
    out_b: f32,
}

struct AdamState {
    m_ft_w: Vec<f32>,
    v_ft_w: Vec<f32>,
    m_ft_b: Vec<f32>,
    v_ft_b: Vec<f32>,
    m_out_w: Vec<f32>,
    v_out_w: Vec<f32>,
    m_out_b: f32,
    v_out_b: f32,
    step: usize,
}

impl FloatNet {
    fn new_xavier() -> Self {
        let mut seed = 0x1234_5678_9ABC_DEF0u64;
        let mut next_f32 = || -> f32 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let u = (seed >> 40) as f32 / ((1u64 << 24) as f32);
            u * 2.0 - 1.0
        };
        let ft_scale = (1.0 / 32.0_f32).sqrt() * 0.25;
        let out_scale = (1.0 / (2 * NNUE_HL) as f32).sqrt() * 0.5;
        Self {
            ft_w: (0..NNUE_INPUTS * NNUE_HL)
                .map(|_| next_f32() * ft_scale)
                .collect(),
            ft_b: vec![0.0; NNUE_HL],
            out_w: (0..2 * NNUE_HL).map(|_| next_f32() * out_scale).collect(),
            out_b: 0.0,
        }
    }

    fn quantize(&self) -> NnueNet {
        let qa = NNUE_QA as f32;
        let qb = NNUE_QB as f32;
        let mut ft_weights = vec![0i16; NNUE_INPUTS * NNUE_HL];
        for (dst, &src) in ft_weights.iter_mut().zip(&self.ft_w) {
            *dst = (src * qa).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        }
        let mut ft_biases = vec![0i16; NNUE_HL];
        for (dst, &src) in ft_biases.iter_mut().zip(&self.ft_b) {
            *dst = (src * qa).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        }
        let mut out_weights = vec![0i16; 2 * NNUE_HL];
        for (dst, &src) in out_weights.iter_mut().zip(&self.out_w) {
            *dst = (src * qb).round().clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        }
        let out_bias = (self.out_b * qa * qb).round() as i32;

        NnueNet {
            ft_weights: ft_weights.into_boxed_slice().try_into().unwrap(),
            ft_biases: ft_biases.into_boxed_slice().try_into().unwrap(),
            out_weights: out_weights.into_boxed_slice().try_into().unwrap(),
            out_bias,
        }
    }
}

impl AdamState {
    fn new() -> Self {
        Self {
            m_ft_w: vec![0.0; NNUE_INPUTS * NNUE_HL],
            v_ft_w: vec![0.0; NNUE_INPUTS * NNUE_HL],
            m_ft_b: vec![0.0; NNUE_HL],
            v_ft_b: vec![0.0; NNUE_HL],
            m_out_w: vec![0.0; 2 * NNUE_HL],
            v_out_w: vec![0.0; 2 * NNUE_HL],
            m_out_b: 0.0,
            v_out_b: 0.0,
            step: 0,
        }
    }
}

struct GradAccum {
    ft_w: Vec<f32>,
    ft_b: Vec<f32>,
    out_w: Vec<f32>,
    out_b: f32,
    loss_sum: f64,
}

impl GradAccum {
    fn new() -> Self {
        Self {
            ft_w: vec![0.0; NNUE_INPUTS * NNUE_HL],
            ft_b: vec![0.0; NNUE_HL],
            out_w: vec![0.0; 2 * NNUE_HL],
            out_b: 0.0,
            loss_sum: 0.0,
        }
    }

    fn zero(&mut self) {
        self.ft_w.fill(0.0);
        self.ft_b.fill(0.0);
        self.out_w.fill(0.0);
        self.out_b = 0.0;
        self.loss_sum = 0.0;
    }
}

fn process_chunk(net: &FloatNet, chunk: &[BulletRecord], wdl_lambda: f32, grad: &mut GradAccum) {
    let mut w_feats = [0usize; 32];
    let mut b_feats = [0usize; 32];
    let mut h_w = [0.0f32; NNUE_HL];
    let mut h_b = [0.0f32; NNUE_HL];

    for rec in chunk {
        let mut n = 0usize;
        for (color, pt, sq) in rec.iter_pieces() {
            if n >= 32 {
                break;
            }
            w_feats[n] = NnueNet::feature_index(Color::White, color, pt, sq);
            b_feats[n] = NnueNet::feature_index(Color::Black, color, pt, sq);
            n += 1;
        }

        h_w.copy_from_slice(&net.ft_b);
        h_b.copy_from_slice(&net.ft_b);
        for &fw in &w_feats[..n] {
            let row = &net.ft_w[fw * NNUE_HL..(fw + 1) * NNUE_HL];
            for i in 0..NNUE_HL {
                h_w[i] += row[i];
            }
        }
        for &fb in &b_feats[..n] {
            let row = &net.ft_w[fb * NNUE_HL..(fb + 1) * NNUE_HL];
            for i in 0..NNUE_HL {
                h_b[i] += row[i];
            }
        }

        let mut raw = net.out_b;
        for i in 0..NNUE_HL {
            let cw = h_w[i].clamp(0.0, 1.0);
            let cb = h_b[i].clamp(0.0, 1.0);
            raw += cw * cw * net.out_w[i] + cb * cb * net.out_w[NNUE_HL + i];
        }

        let pred = sigmoid(raw);
        let score_target = sigmoid(rec.score_white_cp as f32 / NNUE_SCALE as f32);
        let wdl_target = rec.result_wdl as f32 * 0.5;
        let target = (1.0 - wdl_lambda) * score_target + wdl_lambda * wdl_target;

        let diff = pred - target;
        grad.loss_sum += (diff * diff) as f64;
        let d_raw = 2.0 * diff * pred * (1.0 - pred);
        grad.out_b += d_raw;

        for i in 0..NNUE_HL {
            let cw = h_w[i].clamp(0.0, 1.0);
            let cb = h_b[i].clamp(0.0, 1.0);
            grad.out_w[i] += d_raw * cw * cw;
            grad.out_w[NNUE_HL + i] += d_raw * cb * cb;

            let dw = if h_w[i] > 0.0 && h_w[i] < 1.0 {
                d_raw * net.out_w[i] * 2.0 * cw
            } else {
                0.0
            };
            let db = if h_b[i] > 0.0 && h_b[i] < 1.0 {
                d_raw * net.out_w[NNUE_HL + i] * 2.0 * cb
            } else {
                0.0
            };
            grad.ft_b[i] += dw + db;

            if dw != 0.0 {
                for &fw in &w_feats[..n] {
                    grad.ft_w[fw * NNUE_HL + i] += dw;
                }
            }
            if db != 0.0 {
                for &fb in &b_feats[..n] {
                    grad.ft_w[fb * NNUE_HL + i] += db;
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
struct AdamHyper {
    lr: f32,
    inv_b: f32,
    b1_corr: f32,
    b2_corr: f32,
}

fn adamw_step(params: &mut [f32], grads: &[f32], m: &mut [f32], v: &mut [f32], h: AdamHyper) {
    let b1 = 0.9f32;
    let b2 = 0.999f32;
    let eps = 1e-8f32;
    let wd = 0.005f32;
    for i in 0..params.len() {
        let g = grads[i] * h.inv_b;
        m[i] = b1 * m[i] + (1.0 - b1) * g;
        v[i] = b2 * v[i] + (1.0 - b2) * g * g;
        let m_hat = m[i] / h.b1_corr;
        let v_hat = v[i] / h.b2_corr;
        params[i] -= h.lr * (m_hat / (v_hat.sqrt() + eps) + wd * params[i]);
        params[i] = params[i].clamp(-1.98, 1.98);
    }
}

fn load_dataset(path: &PathBuf) -> Result<Vec<BulletRecord>, String> {
    let file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let len = file.metadata().map_err(|e| e.to_string())?.len() as usize;
    let count = len / BulletRecord::BYTE_SIZE;
    let mut reader = BufReader::new(file);
    let mut records = Vec::with_capacity(count);
    let mut buf = [0u8; BulletRecord::BYTE_SIZE];
    for _ in 0..count {
        reader.read_exact(&mut buf).map_err(|e| e.to_string())?;
        records.push(BulletRecord::from_bytes(&buf));
    }
    Ok(records)
}

fn main() {
    let cfg = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let records = load_dataset(&cfg.data_path).unwrap_or_else(|e| {
        eprintln!("Failed to load dataset: {e}");
        std::process::exit(1);
    });
    if records.is_empty() {
        eprintln!("Dataset is empty.");
        std::process::exit(1);
    }

    println!(
        "Monolith CPU Trainer: {} positions | epochs={} | batch={} | threads={}",
        records.len(),
        cfg.epochs,
        cfg.batch_size,
        cfg.threads
    );

    let mut net = FloatNet::new_xavier();
    let mut adam = AdamState::new();
    let mut thread_grads: Vec<GradAccum> = (0..cfg.threads).map(|_| GradAccum::new()).collect();
    let mut total_grad = GradAccum::new();

    for epoch in 0..cfg.epochs {
        let ep_start = Instant::now();
        let mut epoch_loss = 0.0f64;
        let progress = epoch as f32 / cfg.epochs.max(1) as f32;
        let lr = cfg.lr * 0.5 * (1.0 + (std::f32::consts::PI * progress).cos());

        for batch in records.chunks(cfg.batch_size) {
            let chunk_size = batch.len().div_ceil(cfg.threads).max(1);
            for g in &mut thread_grads {
                g.zero();
            }

            thread::scope(|scope| {
                for (chunk, g) in batch.chunks(chunk_size).zip(thread_grads.iter_mut()) {
                    let net_ref = &net;
                    let wdl_l = cfg.wdl_lambda;
                    scope.spawn(move || process_chunk(net_ref, chunk, wdl_l, g));
                }
            });

            total_grad.zero();
            for g in &thread_grads {
                total_grad.loss_sum += g.loss_sum;
                total_grad.out_b += g.out_b;
                for i in 0..total_grad.ft_w.len() {
                    total_grad.ft_w[i] += g.ft_w[i];
                }
                for i in 0..total_grad.ft_b.len() {
                    total_grad.ft_b[i] += g.ft_b[i];
                }
                for i in 0..total_grad.out_w.len() {
                    total_grad.out_w[i] += g.out_w[i];
                }
            }
            epoch_loss += total_grad.loss_sum;

            adam.step += 1;
            let inv_b = 1.0 / batch.len() as f32;
            let b1_corr = 1.0 - 0.9f32.powi(adam.step as i32);
            let b2_corr = 1.0 - 0.999f32.powi(adam.step as i32);

            let hyper = AdamHyper {
                lr,
                inv_b,
                b1_corr,
                b2_corr,
            };
            adamw_step(
                &mut net.ft_w,
                &total_grad.ft_w,
                &mut adam.m_ft_w,
                &mut adam.v_ft_w,
                hyper,
            );
            adamw_step(
                &mut net.ft_b,
                &total_grad.ft_b,
                &mut adam.m_ft_b,
                &mut adam.v_ft_b,
                hyper,
            );
            adamw_step(
                &mut net.out_w,
                &total_grad.out_w,
                &mut adam.m_out_w,
                &mut adam.v_out_w,
                hyper,
            );

            let g_b = total_grad.out_b * inv_b;
            adam.m_out_b = 0.9 * adam.m_out_b + 0.1 * g_b;
            adam.v_out_b = 0.999 * adam.v_out_b + 0.001 * g_b * g_b;
            net.out_b -= lr * (adam.m_out_b / b1_corr) / ((adam.v_out_b / b2_corr).sqrt() + 1e-8);
        }

        let mean_loss = epoch_loss / records.len() as f64;
        println!(
            "Epoch {:2}/{} | loss={:.6} | lr={:.6} | time={:.2}s",
            epoch + 1,
            cfg.epochs,
            mean_loss,
            lr,
            ep_start.elapsed().as_secs_f64()
        );
    }

    let quantized = net.quantize();
    quantized
        .save_to_file(&cfg.out_path)
        .expect("save .mnue weights");
    println!("Saved quantized NNUE weights -> {}", cfg.out_path.display());
}

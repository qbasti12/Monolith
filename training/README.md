# Monolith Engine Training & Elo-Testing Guide

Diese Anleitung deckt **beide** Setups ab:
1. **Mac (Apple Silicon / unterwegs):** Baseline-Elo-Messung mit `fastchess`, leichtes Self-Play Data-Gen und CPU-Training mit `monolith-train`.
2. **Main PC (i7-8700 + RTX 2070 Super unter Linux/WSL2):** Schnelle 100M+ Datengenerierung (12 Threads + AVX2) und GPU-NNUE-Training.

---

## 1. Release-Binaries mit CPU-Optimierung (`NEON` / `AVX2`) bauen

Auf jedem Rechner lokal einmal mit `-C target-cpu=native` bauen (bringt ca. +20–40 % Speed bei Suche & NNUE):

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release -p monolith-engine
```

Das erzeugt:
- `target/release/monolith-engine` (UCI-Engine mit `Threads`, `UseNNUE`, `EvalFile`)
- `target/release/monolith-datagen` (Multi-Thread Self-Play Data-Generator, 32-Byte `bulletformat`)
- `target/release/monolith-train` (Reiner Rust CPU-AdamW-Trainer für `.mnue`)

---

## 2. Echte Elo-Stärke messen (`fastchess` + Stockfish)

Einmalig einrichten (installiert Stockfish via Homebrew falls vorhanden, kompiliert `fastchess` nach `tools/fastchess` und erstellt `tools/openings.pgn`):

```bash
./scripts/setup_fastchess.sh
```

Danach Baseline messen (z. B. 100 Runden = 200 Partien bei `10s + 0.1s` gegen Stockfish 2000 Elo):

```bash
OPPONENT_ELO=2000 ROUNDS=100 CONCURRENCY=4 ./scripts/elo_test.sh
```

Mit trainiertem NNUE vergleichen:

```bash
EVAL_FILE=nnue/monolith_v1.mnue OPPONENT_ELO=2200 ROUNDS=100 ./scripts/elo_test.sh
```

---

## 3. Jetzt auf dem Mac: Kleines Training testen

### Schritt A: 500.000 Positionen generieren (~1–3 Minuten)
```bash
./target/release/monolith-datagen --positions 500000 --nodes 5000 --threads 8 --out data/mac_500k.bin
```

### Schritt B: CPU-NNUE trainieren
```bash
./target/release/monolith-train --data data/mac_500k.bin --epochs 20 --batch 2048 --threads 8 --out nnue/monolith_mac_v1.mnue
```

### Schritt C: Direkt in `monolith-tui -gui` oder `fastchess` testen
In `fastchess`:
```bash
EVAL_FILE=nnue/monolith_mac_v1.mnue OPPONENT_ELO=2000 ./scripts/elo_test.sh
```

---

## 4. Zuhause auf dem Main-PC (i7-8700 + RTX 2070 Super)

Für das volle Training (50M–200M+ Positionen) empfiehlt sich **Ubuntu Linux** (oder **WSL2 Ubuntu** unter Windows mit NVIDIA CUDA Toolkit):

1. **Repo klonen & optimiert bauen:**
   ```bash
   RUSTFLAGS="-C target-cpu=native" cargo build --release -p monolith-engine
   ```
2. **Großen Datensatz mit allen 12 Threads generieren (über Nacht):**
   ```bash
   ./target/release/monolith-datagen --positions 50000000 --nodes 6000 --threads 12 --out data/pc_50m.bin
   ```
   *(Tipp: Du kannst auch am Mac generierte `.bin`-Dateien per `cat data/mac.bin data/pc.bin > data/all.bin` einfach zusammenfügen, da das 32-Byte-Format zustandslos ist!)*
3. **Training auf der RTX 2070 Super (CUDA):**
   - Entweder mit `monolith-train` oder – für 100M+ Positionen 20–50x schneller – mit **`bullet`** (`https://github.com/jw1912/bullet`) auf der 2070 Super (8 GB VRAM reichen locker für `Chess768 -> 256` mit Batch-Größe `16384`), da `monolith-datagen` direkt das Standard-32-Byte-`bulletformat` schreibt.
4. **Bootstrapping (Gen 2):**
   Sobald `v1.mnue` stärker als PeSTO ist, `monolith-datagen --eval-file nnue/v1.mnue` starten und die nächste Generation auf noch stärkeren Partien trainieren.

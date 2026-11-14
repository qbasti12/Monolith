# Monolith Engine Training & Elo-Testing Guide

Diese Anleitung deckt **beide** Setups ab:
1. **Main PC (Omarchy / Arch Linux, i7-8700 + RTX 2070 Super):** Vollautomatisches Multi-Generationen-Bootstrapping über Nacht (`bullet` CUDA für GPU-Training + `monolith-datagen` mit 12 Threads & AVX2).
2. **Mac (Apple Silicon / unterwegs):** Elo-Messung mit `fastchess` und CPU-Testläufe mit `monolith-train`.

---

## 1. Release-Binaries mit CPU-Optimierung (`NEON` / `AVX2`) bauen

Auf jedem Rechner einmalig mit `-C target-cpu=native` bauen:

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release -p monolith-engine
```

Das erzeugt:
- `target/release/monolith-engine` (UCI-Engine mit Inkrementellem `(768 -> 512) x 2 -> 1` NNUE, `Threads`, `UseNNUE`, `EvalFile`)
- `target/release/monolith-datagen` (Multi-Thread Self-Play Data-Generator, `--format bullet` oder `--format text`)
- `target/release/monolith-train` (Multi-Thread CPU-AdamW-Trainer mit Per-Epoch-Shuffle & Checkpoints)

---

## 2. Zuhause auf Omarchy / Arch Linux (i7-8700 + RTX 2070 Super): One-Command Overnight Pipeline

### Einmalig vorbereiten

```bash
sudo pacman -S --needed base-devel git cuda stockfish python
./scripts/setup_fastchess.sh
./scripts/setup_bullet_cuda.sh
```

*(Falls `nvcc` direkt nach `pacman -S cuda` noch nicht im `PATH` liegt: `setup_bullet_cuda.sh` und `train_overnight.sh` binden `/opt/cuda/bin` automatisch ein.)*

### Training über Nacht starten (läuft danach komplett unbeaufsichtigt)

```bash
./scripts/train_overnight.sh
```

Was `train_overnight.sh` automatisch nacheinander ausführt:
1. **Native Rebuild:** Baut `monolith-engine`, `monolith-datagen`, `monolith-train` mit `-C target-cpu=native` (AVX2).
2. **Gen 1 (`15M` Positionen):** `monolith-datagen` erzeugt Text-Daten (`<FEN> | <cp> | <wdl>`) auf 12 Threads mit PeSTO + optimierter Suche, `bullet-utils` konvertiert & mischt sie, `monolith_cuda` trainiert auf der RTX 2070 Super ein `(768 -> 512) x 2 -> 1` SCReLU-Netz, und `fastchess` prüft per SPRT gegen die PeSTO-Baseline. Gewinnt Gen 1, wird es `nnue/best.mnue`.
3. **Gen 2 (`30M` Positionen):** `monolith-datagen --eval-file nnue/best.mnue` erzeugt stärkere Selbstspiel-Daten mit Gen 1, trainiert Gen 2 auf der GPU und befördert nur bei echtem SPRT/Elo-Gewinn nach `nnue/best.mnue`.
4. **Gen 3 (`50M` Positionen):** Bootstrapping auf Basis von Gen 2 bis `nnue/best.mnue`, danach automatischer Schluss-Benchmark gegen Stockfish 2800 Elo.

### Laufzeit & Parameter anpassen (optional)
Standardmäßig sind `GEN1_POS=15000000 GEN2_POS=30000000 GEN3_POS=50000000 NODES_PER_MOVE=6000` gesetzt (~8–14 Stunden auf einem i7-8700 + RTX 2070 Super). Du kannst alle Parameter direkt überschreiben:

```bash
GENS=2 GEN1_POS=5000000 GEN2_POS=15000000 NODES_PER_MOVE=5000 ./scripts/train_overnight.sh
```

---

## 3. Echte Elo-Stärke manuell messen (`fastchess` + Stockfish)

Baseline ohne NNUE (PeSTO):
```bash
OPPONENT_ELO=2650 THREADS=1 ROUNDS=60 CONCURRENCY=4 TC="10+0.1" ./scripts/elo_test.sh
```

Trainiertes Netz (funktioniert sowohl mit `.mnue` als auch direkt mit `bullet` `quantised.bin`) testen:
```bash
EVAL_FILE=nnue/best.mnue OPPONENT_ELO=2800 THREADS=1 ROUNDS=60 ./scripts/elo_test.sh
```

Multi-Thread testen (z. B. `THREADS=4`):
```bash
EVAL_FILE=nnue/best.mnue OPPONENT_ELO=2900 THREADS=4 CONCURRENCY=2 ROUNDS=60 ./scripts/elo_test.sh
```

---

## 4. Kurzer CPU-Smoke-Test auf dem Mac (ohne CUDA)

```bash
./target/release/monolith-datagen --positions 500000 --nodes 5000 --threads 8 --out data/mac_500k.bin
./target/release/monolith-train --data data/mac_500k.bin --epochs 20 --batch 2048 --threads 8 --out nnue/monolith_mac.mnue
```
*(Hinweis: Ein `HL=512`-Netz braucht für 2800–3000 Elo mindestens 15M–50M+ diversifizierte Selbstspiel-Positionen – auf 500k Positionen überangepasst/untertrainiert verliert jedes große NNUE gegen kalibrierte Heuristik wie PeSTO.)*

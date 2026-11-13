# Monolith

**Monolith** is a high-performance chess engine, core rules library, and Catppuccin Mocha terminal workstation written in Rust (Edition 2021). It combines a bitboard-based chess core supporting arbitrary piece counts (e.g., Infinite Queens presets), a zero-allocation alpha-beta UCI engine with Elo strength throttling (800–3200), a responsive `ratatui` chess client with PGN background analysis and up to 3 concurrent UCI engines, and a decoupled live UDP telemetry viewer.

---

## Architecture

```text
Monolith/
├── Cargo.toml                 # Workspace (resolver = "2", release: opt-level=3, lto="fat", panic="abort")
├── monolith-core/             # Bitboards, runtime Magic Bitboards, Zobrist hashing, FEN, Movegen, SAN/PGN, move classifier
├── monolith-engine/           # PeSTO evaluation, Transposition Table, PVS/Alpha-Beta search, Elo skill, UCI server + UciProcess
├── monolith-tui/              # Client (--mode client, -gui / -tui) + UDP telemetry viewer (--mode logs)
└── scripts/
    ├── check.sh               # Workspace fmt check, strict clippy (-D warnings), and full release test suite
    └── smoke.sh               # Release binary smoke tests (--help, --version, error exit, UCI handshake)
```

### Data & Telemetry Flow

```text
  Terminal 1 (Client: -gui or -tui)                       Terminal 2 (Telemetry Logs Viewer)
 ┌──────────────────────────────────┐    UDP JSON-Lines   ┌──────────────────────────────────┐
 │ monolith-tui --mode client       │ ──────────────────► │ monolith-tui --mode logs         │
 │  ├─ Board / Eval-Bar / Moves     │   127.0.0.1:42424   │  ├─ Engine Metrics (Depth/SelDepth│
 │  ├─ Player View (Eval, Depth,    │     (< 1400 B)      │  │   MPV, Nodes, NPS, Hash, Time) │
 │  │   Best Move, Reply, PV)       │                     │  ├─ EvalBreakdown + FEN / Last Move│
 │  └─ Up to 3 x UciProcess ◄──pipe──► monolith-engine    │  └─ 5000-line Ring-Buffer Stream  │
 └──────────────────────────────────┘                     └──────────────────────────────────┘
```

Terminal 1 strictly shows player-facing information (evaluation, compact search depth, best move, expected reply, and principal variation), while technical search diagnostics (`Nodes`, `NPS`, `Hashfull`, `SelDepth`, `MultiPV`, search time, raw UCI lines, and static evaluation breakdown) live exclusively in Terminal 2 (`--mode logs`).

---

## Key Features

### `monolith-core`
- **Pure `u64` Bitboards without piece-count caps:** No per-piece fixed arrays; positions with 15+ queens on the board load, generate legal moves, and play cleanly.
- **Stack-Allocated `MoveList` (512 slots) & 16-bit `Move`:** Prevents overflows even in saturated queen positions (`> 256` legal moves), with `debug_assert!` and saturating release push behavior.
- **Deterministic Runtime Magic Bitboards:** Precomputed sliding-piece attack tables (rook/bishop/queen), knight/king/pawn tables, ray/between/line tables initialized once in `OnceLock`.
- **Incremental Zobrist Hashing & Draw Detection:** 3-fold repetition, 50-move rule, and insufficient material detection with 2048-ply preallocated undo history.
- **FEN & PGN Support:** Strict validation returning typed `thiserror` errors on malformed input; SAN disambiguation/formatting, PGN header/comment/variation handling, and move classification (`!!`, `!`, `Best`, `Good`, `?!`, `?`, `??`).

### `monolith-engine`
- **Zero-Allocation Search Hotpath:** Iterative deepening, Negamax Alpha-Beta, Principal Variation Search (PVS), Aspiration Windows, Quiescence search (captures/promotions/checks with delta pruning), SEE-light + MVV-LVA + Killer + History move ordering, Null-Move Pruning, Reverse Futility Pruning, LMR, Check Extensions, and Multi-PV. Verified `0` heap allocations during `search()` via custom `GlobalAlloc` test.
- **PeSTO + Positional Evaluation (`i32` overflow-safe):** Clamped game-phase (`0..=24`), bishop pair, passed/isolated/doubled pawns, king pawn-shield/open-file safety, and piece mobility, plus full component `EvalBreakdown`.
- **Transposition Table:** 16-byte compact entries, multiplicative index mapping, ply-relative mate distance adjustment, configurable 1–2048 MB (`Default: 64 MB`).
- **Elo Strength Throttling (`800..=3200`):** Multi-PV depth/node throttling, controlled evaluation noise/softmax sampling, and probabilistic mate-in-1 oversight (`>= 3000` runs unthrottled).
- **Full UCI Server & External Runner (`UciProcess`):** Threaded UCI stdin/stdout loop (`uci`, `isready`, `ucinewgame`, `position`, `go`, `stop`, `setoption`, `quit`) plus child-process manager supporting third-party engines such as Stockfish.

### `monolith-tui`
- **Dual Client Frontend (`-gui` & `-tui`):** Native graphical `eframe` desktop studio (`-gui` / `--gui`, default) with SVG Lichess/Monolith piece sets, mouse Drag & Drop, right-click arrows/highlights, synthesized Chess.com-style sounds, Eval-Graph, Opening Explorer, and Move Trainer — alongside the fast `ratatui` terminal client (`-tui` / `--tui`).
- **Catppuccin Mocha Theme:** Consistent Mocha palette across GUI and terminal (with automatic detection of TrueColor, 256-color, or basic ANSI fallback).
- **Responsive Board & Vertical Eval Bar:** Centered Unicode pieces (`♔♕♖♗♘♙` / `♚♛♜♝♞♟`) in TUI or SVG pieces in GUI, last-move and engine best-move highlights, legal target indicators, coordinate labels, board-flip aware sigmoid win-probability eval bar (`1 / (1 + exp(-0.004 * cp))`), and move classification badges on both move history and target squares.
- **Multi-Engine Analysis (1–3 Concurrent Engines):** Compare up to three engines simultaneously with independent Elo settings (`800..=3200`), search depth limits (`1..=30`), external UCI binary paths, or player-vs-engine role.
- **Background PGN Game Analyzer:** Analyzes every ply in a background worker thread with progress percentage, cancellation, move badges (`!!`, `!`, `Best`, `?!`, `?`, `??`), and 1-sentence explanations.
- **Decoupled UDP Telemetry (`--mode logs`):** Non-blocking localhost JSON-Lines packets (`< 1400` bytes MTU safe with PV truncation), malformed-packet resilience, connection status (`Hello`/`Bye`/Reconnect), 5000-line ring buffer, Auto-Follow toggle, slot filtering, and static `EvalBreakdown` inspector.
- **Terminal Safety:** Panic hook + RAII `TerminalGuard` cleanly restore Raw Mode, cursor visibility, and Alternate Screen even with `panic = "abort"`.

---

## Build & Verification

Requires a standard Rust toolchain (`rustc` / `cargo`, Edition 2021).

```bash
# Build optimized release binaries
cargo build --release

# Run formatting check, strict clippy (-D warnings), and full release test suite
./scripts/check.sh

# Run CLI & binary smoke checks
./scripts/smoke.sh
```

---

## Running Monolith

### Terminal 1 — Interactive Chess Client (`--mode client`)

```bash
# Graphical client (-gui, default)
./target/release/monolith-tui -gui --mode client
# Terminal UI client (-tui)
./target/release/monolith-tui -tui --mode client
# or directly via Cargo:
cargo run --release -p monolith-tui -- -tui --mode client
```

Optional flags:
- `-gui` / `--gui` — Start the graphical desktop client (default when neither `-gui` nor `-tui` is passed)
- `-tui` / `--tui` — Start the `ratatui` terminal client
- `--port <PORT>` — UDP telemetry port on `127.0.0.1` (default: `42424`)
- `--no-ipc` — Disable localhost UDP telemetry emission
- `--fen <FEN>` — Load a custom starting position
- `--pgn <PATH>` — Load and analyze a `.pgn` file immediately at startup
- `--engine <PATH>` — Override Engine 1 UCI binary (defaults to sibling `monolith-engine` or `PATH`)

### Terminal 2 — Live Telemetry & Logs Viewer (`--mode logs`)

Start in a second terminal (before, during, or after starting the client):

```bash
./target/release/monolith-tui --mode logs
# custom port:
./target/release/monolith-tui --mode logs --port 42424
```

### Standalone Engine (UCI)

Compatible with any standard UCI GUI (`CuteChess`, `Arena`, `Nibbler`, `En Croissant`, CLI piping):

```bash
./target/release/monolith-engine
```

---

## Keybindings

### Client Mode (`--mode client`)
| Key | Action |
|---|---|
| `←` `↑` `↓` `→` + `Enter` / `Space` | Move cursor, pick piece, confirm move (opens promotion menu when needed) |
| `/` or `:` or `M` or `Tab` | Focus text move input (accepts UCI like `e2e4` or SAN like `Nf3`, `O-O`) |
| `U` | Undo last half-move |
| `X` | Flip board orientation (and eval bar) |
| `[` / `]` or `PgUp` / `PgDn` | Step backward / forward in move history (`←`/`→` also steps during PGN review) |
| `Home` / `End` | Jump to start or end of move list |
| `F` | Open FEN modal (`F1` Standard, `F2` Infinite Queens, `F3` Kiwipete) |
| `P` | Open PGN loader & start background analysis (`F1` loads built-in Morphy Opera Game) |
| `E` | Open Multi-Engine configuration (`1`/`2`/`3` slots, Elo `800..3200`, depth, external binary, play role) |
| `C` | Cancel running PGN analysis or toggle PGN arrow navigation mode |
| `?` | Toggle help overlay |
| `Q` / `Ctrl+Q` / `Ctrl+C` | Exit cleanly and restore terminal |

### Logs Mode (`--mode logs`)
| Key | Action |
|---|---|
| `↑` / `↓` / `PgUp` / `PgDn` / `Home` / `End` | Scroll 5000-line log ring buffer |
| `F` or `Space` | Toggle Auto-Follow mode |
| `0` / `1` / `2` / `3` | Show all slots or filter to Engine 1 / 2 / 3 |
| `C` | Clear ring buffer |
| `Q` / `Ctrl+Q` / `Ctrl+C` | Exit cleanly |

---

## UCI Options (`monolith-engine`)

| Option | Type | Range / Values | Default | Description |
|---|---|---|---|---|
| `Hash` | spin | `1..=2048` MB | `64` | Transposition table size |
| `MultiPV` | spin | `1..=16` | `1` | Number of principal variations |
| `UCI_LimitStrength` | check | `true` / `false` | `false` | Enable Elo strength throttling |
| `UCI_Elo` | spin | `800..=3200` | `2400` | Target Elo when strength limiting is enabled |
| `Skill_Depth` | spin | `1..=64` | `64` | Additional search depth cap |

---

## UDP IPC Telemetry Protocol

- **Transport:** Non-blocking UDP on `127.0.0.1:42424` (configurable via `--port`).
- **Framing:** UTF-8 JSON-Lines (`\n`-terminated), strictly enforced `< 1400` bytes per datagram via progressive `pv` / log line truncation.
- **Message Types (`"type"` discriminator):**
  - `hello` (`client_version`, `timestamp_ms`)
  - `bye` (`timestamp_ms`)
  - `position` (`fen`, `last_move`, `ply`, `timestamp_ms`)
  - `engine_info` (`slot`, `name`, `elo`, `limit_strength`, `depth`, `seldepth`, `score_cp`, `score_mate`, `nodes`, `nps`, `hashfull`, `multipv`, `time_ms`, `pv`, `timestamp_ms`)
  - `uci_line` (`slot`, `engine`, `line`, `timestamp_ms`)
  - `eval_breakdown` (`fen`, `white_pov_cp`, `stm_pov_cp`, `material_cp`, `pst_cp`, `pawn_structure_cp`, `king_safety_cp`, `mobility_cp`, `bishop_pair_cp`, `phase`, `timestamp_ms`)

---

## Benchmarks (Apple Silicon Release Build)

- **Perft (Startpos d1–5):** `242.31 MN/s` (`5,072,212` nodes in `0.021 s`)
- **Search Speed (Startpos, Depth 12):** `3,741,514 NPS` (`1,279,598` nodes in `342 ms`, single-threaded, `0` heap allocations)
- **Skill Self-Play (Elo 2400 vs. Elo 1200, 20 games <= 40 moves/game):** `20W - 0D - 0L` (`100.0%` score in `14.23 s`)

---

## License

Licensed under the [GNU General Public License v3.0](LICENSE).

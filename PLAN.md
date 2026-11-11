# Monolith – Implementierungsplan

## Meilensteine

- [x] **M0 – Gerüst**: Cargo-Workspace (`monolith-core`, `monolith-engine`, `monolith-tui`), `scripts/check.sh`, kompilierende Modulstruktur. Gate: `cargo build --release` grün.
- [x] **M1 – Core (Typen, Bitboards, Zobrist, Board, FEN)**: `types.rs`, `bitboard.rs`, `zobrist.rs`, `board.rs`, `fen.rs`. Gate: FEN-Roundtrip (Start, Kiwipete, Infinite Queens) + Invalid-FEN liefert `Err`.
- [x] **M2 – Core (Movegen + Perft)**: Legale Zuggenerierung, `generate_captures`, `perft`, `perft_divide`, `tests/perft.rs` (Start d1–5, Kiwipete d1–4, Pos 3–6, Infinite Queens). Gate: Exakte Perft-Werte + `make/unmake`-Konsistenz.
- [ ] **M3 – Core (PGN & Zug-Klassifizierung)**: SAN-Parser/-Generator, PGN-Reader, `classify_move` (`!!`, `!`, Best, Good, `?!`, `?`, `??`). Gate: PGN-Tests + SAN-Roundtrip auf allen Perft-Positionen.
- [ ] **M4 – Engine (Eval + TT)**: PeSTO-Tabellen, Game-Phase, strukturelle/Königssicherheits-Terme, `eval_breakdown`, Transpositionstabelle. Gate: Eval-Symmetrie, TT-Tests, Infinite-Queens-Eval ohne Overflow.
- [ ] **M5 – Engine (Suche)**: Iterative Deepening, Alpha-Beta, PVS, Aspiration, Quiescence, Ordering, NMP, RFP, LMR, Extensions, Mate-Scores. Gate: Mate-in-1/2/3, Taktik-Suite >= 80 %, Infinite Queens < 5 s, 0 Heap-Allokationen in Suche.
- [ ] **M6 – Engine (Skill, UCI, Multi-Engine)**: Skill/Elo-Drosselung (800–3200), vollständiges UCI + Threading, `UciProcess` Kindprozess-Runner. Gate: UCI-Piping-Test + Elo 1200 vs. 2400 Selbstspiel-Test.
- [ ] **M7 – TUI Client (`--mode client`)**: Catppuccin-Mocha-Theme, Eval-Bar, skalierbares Brett, bis zu 3 Engine-Panels, Züge/SAN/Pfeiltasten, Modi `[F]`, `[P]`, `[E]`, `[?]`, `[Q]`, Panic-Hook. Gate: Headless `App`-Tests + `scripts/smoke.sh`.
- [ ] **M8 – TUI Logs + IPC (`--mode logs`)**: UDP JSON-Lines IPC (`127.0.0.1:42424`, `< 1400 B`), Live-Metriken, Ring-Puffer-Logs, Eval-Breakdown-Ansicht. Gate: Loopback-Roundtrip-Tests + Fehlertoleranz-Test.
- [ ] **Abschluss**: `scripts/check.sh` grün, `README.md`, Benchmarks in `PROGRESS.md`, `grep`-Audit auf `todo!`/`unimplemented!`/`unwrap()`, Git-Push nach `origin/main`.

# Monolith – Fortschritt (`PROGRESS.md`)

## Aktueller Stand
- **Meilenstein:** M7 abgeschlossen (TUI Client `--mode client`: Catppuccin Mocha, skalierbares Brett, Eval-Bar, bis zu 3 Engines, PGN-Hintergrundanalyse, FEN/PGN/Engine/Help/Promotion-Modals, Panic-Hook, `scripts/smoke.sh` + `app_tests` grün). Aktuell in Arbeit: M8 (TUI Logs + UDP JSON-Lines IPC `--mode logs`).
- **Offene Probleme:** Keine.
- **Nächster Schritt:** `ipc.rs`, `logs_app.rs`/`logs_ui.rs`, UDP-Sender im Client sowie Loopback- und Fehlertoleranz-Tests implementieren und M8-Gate bestehen.

## Messwerte & Benchmarks
- *Perft-NPS:* `242.31 MN/s` (Startposition d1–5, 5.072.212 Nodes in 0.021 s, Release)
- *Suche-NPS (Startposition, Tiefe 12):* (folgt nach Abschluss-Benchmark)
- *Elo-Selbstspiel (1200 vs. 2400, 20 Partien):* `W=20, D=0, L=0` für Elo 2400 (`100.0%` Score, <= 40 Züge/Partie, 14.23 s)

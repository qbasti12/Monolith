# Monolith – Fortschritt (`PROGRESS.md`)

## Aktueller Stand
- **Meilenstein:** M6 abgeschlossen (Skill 800–3200, vollständiges UCI + Worker-Thread, `UciProcess` Kindprozess-Runner; Elo 2400 vs. 1200 Selfplay 20/20 = 100 % Score in 14.23 s). Aktuell in Arbeit: M7 (TUI Client `--mode client`).
- **Offene Probleme:** Keine.
- **Nächster Schritt:** `monolith-tui` Client-Modus (`app.rs`, `ui.rs`, `main.rs`), Panic-Hook und `scripts/smoke.sh` implementieren und M7-Gate bestehen.

## Messwerte & Benchmarks
- *Perft-NPS:* `242.31 MN/s` (Startposition d1–5, 5.072.212 Nodes in 0.021 s, Release)
- *Suche-NPS (Startposition, Tiefe 12):* (folgt nach Abschluss-Benchmark)
- *Elo-Selbstspiel (1200 vs. 2400, 20 Partien):* `W=20, D=0, L=0` für Elo 2400 (`100.0%` Score, <= 40 Züge/Partie, 14.23 s)

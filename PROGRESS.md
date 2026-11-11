# Monolith – Fortschritt (`PROGRESS.md`)

## Aktueller Stand
- **Meilenstein:** M8 abgeschlossen (UDP JSON-Lines IPC `127.0.0.1:42424` `< 1400 B`, `--mode logs` mit Live-Engine-Metriken, Eval-Breakdown, 5000-Zeilen Ringpuffer & Auto-Follow; Loopback- und Fehlertoleranz-Tests grün). Aktuell in Arbeit: Abschluss-Checks, Benchmarks & `README.md`.
- **Offene Probleme:** `search.rs` aktuell 960 Zeilen (optional auf `< 800` Zeilen aufteilen).
- **Nächster Schritt:** Depth-12 Startpos-NPS messen, `search.rs` modularisieren (< 800 Zeilen), `grep`-Audit ausführen, `README.md` vervollständigen und nach `origin/main` pushen.

## Messwerte & Benchmarks
- *Perft-NPS:* `242.31 MN/s` (Startposition d1–5, 5.072.212 Nodes in 0.021 s, Release)
- *Suche-NPS (Startposition, Tiefe 12):* (folgt nach Abschluss-Benchmark)
- *Elo-Selbstspiel (1200 vs. 2400, 20 Partien):* `W=20, D=0, L=0` für Elo 2400 (`100.0%` Score, <= 40 Züge/Partie, 14.23 s)

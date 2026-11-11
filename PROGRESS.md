# Monolith – Fortschritt (`PROGRESS.md`)

## Aktueller Stand
- **Meilenstein:** Abschluss abgeschlossen — alle Meilensteine M0–M8 und Abschluss-Checks bestanden (`./scripts/check.sh` & `./scripts/smoke.sh` grün, alle Quelldateien < 780 Zeilen, 0 `todo!`/`unimplemented!`/`unwrap()`/`expect()` in `*/src/*.rs`, `README.md` aktualisiert).
- **Offene Probleme:** Keine.
- **Nächster Schritt:** Finale Commits nach `origin/main` pushen.

## Messwerte & Benchmarks
- *Perft-NPS:* `242.31 MN/s` (Startposition d1–5, 5.072.212 Nodes in 0.021 s, Release)
- *Suche-NPS (Startposition, Tiefe 12):* `3.741.514 NPS` (`3.74 MN/s`, 1.279.598 Nodes in 342 ms, Seldepth 24, PV `e2e4 d7d5 ...`, Release)
- *Elo-Selbstspiel (1200 vs. 2400, 20 Partien):* `W=20, D=0, L=0` für Elo 2400 (`100.0%` Score, <= 40 Züge/Partie, 14.23 s)

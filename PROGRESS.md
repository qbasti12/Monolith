# Monolith – Fortschritt (`PROGRESS.md`)

## Aktueller Stand
- **Meilenstein:** M0–M8 sowie vollständiger Audit-Sanierungsplan ([`docs/AUDIT_AND_FIX_PLAN.md`](docs/AUDIT_AND_FIX_PLAN.md), Phasen 1–4) komplett abgeschlossen und verifiziert.
- **Offene Probleme:** Keine. Alle 13 identifizierten Audit-Punkte (`BUG-ENG-01..05`, `BUG-CORE-01..03`, `BUG-TUI-01..05`) sind behoben und durch Regressionstests abgesichert.
- **Nächster Schritt:** Abgeschlossen.

## Audit & Sanierungsdokumentation
- Master-Dokument: [`docs/AUDIT_AND_FIX_PLAN.md`](docs/AUDIT_AND_FIX_PLAN.md)
  - [x] Phase 1: Engine & UCI Concurrency (`BUG-ENG-01` bis `BUG-ENG-05`).
  - [x] Phase 2: Core Parsing & Edge Cases (`BUG-CORE-01` bis `BUG-CORE-03`).
  - [x] Phase 3: TUI Robustness, IPC, Theming (`BUG-TUI-01` bis `BUG-TUI-05`).
  - [x] Phase 4: CI & Release Test-Gates (`./scripts/check.sh` & `./scripts/smoke.sh`).

## Messwerte & Benchmarks
- *Perft-NPS:* `242.31 MN/s` (Startposition d1–5, 5.072.212 Nodes in 0.021 s, Release)
- *Suche-NPS (Startposition, Tiefe 12):* `3.741.514 NPS` (`3.74 MN/s`, 1.279.598 Nodes in 342 ms, Seldepth 24, PV `e2e4 d7d5 ...`, Release)
- *Elo-Selbstspiel (1200 vs. 2400, 20 Partien):* `W=20, D=0, L=0` für Elo 2400 (`100.0%` Score, <= 40 Züge/Partie, 14.23 s)

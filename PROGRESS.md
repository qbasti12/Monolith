# Monolith – Fortschritt (`PROGRESS.md`)

## Aktueller Stand
- **Meilenstein:** M0–M8 abgeschlossen & Tiefen-Audit durchgeführt.
- **Audit-Ergebnis:** Vollständige Codebase-Analyse abgeschlossen. 10 spezifische Schwachstellen, Randfälle und Race-Conditions (UCI-Protokoll, SAN/FEN-Parsing, TUI-Engine-Rennen, TrueColor-Fallback, UTF-8 Truncation) identifiziert und in [`docs/AUDIT_AND_FIX_PLAN.md`](docs/AUDIT_AND_FIX_PLAN.md) detailliert dokumentiert.
- **Nächster Schritt:** Phasenweise Abarbeitung des Sanierungsplans gemäß `docs/AUDIT_AND_FIX_PLAN.md`.

## Audit & Sanierungsdokumentation
- Master-Dokument: [`docs/AUDIT_AND_FIX_PLAN.md`](docs/AUDIT_AND_FIX_PLAN.md)
  - 10 inventarisierte Bugs mit exakter Zeilennummer, Ursache, Reproduktion und Code-Fix.
  - Phase 1: Engine & UCI Concurrency (BUG-ENG-01 bis BUG-ENG-05).
  - Phase 2: Core Parsing & Edge Cases (BUG-CORE-01 bis BUG-CORE-03).
  - Phase 3: TUI Robustness, IPC, Theming (BUG-TUI-01 bis BUG-TUI-05).
  - Phase 4: CI & Release Test-Gates.

## Messwerte & Benchmarks
- *Perft-NPS:* `242.31 MN/s` (Startposition d1–5, 5.072.212 Nodes in 0.021 s, Release)
- *Suche-NPS (Startposition, Tiefe 12):* `3.741.514 NPS` (`3.74 MN/s`, 1.279.598 Nodes in 342 ms, Seldepth 24, PV `e2e4 d7d5 ...`, Release)
- *Elo-Selbstspiel (1200 vs. 2400, 20 Partien):* `W=20, D=0, L=0` für Elo 2400 (`100.0%` Score, <= 40 Züge/Partie, 14.23 s)

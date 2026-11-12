# Monolith – Fortschritt (`PROGRESS.md`)

## Aktueller Stand
- **Meilenstein:** M0–M8, vollständiger Audit-Sanierungsplan ([`docs/AUDIT_AND_FIX_PLAN.md`](docs/AUDIT_AND_FIX_PLAN.md), Phasen 1–4) sowie grafischer GUI-Client (`-gui`) und vergrößerte TUI-Figuren (`-tui`) komplett abgeschlossen und verifiziert.
- **Offene Probleme:** Keine. Alle Gates, Builds und Release-Tests sind grün.
- **Nächster Schritt:** Bereit für den Produktiveinsatz.

## GUI & TUI Erweiterung
- [x] **Grafischer Client (GUI):** Native Desktop-Oberfläche via `eframe` / `egui`, aufrufbar mit `-gui` / `--gui` oder Standardstart.
  - Vektor-Rendering aller 6 Figurentypen mit sauberen Silhouetten, Konturen und skalierbarer Darstellung bis zu 960x960 px.
  - Volle Mausinteraktion: Click-to-Move, Legale Zugziel-Markierungen (Ringe für Schläge, Punkte für ruhige Züge), Schachuhr-/Schach-Indikator, Letzter-Zug-Highlight.
  - Interaktiver Umwandlungs-Dialog für Damen, Türme, Läufer und Springer.
  - Live-Bewertungsbalken (Siegwahrscheinlichkeits-Gauge, Zentibauern / Mattanzeige).
  - Multi-Engine-Telemetry: Bis zu 3 Engines parallel aktivierbar mit individuellen Elo- und Tiefen-Limits, Live-Metriken (Tiefe, Nodes, NPS, Hashfull, Bester Zug, PV-Zeile).
  - Zugverlaufs-Tabelle mit SAN, Klassifizierungs-Badges (`!!`, `!`, `★ Best`, `✓ Good`, `?!`, `?`, `??`) und Klick-Navigation zu jedem Halbzug.
  - FEN- und PGN-Modals (Presets für Startposition, Kiwipete, 15 Damen sowie Morphy Opera Game mit taktischem Engine-Review).
- [x] **Vergrößerte TUI-Figuren:** `cell_h` im Terminal von 3 auf bis zu 5 Zeilen erweitert, proportionale Breitenanpassung (`cell_w`) und zentrierte prominente Glyphen.
- [x] **CLI-Selektor & Binaries:** Unterstützt `-gui` / `--gui` und `-tui` / `--tui`. Beide Binary-Namen `monolith` und `monolith-tui` kompilieren und delegieren auf denselben sauberen Entrypoint. Fallback auf TUI in Headless-Umgebungen.

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

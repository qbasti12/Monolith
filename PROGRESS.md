# Monolith – Fortschritt (`PROGRESS.md`)

## MVP-Upgrade-Phasen (Client T1 & Logs T2)

| Phase | Thema | Status | Gate-Ergebnis | Offene Punkte |
|---|---|---|---|---|
| Phase 0 | Audit & `GAP.md` | Abgeschlossen | `GAP.md` existiert, `cargo build --release` grün | — |
| Phase 1 | Brett & Eingabe | Aktiv | — | Layout-Logik trennen (`BoardLayout`, `GuiBoardLayout`), Texteingabe GUI, Tests |
| Phase 2 | Eval-Bar & Zugliste | Ausstehend | — | Sigmoid-Abgleich GUI, `EvalBarLayout`, Unit-Tests Eval-Bar & Navigation |
| Phase 3 | Engine-Panel & Konfigurator | Ausstehend | — | Elo-Slider `800..=3200`, Binary-Restart, Fake-UCI-Engine-Test |
| Phase 4 | FEN-Menü & Presets | Ausstehend | — | Test auf unveränderten State bei ungültiger FEN |
| Phase 5 | PGN-Analyse | Ausstehend | — | Zielfeld-Klassifizierungs-Icon (TUI+GUI), 1-Satz-Erklärung GUI, Abbruch-Test |
| Phase 6 | Logs-Terminal & IPC (C1–C3) | Ausstehend | — | Nodes/NPS/Hashfull/SelDepth aus Client entfernen, `42424`, `last_move`/Time/MultiPV in T2 |
| Phase 7 | Politur & Abnahme | Ausstehend | — | Fehler-Statuszeile, GUI-Hilfe/Status, `README.md`, `GAP.md` abnehmen, `check.sh` |

## Historie (M0–M8 & GUI-Erweiterungen)
- **Meilenstein M0–M8 & Audit-Plan ([`docs/AUDIT_AND_FIX_PLAN.md`](docs/AUDIT_AND_FIX_PLAN.md)):** Abgeschlossen.
- **Grafischer Client (`-gui`) & vergrößerte TUI-Figuren (`-tui`):** Bereits integriert und funktionsfähig (bleiben gemäß Regel 1 erhalten).
- *Messwerte (Apple Silicon Release):* Perft d1–5 `242.31 MN/s`, Suche Tiefe 12 `3.74 MN/s` (`0` Heap-Allokationen), Elo 2400 vs 1200 `20W-0D-0L`.

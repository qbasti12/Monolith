# Monolith – Fortschritt (`PROGRESS.md`)

## MVP-Upgrade-Phasen (Client T1 & Logs T2)

| Phase | Thema | Status | Gate-Ergebnis | Offene Punkte |
|---|---|---|---|---|
| Phase 0 | Audit & `GAP.md` | Abgeschlossen | `GAP.md` existiert, `cargo build --release` grün | — |
| Phase 1 | Brett & Eingabe | Abgeschlossen | `app_tests` (5/5) + `gui` (5/5) + `clippy` grün | — |
| Phase 2 | Eval-Bar & Zugliste | Abgeschlossen | `eval_to_win_prob`, `compute_eval_bar_layout` & Navigation-Tests + `clippy` grün | — |
| Phase 3 | Engine-Panel & Konfigurator | Abgeschlossen | `fake_uci_test` (Fake-UCI Parsing + Live-Konfig) + `clippy` grün | — |
| Phase 4 | FEN-Menü & Presets | Abgeschlossen | `headless_app_moves_undo_flip_and_fen_presets` (`F1/F2/F3` + Bad-FEN State-Preservation) + `clippy` grün | — |
| Phase 5 | PGN-Analyse | Abgeschlossen | `pgn_background_analysis` + Abbruch-Konsistenz + Zielfeld-Badges (TUI+GUI) + `clippy` grün | — |
| Phase 6 | Logs-Terminal & IPC (C1–C3) | Abgeschlossen | `ipc_tests` (4/4), `client_ui_excludes_internal_telemetry`, `gui` (5/5) + `clippy` grün | — |
| Phase 7 | Politur & Abnahme | Abgeschlossen | `./scripts/check.sh` (43/43 Tests + `clippy` + `fmt`) & `./scripts/smoke.sh` grün | — |

## Historie (M0–M8 & GUI-Erweiterungen)
- **Meilenstein M0–M8 & Audit-Plan ([`docs/AUDIT_AND_FIX_PLAN.md`](docs/AUDIT_AND_FIX_PLAN.md)):** Abgeschlossen.
- **Grafischer Client (`-gui`) & vergrößerte TUI-Figuren (`-tui`):** Bereits integriert und funktionsfähig (bleiben gemäß Regel 1 erhalten).
- *Messwerte (Apple Silicon Release):* Perft d1–5 `242.31 MN/s`, Suche Tiefe 12 `3.74 MN/s` (`0` Heap-Allokationen), Elo 2400 vs 1200 `20W-0D-0L`.

# Monolith – MVP-Gap-Analyse (`GAP.md`)

Stand: Phase 0 Audit (Basis: vollständiges Lesen aller Dateien in `monolith-tui/src/`).

## 1. MUST-Anforderungen (Abschnitt B) & Architektur-Regeln (C, D, Regel 3)

| # | Anforderung | Status | Dateiverweis | Festgestellte Lücke / Handlungsbedarf |
|---|---|---|---|---|
| M1 | Brett mit Koordinaten, Flip, Unicode-Figuren (TUI) & Vektor-Figuren (GUI) | vorhanden | [ui.rs](monolith-tui/src/ui.rs), [gui_board.rs](monolith-tui/src/gui_board.rs), [gui_pieces.rs](monolith-tui/src/gui_pieces.rs) | Layout-Mathe (`BoardLayout`, `GuiBoardLayout`) vom Rendering entkoppelt und headless getestet. |
| M2 | Zugeingabe: Cursor/Pfeiltasten + Enter, Text (`e2e4` / `Nf3`), nur legale Züge, Promotion-Abfrage, Undo | vorhanden | [input.rs](monolith-tui/src/input.rs), [app.rs](monolith-tui/src/app.rs), [gui_board.rs](monolith-tui/src/gui_board.rs), [gui_panels.rs](monolith-tui/src/gui_panels.rs) | TUI- und GUI-Texteingabe (`e2e4` / `Nf3`), legale Validierung, Promotion und Undo vollständig. |
| M3 | Hervorhebung: Letzter Zug, empfohlener Zug, legale Zielfelder der gewählten Figur | vorhanden | [ui.rs](monolith-tui/src/ui.rs), [gui_board.rs](monolith-tui/src/gui_board.rs) | TUI (`is_last`, `is_best`, `•`) und GUI (Feldfarben, Punkte/Ringe, Best-Move-Pfeile) vollständig implementiert. |
| M4 | Eval-Bar (Sigmoid `1 / (1 + exp(-0.004 * cp))`, `+1.45` / `M3`, folgt Flip) | vorhanden | [theme.rs](monolith-tui/src/theme.rs), [ui.rs](monolith-tui/src/ui.rs), [gui_panels.rs](monolith-tui/src/gui_panels.rs) | Einheitliche Sigmoid-Kurve in TUI & GUI via `compute_eval_bar_layout`, Flip-konforme Textposition, Unit-Tests (`cp=0`, `Mate`, `Flip`). |
| M5 | Zugliste mit Navigation (`<-` / `->`, Anfang/Ende) | vorhanden | [ui.rs](monolith-tui/src/ui.rs), [input.rs](monolith-tui/src/input.rs), [app.rs](monolith-tui/src/app.rs), [gui_panels.rs](monolith-tui/src/gui_panels.rs) | SAN-Zugliste, Klick-/Tastaturnavigation (`[`/`]`/`Home`/`End`/`<-`/`->`) und Unit-Tests vollständig. |
| M6 | Engine-Panel: 1–3 Engines, Name, Elo, Tiefe, Eval, „Bester Zug für dich“, „Erwartete Antwort“ (PV[1]), PV-Zeile | vorhanden | [engines.rs](monolith-tui/src/engines.rs), [ui.rs](monolith-tui/src/ui.rs), [gui_panels.rs](monolith-tui/src/gui_panels.rs), [fake_uci_test.rs](monolith-tui/tests/fake_uci_test.rs) | Fake-UCI-Integrationstest prüft `info`-Parsing, Panel-Updates & Live-`setoption`. (Telemetrie-Trennung folgt in Phase 6). |
| M7 | Engine-Konfigurator: Anzahl, Elo `800..=3200`, Tiefe `1..=30` live änderbar | vorhanden | [modals.rs](monolith-tui/src/modals.rs), [input.rs](monolith-tui/src/input.rs), [gui_panels.rs](monolith-tui/src/gui_panels.rs), [engines.rs](monolith-tui/src/engines.rs) | GUI-Slider `800..=3200` angeglichen; `sync_slots` startet Prozess bei Binary-Wechsel neu. |
| M8 | Custom-Position: FEN einfügen (mit Fehleranzeige), Presets (Start, Infinite Queens, taktisches Mittelspiel) | teilweise | [app.rs](monolith-tui/src/app.rs), [modals.rs](monolith-tui/src/modals.rs), [gui_modals.rs](monolith-tui/src/gui_modals.rs) | Funktional komplett; explizite Test-Assertion fehlt, dass ungültige FEN den bestehenden Board-/History-State unverändert lässt. |
| M9 | PGN-Import + Analyse: Zug für Zug, Klassifizierung (`!!`, `!`, `Best`, `?!`, `?`, `??`) + 1-Satz-Erklärung, Fortschritt, Icon in Zugliste **und auf dem Zielfeld** | teilweise | [analysis.rs](monolith-tui/src/analysis.rs), [ui.rs](monolith-tui/src/ui.rs), [gui_board.rs](monolith-tui/src/gui_board.rs), [gui_panels.rs](monolith-tui/src/gui_panels.rs) | **Klassifizierungs-Icon auf dem Zielfeld fehlt** in TUI und GUI; 1-Satz-Erklärung fehlt im GUI-Partien-Panel; Abbruch-Konsistenztest (`cancel_pgn_analysis`) fehlt. |
| M10 | Hilfe-Overlay (`[?]`), Quit (`[Q]`), Statuszeile mit Fehlern | teilweise | [modals.rs](monolith-tui/src/modals.rs), [ui.rs](monolith-tui/src/ui.rs), [app.rs](monolith-tui/src/app.rs), [gui.rs](monolith-tui/src/gui.rs) | TUI-Hilfe & Quit vorhanden; Fehlerstatus wird in `ui.rs` nicht rot hervorgehoben, Engine-Fehler werden nicht in Statuszeile propagiert; GUI fehlen Statuszeile und Hilfe-Dialog. |
| M11 | Telemetrie-Trennung (C1/C3/D): Nodes, NPS, Hashfull, SelDepth, rohe UCI-Zeilen, Eval-Aufschlüsselung **nur in Terminal 2 (`--mode logs`)** auf `127.0.0.1:42424` | fehlt | [ui.rs](monolith-tui/src/ui.rs), [gui_panels.rs](monolith-tui/src/gui_panels.rs), [gui_settings.rs](monolith-tui/src/gui_settings.rs), [logs.rs](monolith-tui/src/logs.rs) | **Client zeigt noch Nodes, NPS, SelDepth (`ui.rs` & `gui_panels.rs`) und Hashfull (`gui_panels.rs`)**; `gui_settings.rs` zeigt Port `53333` statt `42424`; `logs.rs` zeigt `last_move`, `MultiPV` & `Time` noch nicht explizit an. |
| M12 | Dokumentation (`README.md`) & Abnahme-Gates | teilweise | [README.md](README.md), [PROGRESS.md](PROGRESS.md), [DECISIONS.md](DECISIONS.md) | `README.md` dokumentiert `-gui`/`-tui` und finale T1/T2-Telemetrie-Trennung noch nicht. |

---

## 2. Abgeleitete Phasenreihenfolge

Da alle Phasen 1–7 noch konkrete Lücken (Layout-Trennung, Telemetrie-Isolation C1/C3, Zielfeld-Badges, Fake-UCI-/Navigation-/Abbruch-Tests, Politur) aufweisen, werden sie in logischer Abhängigkeit abgearbeitet:

**Reihenfolge:** `Phase 0 (Audit) -> Phase 1 (Brett & Eingabe) -> Phase 2 (Eval-Bar & Zugliste) -> Phase 3 (Engine-Panel & Konfigurator) -> Phase 4 (FEN-Menü & Presets) -> Phase 5 (PGN-Analyse) -> Phase 6 (Logs-Terminal & IPC / Telemetrie-Trennung) -> Phase 7 (Politur & Abnahme)`

> Hinweis zu bestehendem Code: Vorhandene GUI-Erweiterungen (Eval-Graph, Rechtsklick-Pfeile, Drag & Drop, Sound, Opening Explorer, Trainer) bleiben gemäß Regel 1 & „NICHT TUN“ unberührt funktionsfähig; während der Phasen 1–7 werden keine *neuen* SHOULD/LATER-Features ergänzt.

# Monolith – Architekturentscheidungen (`DECISIONS.md`)

## 1. Crate-Versionen (geprüft via `cargo search` / `cargo info`)
- `ratatui = "0.30.2"` (mit Feature `crossterm_0_29`, da `ratatui-crossterm` auf `crossterm 0.29` abbildet)
- `crossterm = "0.29.0"`
- `serde = "1.0.229"` (mit `derive`)
- `serde_json = "1.0.154"`
- `thiserror = "2.0.21"`

Alle Zielversionen existieren auf crates.io und werden exakt über das Workspace-Manifest gebunden.

## 2. `panic = "abort"` im Release-Profil und Tests
Da `[profile.release]` `panic = "abort"` setzt, dürfen Tests unter `cargo test --release` kein `#[should_panic]` verwenden. Alle Fehlerpfade werden über `Result<_, Error>` abgebildet und mittels `assert!(res.is_err())` getestet.

## 3. Verzicht auf `unsafe`
In sämtlichen Crates wird auf `unsafe` verzichtet, da feste Stack-Arrays (`[Move::NONE; 512]`, `[UndoState; 2048]`) mit `Copy`-Typen sicher und ohne Laufzeit-Overhead initialisierbar sind und `OnceLock` sicheren Zugriff auf vorberechnete Magic-Bitboard- und Zobrist-Tabellen liefert.

## 4. `UciProcess` im `monolith-engine`-Crate
Der externe UCI-Runner (`UciProcess`) wird in `monolith-engine` bereitgestellt und exportiert, sodass sowohl Engine-Integrationstests (z. B. Selfplay/Prozess-Tests) als auch `monolith-tui` dieselbe robuste Kindprozess-Abstraktion verwenden können.

## 5. GUI-Client & `eframe = "0.31.1"`
Für den geforderten grafischen GUI-Modus (`-gui` / `--gui`) mit Mausinteraktion, skalierbaren großen Figuren und parallelem Multi-Engine-Betrieb wird `eframe` (egui) in Version 0.31.1 verwendet. Dies erlaubt eine native macOS/Linux/Windows-Grafikausgabe ohne externe C-Bibliotheken, während der TUI-Modus (`-tui` / `--tui`) unverändert über Ratatui und Crossterm lauffähig bleibt.

## 6. Figma Design Integration
Für das gemeinsame Gestalten und Abstimmen der GUI wurde ein dediziertes Figma Design-File (*"Monolith Chess Client"*) im Team erstellt. Die Benutzeroberfläche (Catppuccin Mocha Farbpalette, linke Navigations-Sidebar, 8x8 Vektorschachbrett mit Staunton/Neo Figuren, Bewertungsbalken, Spielerkarten, Zughistorie und Engine-Telemetrie) wurde über die offizielle Figma Integration als Code-/Design-Ebene übertragen.

## 7. SVG-Figurensätze via `egui_extras = "0.31.1"`
Um die geforderten hochqualitativen Lichess-Figuren (*cburnett*, *staunty*) und den maßgeschneiderten minimalistischen Vektorsatz (*monolith_minimal*) nativ und gestochen scharf darzustellen, wurde `egui_extras = "0.31.1"` mit Feature `svg` eingebunden. Sämtliche Figuren werden zur Kompilierzeit als SVG-Bytes eingebunden, wodurch zur Laufzeit weder externe Netzwerkzugriffe noch Dateisystem-Lookups nötig sind.

## 8. Inkrementelles MVP-Upgrade & Telemetrie-Trennung (Terminal 1 vs. Terminal 2)
Gemäß dem Implementierungsplan wurden die bestehenden grafischen `-gui`-Erweiterungen (Drag & Drop, Rechtsklick-Markierungen/Pfeile, Eval-Graph, Opening-Explorer-Platzhalter, Zug-Trainer „Finde den besten Zug“, Multi-Engine-Farben und synthetisierte Sound-Effekte) **nicht** neu geschrieben oder entfernt, sondern inkrementell auf den formalen MVP-Stand (Sections A–D) gehoben:
- Geometrie-Trennung (`BoardLayout` / `compute_board_layout` in `theme.rs`, `GuiBoardLayout` in `gui_board.rs`, `EvalBarLayout` / `compute_eval_bar_layout` in `theme.rs`) ermöglicht rein headless-testbare Layout-Prüfungen getrennt vom Rendering.
- **Strikte Telemetrie-Isolation (C1/C3/D):** Terminal 1 (`--mode client`, sowohl `-tui` als auch `-gui`) zeigt ausschließlich spielerrelevante Ausgaben (Eval, kleine Tiefen-Statuszahl, bester Zug, erwartete Antwort und Hauptvariante). Technische Such-Metriken (`Nodes`, `NPS`, `Hashfull`, `SelDepth`, `MultiPV`, `Time`, rohe UCI-Zeilen, `EvalBreakdown`, `FEN` + `last_move`) werden exklusiv über Loopback-UDP (`127.0.0.1:42424`) in Terminal 2 (`--mode logs`) dargestellt.

## 9. Standardnutzer-GUI-Redesign (`-gui`) bei unverändertem `-tui` & `--mode logs`
Um `-gui` klar vom Terminal-Gefühl abzugrenzen und ein einheitliches, aufgeräumtes Chess.com-Desktop-Studio bereitzustellen, wurde ausschließlich die grafische Frontend-Schicht (`gui*.rs`) neu strukturiert – ohne das Verhalten von `-tui` oder `--mode logs` zu verändern:
- **Design-System & Vektor-Icons (`gui_theme.rs`, `gui_widgets.rs`):** Flaches Catppuccin-Mocha-Farbsystem ohne Verläufe, proportionale Sans-Typografie als Standard (`monospace_ui: false` statt Terminal-Monospace), einheitliche Buttons (`ButtonVariant`), Karten (`ui_card`), Badges (`ui_pill`) sowie eigens gezeichnete geometrische Vektor-Icons (`IconKind`, keine Emojis, keine ASCII-Klammern wie `[?]` oder `|<<`).
- **Zentrierte Chess.com-Bühne (`gui_stage.rs`, `gui_board.rs`):** Integrierter Eval-Balken direkt links neben dem Brett sowie obere/untere Spielerkarten mit Avatar-Icon, Name, Elo-Pill, geschlagenen Figuren, Materialvorteil (`compute_material_summary`) und Zug-/Denkstatus. Zusätzlich drei Schachbrett-Themes (`MochaSlate`, `ChessSage`, `MochaWalnut`).
- **Strukturierte Review- & Engine-Module (`gui_analysis_widgets.rs`, `gui_panels.rs`, `gui_engines.rs`):** Zweispaltige Zugliste ohne Kastenränder, Navigation-Leiste mit Vektor-Icons, Game Review mit Coach-Karte, Accuracy-%-Anzeige (`compute_side_accuracies`) und 7-Klassen-Zusammenfassung (`classification_summary`) sowie ausgelagerte Multi-Engine-Konfiguration (Slots 1–3).

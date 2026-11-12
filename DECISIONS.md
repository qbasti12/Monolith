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

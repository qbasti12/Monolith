# Monolith – Fortschritt (`PROGRESS.md`)

## Aktueller Stand
- **Meilenstein:** M0 abgeschlossen (`cargo build --release` + `scripts/check.sh` grün). Aktuell in Arbeit: M1 (Core: Typen, Bitboards, Zobrist, Board, FEN).
- **Offene Probleme:** Keine.
- **Nächster Schritt:** `types.rs`, `bitboard.rs`, `zobrist.rs`, `board.rs`, `fen.rs` implementieren und M1-Gate-Tests bestehen.

## Messwerte & Benchmarks
- *Perft-NPS:* (folgt nach M2)
- *Suche-NPS (Startposition, Tiefe 12):* (folgt nach M5/Abschluss)
- *Elo-Selbstspiel (1200 vs. 2400, 20 Partien):* (folgt nach M6)

# Monolith – Fortschritt (`PROGRESS.md`)

## Aktueller Stand
- **Meilenstein:** M1 abgeschlossen (FEN-Roundtrip, Invalid-FEN `Err`, Magic Bitboards, Zobrist, make/unmake, Draw-Detection). Aktuell in Arbeit: M2 (Movegen + Perft).
- **Offene Probleme:** Keine.
- **Nächster Schritt:** `movegen.rs` und `tests/perft.rs` implementieren und alle Perft-Positionen (Start d1–5, Kiwipete d1–4, Pos 3–6, Infinite Queens) exakt verifizieren.

## Messwerte & Benchmarks
- *Perft-NPS:* (folgt nach M2)
- *Suche-NPS (Startposition, Tiefe 12):* (folgt nach M5/Abschluss)
- *Elo-Selbstspiel (1200 vs. 2400, 20 Partien):* (folgt nach M6)

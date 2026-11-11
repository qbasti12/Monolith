# Monolith – Fortschritt (`PROGRESS.md`)

## Aktueller Stand
- **Meilenstein:** M2 abgeschlossen (alle 6 Perft-Positionen exakt bestanden, Infinite Queens & > 256 Züge bestanden). Aktuell in Arbeit: M3 (PGN, SAN & Zug-Klassifizierung).
- **Offene Probleme:** Keine.
- **Nächster Schritt:** `pgn.rs` (SAN-Parser/-Generator, PGN-Reader, `classify_move`) implementieren und testen.

## Messwerte & Benchmarks
- *Perft-NPS:* `242.31 MN/s` (Startposition d1–5, 5.072.212 Nodes in 0.021 s, Release)
- *Suche-NPS (Startposition, Tiefe 12):* (folgt nach M5/Abschluss)
- *Elo-Selbstspiel (1200 vs. 2400, 20 Partien):* (folgt nach M6)

# Monolith – Fortschritt (`PROGRESS.md`)

## Aktueller Stand
- **Meilenstein:** M4 abgeschlossen (PeSTO Eval + Symmetrie + Infinite Queens, Transpositionstabelle). Aktuell in Arbeit: M5 (Suche).
- **Offene Probleme:** Keine.
- **Nächster Schritt:** `search.rs` implementieren (ID, Alpha-Beta, PVS, Aspiration, Quiescence, Move Ordering, NMP, RFP, LMR, Check-Extensions, Zero-Alloc-Nachweis) und Taktik-/Matt-Suite bestehen.

## Messwerte & Benchmarks
- *Perft-NPS:* `242.31 MN/s` (Startposition d1–5, 5.072.212 Nodes in 0.021 s, Release)
- *Suche-NPS (Startposition, Tiefe 12):* (folgt nach M5/Abschluss)
- *Elo-Selbstspiel (1200 vs. 2400, 20 Partien):* (folgt nach M6)

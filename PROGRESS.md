# Monolith – Fortschritt (`PROGRESS.md`)

## Aktueller Stand
- **Meilenstein:** M5 abgeschlossen (Mate-in-1/2/3, Taktik-Suite 15/16 = 93 %, Infinite Queens < 5 s, 0 Heap-Allokationen in `search()` nachgewiesen). Aktuell in Arbeit: M6 (Skill, UCI, externer UCI-Runner).
- **Offene Probleme:** Keine.
- **Nächster Schritt:** `skill.rs`, `uci.rs`, `main.rs` implementieren und M6-Gate (UCI-Piping + Elo 1200 vs. 2400 Selbstspiel) bestehen.

## Messwerte & Benchmarks
- *Perft-NPS:* `242.31 MN/s` (Startposition d1–5, 5.072.212 Nodes in 0.021 s, Release)
- *Suche-NPS (Startposition, Tiefe 12):* (folgt nach M6/Abschluss)
- *Elo-Selbstspiel (1200 vs. 2400, 20 Partien):* (folgt nach M6)

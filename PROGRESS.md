# Monolith – Fortschritt (`PROGRESS.md`)

## Aktueller Stand
- **Meilenstein:** M3 abgeschlossen (SAN-Roundtrip auf allen Perft-Positionen, PGN-Parser, Zug-Klassifizierung). Aktuell in Arbeit: M4 (Engine Eval + Transpositionstabelle).
- **Offene Probleme:** Keine.
- **Nächster Schritt:** `eval.rs` (PeSTO + strukturelle Terme + `eval_breakdown`) und `tt.rs` implementieren und M4-Gate-Tests bestehen.

## Messwerte & Benchmarks
- *Perft-NPS:* `242.31 MN/s` (Startposition d1–5, 5.072.212 Nodes in 0.021 s, Release)
- *Suche-NPS (Startposition, Tiefe 12):* (folgt nach M5/Abschluss)
- *Elo-Selbstspiel (1200 vs. 2400, 20 Partien):* (folgt nach M6)

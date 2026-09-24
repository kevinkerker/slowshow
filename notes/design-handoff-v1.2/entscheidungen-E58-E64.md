# Entscheidungsvorschläge E-58 bis E-64

Zum Eintragen in `lastenheft.md`, Abschnitt 9. Stand 24.09.2026, vorgeschlagen von Claude Design, **von Kevin zu bestätigen**. E-63 und E-64 gehen über die offenen Fragen der Übergabe hinaus.

| E-Nr. | Frage | Entscheidung | Begründung |
| --- | --- | --- | --- |
| E-58 | F4 Knopfsystem | Option A „Fläche und Rand“: Primär = Messingfläche, Sekundär = Rand, Gefahr = Rand + Fehlerfarbe, Ghost = nur Text, Textverweis = Messingtext, Symbolknopf = runder Rand. Höhe 48 px. Gedrückt: Primär `--ss-accent-hover`, sonst `--ss-surface-accent`. Gesperrt 40 %. Beschäftigt: Kreisel vor der Aufschrift, Breite bleibt. | Entspricht den Android-Varianten Filled / Outlined / Text / Icon. Nur eine helle Fläche je Ansicht. |
| E-59 | F5 Rückfragedialog | Option A: eigener Dialog in der Mitte statt `confirm()`. Titel als Frage, Text darunter, Aktionen rechts: „Abbrechen“ (Ghost) links neben der bestätigenden Aktion (Gefahr). Bei drei Ausgängen Knöpfe untereinander, jede Aufschrift sagt, was passiert. | Android-Muster, löst den Missbrauch von OK/Abbrechen bei „Absender entfernen“. |
| E-60 | F6 Skala | Option C: vier UI-Stufen 12 / 14 / 16 / 22 plus Anzeigegrößen; Abstände im 8er-Raster. | Entspricht der Material-Typografie; Hinweise steigen von 13 auf 14 px. |
| E-61 | F7 Radien | Option B: 8 / 12 / 16 + Pille. | Material Small / Medium / Large; nächste am Code. |
| E-62 | F8 Rückmeldungen | Option B: neben dem Auslöser, Erfolg in `--ss-accent` verblasst nach 6 s, Fehler in `--ss-error` bleibt bis zum nächsten Erfolg. `aria-live="polite"`, Fehler `assertive`. | Eine Regel für alle Bereiche; E-33 bleibt gewahrt. |
| E-63 | — | Tippziel 48 px statt 44. `--ss-text-dim` → `#858177`, `--ss-text-faint` → `#7d7970`. | Android-Richtlinie 48 dp; beide Textstufen lagen unter 4,5 : 1. Nachtmodus ausgenommen. |
| E-64 | — | Helles Erscheinungsbild für Einstellungen und Dialoge, gesteuert über `prefers-color-scheme`, ohne eigene Einstellung. Diashow, Nachtmodus, Start- und Leerzustand bleiben immer dunkel (`.ss-always-dark`). | Wunsch: Systemvorgabe übernehmen. **Hebt E-13 („nur dunkel“) für die Einstellungen auf.** |

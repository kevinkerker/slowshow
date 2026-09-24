# Handoff: Slowshow v1.2 Designsystem (Knöpfe, Skalen, Rückfrage, Rückmeldung, helles Erscheinungsbild)

Ablage im Repo: `notes/design-handoff-v1.2/` (dieser Ordner).

## Überblick

Das Design auf den Stand v1.1.0 (Commit `0dba581`) gebracht und die Grundlagen festgelegt, die vorher fehlten. Umzusetzen sind:

1. **Tokens** aus `tokens.proposed.css` übernehmen (ersetzt `src/styles/tokens.css`).
2. **Knopfsystem** (E-58) in allen Bereichen vereinheitlichen.
3. **Eigener Rückfragedialog** (E-59) statt `window.confirm()`.
4. **Rückmeldungen** (E-62) einheitlich setzen.
5. **Helles Erscheinungsbild** (E-64) für Einstellungen und Dialoge.
6. Entscheidungen aus `entscheidungen-E58-E64.md` ins Lastenheft eintragen, Texte aus `texte-de.md` in `de.json`/`en.json`.

**Voraussetzung:** Kevin bestätigt E-58 bis E-64. E-64 hebt E-13 teilweise auf.

## Über die Design-Dateien

Die Dateien in `canvas/` sind **Entwurfsvorlagen in HTML** und kein Produktionscode. Sie werden in Vue 3 nachgebaut, mit den vorhandenen Komponenten (`SettingRow.vue`, `ToggleSwitch.vue`, `SourceCard.vue`, `SourceDialog.vue`, Panes) und nur mit `var(--ss-*)`-Tokens. `canvas/` ersetzt `slowshow-app-design.html` als Canvas: eine `.dc.html` je Artboard und dazu `canvas.json`. Die Übersicht ist `Slowshow App-Design.dc.html`.

## Genauigkeit

**High-Fidelity** für alles unter „Vorschlag“ und „Thema“. Die Artboards D/S/G zeigen den **Ist-Stand v1.1.0** mit Befundmarkern (blau, nicht Teil des Designs). Die Optionsboards F4–F8 sind Grundlage der Entscheidung und werden nur umgesetzt, soweit sie gewählt sind.

## Umsetzung im Einzelnen

### 1. Tokens

- `tokens.proposed.css` → `src/styles/tokens.css`. Alle harten Werte in den Komponenten auf Tokens umstellen:
  - `rgba(0,0,0,.72)` in `ImagesPane.vue`, `SystemPane.vue` und `SourceDialog.vue` → `var(--ss-dialog-scrim)`
  - `rgba(10,10,10,.82)` (Pillen, Abzeichen) → `var(--ss-overlay-scrim)`
  - Verläufe unter Uhr und Kachel → `var(--ss-clock-scrim)` / `var(--ss-tile-scrim)`
  - `--ss-surface-2` (undefiniert, `SourceDialog.vue:1015`) → `var(--ss-surface-accent)`
- **Schriftgrößen** abbilden: 10, 11, 12 → `--ss-fs-s` · 13, 14, `0.85rem` → `--ss-fs-m` · 15, 16, 17 → `--ss-fs-l` · 22 → `--ss-fs-xl`. Die Anzeigegrößen (Uhr, Nachtuhr, Wortmarke, Bildunterschrift) bekommen eigene Tokens.
- **Abstände:** Einstellungszeile 16/16 statt 14. Abschnitt 32 statt 28. Karte 20/24. Inhalt des Bereichs 32/48 statt 32/40. Navigation: Punkte 48 px hoch, 8 px Abstand, Innenabstand 32/16.
- **Radien:** `--ss-radius-card` → `--ss-radius-md`, `--ss-radius-nav` → `--ss-radius-sm`. Alle Dialoge `--ss-radius-lg`, auch der Quellendialog (heute 12). Symbolkachel 12 statt 10.
- **Tippziel 48:** Kippschalter 52 × 32 (Knopf 24, Rand 4), Segmente 42 hoch plus 3 px Rahmenabstand, Eingaben 48 hoch mit 16 px Schrift, alle runden Knöpfe 48.

### 2. Knöpfe (E-58) · Vorlagen `Vorschlag-Knoepfe`, `Thema-*-Knoepfe`

Eine Komponente `SsButton.vue`, Props `variant: 'primary'|'secondary'|'danger'|'ghost'|'link'`, `busy`, `disabled`. Dazu `SsIconButton.vue`.

| Variante | Normal | Gedrückt (`:active`) |
| --- | --- | --- |
| primary | Fläche `--ss-accent-fill`, Text `--ss-on-accent`, 500 | Fläche `--ss-accent-hover` |
| secondary | Rand 1 px `--ss-border-strong`, Text `--ss-text-body` | Fläche `--ss-surface-accent`, Text `--ss-text-accent` |
| danger | Rand `--ss-border-strong`, Text `--ss-error` | Rand `--ss-error`, Fläche `--ss-surface-accent` |
| ghost | kein Rand, Text `--ss-text-muted` | Fläche `--ss-surface-accent`, Text `--ss-text-accent` |
| link | nur Text `--ss-accent`, kein Innenabstand | Text `--ss-text-accent` |
| icon | 48 × 48, rund, Rand `--ss-border-strong`, Symbol `--ss-icon-soft` 20 px | Fläche `--ss-surface-accent`, Symbol `--ss-accent` |

Für alle gilt:
- Höhe `--ss-button-height`, Pille, 14 px, 500, Innenabstand 0 22 px, Abstand 8 px.
- **Gesperrt:** `opacity: var(--ss-opacity-disabled)`.
- **Beschäftigt:** 16-px-Kreisel in der Textfarbe vor der Aufschrift, die Breite bleibt. Beim Verbindungstest wechselt die Aufschrift zu „Prüfe …“. Beim Sync dreht der Symbolknopf sein Symbol in Messing.
- **Fokus:** 2 px `--ss-accent`, Abstand 2 px.

Heutige Abweichungen, die zu beheben sind:
- Freigabe-Primär hat Text `#14100a` und Gewicht 400.
- Der Primärknopf im Diagnosebericht ist ungestaltet.
- „Durchlauf neu starten“ ist bloßer Text.
- „Historie zurücksetzen“ und „Aufräumen“ sind nicht rot.
- Der Knopf im Leerzustand hat Messingrand: wird Primär.

### 3. Rückfragedialog (E-59) · Vorlagen `Vorschlag-Rueckfrage`, `Thema-*-Rueckfrage`

`SsConfirm.vue`, promise-basiert: `await confirm({ title, body, actions })`. `actions` ist eine Liste aus `{ id, label, variant }`. Rückgabe: `id` oder `'cancel'`.

- **Aufbau:** Hintergrund `--ss-dialog-scrim`, Tafel bis 520 px breit, `--ss-surface`, Rand `--ss-border`, Radius `--ss-radius-lg`, Schatten `--ss-shadow-dialog`, Innenabstand 24, Abstand 16.
- **Titel:** Display-Schrift 22, `--ss-text`.
- **Text:** 14 px, `--ss-text-dim`, Zeilenhöhe 1,55.
- **Zwei Ausgänge:** Knöpfe rechtsbündig, Reihenfolge [Abbrechen (ghost)] [Aktion (danger)].
- **Drei Ausgänge:** Knöpfe untereinander über die volle Breite: [Entfernen · Fotos bleiben sichtbar (secondary)], [Entfernen · {n} Fotos zurück in die Quarantäne (danger)], [Abbrechen (ghost)].
- **Abbrechen:** Hintergrund antippen und die Zurück-Taste von Android zählen als Abbrechen.
- **Ersetzt alle fünf `confirm()`-Aufrufe:** Quelle entfernen, Postfach neu abgleichen, Absender entfernen, Anzeige-Historie zurücksetzen, Datenbank aufräumen. Das Problem mit dem wörtlichen „\n“ entfällt dabei.

### 4. Rückmeldungen (E-62) · Vorlage `Vorschlag-Rueckmeldung`

`SsFeedback.vue` mit Props `kind: 'ok'|'error'` und `text`.

- **Ort:** direkt neben dem Auslöser, auf derselben Zeile.
- **Aussehen:** 14 px, Erfolg in `--ss-accent`, Fehler in `--ss-error`.
- **Erfolg** verschwindet nach `--ss-feedback-duration` (6 s) mit `opacity`, 0,2 s.
- **Fehler** bleibt stehen, bis ein Versuch gelingt.
- **Vorlesen:** `aria-live="polite"`, bei Fehlern `assertive`.

Heute weichen ab:
- Quellen 6 s dim, Diashow 6 s Messing, System 5 s dim.
- MQTT-Meldungen stehen unter „Konfiguration“: nach oben neben „Speichern“ bzw. „Neu verbinden“ holen.

### 5. Helles Erscheinungsbild (E-64) · Vorlagen `Thema-Tokens`, `Thema-Hell-*`

- **Umschalten:** nur über die Tokens in `@media (prefers-color-scheme: light)`. In den Komponenten ist keine Logik nötig, solange sie ausschließlich Tokens verwenden.
- **Immer dunkel:** Die Wurzel von `SlideshowView.vue`, der Nachtuhr, des Start- und des Leerzustands bekommt die Klasse `ss-always-dark`.
- **Übergang zwischen Diashow und Einstellungen:** Die Einstellungen öffnen im hellen Bild über der dunklen Diashow. Der Wechsel geschieht ohne Animation der Farben, nur mit der üblichen Einblendung über `opacity`.
- **Messing:** Als Fläche bleibt es gleich (`--ss-accent-fill`). Als Text und Symbol wird es im hellen Bild dunkler (`--ss-accent` `#82673a`). Komponenten dürfen deshalb `--ss-accent` **nicht** als Knopffläche verwenden.
- **Kippschalter:** an = `--ss-toggle-on` mit Knopf `--ss-toggle-knob-on` (im hellen Bild dunkles Messing mit hellem Knopf).
- **Auf dem Gerät prüfen:**
  - Gibt die Android-WebView unter Tauri `prefers-color-scheme` weiter? Nötig ist eventuell `WebSettings.setAlgorithmicDarkeningAllowed(false)` bzw. `forceDark` aus, damit Android nicht selbst umfärbt.
  - Die Farbe der Statusleiste bzw. `theme-color` muss mitziehen.

## Tokens

Vollständig in `tokens.proposed.css`, Übersicht auf dem Artboard `Vorschlag-Grundlagen`, hell und dunkel in `Thema-Tokens`.

## Assets

Keine neuen. Die Symbole bleiben die bisherigen Inline-SVGs mit 1,5er Strich. Fotos in den Vorlagen sind Verlaufsflächen als Platzhalter.

## Dateien

- `tokens.proposed.css`: Ersatz für `src/styles/tokens.css`
- `entscheidungen-E58-E64.md`: für `lastenheft.md` Abschnitt 9
- `texte-de.md`: neue, entfallende und zu korrigierende Texte
- `canvas/`: alle Artboards, Übersicht `Slowshow App-Design.dc.html`, Layout `canvas.json`

## Nachzuziehen

- `CLAUDE.md`: „Canvas mit 4 Artboards“ → neue Zahl und neuer Ort.
- Tests:
  - `i18n-keys.test.ts` um die neuen Schlüssel ergänzen.
  - Die Tests zu `SourceDialog` und `SystemPane` auf den neuen Dialog statt `confirm()` umstellen.

## Noch offen (Phase 3)

Hochformat H1–H3, F1–F3 und F9–F14 sind noch nicht entworfen und nicht Teil dieses Pakets.

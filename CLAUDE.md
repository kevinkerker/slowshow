# Slowshow — Claude-Projektanweisungen

Digitaler Bilderrahmen fuer Android-Tablets. Tauri 2 + Vue 3 + Rust.
Verbindliche Spezifikation: [lastenheft.md](lastenheft.md) — Anforderungs-IDs (FA-xx, NF-xx, RB-xx, E-xx, R-xx) im Code als Kommentar referenzieren.

## Architekturregel

**Geschaeftslogik gehoert nach Rust, nicht in die WebView.**
Bilddekodierung, Skalierung, Cache, Sync und Zeitsteuerung laufen im Rust-Backend (NF-12, NF-13, NF-14, FA-27, FA-31).
Das Frontend ist reine Darstellung und Bedienung. Grund: R-03 (WebView-OOM) und R-10 (Framework-Neutralitaet der Kernlogik).

Konkret: Nie ganze Bilddateien ueber IPC als Base64 schicken. Bilder kommen ausschliesslich ueber das
Asset-Protokoll `slowshow://img/<id>` aus dem Cache.

## Unit Tests

**Nach jeder Code-Aenderung direkt passende Unit Tests implementieren.**

- Rust: `#[cfg(test)]`-Modul in derselben Datei; Testrunner `cargo test`
- TypeScript pure functions -> eigene `.test.ts`-Datei neben der Quelle
- Vue-Komponenten mit Logik -> `@vue/test-utils`
- UI-Rendering ohne extrahierbare Logik -> explizit erwaehnen, warum kein Test noetig ist
- Testrunner Frontend: `vitest` (`npm test`)

## Nativer Android-Code

Handgeschriebener Kotlin-Code liegt **ausschliesslich** in `src-tauri/android-src/`.
`src-tauri/gen/` ist generiert und gitignored — dort nie direkt editieren.
Aenderungen per `npm run android:patch` einspielen.

## Design

Verbindlich ist E-13 und `slowshow-app-design.html` (Canvas mit 4 Artboards).
Farben und Typografie ausschliesslich ueber die Tokens in `src/styles/tokens.css`.
Schriften werden **lokal gebuendelt**, nie von Google Fonts geladen (NF-04, Offlinebetrieb FA-26).

## Commits

Nur committen wenn der User es explizit verlangt.

## Design-Entscheidungen

Immer Optionen vorlegen, nie selbst entscheiden. Getroffene Entscheidungen im
Entscheidungsprotokoll (lastenheft.md, Abschnitt 9) als neue E-Nummer nachtragen.

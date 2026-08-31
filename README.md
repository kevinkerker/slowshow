# Slowshow

Digitaler Bilderrahmen für Android-Tablets. Zeigt Fotos aus lokalen Ordnern,
vom NAS (WebDAV) und aus Nextcloud-Alben als Endlos-Diashow — ohne Werbung,
ohne Tracking, ohne Abo.

Gebaut mit Tauri 2 (Rust-Backend) und Vue 3. Verbindliche Spezifikation:
[lastenheft.md](lastenheft.md).

---

## Aufbau

Die Geschäftslogik liegt bewusst im Rust-Backend, nicht in der WebView. Das ist
keine Stilfrage: große Bilder in einer Android-WebView sprengen den
Arbeitsspeicher, und Android beendet die App dann kommentarlos (Risiko R-03 im
Lastenheft). Das Frontend bekommt ausschließlich fertig dekodierte, auf
Displayauflösung skalierte Bilder über ein eigenes Asset-Protokoll.

```text
src/                      Vue-3-Frontend — Darstellung und Bedienung
├── views/                Diashow und Einstellungen
├── components/           Bildbühne, Einblendungen, Quellenverwaltung
├── stores/               Pinia: Konfiguration und Ablauf der Diashow
├── lib/                  Tauri-Brücke, SAF-Zugriff, Formatierung
└── styles/               Design-Tokens aus slowshow-app-design.html

src-tauri/                Rust-Backend — der eigentliche Kern
├── src/
│   ├── model.rs          Konfiguration und Quellen
│   ├── config.rs         Persistenz          (FA-42, FA-45)
│   ├── secrets.rs        Zugangsdaten        (NF-05)
│   ├── decode.rs         Bilddekodierung     (FA-04, NF-12, NF-13)
│   ├── cache/            Rotierender Cache   (FA-26, FA-27)
│   ├── sources/          WebDAV, Nextcloud   (FA-21, FA-23)
│   ├── sync.rs           Delta-Sync          (FA-28, NF-14)
│   ├── playlist.rs       Reihenfolge         (FA-01, FA-03, FA-08)
│   ├── schedule.rs       Zeitplan            (FA-52 bis FA-54)
│   ├── remote.rs         Heimnetz-Steuerung  (FA-55)
│   └── commands.rs       Schnittstelle zum Frontend
└── android-src/          Handgeschriebener Kotlin-Code (siehe unten)
```

### Warum `android-src/` und nicht `gen/`

`src-tauri/gen/` erzeugt Tauri selbst und ist gitignored. Wer die
`MainActivity.kt` dort direkt bearbeitet, verliert die Änderungen beim nächsten
`tauri android init` — und merkt es nicht, weil das Verzeichnis ignoriert ist.

Für Slowshow wäre das fatal: Vollbild (FA-01), Bildschirm-an (FA-50) und
Helligkeit (FA-53) leben genau dort. Deshalb ist die Quelle der Wahrheit
`src-tauri/android-src/`; `scripts/patch-android.mjs` spiegelt sie vor jedem
Build nach `gen/` und prüft das Ergebnis.

---

## Einrichten

```bash
npm install
node scripts/fetch-fonts.mjs    # einmalig: Schriften ins Repository holen
npm run icons                   # einmalig: App-Icons erzeugen
```

Die Schriften (Instrument Sans, Cormorant Garamond, beide SIL OFL) werden
bewusst mitgeliefert statt zur Laufzeit geladen — die App darf keine
Drittanbieter kontaktieren (NF-04) und muss offline laufen (FA-26). Fehlen sie,
greift die Systemschrift; die App bleibt voll bedienbar.

### Voraussetzungen

- Node.js 18+
- Rust (stable)
- Für Android: Android SDK und NDK, `ANDROID_HOME` gesetzt
- ADB im PATH **oder** SDK unter `%LOCALAPPDATA%\Android\Sdk\platform-tools\`

---

## Entwickeln

| Befehl | Wirkung |
| --- | --- |
| `npm run dev` | Nur Vite im Browser — für Layout-Arbeit |
| `npm run tauri dev` | Desktop-App mit Hot Reload |
| `npm run android:dev` | Android-App mit Hot Reload (Gerät angeschlossen) |
| `npm run verify` | Typecheck, Rust-Check, beide Testreihen |

Der Desktop-Build ist ein Nebenprodukt und kein Abnahmegegenstand
(Lastenheft 1.3), aber die schnellste Art, das Frontend zu prüfen.

---

## Bauen und Installieren

Einmalig das Android-Projekt erzeugen:

```bash
npx tauri android init
node scripts/patch-android.mjs
```

Danach:

```powershell
.\deploy-android.ps1                # Release-APK bauen, installieren, starten
.\deploy-android.ps1 -dev           # Debug-APK mit Chrome DevTools
.\deploy-android.ps1 -dev -arm64    # nur fuers Tablet  <- schnellster Weg
```

`-arm64` baut nur für `aarch64` statt für alle vier ABIs. Gradle ruft den
Rust-Build einmal *je ABI* auf, es entfällt also drei Viertel der Übersetzung,
und das Debug-APK schrumpft von rund 1.441 MB auf 325 MB — die ungestrippten
Rust-Debug-Bibliotheken machen den Großteil aus. Für ein Play-Store-Release
werden weiterhin alle ABIs gebraucht.

Das Skript löscht vor dem Bauen das vorhandene APK. Gradles `zipflinger`
schreibt sonst inkrementell in das bestehende Archiv und lässt verdrängte
Blöcke darin stehen: gemessen 633 MB Datei bei 325 MB tatsächlichem Inhalt.
Installiert wird zwar nur der gültige Teil, übertragen aber die ganze Datei.

`patch-android.mjs` läuft dabei automatisch mit. Die APK-Pfade:

| Variante | Pfad |
| --- | --- |
| Release | `src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk` |
| Debug | `src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk` |

Ein Release-Build braucht einen Signaturschlüssel — ohne ihn heißt das Ergebnis
`…-release-unsigned.apk` und lässt sich nicht installieren. Einrichtung in
[notes/signing.md](notes/signing.md). Zum Ausprobieren genügt der Debug-Build.

### Auf einem Smartphone testen

Geht — ein Tablet ist zum Ausprobieren nicht nötig:

```powershell
.\deploy-android.ps1 -dev
```

Was dabei zu erwarten ist:

- **Android 10 oder neuer** genügt (`minSdk 29`), also praktisch jedes Gerät
  der letzten Jahre.
- Die App startet im **Querformat** und lässt sich in den Einstellungen auf
  Hochformat oder Lagesensor umstellen (E-26). Die Voreinstellung ist quer,
  weil ein Bilderrahmen an der Wand hängt — das Telefon also quer halten.
- Das Layout schaltet unter 900 px Breite und unter 520 px Höhe auf kompaktere
  Maße um. Ein Telefon im Querformat liegt bei rund 850 × 390 CSS-Pixeln und
  trifft damit beide Stufen.
- Die Einstellungen sind bedienbar, aber eng — sie sind für 1280 × 800
  entworfen (RB-02). Für einen Funktionstest reicht es; für Design-Urteile
  nicht.

Alles Übrige verhält sich identisch: Ordnerauswahl über den Systemdialog,
WebDAV, Nextcloud, Cache und Zeitplan hängen nicht an der Bildschirmgröße.

---

## Tests

```bash
npm run test:run                              # Frontend (vitest)
cargo test --manifest-path src-tauri/Cargo.toml   # Backend
npm run verify                                # alles zusammen
```

Der Schwerpunkt liegt im Rust-Backend, weil dort die Logik sitzt, die im
Dauerbetrieb stillschweigend schiefgehen kann: Ringpuffer-Verdrängung,
Delta-Abgleich, Zeitplan über Mitternacht, EXIF-Orientierung.

---

## Einrichtung auf dem Tablet

1. APK installieren und Slowshow starten.
2. Einstellungen öffnen — lange auf das Bild drücken (Voreinstellung).
3. Unter **Quellen** eine Quelle hinzufügen:
   - **Lokaler Ordner**: Android-Ordnerdialog, die Freigabe bleibt bestehen.
   - **NAS über WebDAV**: Adresse, Benutzername, Passwort. Bei
     selbstsigniertem Zertifikat den entsprechenden Schalter setzen.
   - **Nextcloud**: Adresse und Zugangsdaten eintragen, dann **Alben laden**.
4. **Jetzt synchronisieren** antippen. Die Diashow startet, sobald die ersten
   Bilder im Cache liegen.

### Bedienung der Diashow

```text
┌───────────┬───────────┬───────────┐
│  zurück   │   Pause   │  weiter   │   kurz tippen
└───────────┴───────────┴───────────┘
     ← wischen: weiter · wischen: zurück →
          lange drücken: Einstellungen
```

Wischen und Tippen führen zum selben Ergebnis — was bequemer ist, hängt davon
ab, ob der Rahmen in Reichweite hängt. Die Mitte ist bewusst groß: Pause ist die
harmlose Aktion.

### Für den Dauerbetrieb empfohlen

- **Akku-Optimierung ausnehmen.** Android-Einstellungen → Apps → Slowshow →
  Akku → „Nicht optimiert". Ohne das beenden vor allem Samsung-, Xiaomi- und
  Huawei-Geräte langlaufende Apps (Risiko R-04).
- **Ladung begrenzen.** Ein Tablet, das dauerhaft an 100 % hängt, altert
  schnell und kann sich aufblähen (Risiko R-08). Eine Zeitschaltuhr oder
  Smart-Plug hilft; der Zeitplan der App kann den Bildschirm zusätzlich
  nachts abschalten.
- **Eigenes NAS-Konto mit Nur-Lese-Rechten.** Das Gerät steht offen im
  Wohnzimmer (Risiko R-12).

### Steuerung aus dem Heimnetz

Ist die Steuerung in den Einstellungen aktiv, antwortet die App auf Port 8127:

```bash
curl -X POST http://tablet.local:8127/api/screen \
     -H 'Content-Type: application/json' \
     -d '{"on": true}'
```

Endpunkte: `GET /api/status`, `POST /api/slideshow`, `POST /api/screen`,
`POST /api/next`, `POST /api/prev`, `POST /api/sync`,
`GET|POST /api/config`. Ist ein Token gesetzt, muss es als
`Authorization: Bearer …` mitgeschickt werden.

`POST /api/config` nimmt einzelne Felder, darunter `brightness` und
`deviceBrightness` — Letzteres gibt die Helligkeitsregelung an das Tablet
zurück (E-22). `GET /api/status` liefert zusätzlich `battery` mit Ladestand,
Temperatur und Ladezustand (E-23); daraus lässt sich in Home Assistant eine
Ladeautomatik bauen, die den Akku im Dauerbetrieb schont.

Fertige Konfiguration für Home Assistant — Sensoren, Schalter und
Automatisierungen: [docs/home-assistant.md](docs/home-assistant.md).
Neben REST gibt es eine MQTT-Anbindung mit Discovery: die Entitäten
erscheinen in Home Assistant von selbst, ohne YAML.

Damit ersetzt ein vorhandener Bewegungsmelder im Smart Home die
Präsenzerkennung per Kamera, die im Lastenheft bewusst gestrichen wurde (E-05).

---

## Bekannte Grenzen

- **HEIC/HEIF wird nicht dekodiert** (E-04). Nextcloud liefert über die
  Preview-API JPEG-Versionen auch von HEIC-Originalen; lokale und
  NAS-HEIC-Dateien werden übersprungen und protokolliert.
- **Kein Autostart nach dem Booten** (E-01). Nach einem Stromausfall muss die
  App von Hand gestartet werden; sie beginnt dann ohne weitere Interaktion.
- **Keine Videowiedergabe** (E-07). Videodateien in Quellen werden ignoriert.
- **Kein SMB** (E-02). NAS-Anbindung ausschließlich über WebDAV.
- **Zugangsdaten sind verschlüsselt, aber nicht Keystore-gebunden.** Details
  und die offene Entscheidung stehen in `src-tauri/src/secrets.rs`.

---

## Lizenz

Apache-Lizenz 2.0 — siehe [LICENSE](LICENSE).

Die Übersicht der eingebundenen Fremdbibliotheken steht in
[docs/third-party-licenses.md](docs/third-party-licenses.md). Sie wird nicht von
Hand gepflegt, sondern mit `npm run licenses` aus `cargo metadata` und den
installierten npm-Paketen erzeugt — eine handgeschriebene Liste wäre beim
nächsten `cargo update` still veraltet.

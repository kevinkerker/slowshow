# Übergabe an Claude Design: Slowshow v1.1.0

Stand 24.09.2026 · Branch `feat/slowshow-mvp` · Commit `0dba581`

Mitgeben: dieses Dokument, `slowshow-app-design.html` (der bisherige Canvas),
`src/styles/tokens.css`, `src/locales/de.json`. Bei Bedarf dazu
`lastenheft.md`, Abschnitt 9 (Entscheidungsprotokoll).

---

## 1. Auftrag

Der Canvas „Slowshow App-Design“ hat vier Artboards und zeigt den Stand vom
29.08.2026. Das war vor E-18. Seitdem ist die App stark gewachsen:

- fünf Einstellungsbereiche
- vier Quellenarten
- Bild-Browser mit Quarantäne
- Analoguhr
- Hochformat
- Anbindung an Home Assistant

All das ist direkt im Code gestaltet worden, ohne Entwurf.

### Aufgaben

1. **Den Canvas auf den Ist-Stand bringen.** Zeichnen, was heute läuft;
   Abschnitt 4 ist die Vorlage dafür. Befunde aus Abschnitt 5a nicht
   stillschweigend „schöner“ zeichnen, sondern als Frage aufwerfen.
2. **Die Lücken gestalten, die das Lastenheft ausdrücklich offen führt:**
   - Hochformat (E-26: „die Einblendungen und die Einstellungsnavigation sind
     hochkant funktionsfähig, aber nicht gestaltet“)
   - App-Icon (E-27: das Artboard „zeigt weiterhin die quadratische Fassung
     und ist nachzuziehen“)
   - Schaltflächen in der Diashow (E-19: „Der Entwurf sieht im Artboard
     ‚Diashow‘ keine Schaltflächen vor“)
3. **Zu jeder offenen Frage in Abschnitt 6 zwei bis drei Optionen vorlegen.**

### Arbeitsregel: Optionen vorlegen, nicht entscheiden

- Kevin entscheidet.
- Jede Entscheidung wird als neue E-Nummer protokolliert, ab **E-58**.
- Was in Abschnitt 7 steht, ist entschieden und wird nicht neu aufgemacht.

**Nicht Teil des Auftrags:** neue Funktionen, ein heller Modus, neue Schriften,
eine andere Marke.

---

## 2. Produkt und Umfeld

### Was es ist

- Ein digitaler Bilderrahmen auf einem Android-Tablet, das an der Wand hängt
  oder steht.
- Er läuft wochenlang unbeaufsichtigt und wird selten bedient
  (Lastenheft 1.4).
- Nach dem Start geht die App direkt in die Diashow (E-01).

### Zwei Welten

- *Diashow:* fast die ganze Zeit sichtbar. Das Foto ist die einzige helle
  Fläche.
- *Einstellungen:* selten geöffnet, per langem Druck oder Zahnrad.

### Wer es nutzt

- Familien, auch technikferne Menschen. Großeltern bekommen Fotos per Mail.
- Die Ersteinrichtung soll in unter 5 Minuten ohne Anleitung gehen (NF-08).

### Geräte und Maße

- Referenzgerät ist das Xiaomi Pad 6: 11 Zoll, 2880 × 1800 px. Daraus
  entstehen **1280 × 800 CSS-px quer** (RB-02, E-10).
- Hochkant montiert: **800 × 1280**.
- Kompaktere Stufen greifen ab **Breite ≤ 900 px** und **Höhe ≤ 520 px**. Ein
  Telefon im Querformat hat rund 850 × 390.

### Sprache

- Deutsch führt, Englisch ist vollständig übersetzt.
- Prüfen an den deutschen Textlängen: dort werden Texte am längsten.

### Technische Grenzen für das Design

- Animiert werden nur `opacity` und `transform`. Es liegen höchstens zwei
  Bilder gleichzeitig im DOM (NF-16).
- Das CSS bleibt konservativ, weil ältere WebViews mitlaufen: kein `:has()`
  (R-02).
- Alle festen Einblendungen wandern alle 90 s um bis zu ±8 px, damit nichts
  einbrennt (Pixel-Shift, NF-07). Layouts brauchen diesen Spielraum.
- `prefers-reduced-motion` schaltet jede Bewegung ab (NF-11).
- Bedient wird nur mit dem Finger: kein Hover, keine Maus.

---

## 3. Verbindliche Vorgaben

### 3.1 Designsystem (E-13)

„Galerie-minimal“: Tiefschwarz als Grund, Off-White als Text, Messing als
einziger Akzent. Tiefschwarz schont OLED-Displays und stützt damit den
Einbrennschutz.

Es gibt **nur ein Erscheinungsbild, dunkel**. Der Kommentar in `tokens.css`
begründet das: „Der Rahmen soll im dunklen Wohnzimmer nicht leuchten, und die
Fotos sollen den einzigen hellen Bereich bilden.“

#### Farbtokens aus `src/styles/tokens.css`

| Token | Wert | Zweck |
| --- | --- | --- |
| `--ss-bg` | `#0a0a0a` | Grund |
| `--ss-bg-night` | `#000000` | Nachtmodus |
| `--ss-surface` | `#100f0e` | Karten, Dialoge |
| `--ss-surface-accent` | `#17150f` | aktive Navigation, Symbolkacheln, gewähltes Segment |
| `--ss-border` | `#1e1c19` | Karten |
| `--ss-border-soft` | `#1c1b19` | Trennlinien |
| `--ss-border-strong` | `#26241f` | Eingabefelder, Knöpfe mit Rand, Segmente |
| `--ss-border-dashed` | `#2c2a25` | Kachel „Quelle hinzufügen“ |
| `--ss-text` | `#f2efe9` | Off-White |
| `--ss-text-strong` | `#eeebe5` | Zeilenbeschriftung, Kartentitel |
| `--ss-text-body` | `#e8e5e0` | Fließtext |
| `--ss-text-accent` | `#e9dfc9` | Text auf Messingfläche |
| `--ss-text-muted` | `#8d8a83` | inaktive Navigation |
| `--ss-text-dim` | `#767268` | Hinweise, Statuszeilen |
| `--ss-text-faint` | `#5c5952` | Abschnittsüberschriften, Platzhalter |
| `--ss-icon-muted` | `#6f6c65` | inaktive Symbole |
| `--ss-icon-soft` | `#a6a29a` | Schließen-Kreuz |
| `--ss-accent` | `#c2a878` | Messing |
| `--ss-accent-hover` | `#d8c29a` | gedrückt |
| `--ss-toggle-off` | `#26241f` | Kippschalter aus, Schiene |
| `--ss-toggle-knob-off` | `#55524b` | Kippschalter aus, Knopf |
| `--ss-night-clock` | `#2e2c29` | Nachtuhr |
| `--ss-night-label` | `#1f1e1c` | Zeile unter der Nachtuhr |
| `--ss-error` | `#c46686` | Fehler |
| `--ss-ok` | = Akzent | Erfolg |

**Formtokens:** Kartenradius 12 px, Navigationsradius 8 px, Pille 999 px,
Übergang 0,2 s ease, Navigationsbreite 250 px (180 px bei ≤ 900),
Randabstand der Einblendungen 56 px (32 px bei ≤ 900), Tippziel 44 px.

**Es gibt keine Tokens für Schriftgrößen und Abstände.** Siehe Frage F6.

### 3.2 Schriften

| Familie | Rolle | Schnitte |
| --- | --- | --- |
| Instrument Sans | gesamte Bedienung, Uhr | 400, 500 (600 wäre vorhanden, wird nicht genutzt) |
| Cormorant Garamond | Wortmarke „Slowshow“, Bildunterschrift | 300, 400, kursiv 400 |

- Keine weiteren Familien und keine weiteren Schnitte.
- In der App sind die Schriften lokal gebündelt (NF-04, FA-26). Der Canvas
  darf sie zur Vorschau wie bisher über Google Fonts laden.

### 3.3 Leitlinien aus dem Lastenheft (wörtlich)

- **Ruhe.** „der Hinweis allein widersprach der Ruhe des Entwurfs, wenn er
  sich nicht abschalten liesse“ (E-31). Jede Einblendung in der Diashow ist
  einzeln abschaltbar (FA-07).
- **Das Foto ist die helle Fläche.** „auf einem Rahmen, dessen Fotos der
  einzige helle Bereich sein sollen, ist das zu viel“ (E-20).
- **Zustand statt Meldung.** „es meldet einen Zustand, keine Meldung“ und
  „Ein Rahmen, der stehenbleibt, sieht danach aus wie einer, der hängt“
  (E-21).
- **Nachts bleibt es dunkel.** „dort soll der Schirm dunkel bleiben“ (E-21).
- **Keine wirkungslosen Regler.** „ausgeblendet statt wirkungslos
  stehenzubleiben“ (E-22). „Ein Regler, der sichtbar nichts tut, ist die
  schlechteste Variante“ (E-48).
- **Fehlgriffe abfangen.** „die großzügige Mitte fängt Fehlgriffe auf die
  harmlose Aktion“ (E-18). „Gegen zwei kleine Ziele nebeneinander
  entschieden: ein Fehltipp gäbe dort dauerhaft alles frei“ (E-35).
- **Destruktives liegt eine Ebene tiefer** (FA-43): Löschen steht nur im
  Bearbeiten-Dialog, „die Liste bleibt nach Entwurf ruhig“.
- **Funktion am erwarteten Ort.** „jede Funktion dort, wo man sie sucht“
  (E-31). Hinweise stehen „an den Feldern, wo die Entscheidung fällt — nicht
  in einer Anleitung, die niemand aufschlägt“ (E-39). Rückmeldungen stehen
  „neben ihrem Auslöser“ (E-33).
- **Keine stillen Überraschungen.** „Stilles Verschwinden aus der laufenden
  Show wäre die schlechtere Überraschung“ (E-32).

---

## 4. Canvas und Code im Abgleich

### 4.1 Bestehende Artboards

| Artboard | Zustand | Was nachzuziehen ist |
| --- | --- | --- |
| Diashow | Uhr, Datum und Bildunterschrift stimmen, am Gerät bestätigt | Es fehlen: Zahnrad und Auge oben rechts (E-19), Pausenabzeichen (E-21), Quarantäne-Hinweis (E-31), Toast, Analoguhr (E-20), Paar-Modus (FA-08) |
| Nachtmodus | stimmt | Analoge Nachtuhr fehlt (E-20) |
| Einstellungen · Quellen | veraltet | 5 statt 4 Navigationspunkte. Kopfzeile zeigt den Bereichsnamen statt „Einstellungen“. Quellenkarte hat zusätzlich Sync- und Bearbeiten-Knopf, Fortschritt und Fehlerzeile. Postfach ist die vierte Quellenart |
| App-Icon (final) | überholt durch E-27 | Kein gezeichneter Rahmen mehr: auf Android bildet ihn die Launcher-Maske, sonst eine schwarze Kreisscheibe. Horizont und Sonne in Messing auf `#0A0A0A`, unter 48 px ohne Sonne, Off-White kommt nicht mehr vor. Quelle: `scripts/generate-icon.mjs`, `docs/slowshow-icon-512.png` |

### 4.2 Gliederung des neuen Canvas

Nur ein Vorschlag, die Nummern dienen als Bezug.

| Nr. | Artboard | Maß |
| --- | --- | --- |
| D1 | Diashow mit allen Einblendungen | 1280 × 800 |
| D2 | Diashow: pausiert, Fotos warten, Toast (alle Zustände oben Mitte) | 1280 × 800 |
| D3 | Diashow mit Analoguhr | 1280 × 800 |
| D4 | Paar-Modus (zwei Hochformate nebeneinander) | 1280 × 800 |
| D5 | Leerzustand „Noch keine Bilder“ | 1280 × 800 |
| D6 | Nachtmodus: Ziffern und Zeiger | 1280 × 800 |
| S1 | Einstellungen · Quellen | 1280 × 800 |
| S2 | Einstellungen · Bilder (Filter „Wartet auf Freigabe“) | 1280 × 800 |
| S3 | Einstellungen · Diashow | 1280 × 800 |
| S4 | Einstellungen · Zeitplan | 1280 × 800 |
| S5 | Einstellungen · System | 1280 × 800 |
| G1 | Dialog „Quelle hinzufügen“ und „Postfach bearbeiten“ | 1280 × 800 |
| G2 | Dialog „Foto freigeben“ | 1280 × 800 |
| G3 | Dialog Diagnosebericht | 1280 × 800 |
| G4 | Bestätigungsdialog (heute nativ, siehe F5) | 1280 × 800 |
| H1 | Diashow hochkant (Paar übereinander) | 800 × 1280 |
| H2 | Einstellungen hochkant | 800 × 1280 |
| H3 | Nachtmodus hochkant | 800 × 1280 |
| I1 | App-Icon nach E-27 | frei |

### 4.3 Diashow: was heute läuft

#### Schichten, von unten nach oben

1. Grund `--ss-bg`.
2. Fotobühne mit zwei Ebenen. Überblendung nur über die Deckkraft,
   Voreinstellung 1,2 s. Einpassen mit schwarzen Balken (Voreinstellung) oder
   formatfüllend. Kein unscharfer Hintergrund.
   - Wahlweise Ken Burns: 40 s, Zoom auf 1,08.
   - Paar-Modus: zwei Bilder zu je 50 %, dazwischen eine 2-px-Fuge.
3. Verlauf am unteren Rand, 220 px hoch, bis 55 % Schwarz.
4. **Uhr** unten links, 56 px vom Rand, 48 px vom unteren Rand.
   - *Ziffern:* 92 px, 400, −0,01 em, Ziffern gleicher Breite, weicher
     Schatten. 64 px bei ≤ 900.
   - *Zeiger:* 150 px. Dünner Ring mit 12 Marken, die bei 12/3/6/9 länger
     sind. Keine Ziffern, kein Sekundenzeiger.
   - *Datum darunter:* 15 px, 500, Versalien, 0,22 em, 72 % Deckkraft. Form
     „SAMSTAG · 29. AUGUST“.
5. **Bildunterschrift** unten rechts, höchstens 45 % der Breite.
   - *Titel:* Dateiname ohne Endung, Cormorant kursiv 24 px, 85 %.
   - *Zeile darunter:* „JUNI 2025 · {Quellenname}“, 12 px, 500, 0,18 em,
     55 %.
   - Standardmäßig **aus**.
   - Im Paar-Modus gilt sie nur für das linke bzw. obere Bild.
6. **Oben Mitte**, als Stapel mit 10 px Abstand, 32 px vom oberen Rand. Alle
   drei sind Pillen: Hintergrund 82 % `--ss-bg`, Rand `--ss-border-strong`.
   - *Pausenabzeichen:* Pausensymbol plus „PAUSIERT“, Messing, 12 px, 0,2 em.
   - *Quarantäne-Hinweis:* „{n} Fotos warten auf Freigabe“, Messing, lässt
     sich antippen.
   - *Toast:* „Bild aus der Diashow entfernt“, Off-White 14 px, 2,2 s. Er
     erscheint nur nach dem Auge-Knopf.
7. **Oben rechts** zwei runde Knöpfe, 44 px groß, 24 px vom Rand. Symbol in
   28 % Off-White, gedrückt in Messing.
   - Zahnrad: öffnet die Einstellungen.
   - Durchgestrichenes Auge: blendet das laufende Bild aus, ohne Rückfrage.
8. **Leerzustand**
   - Wortmarke „Slowshow“ 42 px
   - „Noch keine Bilder“ 17 px
   - „Fügen Sie eine Quelle hinzu, um die Diashow zu starten.“ 14 px, dim
   - Pillenknopf mit Messingrand „Quelle hinzufügen“
9. **Nachtmodus**
   - Schwarz `#000`, Uhr mittig.
   - *Ziffern:* 120 px `--ss-night-clock`.
   - *Zeiger:* 220 px.
   - *Zeile darunter:* „RUHEMODUS BIS 07:00“, 13 px, 0,28 em,
     `--ss-night-label`.
   - Ohne Nachtuhr ist der Schirm ganz schwarz.
10. **Home Assistant:** Ein Fremdbild (Bild oder Kamerastrom) ist eine normale
    Folie, ohne Unterschrift und ohne Kennzeichnung (E-52).

#### Bedienung

- Tippen links: zurück. Mitte: Pause. Rechts: weiter.
- Wischen: vor und zurück.
- Langer Druck, 650 ms: öffnet die Einstellungen, immer.
- Es gibt keine sichtbare Rückmeldung außer dem Bildwechsel und dem
  Pausenabzeichen.

**Pixel-Shift** betrifft Uhr, Unterschrift, Pausenabzeichen und Nachtuhr.
**Nicht** betroffen sind Quarantäne-Hinweis, Toast und Eckknöpfe.

### 4.4 Einstellungen: was heute läuft

#### Rahmen

- **Kopfzeile**
  - Links die Wortmarke „Slowshow“, 26 px, und daneben der **Name des offenen
    Bereichs** in Versalien, 12 px, `--ss-text-faint`.
  - Rechts ein runder Schließen-Knopf, 44 px.
- **Navigation links**, 250 px breit, in dieser Reihenfolge:
  1. **Quellen** (Ordner)
  2. **Bilder** (2×2-Raster)
  3. **Diashow** (Rahmen mit Berglinie)
  4. **Zeitplan** (Uhr)
  5. **System** (Kreis mit Strahlen)

  Der aktive Punkt steht auf `--ss-surface-accent`, Text `--ss-text-accent`,
  Symbol in Messing.
- **Inhalt**: 32/40 px Innenabstand. Jeder Bereich scrollt für sich, ohne
  sichtbaren Balken.
- Es gibt **keinen Speichern-Knopf**: jede Änderung wirkt sofort (FA-42).
- Die Einstellungen öffnen immer im Bereich „Quellen“.

#### Wiederkehrende Bausteine

- **Einstellungszeile**
  - 14 px Abstand oben und unten, Trennlinie `--ss-border-soft`.
  - Beschriftung 15 px `--ss-text-strong`, Hinweis 13 px `--ss-text-dim`.
  - Das Bedienelement steht rechts, in der Variante „gestapelt“ darunter.
- **Abschnittsüberschrift:** 12 px, 500, Versalien, 0,2 em,
  `--ss-text-faint`. 28 px Abstand zwischen den Abschnitten.
- **Kippschalter**
  - Schiene 52 × 30, Knopf 24 px. An: Messing mit dunklem Knopf.
  - Tippfläche 44 × 44.
- **Segmentsteuerung:** Pille mit Rand `--ss-border-strong`, Segmente 40 px
  hoch und 14 px Schrift. Gewählt: `--ss-surface-accent` mit Messingtext.
- **Eingabefeld:** 44 px hoch, Hintergrund `--ss-bg` (also dunkler als die
  Fläche darunter), Rand `--ss-border-strong`, Radius 8 px.
- **Chip:** Pille mit Rand `--ss-border-strong`, 13 px. Gewählt:
  `--ss-surface-accent` mit Messingtext.
- **Karte:** `--ss-surface`, Rand `--ss-border`, Radius 12 px.
- **Fokusring:** 2 px Messing, 2 px Abstand.

#### S1 Quellen

- **Quellenkarte**, 22/24 px Innenabstand:
  - Symbolkachel 48 px auf `--ss-surface-accent`: Ordner, NAS, Wolke oder
    Umschlag.
  - Name 16 px, 500.
  - Statuszeile 13 px dim, Teile mit „ · “ verbunden. Beispiele:
    - „NAS · WebDAV · zuletzt synchronisiert vor 12 Min · 1 840 Fotos im
      Cache“
    - „Postfach · 212 Fotos im Cache · …“
  - Während des Abgleichs: Fortschrittsbalken 2 px, Text „Lädt 12 von 80 ·
    datei.jpg“. Ohne bekannte Gesamtzahl läuft ein unbestimmter Balken.
  - Nach einem Fehler: Fehlerzeile in `--ss-error`, bis ein Lauf gelingt
    (E-45).
  - Rechts: Knopf Sync (dreht sich beim Abgleich), Knopf Bearbeiten (Stift),
    Kippschalter „in der Diashow“.
  - Eine abgeschaltete Quelle hat 62 % Deckkraft und den Zusatz „aus Diashow
    ausgenommen“.
- **Kachel „Quelle hinzufügen“** mit gestricheltem Rand.
- Ohne Quellen: „Noch keine Quelle eingerichtet“.
- **Fußzeile:** „1,4 GB von 2,0 GB“, ein 3-px-Balken, „Vorausgeladene Bilder:
  5“.

#### G1 Dialog „Quelle hinzufügen“ / „Quelle bearbeiten“

##### Rahmen

- Breite bis 680 px, Hintergrund 72 % Schwarz.
- Kopf mit Titel in der Wortmarken-Schrift, 22 px, und Schließen-Knopf.
- Rumpf scrollt.
- Feste Fußzeile, von links nach rechts:
  1. „Quelle entfernen“ (Gefahr, nur beim Bearbeiten)
  2. „Verbindung testen“ (nicht bei lokalen Ordnern)
  3. das Testergebnis (E-33)
  4. Abstand
  5. „Abbrechen“
  6. „Speichern“ (primär)

##### Felder

1. **Art der Quelle** (nur beim Hinzufügen). Vier Auswahlkarten mit Titel und
   Hinweis:
   - Lokaler Ordner: „Ordner auf dem Tablet oder der SD-Karte“
   - NAS über WebDAV: „Synology, QNAP, ownCloud und andere“
   - Nextcloud-Album: „Alben der Photos-App“
   - Postfach: „Fotos, die per E-Mail eintreffen“
2. **Bezeichnung**
3. Je nach Art:
   - *Lokal:* Knopf „Ordner auswählen“, danach „Ausgewählt: {Pfad}“.
   - *WebDAV und Nextcloud:*
     - Adresse, Benutzername, Passwort mit dem Hinweis „eigenes Konto mit
       Nur-Lese-Rechten“.
     - *Nur Nextcloud:* Album-Auswahl plus Knopf „Alben laden“ und der
       Schalter „Vorschaubilder vom Server beziehen“.
     - Schalter „Selbstsigniertes Zertifikat akzeptieren“.
   - *Postfach* (das längste Formular):
     1. IMAP-Server, mit dem Hinweis auf ein eigenes Postfach (E-39)
     2. Port
     3. Benutzername
     4. Passwort, mit dem Hinweis auf ein App-Passwort (E-39)
     5. Ordner
     6. **Letzter Abruf** (nur beim Bearbeiten):
        - Statuszeile
        - Knopf „Jetzt abrufen“
        - Aufklappbares Abruf-Protokoll: die letzten 50 Läufe als Liste mit
          Zeit, Auslöser und Ergebnis
        - Verweis „Postfach neu abgleichen“ mit laufendem Zähler
     7. „Auch gelesene Nachrichten“ (E-34)
     8. „Alles erst freigeben“
     9. **Freigegebene Absender** (nur beim Bearbeiten): Liste aus Adresse,
        Zahl der Fotos und ✕ (E-32)
     10. Größter Anhang (MB)
     11. Mails je Stunde
4. Am Ende, je nach Art:
   - „Synchronisieren alle“: 15 Min, 1 h, 6 h oder 24 h
   - „Nur diese Unterordner“
   - „Mindestauflösung“: B × H

- Prüffehler stehen unten im Rumpf, nicht in der Fußzeile.
- Kompaktstufe bei Höhe ≤ 520.

#### S2 Bilder (E-25)

- **Kopf**
  - Segmentsteuerung mit **fünf** Segmenten: Alle | In der Diashow |
    Ausgeblendet | Wartet auf Freigabe | Nie gezeigt.
  - Rechts „{n} Bilder“.
- **Raster**
  - Kacheln mindestens 128 px, 10 px Abstand, Radius 8, Bild formatfüllend.
  - Verlauf unten, darauf das Datum „Juni 2025“, ersatzweise der Dateiname.
  - Ausgeblendete Kacheln haben 28 % Deckkraft und das Abzeichen „AUS“.
- **Im Filter „Wartet auf Freigabe“**
  - Ein Erklärsatz über dem Raster.
  - Der Verweis „{n} Absender freigegeben ›“ in Messing. Er öffnet den
    Postfach-Dialog im Bereich Quellen.
  - Die Kachel zeigt zweizeilig Absender und Datum, dazu das Abzeichen
    „WARTET“.
- **Tippen auf eine Kachel**
  - Normale Kachel: schaltet sofort zwischen aus- und eingeblendet um.
  - Wartende Kachel: öffnet G2.
- Nachladen in Seiten zu 200. Am Ende steht „Alle Bilder geladen · 38 MB
  Vorschau“.
- Leer: „Noch keine Bilder im Cache.“, für jeden Filter derselbe Text.

#### G2 Foto freigeben (E-35)

- Tafel bis 520 px breit, Radius 16.
- Großes Vorschaubild, bis 38 % der Bildschirmhöhe.
- „Von“ mit Absender, „Betreff“ mit Betreff.
- Drei Knöpfe übereinander:
  1. **„Nur dieses Bild“**: Messingfläche
  2. **„Alle von {Adresse}“**: mit Rand. Darunter der Hinweis „Nimmt die
     Adresse dauerhaft auf. Künftige Fotos dieser Person gehen ohne Nachfrage
     in die Diashow.“
  3. „Abbrechen“: nur Text
- Der Dialog hat **keinen Titel**, obwohl der Text „Foto freigeben“ vorhanden
  ist.

#### S3 Diashow

##### Ohne Überschrift

- Anzeigedauer je Bild: 5 s bis 30 Min.
- Reihenfolge: Intelligente Mischung | Einfacher Zufall | Dateiname |
  Chronologisch.
- Ausrichtung: Quer | Hoch | Automatisch.
- Zeitraum: Alle | Letzte 12 Monate | Dieses Jahr.
- Einzelne Jahre und Absender als Chips mit Anzahl.
- Schalter „Ohne Datum ({n})“.
- Nur bei „Intelligente Mischung“: drei Schalter. „Neue Fotos öfter zeigen“,
  „Lange nicht Gezeigte bevorzugen“, „Serienaufnahmen auseinanderziehen“.
- Nur bei „Chronologisch“: Richtung.
- Bildanpassung: Einpassen | Formatfüllend.

##### DURCHLAUF UND STATISTIK

- Drei Kennzahlen, 22 px in Messing: Fotos im Cache, davon in der Diashow,
  noch nie gezeigt.
- Die Zeile „Durchlauf: noch {n} von {total}“.
- Zwei Listen: „Am häufigsten gezeigt“ und „Am längsten nicht gezeigt“.
- Die Knöpfe „Durchlauf neu starten“ und „Anzeige-Historie zurücksetzen“, der
  zweite mit Rückfrage.

##### DIASHOW

- Weiche Überblendung.
- Dauer als Regler, 0,2 bis 4 s.
- Langsames Zoomen.
- Hochformat paarweise zeigen.

##### EINBLENDUNGEN

- Uhrzeit, darunter die Darstellung Ziffern | Zeiger.
- Datum, Dateiname, Aufnahmedatum.
- Zahnrad, Knopf zum Ausblenden, Hinweis auf wartende Fotos.
- Einbrennschutz.

#### S4 Zeitplan

- **Statuskarte** oben mit 8-px-Punkt: „Diashow läuft“ in Messing oder
  „Ruhemodus“ in dim.
- **Zeitplan**
  - Schalter „Zeitplan verwenden“.
  - Darunter: Aktiv ab und Aktiv bis (Zeitfelder), mit dem Hinweis bei einem
    Zeitraum über Mitternacht.
  - „Nachts Uhr anzeigen“ und deren Darstellung Ziffern | Zeiger.
- **HELLIGKEIT**
  - Schalter „Helligkeit vom Gerät regeln lassen“.
  - Ist er aus, erscheinen:
    - Regler Helligkeit, 5–100 %.
    - „Abends automatisch abdunkeln“.
    - Nur wenn dieser Schalter an ist: „Abdunkeln ab“ und „Helligkeit abends“.

#### S5 System

- **CACHE**
  - Belegung „1,4 GB von 2,0 GB · {n} Bilder“.
  - Maximale Cachegröße: 512 MB bis 16 GB.
  - Vorausgeladene Bilder: 1–12.
  - Bildqualität im Cache: 40–100.
- **SYSTEM**
  - Sprache: Automatisch | Deutsch | English.
  - Einstellungen schützen (siehe 5b).
- **SPEICHER**
  - Aufschlüsselung in zwei Spalten, nach Jahr und nach Absender.
  - Datenbank: Knopf „Prüfen“. Als Ergebnis „Alles in Ordnung.“ oder die
    Befundzeile mit „Aufräumen“.
  - Diagnosebericht: Knopf „Bericht erzeugen“, öffnet G3.
- **STEUERUNG IM HEIMNETZ:** Schalter, darunter Port und Zugriffstoken.
- **AUTOMATISCH IN HOME ASSISTANT:** Schalter (DLNA, E-47), darunter der Name
  in Home Assistant.
- **MQTT (HOME ASSISTANT)**
  - Schalter.
  - Broker, Port, Benutzer.
  - Passwort mit eigenem Knopf „Speichern“.
  - Basistopic, automatische Anmeldung, Discovery-Präfix.
  - Verbindungszeile mit Punkt („Verbunden“ oder „Nicht verbunden“) und Knopf
    „Neu verbinden“.
- **KONFIGURATION:** Knöpfe „Exportieren“ und „Importieren“, über den
  Dateidialog (E-40).
- **DAUERBETRIEB:** ein erklärender Absatz (E-45).
- **Über die App:** Wortmarke 22 px, „Version 1.1.0“, „Apache-Lizenz 2.0 · Open
  Source“.

#### G3 Diagnosebericht

- Tafel bis 760 px, Radius 16.
- Der Bericht in Festbreitenschrift, 12 px.
- Knöpfe „In die Zwischenablage“ und „Schließen“.

#### G4 Rückfragen

Heute alle über das **native `confirm()`** von Android:

- Quelle entfernen
- Postfach neu abgleichen
- Absender entfernen
- Anzeige-Historie zurücksetzen
- Datenbank aufräumen

---

## 5. Befunde im Code

### 5a. Uneinheitlichkeiten, die der Entwurf auflösen soll

- **Knöpfe:** Dieselbe Variante ist je nach Bereich anders umgesetzt, teils
  gar nicht gestaltet.

  | Variante | Bereich | Umsetzung heute |
  | --- | --- | --- |
  | Primär | Quellendialog | Messingfläche, Text `--ss-bg`, 500 |
  | Primär | Freigabe | Messingfläche, Text `#14100a`, 400 |
  | Primär | Diagnosebericht | ungestaltet |
  | Sekundär | Quellendialog, System | mit Rand |
  | Sekundär | Freigabe | mit weicherem Rand |
  | Sekundär | Diashow | ungestaltet, bloßer Text |
  | Gefahr | Quellendialog | rot |
  | Gefahr | Diashow, System | **nicht rot** |

- **Radien** ohne System: 8, 10, 12, 16 und 999 px. Die beiden neueren
  Dialoge haben 16, der Quellendialog 12.
- **Elf Schriftgrößen** zwischen 10 und 26 px sowie einmal `0.85rem`. Keine
  davon ist ein Token.
- **Hart kodierte Werte**
  - Deckkraftstufen des Off-White: 28 / 55 / 72 / 85 %.
  - Verläufe unter Uhr und Kachel: 55 % und 80 %.
  - Hintergrund der Dialoge: dreimal `rgba(0,0,0,.72)`.
  - Pillen und Abzeichen: `rgba(10,10,10,.82)`.
  - `--ss-surface-2` wird benutzt, ist aber nirgends definiert.
- **Rückmeldungen** in den Einstellungen stehen an wechselnden Orten, in
  wechselnden Farben und unterschiedlich lange:
  - Quellen: 6 s, dim
  - Diashow: 6 s, Messing
  - System: 5 s, dim

  MQTT-Meldungen erscheinen ganz unten bei „Konfiguration“ statt neben den
  MQTT-Feldern. Das widerspricht E-33.
- **Rückfragen** laufen über den nativen Systemdialog und brechen den Stil.
  „Absender entfernen“ missbraucht OK und Abbrechen für eine Wahl mit drei
  Ausgängen: Der Absender wird in beiden Fällen entfernt, OK heißt „Fotos
  zurück in die Quarantäne“, Abbrechen heißt „sichtbar lassen“.
- **Segmentsteuerung mit fünf Segmenten** im Bereich Bilder: Sie bricht nicht
  um. Bei 900 px Breite und hochkant ist das ungeprüft.
- Der **Kippschalter** springt, statt zu gleiten. Nur die Farbe hat einen
  Übergang.
- **Anrede gemischt:** „du“ im Quellendialog, „Sie“ im Leerzustand, sonst
  unpersönlich.
- **Hochkant:** Die Einstellungen bleiben zweispaltig mit 250-px-Navigation.
  Es gibt keine Regel für das Hochformat.
- **Zustände ohne Gestaltung**
  - Start und Laden (E-54): heute schwarz. Der Leerzustand kann kurz
    aufblitzen.
  - Fehler oder Offline in der Diashow: zeigt bewusst nichts.
  - Wartender Abgleich (E-43).
  - Fehlende Ordnerfreigabe nach einem Import (E-40, „Die Oberfläche sagt das
    bisher nicht“).
- **Ungenutzte Texte**, die auf eine offene Absicht deuten:
  - „Zum Öffnen gedrückt halten“
  - „Bilder werden vorbereitet …“
  - „Foto freigeben“ (Titel für G2)

### 5b. Fehler, die im Code behoben werden

Nicht nachzeichnen. Diese Punkte stehen hier nur, damit sie nicht in den
Entwurf wandern.

- **Schriften fehlen auf dem Gerät.** Die gebündelten Dateien enthalten nur
  die Zeichensätze *latin-ext* bzw. *cyrillic-ext*, ohne a–z, 0–9 und
  Umlaute.
  - Uhr, Datum und Bildunterschrift erscheinen deshalb in Ersatzschriften.
  - **Bildschirmfotos vom Gerät taugen nicht als Vorlage für die
    Typografie.** Maßgeblich ist der Canvas.
- Der Quarantäne-Hinweis öffnet „Quellen“ statt „Bilder“, obwohl der
  Kommentar im Code den Bild-Browser nennt.
- „1 Fotos warten auf Freigabe“: die Einzahl fehlt.
- Drei Rückfragen zeigen ein wörtliches „\n“ statt eines Zeilenumbruchs.
- „Einstellungen schützen“ hat keine Wirkung mehr. Das Zahnrad öffnet immer.
- „Ruhemodus bis {Zeit}“ erscheint auch, wenn Home Assistant den Nachtmodus
  bei abgeschaltetem Zeitplan auslöst.

---

## 6. Offene Fragen: je 2–3 Optionen erbeten

### F1 · Einstellungen hochkant (E-26)

Bei 800 px Breite nimmt die
Navigation mit 250 px fast ein Drittel ein. Wie wird navigiert, und wie
verhalten sich Einstellungszeilen, Raster und Dialoge?

### F2 · Diashow hochkant (E-26)

- Wo stehen Uhr, Datum, Bildunterschrift und Knöpfe, wenn nur 800 px Breite
  zur Verfügung stehen? Die Unterschrift ist auf 45 % der Breite begrenzt,
  das wären dort 360 px.
- Im Paar-Modus liegen zwei Querformate übereinander: Wo bleibt dann Platz
  für die Einblendungen?

### F3 · Zahnrad im Nachtmodus

- Heute bleibt das Zahnrad über der Nachtuhr sichtbar, in 28 % Off-White.
  Das widerspricht „nachts bleibt es dunkel“.
- Außerdem pausiert oder blättert ein Tipp auf die Nachtuhr unsichtbar die
  Diashow darunter.
- Ist beides gewollt?

### F4 · Ein Knopfsystem

- Varianten: Primär, Sekundär, Gefahr, Ghost, Textverweis, runder
  Symbolknopf.
- Je Variante die Zustände gedrückt, gesperrt und beschäftigt, also z. B.
  „Prüfe …“ oder der drehende Sync-Pfeil.

### F5 · Eigener Rückfragedialog statt `confirm()`

- Einschließlich der Wahl mit drei Ausgängen bei „Absender entfernen“, mit
  Knöpfen, deren Aufschrift sagt, was passiert.
- Gefahrenknopf und Standardknopf klar getrennt, im Sinne von FA-43.

### F6 · Schrift- und Abstandsskala als Tokens

Wie viele Stufen reichen, um
die elf heutigen Größen abzulösen? Wie heißen sie?

### F7 · Radien

Eine Skala für Eingaben, Kacheln, Karten und Dialoge.

### F8 · Rückmeldungen in den Einstellungen

Ort (neben dem Auslöser, E-33),
Farbe für Erfolg und Fehler, Dauer, und ob sie von selbst verschwinden.

### F9 · Startzustand

Was ist in den ersten Sekunden nach dem Einschalten zu
sehen, bis das erste Bild steht? Heute ist es schwarz, im Rennen gegen die
Konfiguration (E-54) ggf. mehrere Sekunden lang.

### F10 · Diashow bei Störung

Heute gibt es keinerlei Anzeige: kein Netz,
Quelle nicht erreichbar, Abgleich hängt.

- Bleibt das so, gemäß „Ruhe“?
- Oder gibt es einen abschaltbaren Zustand im Sinne von „Zustand statt
  Meldung“, wie das Pausenabzeichen?

### F11 · Rückmeldung für Gesten

Heute gibt es keine Rückmeldung. Soll der
lange Druck einen Fortschritt zeigen? Gibt es einen Ort für den Text „Zum
Öffnen gedrückt halten“, oder fällt er weg?

### F12 · Filter im Bild-Browser bei wenig Breite

Fünf Segmente bei ≤ 900 px
und hochkant.

### F13 · Fehlende Ordnerfreigabe nach einem Import (E-40)

Wie zeigt die
Quellenkarte, dass ein lokaler Ordner neu gewählt werden muss? Wie kommt man
mit einem Tipp dorthin?

### F14 · Anrede

„du“, „Sie“ oder durchgehend unpersönlich.

---

## 7. Bereits entschieden: nicht neu aufmachen

| Thema | Entscheidung | E-Nr. |
| --- | --- | --- |
| Erscheinungsbild | nur dunkel, kein heller Modus | E-13 |
| Farben und Schriften | Tiefschwarz, Off-White, Messing; Instrument Sans und Cormorant Garamond | E-13 |
| Einblendungen | jede einzeln abschaltbar | FA-07 |
| Analoguhr | dünner Ring, 12 Marken (bei 12/3/6/9 länger), keine Ziffern, kein Sekundenzeiger | E-20 |
| Tag- und Nachtuhr | getrennt wählbar, z. B. tags Ziffern und nachts Zeiger | E-20 |
| Pause | dauerhaftes Abzeichen oben Mitte, Messing, nicht im Nachtmodus | E-21 |
| Knöpfe in der Diashow | Zahnrad und durchgestrichenes Auge oben rechts; Strahlen-Symbol und Minus verworfen | E-19 |
| Bedienung | Tippzonen in Dritteln, Wischen, langer Druck öffnet immer | E-18, FA-43 |
| Wartung | kein sechster Bereich: Statistik bei Diashow, Postfach-Werkzeuge im Postfach-Dialog, Speicher und Datenbank bei System | E-31 |
| Quarantäne | Filter im Bild-Browser **und** abschaltbarer Hinweis in der Diashow | E-31 |
| Freigabe | Tipp öffnet die Auswahl „Nur dieses Bild“ / „Alle von {Adresse}“, die Adresse steht auf dem Knopf; die Kachel zeigt zweizeilig Absender und Datum | E-35 |
| Freigegebene Absender | Liste im Postfach-Dialog, Verweis mit Zahl im Bild-Browser, Rückfrage beim Entfernen | E-32 |
| Verbindungstest | Ergebnis in der Fußzeile neben dem Knopf | E-33 |
| Sicherheitshinweise | App-Passwort und eigenes Postfach, direkt am Feld | E-39 |
| Helligkeit | Regler ausblenden, wenn das Gerät regelt; nicht wirkungslos stehen lassen | E-22, E-48 |
| Ausrichtung | Einstellung Quer / Hoch / Automatisch, kein Drehen nach der Montage | E-26 |
| Fremdbild aus Home Assistant | ohne Kennzeichnung, „kein Sonderfall“ | E-52 |
| Löschen | nur im Bearbeiten-Dialog, die Liste bleibt ruhig | FA-43 |
| Speichern | kein Speichern-Knopf, jede Änderung wirkt sofort | FA-42 |
| App-Icon | kein gezeichneter Rahmen, Horizont und Sonne in Messing | E-27 |
| Favoriten | gibt es nicht | E-28 |

---

## 8. Lieferung und Rückweg

1. **Format.** Derselbe Canvas-Aufbau wie bisher: eine `.dc.html` je Artboard
   plus `canvas.json`. So kann `slowshow-app-design.html` im Repo ersetzt
   werden. Titel der Artboards wie in 4.2.
2. **Werte.** Farben und Maße nur aus Tokens. Neue oder geänderte Tokens als
   Liste „Name · Wert · Zweck“, im Vergleich zu `tokens.css`.
3. **Texte.** Aus `src/locales/de.json` übernehmen. Neue oder geänderte Texte
   kennzeichnen.
4. **Optionen.** Je Frage nebeneinander auf eigenen Artboards, beschriftet
   „F4 · Option A“ usw., jeweils mit einem Satz zu Vor- und Nachteil.
5. **Danach**, durch Claude Code:
   - Entscheidungen als E-58 ff. ins Lastenheft eintragen.
   - `tokens.css` und die Komponenten anpassen.
   - In `CLAUDE.md` die Zahl der Artboards nachziehen („Canvas mit 4
     Artboards“).

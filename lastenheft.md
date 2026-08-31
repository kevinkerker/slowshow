# Lastenheft: „Slowshow" – Digitaler Bilderrahmen für Android-Tablets

| | |
|---|---|
| **Projekt** | Tauri-Android-App zur Nutzung eines Tablets als digitaler Bilderrahmen |
| **Auftraggeber** | Kevin (privates Hobbyprojekt) |
| **Version** | 0.5 (E-14 bis E-17 aus der Umsetzung ergänzt; Umsetzungsstand in Abschnitt 11) |
| **Datum** | 2026-08-29 |
| **Status** | In Umsetzung — MVP steht, Abnahmetests offen (E-10) |

---

## 1. Einleitung und Zielsetzung

### 1.1 Ausgangslage

Ein vorhandenes Android-Tablet soll als dauerhaft betriebener digitaler Bilderrahmen wiederverwendet werden. Kommerzielle Bilderrahmen-Apps sind häufig werbefinanziert, an Cloud-Abos gebunden oder bieten keine Anbindung an eigene Speicherorte (NAS, Selfhosted-Cloud). Es soll daher eine eigene App auf Basis von **Tauri 2.x** (Rust-Backend, WebView-Frontend) entstehen.

### 1.2 Projektziel

Eine App, die auf einem Android-Tablet im Dauerbetrieb Fotos aus verschiedenen Quellen (lokaler Speicher, NAS, Cloud) als ansprechende Diashow anzeigt – zuverlässig, wartungsarm und ohne laufende Kosten.

### 1.3 Abgrenzung (Nicht-Ziele)

- Keine Foto-Bearbeitung (Filter, Zuschnitt, Retusche).
- Keine eigene Cloud-Infrastruktur / kein eigener Server-Betrieb als Teil des Projekts.
- Keine Unterstützung für iOS, Desktop oder Smart-TVs (Tauri-Desktop-Builds dürfen als Nebenprodukt existieren, sind aber nicht Abnahmegegenstand).
- Keine Mehrbenutzer-/Familienverwaltung mit Rechten und Rollen.
- Keine Videowiedergabe in Version 1 (siehe KANN-Anforderungen).

### 1.4 Zielgruppe und Einsatzumgebung

- Privater Haushalt, ein bis wenige Geräte.
- Tablet steht dauerhaft mit Netzteil an einem festen Ort (Regal, Wandhalterung), verbunden mit dem heimischen WLAN.
- Bedienung erfolgt selten (Ersteinrichtung, gelegentliche Anpassung); die App läuft ansonsten unbeaufsichtigt.

---

## 2. Rahmenbedingungen

| Nr. | Rahmenbedingung |
|---|---|
| RB-01 | Technologie-Basis: Tauri 2.x mit Rust-Backend; Frontend-Framework: **Vue 3** (Entscheidung E-06). |
| RB-02 | Zielplattform: Android 10 oder neuer, Tablets mit min. 2 GB RAM; Ziel-Formfaktor 7–13 Zoll, quer oder hochkant montiert (E-26). Referenzgerät ist das Xiaomi Pad 6 (E-10). |
| RB-03 | Distribution: Veröffentlichung im **Google Play Store** (Entscheidung E-12); zusätzlich Sideloading per APK/GitHub-Release. Konsequenzen: Play-Console-Konto (einmalig), App-Bundle-Format (AAB), Einhaltung der jeweils aktuellen Target-API-Vorgaben, Data-Safety-Formular und öffentliche Datenschutzerklärung. |
| RB-04 | Budget: keine laufenden Kosten für Dritt-Dienste im Regelbetrieb (einmalige Kosten, z. B. Play-Console-Konto, sind zulässig). |
| RB-05 | Lizenz: Das Projekt wird als **Open Source unter Apache 2.0** veröffentlicht (Entscheidung E-12); eingebundene Bibliotheken müssen Apache-2.0-kompatibel sein (permissive Lizenzen). |
| RB-06 | Das Projekt wird von einer Person in der Freizeit entwickelt; Wartungsaufwand ist zu minimieren. |

---

## 3. Funktionale Anforderungen

Priorisierung: **MUSS** (zwingend), **SOLL** (wichtig, verhandelbar), **KANN** (wünschenswert).

### 3.1 Diashow und Anzeige

| Nr. | Prio | Anforderung |
|---|---|---|
| FA-01 | MUSS | Die App zeigt Fotos als Endlos-Diashow im Vollbild an (Immersive Mode, ohne Status- und Navigationsleiste). |
| FA-02 | MUSS | Das Anzeigeintervall ist konfigurierbar (mindestens 5 Sekunden bis 30 Minuten). |
| FA-03 | MUSS | Die Reihenfolge ist wählbar: zufällig oder sortiert (Dateiname, Aufnahme-/Änderungsdatum). |
| FA-04 | MUSS | Unterstützte Bildformate: JPEG, PNG, WebP; Fotos werden unter Berücksichtigung der EXIF-Orientierung korrekt gedreht angezeigt. |
| FA-05 | MUSS | Hoch- und Querformatbilder werden ohne Verzerrung dargestellt (Modi: „Einpassen mit Hintergrund" und „Formatfüllend mit Beschnitt", umschaltbar). |
| FA-06 | SOLL | Weiche Überblendungen zwischen Bildern (Dauer konfigurierbar, abschaltbar). |
| FA-07 | SOLL | Einblendbare Zusatzinformationen: Uhrzeit, Datum, optional Dateiname/Aufnahmedatum des Fotos (einzeln zu-/abschaltbar). Erweitert durch E-19 um die beiden Schaltflaechen oben rechts. Die Uhr steht wahlweise als Ziffern oder als Zeiger (E-20). |
| FA-08 | SOLL | Bei Hochformatfotos auf Querformat-Display: zwei Hochformatbilder nebeneinander anzeigen (Paar-Modus). Hochkant montiert gilt das gespiegelt: zwei Querformatfotos übereinander (E-26). |
| FA-09 | KANN | HEIC/HEIF wird **nicht nativ dekodiert** (Entscheidung E-04): Für Nextcloud-Quellen liefert die Preview-API JPEG-Versionen auch von HEIC-Originalen; HEIC-Dateien aus lokalen/NAS-Quellen werden übersprungen und im Log vermerkt. |
| FA-10 | KANN | „Ken-Burns-Effekt" (langsames Zoomen/Schwenken) als optionaler Anzeigemodus. |
| FA-11 | — | *Entfällt* (Entscheidung E-07): Keine Videowiedergabe – Widerspruch zum Nicht-Ziel in Abschnitt 1.3 aufgelöst; Videodateien in Quellen werden ignoriert. |

### 3.2 Bildquellen

| Nr. | Prio | Anforderung |
|---|---|---|
| FA-20 | MUSS | Lokale Quelle: Auswahl eines oder mehrerer Ordner auf dem Gerätespeicher bzw. der SD-Karte über den Android-Systemdialog (Storage Access Framework); die App merkt sich die Freigaben dauerhaft. |
| FA-21 | MUSS | Netzwerkquelle: Anbindung eines NAS über **WebDAV** (URL, Benutzername, Passwort; HTTPS und HTTP im Heimnetz). |
| FA-22 | — | *Entfällt* (Entscheidung E-02): Keine SMB-Unterstützung; NAS-Anbindung erfolgt ausschließlich über WebDAV (FA-21). |
| FA-23 | SOLL | Cloud-Quelle: Anbindung an **Nextcloud** (Entscheidung E-03): Auswahl von **Photos-Alben** über den Photos-WebDAV-Endpunkt (`remote.php/dav/photos/…`); Bilder werden bevorzugt über die **Preview-API** in displaygerechter Größe bezogen (entlastet NF-12 und löst HEIC serverseitig, siehe FA-09). |
| FA-24 | — | *Entfällt* (Entscheidung E-08): Keine Google-Photos-Anbindung (API-Lage, R-07); Einspeisung ggf. über Nextcloud-Auto-Upload der Handy-Fotos. |
| FA-25 | MUSS | Mehrere Quellen können gleichzeitig aktiv sein; pro Quelle ist einstellbar, ob sie in die Diashow einfließt. |
| FA-26 | MUSS | Entfernte Quellen (NAS/Cloud) werden in einen lokalen Cache synchronisiert; die Diashow läuft ausschließlich aus dem Cache und damit auch bei Netzwerkausfall unterbrechungsfrei weiter. |
| FA-27 | MUSS | Der Cache ist **permanent** (überlebt App- und Geräteneustart) und arbeitet als **rotierender Speicher** (Ringpuffer): Die Cachegröße ist konfigurierbar (Standard z. B. 2 GB); bei Überschreitung werden die ältesten bzw. am längsten nicht angezeigten Bilder automatisch verdrängt und durch neue von der Quelle ersetzt. |
| FA-31 | MUSS | **Vorausladen (Prefetch):** Für eine flüssige Wiedergabe werden die nächsten Fotos der Diashow im Voraus geladen und dekodiert (Standard: 5 Bilder, konfigurierbar); Bildwechsel greifen ausschließlich auf bereits vorgeladene Bilder zu. |
| FA-28 | SOLL | Synchronisierung erfolgt periodisch (Intervall konfigurierbar) sowie manuell auslösbar; neue Bilder erscheinen ohne App-Neustart in der Diashow. |
| FA-29 | SOLL | Filterung pro Quelle: nur bestimmte Unterordner/Alben, optional Mindestauflösung. |
| FA-30 | KANN | Ausschlussliste: einzelne Bilder können aus der Diashow entfernt werden („dieses Bild nicht mehr zeigen"), ohne sie an der Quelle zu löschen. |

### 3.3 Bedienung und Konfiguration

| Nr. | Prio | Anforderung |
|---|---|---|
| FA-40 | MUSS | Einstellungen sind direkt auf dem Tablet über eine Touch-Oberfläche erreichbar (z. B. Tippen/Wischen holt ein Menü hervor). |
| FA-41 | MUSS | Während der Diashow: Wischgesten für vor/zurück, Tippen für Pause/Weiter. Konkretisiert durch E-18: zusätzlich Tippzonen (links zurück, Mitte Pause, rechts weiter). Die Pause wird dauerhaft angezeigt, solange sie gilt (E-21). |
| FA-42 | MUSS | Alle Einstellungen und Quellen-Konfigurationen überleben App- und Geräteneustart. |
| FA-43 | SOLL | Versehentliche Bedienung ist erschwert (z. B. Einstellungen erst nach Doppeltipp oder langem Druck erreichbar). |
| FA-44 | — | *Entfällt als eigene Anforderung* (Entscheidung E-09): Fernzugriff auf Grundeinstellungen (Intervall, Zeitplan, Sync anstoßen, Bildschirm an/aus) erfolgt über die REST-Endpunkte aus FA-55; keine eigene Weboberfläche. |
| FA-45 | KANN | Import/Export der Konfiguration als Datei. |

### 3.4 Dauerbetrieb und Zeitsteuerung

| Nr. | Prio | Anforderung |
|---|---|---|
| FA-50 | MUSS | Der Bildschirm bleibt während der Diashow dauerhaft an (Wakelock / FLAG_KEEP_SCREEN_ON). |
| FA-51 | — | *Entfällt* (Entscheidung E-01): Kein automatischer Start nach dem Booten; nach einem Neustart/Stromausfall wird die App manuell gestartet. Die laufende App muss die Diashow nach App-Start ohne weitere Interaktion beginnen. |
| FA-52 | MUSS | Zeitplan: Konfigurierbare Aktiv-Zeiten (z. B. 07:00–22:00 Uhr); außerhalb wird der Bildschirm geschwärzt bzw. die Helligkeit maximal reduziert. Bei gerätegesteuerter Helligkeit (E-22) wird nur geschwärzt. |
| FA-53 | SOLL | Helligkeitssteuerung: manuelle Einstellung in der App; optional automatische Absenkung in Abendstunden. Alternativ gibt die App die Regelung vollständig an das Gerät ab (E-22); der Schalter dafür ist auch über FA-55 erreichbar. |
| FA-54 | SOLL | Nachtmodus zeigt optional eine dunkle Uhr statt eines komplett schwarzen Bildschirms. Ihre Darstellung wird getrennt von der Tagesuhr gewählt (E-20). |
| FA-55 | SOLL | Steuerung über das Heimnetz (einfache REST-Endpunkte oder MQTT) zur Integration in Home Assistant: Diashow an/aus, Bildschirm an/aus. Ersetzt die gestrichene Kamera-Präsenzerkennung (Entscheidung E-05) – ein vorhandener Smart-Home-Bewegungsmelder übernimmt das Aufwecken. Zusätzlich stellen die REST-Endpunkte Grundeinstellungen bereit (Intervall, Zeitplan, Sync anstoßen; Entscheidung E-09). |
| FA-56 | — | *Entfällt* (Entscheidung E-05): Keine Präsenzerkennung per Frontkamera (hoher Aufwand: eigenes CameraX-Plugin; Privatsphäre); Funktion wird durch FA-55 abgedeckt. |

---

## 4. Nichtfunktionale Anforderungen

| Nr. | Prio | Anforderung |
|---|---|---|
| NF-01 | MUSS | Stabilität: Die App läuft mindestens 7 Tage ohne Neustart, ohne Absturz und ohne sichtbare Verschlechterung (Speicherlecks, Ruckeln). |
| NF-02 | MUSS | Selbstheilung: Nach einem Absturz startet die Diashow automatisch neu (Watchdog/Recovery-Mechanismus). |
| NF-03 | MUSS | Performance: Bildwechsel erfolgen flüssig (keine sichtbaren Ladepausen, siehe FA-31); Bilder werden vor der Anzeige auf Displayauflösung herunterskaliert, um RAM zu schonen. Der Prefetch-Puffer (FA-31) ist so begrenzt, dass er den RAM-Verbrauch nicht gefährdet (vgl. R-03). |
| NF-04 | MUSS | Datenschutz: Fotos und Zugangsdaten verlassen das Gerät ausschließlich Richtung der konfigurierten eigenen Quellen; keine Telemetrie, keine Werbung, keine Drittanbieter-Tracker. |
| NF-05 | MUSS | Zugangsdaten (NAS/Cloud) werden verschlüsselt gespeichert (Android Keystore oder gleichwertige Verschlüsselung, z. B. Stronghold; vgl. Abschnitt 8). |
| NF-06 | SOLL | Ressourcen: CPU-Last im Anzeigebetrieb im Mittel < 15 % (Referenzgerät), um Wärmeentwicklung und Energieverbrauch im 24/7-Betrieb gering zu halten. |
| NF-07 | SOLL | Einbrennschutz: Statische Overlays (Uhr etc.) verschieben sich periodisch um wenige Pixel (Pixel-Shift), relevant für OLED-Displays. |
| NF-08 | SOLL | Die Ersteinrichtung (lokaler Ordner als Quelle bis zur laufenden Diashow) ist in unter 5 Minuten ohne Anleitung möglich. |
| NF-09 | SOLL | Sprache der Oberfläche: Deutsch; Architektur soll weitere Sprachen ermöglichen (i18n). |
| NF-10 | SOLL | APK-Größe unter 30 MB; Updates sind durch einfaches Überinstallieren der neuen APK möglich, ohne Konfigurationsverlust. |
| NF-11 | KANN | Barrierefreiheit: ausreichende Kontraste und Schriftgrößen in der Einstellungsoberfläche. |

### 4.1 Performance-Optimierungen

| Nr. | Prio | Anforderung |
|---|---|---|
| NF-12 | MUSS | **Skalierte Cache-Ablage:** Bilder werden bereits beim Synchronisieren einmalig auf Displayauflösung herunterskaliert und platzsparend re-encodiert (z. B. WebP) im Cache abgelegt; Originale an der Quelle bleiben unverändert. Anzeige und Prefetch (FA-31) arbeiten ausschließlich mit diesen optimierten Versionen. |
| NF-13 | MUSS | **Dekodierung im Rust-Backend:** Bilddekodierung und -skalierung erfolgen im Rust-Prozess, nicht in der WebView; das Frontend erhält fertige, displaygroße Bilder über das Tauri-Asset-Protokoll. Dies begrenzt den WebView-Speicherverbrauch (vgl. R-03). |
| NF-14 | MUSS | **Delta-Synchronisierung:** Beim Sync entfernter Quellen werden nur neue oder geänderte Dateien übertragen (Abgleich über ETag bzw. Änderungsdatum/Dateigröße); vollständige Neuübertragungen finden nicht statt. |
| NF-15 | SOLL | **Thumbnail-Index:** Für die Einstellungs- und Auswahloberfläche (Alben-Browsing, Ausschlussliste FA-30) werden kleine Vorschaubilder vorgehalten, damit die Bedienung auch bei großen Bibliotheken flüssig bleibt. |
| NF-16 | MUSS | **GPU-schonende Übergänge:** Überblendungen nutzen ausschließlich compositor-freundliche Eigenschaften (transform/opacity) ohne Layout-Reflows; es befinden sich maximal zwei Bilder gleichzeitig im DOM. |

---

## 5. Lieferumfang und Abnahme

### 5.1 Lieferumfang

- Signierte, installierbare APK (arm64-v8a, optional armeabi-v7a).
- Quellcode in einem Git-Repository inkl. Build-Anleitung (Rust-/Android-Toolchain, `cargo tauri android build`).
- Kurze Bedienungs-/Einrichtungsanleitung (README).
- Play-Store-Release: signiertes App-Bundle (AAB), Store-Eintrag mit Screenshots, Data-Safety-Angaben und öffentlicher Datenschutzerklärung (RB-03).
- LICENSE-Datei (Apache 2.0) und Drittlizenz-Übersicht im Repository (RB-05).

### 5.2 Abnahmekriterien (Auszug)

- Alle MUSS-Anforderungen sind erfüllt und auf dem Referenz-Tablet nachgewiesen.
- Dauertest: 7 Tage unterbrechungsfreier Betrieb mit aktiver NAS-Quelle, inkl. eines simulierten Netzwerkausfalls von mindestens 24 h (Diashow läuft aus dem Cache weiter, FA-26).
- Neustart-Test: Nach manuellem App-Start beginnt die Diashow ohne weitere Interaktion; Konfiguration und Cache sind nach Stromtrennung vollständig erhalten (FA-42, FA-27).
- Speicher-Test: Bibliothek mit mindestens 5 000 Bildern, darunter Dateien > 20 MP, ohne Absturz (NF-01, NF-03).

---

## 6. Risiken

| Nr. | Risiko | Auswirkung | W'keit | Gegenmaßnahme |
|---|---|---|---|---|
| R-01 | **Tauri-Android-Reife:** Android-Support in Tauri 2 ist deutlich jünger als der Desktop-Support; Plugins (Filesystem, Autostart, Keystore u. a.) sind mobil teils lückenhaft oder fehlerhaft. | Hoch – Kernfunktionen ggf. nur mit eigenem Rust-/Kotlin-Plugin-Code umsetzbar; Mehraufwand. | Mittel–Hoch | Früher technischer Durchstich (Spike) für die kritischen Punkte: SAF-Ordnerzugriff, Wakelock, Autostart, Vollbild. Erst danach Feature-Entwicklung. Fallback-Entscheidung dokumentieren (siehe R-10). |
| R-02 | **WebView-Fragmentierung:** Die Darstellung hängt von der auf dem Tablet installierten Android-System-WebView-Version ab; alte Tablets haben alte WebViews. | Mittel – Rendering-Fehler, fehlende CSS/JS-Features, Ruckeln bei Überblendungen. | Mittel | Frontend konservativ bauen (Baseline statt neuester Web-APIs), auf dem realen Zielgerät testen, GPU-lastige Effekte (FA-06, FA-10) abschaltbar machen. |
| R-03 | **Speicherverbrauch der WebView:** Große Bilder in einer WebView können den RAM sprengen; Android beendet die App dann kommentarlos (OOM-Kill). | Hoch – App „verschwindet" im Dauerbetrieb. | Mittel | Downscaling vor Anzeige (NF-03), nur 2–3 Bilder gleichzeitig im DOM halten, Dekodierung im Rust-Backend statt im Frontend erwägen; Recovery-Mechanismus (NF-02). |
| R-04 | **Android-Energieverwaltung:** Hersteller-spezifische Batterie-Optimierungen (v. a. Samsung, Xiaomi, Huawei) beenden oder drosseln langlaufende Apps trotz Wakelock. | Hoch – Diashow stoppt unbemerkt. | Mittel–Hoch | Foreground-Service, Ausnahme von der Akku-Optimierung anfordern, Einrichtungsanleitung pro Hersteller; Watchdog (NF-02); Test auf dem konkreten Zielgerät. |
| R-05 | **Autostart-Beschränkungen:** `BOOT_COMPLETED`-Autostart wird von neueren Android-Versionen und Hersteller-ROMs eingeschränkt. | *Entschärft durch E-01:* Auf Autostart wird verzichtet; nach Stromausfall ist ein manueller Start nötig (bewusst akzeptiert). | — | Restmaßnahme: App startet nach manuellem Öffnen ohne weitere Interaktion in die Diashow. Kiosk-/Launcher-Modus bleibt als spätere Option dokumentiert. |
| R-06 | **Scoped Storage / SAF:** Seit Android 10/11 ist der Dateizugriff stark reglementiert; Ordnerzugriffe laufen über URIs statt Pfade, was mit Rust-Dateisystem-Code kollidiert. | Mittel – lokaler Quellordner (FA-20) aufwendiger als erwartet. | Hoch | Zugriff konsequent über SAF-URIs planen (ggf. eigener Plugin-Code in Kotlin); Bilder in den App-eigenen Cache kopieren, auf den Rust direkt zugreifen kann. |
| R-07 | **Google-Photos-API:** Google hat die Photos Library API für Dritt-Apps stark beschnitten; Zugriff auf die eigene Bibliothek ist kaum noch sinnvoll möglich. | Niedrig für das Projekt (FA-24 ist KANN), aber Erwartung ggf. enttäuscht. | Hoch | FA-24 bewusst als KANN eingestuft; Nextcloud/Immich (FA-23) als tragfähige Cloud-Alternative festgeschrieben. |
| R-08 | **Dauerbetrieb-Hardware:** 24/7-Betrieb mit dauerhaft geladenem Akku lässt Tablet-Akkus altern und aufblähen (Sicherheitsrisiko); Display-Einbrennen bei OLED. | Mittel – Hardware-Schaden, im Extremfall Brandgefahr. | Mittel | Zeitplan mit Bildschirm-Aus-Phasen (FA-52), Hinweis in der Anleitung (Ladebegrenzung/Smart-Plug), Pixel-Shift (NF-07). Rein organisatorisch/hardwareseitig lösbar, aber im Lastenheft zu benennen. |
| R-09 | **Netzwerkprotokolle in Rust:** SMB-Client-Bibliotheken im Rust-Ökosystem sind unreif; WebDAV ist deutlich besser abgedeckt. | *Geschlossen durch E-02:* SMB wurde gestrichen; NAS-Anbindung ausschließlich über WebDAV. | — | NAS-seitig WebDAV aktivieren (bei gängigen NAS-Systemen mit wenigen Klicks möglich). |
| R-10 | **Technologie-Sackgasse:** Stellt sich Tauri auf Android als ungeeignet heraus, ist ein Wechsel (z. B. zu Kotlin/Compose oder Flutter) spät im Projekt teuer. | Hoch – Neuentwicklung großer Teile. | Niedrig–Mittel | Meilenstein „Proof of Concept" (siehe 7) mit explizitem Go/No-Go; Frontend-Logik framework-neutral halten, Geschäftslogik in Rust kapseln (wiederverwendbar). |
| R-11 | **Ein-Personen-Projekt:** Motivation/Zeit schwanken; Abhängigkeits-Updates (Tauri, Android SDK) erzeugen laufenden Pflegeaufwand. | Mittel – Projekt bleibt unfertig oder veraltet. | Mittel | Kleiner MVP-Schnitt (nur MUSS), Feature-Disziplin, Versionen pinnen, CI-Build zur Reproduzierbarkeit. |
| R-13 | **Play-Store-Pflegepflicht:** Google verlangt jährlich steigende Target-API-Level und reagiert mit Review-Auflagen (Foreground-Service-Begründung, Data Safety); veraltete Apps werden ausgeblendet oder entfernt. | Mittel – laufender Pflegeaufwand über den Funktionsumfang hinaus (verschärft R-11). | Hoch | Jährliches Wartungsfenster einplanen; CI-Build; Sideload-/GitHub-Release als vom Store unabhängiger Verteilweg (RB-03). |
| R-12 | **Zugangsdaten-Sicherheit:** NAS-/Cloud-Passwörter auf einem frei zugänglichen Wohnzimmer-Gerät. | Mittel – bei Geräteverlust/Zugriff Dritter kompromittiert. | Niedrig | Keystore-Verschlüsselung (NF-05), eigenes NAS-Konto mit Nur-Lese-Rechten auf die Fotoordner verwenden (Empfehlung in Anleitung). |

---

## 7. Grobe Meilensteine (Vorschlag)

| Meilenstein | Inhalt | Abschlusskriterium |
|---|---|---|
| M1 – Proof of Concept | Tauri-Android-Build läuft auf dem Zielgerät; Vollbild, Wakelock, SAF-Ordnerzugriff, einfache Diashow aus lokalem Ordner. | Go/No-Go-Entscheidung für Tauri (R-01, R-10). |
| M2 – MVP | Alle MUSS-Anforderungen: lokale Quelle + WebDAV, Cache, Zeitplan, Einstellungen. | 7-Tage-Dauertest bestanden. |
| M3 – Komfort | SOLL-Anforderungen: Nextcloud (Photos-Alben, Preview-API), Heimnetz-Steuerung (FA-55), Überblendungen, Overlays, Paar-Modus, Nachtmodus. | Abnahme der umgesetzten SOLL-Punkte. |
| M4 – Extras | Ausgewählte KANN-Anforderungen nach Bedarf. | — |

---

## 8. Machbarkeitsbewertung Tauri auf Android (Stand: August 2026)

Prüfung aller Anforderungen gegen den aktuellen Stand von Tauri 2 auf Android. Legende: ✅ direkt mit Tauri/offiziellen Plugins abbildbar · 🟡 abbildbar mit Community-Plugin oder wenig eigenem Kotlin-Code · 🔴 kritisch, im Proof of Concept (M1) zwingend zu verifizieren.

| Anforderung(en) | Bewertung | Befund |
|---|---|---|
| Diashow-Frontend, Gesten, Überblendungen, Overlays (FA-01–FA-08, FA-40–FA-43, NF-16) | ✅ | Reines WebView-Frontend (HTML/CSS/JS) – plattformunabhängig; Gesten und transform/opacity-Übergänge sind Standard-Webtechnik. |
| Einstellungen persistent (FA-42) | ✅ | Offizielles Store-Plugin bzw. eigene Config-Datei im App-Verzeichnis (fs-Plugin: App-Ordner ist auf Android ohne Sonderrechte zugreifbar). |
| WebDAV-Anbindung, Delta-Sync (FA-21, FA-23, NF-14) | ✅ | Offizielles HTTP-Plugin unterstützt Android; alternativ direkt `reqwest`/WebDAV-Crates im Rust-Backend (rustls läuft auf Android). Nextcloud/Immich-REST ebenso. |
| Bilddekodierung/-skalierung in Rust, Auslieferung ans Frontend (NF-03, NF-12, NF-13, FA-31) | ✅ | `image`-Crate (JPEG/PNG/WebP) ist reines Rust und läuft auf Android; Auslieferung über Tauri-Custom-Protocol/Asset-Protocol. Funktioniert nach Doku-Lage auch mobil, gehört aber in den PoC-Testumfang. |
| Lokaler Quellordner via SAF (FA-20) | 🟡 | Das offizielle fs-Plugin ist auf Android **auf den App-Ordner beschränkt** – kein SAF, keine Content-URIs. Lösung: Community-Plugin **`tauri-plugin-android-fs`** (SAF-Ordnerauswahl, Dateizugriff) oder eigener Kotlin-Code. |
| Bildschirm dauerhaft an (FA-50) | 🟡 | Kein offizielles Plugin; Community-Plugin **`tauri-plugin-keep-screen-on`** (Android/iOS) oder eine Zeile Kotlin (`FLAG_KEEP_SCREEN_ON`) in der generierten MainActivity. |
| Schutz vor Akku-Optimierung / Dauerbetrieb (NF-01, R-04) | 🟡 | Community-Plugin **`tauri-plugin-android-battery-optimization`** (Ausnahme anfordern); ein Foreground-Service erfordert eigenen Kotlin-Code im generierten Android-Projekt – machbar, da Tauri das Projekt offenlegt. |
| Vollbild/Immersive Mode (FA-01) | 🟡 | Nicht über die Tauri-API steuerbar; wenige Zeilen Kotlin in der MainActivity (Immersive-Sticky-Flags). Unkritisch, aber eigener nativer Code. |
| Autostart nach Boot (FA-51) | 🟡 | Das offizielle Autostart-Plugin ist faktisch **Desktop-only**. *Per E-01 gestrichen* – manueller Start akzeptiert; Kiosk-/Launcher-Modus bleibt dokumentierte Ausbauoption. |
| Zugangsdaten-Verschlüsselung (NF-05) | 🟡 | Stronghold-Plugin unterstützt Android, bietet aber **keine Android-Keystore-Anbindung** (Passwort-/Schlüsselableitung selbst zu lösen). Direkte Keystore-Nutzung nur per eigenem Kotlin-Plugin. Anforderung bleibt erfüllbar, Formulierung „Android Keystore" in NF-05 wird auf „Keystore oder gleichwertige Verschlüsselung (z. B. Stronghold)" erweitert. |
| Zeitplan, Helligkeit, Nachtmodus (FA-52–FA-54) | 🟡 | Zeitsteuerung in Rust/JS trivial; Software-Dimmen (schwarzes Overlay) rein im Frontend ✅; echte Displayhelligkeit erfordert wenige Zeilen Kotlin (WindowManager-Attribute). |
| SMB-Anbindung (FA-22) | 🔴 | Rust-SMB-Clients sind unreif bzw. binden C-Bibliotheken (libsmbclient) ein, deren Android-Cross-Compile aufwendig ist. *Per E-02 gestrichen* – nur WebDAV (R-09 geschlossen). |
| HEIC/AVIF (FA-09) | 🔴 | HEIC benötigt libheif (C, Patentthematik, Android-Build aufwendig); AVIF via `libavif`-Bindings möglich, aber ebenfalls Build-Aufwand. KANN-Einstufung bestätigt – ggf. streichen oder serverseitig konvertieren. |
| Video in Diashow (FA-11) | 🟡 | HTML5-`<video>` wäre machbar gewesen; *per E-07 gestrichen* (Widerspruch zu Abschnitt 1.3 aufgelöst). |
| Fernkonfiguration/REST/MQTT (FA-44, FA-55) | 🟡 | Ein Rust-HTTP-Server (z. B. axum) bzw. MQTT-Client (`rumqttc`, reines Rust) läuft im App-Prozess auch auf Android; benötigt Foreground-Service, damit Android ihn nicht einfriert. |
| Präsenzerkennung per Kamera (FA-56) | 🔴 | Kein Tauri-Zugang zur Kamera-Vorschau; erfordert vollständiges eigenes Kotlin-Plugin (CameraX). *Per E-05 gestrichen* – ersetzt durch Smart-Home-Steuerung (FA-55, jetzt SOLL). |

**Fazit:** Alle MUSS-Anforderungen sind mit Tauri 2 auf Android abbildbar – drei davon (SAF-Ordnerzugriff, Wakelock, Foreground-Service/Akku-Ausnahme) nur über Community-Plugins oder eigenen Kotlin-Code im generierten Android-Projekt. Genau diese bilden den Testumfang des Proof of Concept (M1). Die ursprünglich kritischen Punkte SMB, HEIC-Dekodierung und Kamera-Präsenzerkennung wurden per Entscheidung E-02/E-04/E-05 gestrichen bzw. serverseitig/per Smart-Home gelöst (siehe Abschnitt 9).

## 9. Entscheidungsprotokoll

| Nr. | Entscheidung | Gewählte Option | Begründung / Konsequenz |
|---|---|---|---|
| E-01 | Autostart nach Boot | **Manueller Start genügt** | FA-51 entfällt; R-05 entschärft. Nach Stromausfall startet man die App per Hand; sie muss dann ohne weitere Interaktion in die Diashow gehen. Kiosk-/Launcher-Modus bleibt als spätere Ausbauoption dokumentiert. |
| E-02 | NAS-Protokoll | **Nur WebDAV, SMB gestrichen** | FA-22 entfällt; R-09 geschlossen. Größtes Bibliotheks-Risiko eingespart; WebDAV ist auf gängigen NAS-Systemen schnell aktiviert. |
| E-03 | Cloud-Dienst | **Nextcloud (Photos-Alben + Preview-API)** | FA-23 konkretisiert: Alben über den Photos-WebDAV-Endpunkt, Bildabruf bevorzugt über die Preview-API in displaygerechter Größe – entlastet NF-12 und liefert HEIC-Originale als JPEG. |
| E-04 | HEIC/HEIF | **Serverseitig lösen** | Kein libheif-Build. Nextcloud-Quellen zeigen HEIC über Previews; lokale/NAS-HEIC-Dateien werden übersprungen und geloggt (FA-09 angepasst). |
| E-05 | Anwesenheitssteuerung | **Smart-Home statt Kamera** | FA-56 entfällt; FA-55 (REST/MQTT-Steuerung, z. B. Home Assistant mit Bewegungsmelder) von KANN auf SOLL angehoben. |
| E-06 | Frontend-Framework | **Vue 3** | RB-01 konkretisiert. Offiziell von Tauri-Templates unterstützt, gute Balance aus Ökosystem und Runtime-Größe; NF-10 (APK < 30 MB) bleibt erreichbar. |
| E-07 | Videowiedergabe | **Gestrichen** | FA-11 entfällt; Widerspruch zum Nicht-Ziel (Abschnitt 1.3) aufgelöst. Videodateien in Quellen werden ignoriert. |
| E-08 | Google Photos | **Gestrichen** | FA-24 entfällt (API-Beschränkungen, R-07); Handy-Fotos gelangen über den Nextcloud-Auto-Upload in die Diashow. |
| E-09 | Fernkonfiguration | **In FA-55 integriert** | Keine eigene Weboberfläche (FA-44 entfällt); die REST-Endpunkte aus FA-55 decken Grundeinstellungen mit ab. |
| E-10 | Referenz-Tablet | **Xiaomi Pad 6** (`pipa`, 23043RP34G) | Android 14 unter HyperOS 2.0, 2880 x 1800 bei 400 dpi, 11 Zoll Querformat, 8 GB RAM. Erfuellt RB-02 mit Abstand. Damit sind NF-01 (7 Tage stabil), NF-06 (CPU-Last) und R-02 (WebView-Version, hier Chromium 151) erstmals pruefbar, und die Screenshots fuer den Play-Store-Eintrag koennen vom Zielgeraet kommen. Zu beachten ist die HyperOS-Sperre fuer Installationen ueber USB (siehe 11.4) -- sie betrifft die Entwicklung, nicht das Produkt. |
| E-11 | App-Name | **Slowshow** | Erfundenes, praktisch konfliktfreies Wortspiel („entschleunigte Slideshow“) – gut suchbar im Play Store, international verständlich. Ersetzt den Platzhalter „FrameOS“. |
| E-12 | Open Source & Distribution | **Apache 2.0, Veröffentlichung im Play Store** | RB-03/RB-05 angepasst; Lieferumfang um AAB, Store-Eintrag, Datenschutzerklärung und LICENSE erweitert; neues Risiko R-13 (Play-Store-Pflegepflicht) aufgenommen. Apache 2.0 passt zum Rust-/Tauri-Ökosystem. |
| E-13 | Design & App-Icon | **Galerie-minimal; Icon „Rahmen & Horizont"** (Icon fortgeschrieben durch E-27) | Designsystem: Tiefschwarz #0A0A0A (OLED-freundlich, stützt NF-07), Off-White #F2EFE9, Akzent Messing #C2A878; Instrument Sans (UI) + Cormorant Garamond (Wortmarke/Bildunterschriften). App-Icon: weißer Rahmen mit Messing-Horizont und -Sonne auf Schwarz; unter 48 px entfällt die Sonne. Mockups im Design-Canvas „Slowshow App-Design". |
| E-14 | Format der Cache-Ablage | **JPEG (Qualität 85) statt WebP** | NF-12 nennt WebP als Beispiel („z. B."). Ein verlustbehafteter WebP-Encoder existiert in Rust nur als Bindung an libwebp (C) und müsste für Android cross-kompiliert werden – dieselbe Aufwandsklasse, die bei HEIC (E-04) und SMB (E-02) bewusst gemieden wurde. Der JPEG-Encoder des `image`-Crates ist reines Rust. WebP-**Dekodierung** bleibt erhalten, es ist also weiterhin ein zulässiges Quellformat (FA-04). |
| E-15 | Ablage des Cache-Index | **JSON im Speicher statt SQLite** | Bei der Zielgröße aus 5.2 (5 000 Bilder) sind das wenige MB, die einmal beim Start geladen werden. Erspart die Cross-Kompilierung von libsqlite3 für Android. Der Index wird atomar geschrieben und beim Start gegen die vorhandenen Dateien abgeglichen (NF-02). |
| E-16 | Ablage des nativen Android-Codes | **Versioniert in `src-tauri/android-src/`, per Skript nach `gen/` gespiegelt** | `src-tauri/gen/` ist generiert und gitignored; handgeschriebener Code dort ginge bei `tauri android init` unbemerkt verloren. Für Slowshow kritisch, weil FA-01, FA-50 und FA-53 genau dort liegen. `scripts/patch-android.mjs` spielt den Code ein, ergänzt das Manifest und prüft das Ergebnis; es läuft vor jedem Android-Build und in der CI. |
| E-19 | Schaltflaechen in der Diashow | **Zahnrad und durchgestrichenes Auge, einzeln abschaltbar** | Der Entwurf sieht im Artboard „Diashow“ keine Schaltflaechen vor. Zwei sind trotzdem sinnvoll: ein kurzer Weg in die Einstellungen (FA-40) und das Ausblenden des laufenden Bildes (FA-30) — letzteres ist nur im Moment des Anschauens praktisch. Beide sind wie Uhr und Datum einzeln abschaltbar (FA-07); wer den Rahmen puristisch will, blendet sie aus und nutzt den langen Druck. Zuvor trugen sie das „System“-Symbol aus der Navigation (Kreis mit Strahlen) und ein Minus — beides las sich falsch, das eine als Helligkeit, das andere als gar nichts. |
| E-18 | Bedienung der Diashow | **Tippzonen zusätzlich zum Wischen** | FA-41 verlangt „Wischgesten für vor/zurück, Tippen für Pause/Weiter". Wischen bleibt; ergänzt werden drei Tippzonen: linkes Drittel zurück, Mitte Pause, rechtes Drittel weiter. Auf einem an der Wand hängenden Rahmen ist ein kurzer Tipp bequemer als eine Wischbewegung — und die großzügige Mitte fängt Fehlgriffe auf die harmlose Aktion. Drittel, weil das die verbreitete Aufteilung ist (E-Book-Leser) und damit am wenigsten überrascht. Langer Druck öffnet weiterhin die Einstellungen (FA-43). |
| E-20 | Analoguhr | **Getrennt schaltbar, Strichindex, ohne Sekundenzeiger** | Drei Teilfragen, einzeln entschieden. *Ort:* Diashow (FA-07) und Nachtmodus (FA-54) bekommen je einen eigenen Schalter — analog nachts neben digital tagsüber ist eine sinnvolle Kombination, kein Widerspruch. *Stil:* dünner Ring mit zwölf Marken, die auf zwölf/drei/sechs/neun länger; kein Zifferblatt mit Ziffern. Ziffern in der Display-Serife wären die auffälligste Variante gewesen — auf einem Rahmen, dessen Fotos der einzige helle Bereich sein sollen, ist das zu viel. *Sekundenzeiger:* keiner. `useNow` taktet bewusst nur auf die volle Minute (NF-06); ein Sekundenzeiger hielte die WebView rund um die Uhr im Sekundentakt am Zeichnen. Der Stundenzeiger wandert dafür stufenlos mit der Minute mit, sonst stünde er bei einer Uhr ohne Ziffern schlicht falsch. Zum Einbrennen (NF-07): eine Analoguhr ist nicht automatisch besser als eine digitale — die Zeiger rotieren zwar, Ring und Marken stehen aber dauerhaft. Der Pixel-Shift gilt unverändert. |
| E-21 | Anzeige der Pause | **Dauerhaftes Abzeichen statt kurzer Einblendung** | Bisher erschien „Pausiert" für gut zwei Sekunden und verschwand. Ein Rahmen, der stehenbleibt, sieht danach aus wie einer, der hängt — der Hinweis war genau dann weg, wenn später jemand davorstand. Das Abzeichen steht oben in der Mitte, solange die Pause gilt, in Messing statt Off-White: es meldet einen Zustand, keine Meldung. Nicht im Nachtmodus, dort soll der Schirm dunkel bleiben (FA-54). Es wandert wie die übrigen Einblendungen (NF-07) — eine Pause kann Tage dauern. |
| E-22 | Gerätegesteuerte Helligkeit | **Zusätzliche Option, die die Regelung vollständig abgibt — auch nachts** | FA-53 sah nur die Steuerung *durch* die App vor. Wer die Helligkeitsautomatik des Geräts bevorzugt, schaltet sie nun ein; die App setzt dann in **keinem** Zustand mehr eine Fensterhelligkeit (`BRIGHTNESS_OVERRIDE_NONE`) — weder tagsüber, noch nachts, noch auf einen Schlafbefehl aus dem Heimnetz. Eine Ausnahme „nur nachts doch" wäre nicht zu erklären: der Rahmen verhielte sich abends anders als morgens, ohne dass jemand etwas umgestellt hätte. Die abendliche Absenkung entfällt ebenfalls — sie würde gegen die Systemautomatik arbeiten — und die zugehörigen Regler werden ausgeblendet statt wirkungslos stehenzubleiben. **FA-52 bleibt trotzdem erfüllt:** außerhalb der Aktivzeit legt die Oberfläche den Schirm auf Schwarz (`dimOpacity` in `src/lib/dim.ts`). Geschwärzt wird der Inhalt, nur eben nicht zusätzlich die Hintergrundbeleuchtung. Technisch reist der Zustand als Wert `0` im vorhandenen Helligkeitsfeld statt als zweites Feld: die Helligkeit läuft über das Anzeige-Ereignis, REST (FA-55) und MQTT, und ein zusätzliches Feld wäre an jeder Stelle, die es übersieht, stumm wirkungslos. Der Schalter selbst ist fernsteuerbar — über REST als `deviceBrightness` und über MQTT als `cmd/device_brightness` mit eigener Discovery-Entität: wer die Helligkeit in Home Assistant automatisiert, muss die Automatik auch von dort umlegen können. Ein Helligkeitsbefehl über REST oder MQTT wird währenddessen gespeichert, bleibt aber wirkungslos, bis die Gerätesteuerung wieder aus ist — er schaltet sie **nicht** stillschweigend ab. Sonst entschiede eine Automatisierung über eine Einstellung, die niemand angeordnet hat. |
| E-23 | Akku im Dauerbetrieb | **Messen, nicht regeln** | Ein Tablet, das monatelang bei 100 % am Netz haengt, altert schnell; im schlechtesten Fall blaeht sich der Akku auf und drueckt das Display aus dem Rahmen. Genau dieser Betriebsfall steht in R-08, war aber nicht beobachtbar. Die App veroeffentlicht Ladestand, Temperatur und Ladezustand ueber FA-55 (zwei Sensoren und ein Binaersensor in der MQTT-Discovery, `battery` im REST-Status). Die naheliegende Gegenmassnahme — den Ladevorgang ueber einen Zwischenstecker zwischen 40 und 80 % takten — bleibt bewusst aussen vor: sie braucht Hardware, die es im Smart Home gibt, und gehoert dorthin. Dieselbe Trennung, die E-05 fuer die Praesenzerkennung gezogen hat. Meldet das Geraet keinen Wert, steht `battery` auf `null` und die Entitaeten ueberspringen die Meldung, statt eine erfundene Zahl anzuzeigen. |
| E-24 | Vordergrunddienst | **Ja, mit ehrlich benannter Grenze** | Android stuft eine App ohne sichtbare Activity als entbehrlich ein — der wahrscheinlichste Grund, warum ein Rahmen morgens schwarz ist. `SlowshowService` hebt die Prozesspriroritaet dauerhaft an, `START_STICKY` laesst Android ihn nach einem Abschuss neu anlegen. **Kein vollstaendiger Watchdog:** bei `panic = "abort"` reisst ein Rust-Panic den ganzen Prozess mit, und seit Android 10 duerfen Dienste aus dem Hintergrund keine Activity mehr starten — der Weckversuch in `onStartCommand` gelingt nur, solange die App noch als im Vordergrund gilt. NF-02 ist damit besser erfuellt, nicht abgehakt. Dienst-Typ ist `specialUse`: keiner der vorgegebenen Typen trifft zu, und `mediaPlayback` waere schlicht gelogen — das faellt bei der Play-Pruefung auf. Die Begruendung steht im Manifest und gehoert in den Store-Eintrag (RB-03). Das Recht `POST_NOTIFICATIONS` wird deklariert, aber **nicht abgefragt**: fehlt es, laeuft der Dienst unveraendert und die Benachrichtigung bleibt unsichtbar — auf einem Bilderrahmen das bessere Ergebnis. |
| E-25 | Bild-Browser und Vorschaubilder | **Fuenfter Bereich, 320 px, faul erzeugt** | NF-15 (Thumbnail-Index) war als umgesetzt gefuehrt, existierte im Code aber nicht; die Ausschlussliste aus FA-30 zeigte nur Dateinamen und war bei tausend Bildern unbrauchbar. Jetzt gibt es den Bereich „Bilder" mit einem Raster, in dem sich aus- und wieder einblenden laesst; die Ausschlussliste ist aus „Diashow" dorthin umgezogen. *Groesse:* 320 px lange Kante — das Referenzgeraet hat 2,5-fache Pixeldichte, eine Zelle von 128 CSS-Pixeln braucht also 320 echte; 192 px waeren sichtbar weich. Rund 110 MB bei 5 000 Bildern, getrennt in `CacheStats.thumbBytes` ausgewiesen, damit die Cachegroesse in den Einstellungen nicht systematisch zu klein ist. *Zeitpunkt:* beim ersten Betrachten statt beim Synchronisieren — ein Nachziehlauf ueber 5 000 Bilder beim ersten Start nach dem Update waere minutenlang. Am Geraet gemessen entstehen dabei die Vorschaubilder der gesamten geladenen Seite (200 Eintraege, 4,2 MB), nicht nur der sichtbaren Zellen: wie weit `loading="lazy"` vorausschaut, entscheidet Chromium. Die Obergrenze je Schritt ergibt sich daher aus der Seitengroesse. *Gegen R-03:* seitenweise 200 Eintraege statt aller auf einmal (das waeren rund anderthalb Megabyte JSON durch die IPC-Bruecke), dazu `loading="lazy"` und `content-visibility: auto`. |
| E-26 | Hochformat-Montage | **Einstellung in der App, Paar-Modus umgedreht** | Der Rahmen war auf `sensorLandscape` festgenagelt. *Ausrichtung:* als Einstellung (Quer / Hoch / Automatisch) und nicht ueber den Lagesensor — ein fest an die Wand geschraubter Rahmen wird einmal ausgerichtet und soll danach nie wieder drehen, auch nicht, wenn jemand ihn beim Putzen anstoesst. Gesetzt wird sie zur Laufzeit ueber `requestedOrientation`, und zwar als `SENSOR_LANDSCAPE`/`SENSOR_PORTRAIT`: damit ist gleichgueltig, ob der Rahmen um 180 Grad gedreht haengt, das Kabel darf auf der Seite herauskommen, auf der die Steckdose ist. *Paar-Modus:* FA-08 wird gespiegelt statt abgeschaltet — hochkant sind die Querformatfotos die schlecht passenden, zwei davon uebereinander fuellen den Rahmen genauso wie zwei Hochformate ihn quer fuellen. Gepaart wird also immer das Format, das **nicht** zum Rahmen passt. Der `Slide::Pair`-Typ behaelt `left`/`right`: die Anordnung ist Sache der Darstellung, ein zweiter Variantenname wuerde jede Stelle im Frontend verdoppeln. **Offen bleibt der Entwurf:** E-13 kennt nur Querformat-Artboards; die Einblendungen und die Einstellungsnavigation sind hochkant funktionsfaehig, aber nicht gestaltet. |
| E-27 | App-Icon in runder Maske | **Die Kontur ist der Rahmen** | Auf dem Pixel war das Icon beschnitten: das Telefon verwendet runde Masken, und der gezeichnete Rahmen aus E-13 ist ein Quadrat. Androids Sicherheitszone ist ein Kreis von 66 der 108 dp — die Rahmenecken lagen bei Radius 65, erlaubt sind 48,9. Ein Quadrat, dessen Ecken in einen Kreis passen sollen, wird zwangslaeufig klein (74 % waeren noetig gewesen und wirkten verloren). Deshalb zeichnet das Icon den Rahmen nicht mehr selbst: auf Android bildet ihn die Maske des Launchers, ueberall sonst die Kreisscheibe, die `generate-icon.mjs` rendert. Horizont und Sonne bleiben unveraendert und liegen mit 37,6 bzw. 31,6 bequem in der Zone — eine Verkleinerung ist damit hinfaellig. Gilt fuer den **ganzen** Satz, nicht nur fuer Android: ein Motiv ueberall statt zweier Erscheinungsformen. **Bewusste Folge:** Off-White kommt im Icon nicht mehr vor; von den drei Farben aus E-13 tragen es nur noch Oberflaeche und Wortmarke. **Offen:** der Design-Canvas (`slowshow-app-design.html`, Artboard „App-Icon (final)") zeigt weiterhin die quadratische Fassung und ist nachzuziehen. |
| E-28 | Erweiterungspaket: Uebertragung | **Als Anforderung lesen, nicht als Bauplan** | Das Papier vom 30.08.2026 nennt Kotlin, Jetpack Compose, Room, WorkManager und Gradle-Feature-Module — Slowshow ist Tauri 2 mit Vue 3 und Rust, und die Projektanweisung verlangt Geschaeftslogik in Rust. Woertlich umgesetzt hiesse das eine zweite, native App neben der bestehenden. Uebertragen wird deshalb der Inhalt: IMAP-Client und Wiedergabelogik in Rust, Oberflaeche in Vue, der vorhandene JSON-Index statt Room, Tokio-Aufgaben statt WorkManager. **Vier Widersprueche zum geltenden Lastenheft, einzeln entschieden:** Mail-Fotos werden eine vierte `SourceKind` im vorhandenen Cache statt eines zweiten Bestands (`takenAt`, `lastShown` und `excluded` liegen dort schon). HEIC bleibt gestrichen — E-04 gilt unveraendert, iPhone-Anhaenge kommen also teilweise nicht durch. Das Hochformat-Pairing (FA-08, E-26) bleibt entgegen Punkt 17 des Papiers erhalten: die Ziehung liefert ein Foto, die Paarbildung greift danach. Und Favoriten entfallen ganz — damit faellt auch die LRU-Ausnahme aus 1.4 und der Favoriten-Filter aus F5. |
| E-29 | Wiedergabemodi | **Drei neue plus Dateiname, `Modified` entfaellt** | Das Papier kennt Intelligente Mischung, Einfachen Zufall und Chronologisch; FA-03 kannte Zufall, Dateiname, Aufnahmedatum und Aenderungsdatum. `FileName` bleibt, weil alphabetisch bei durchnummerierten Scans der einzige Weg zur richtigen Reihenfolge ist. `Modified` entfaellt — es unterscheidet sich in der Praxis kaum von `TakenAt`, das in „Chronologisch" aufgeht. Alte Konfigurationen mit `takenAt` oder `modified` werden per serde-Alias auf `Chronological` gehoben, damit ein Update keine Einstellung verliert (NF-10). **Luecke des Papiers, selbst geschlossen:** Das Urnen-Modell kennt kein Zurueck, FA-41 verlangt aber Wischen nach rueckwaerts. Der Planer fuehrt deshalb eine begrenzte Historie der zuletzt gezogenen Bilder. |
| E-30 | Mail-Empfang: Zuschnitt | **Eigenes Budget, drei JPEG-Stufen, keine Ken-Burns-Reserve, IDLE spaeter** | *Speicher:* getrenntes Mail-Budget (Default 2 GB) neben dem Ringpuffer aus FA-27, damit ein grosser Nextcloud-Abgleich keine Mail-Fotos verdraengt; die Summe beider Grenzen kann den Geraetespeicher uebersteigen und wird in der Oberflaeche zusammen ausgewiesen. *Qualitaet:* „Sparsam (WebP)" aus 1.3 ist nicht umsetzbar — der WebP-Encoder des `image`-Crates kann ausschliesslich verlustfrei, und verlustfreies WebP eines Fotos ist groesser als ein JPEG mit Qualitaet 85. Stattdessen drei JPEG-Stufen (70/85/95 mit unterschiedlicher Zielaufloesung); libwebp einzubinden waere die Aufwandsklasse, die E-02 und E-04 gemieden haben. *Ken-Burns-Reserve:* entfaellt — die 1,3-fache Aufloesung vergroesserte jedes Bild um rund 69 %. *„Original exportieren" gestrichen:* Punkt 1.4 sieht vor, den Anhang per IMAP frisch zu laden und nach `Pictures/` zu schreiben. Das legt eine zweite, unskalierte Kopie auf dem Geraet ab — auf einem Rahmen mit begrenztem Speicher der falsche Weg, und der Nutzen ist gering, weil das Postfach die Quelle der Wahrheit bleibt und die Mail jederzeit wieder aufrufbar ist. *IDLE:* erst das 15-Minuten-Intervall ueber die vorhandene Hintergrundschleife; die Dauerverbindung mit Wiederaufbau nach Netzwechsel und 29-Minuten-Timeouts ist der Teil, der im Dauerbetrieb schiefgeht, und laesst sich ohne Umbau nachruesten. |
| E-31 | Quarantaene und Wartung: Ort | **Quarantaene doppelt, Wartung nach Thema verteilt** | *Quarantaene:* vierter Filter im Bild-Browser (E-25) **und** ein abschaltbarer Hinweis in der Diashow, wenn etwas wartet. Der Filter allein setzte voraus, dass jemand in die Einstellungen geht; der Hinweis allein widersprach der Ruhe des Entwurfs, wenn er sich nicht abschalten liesse. *Wartung:* kein sechster Navigationsbereich. Statistik und Durchlauf stehen bei der Diashow, die Postfach-Werkzeuge bei den Mail-Einstellungen, Speicher und Datenbank bei System. Bewusste Folge: es gibt keinen einzelnen Ort mehr zum „mal Nachsehen" — dafuer steht jede Funktion dort, wo man sie sucht. Der Diagnose-Export (F11) sammelt trotzdem alles in eine Datei. |
| E-32 | Freigegebene Absender verwalten | **Liste im Postfach-Dialog, Verweis im Bild-Browser, Rueckfrage beim Entfernen** | Die Freigabeliste aus F4 konnte nur wachsen: `release_quarantine(trust_sender: true)` haengt an, ein Weg zurueck fehlte vollstaendig — weder Oberflaeche noch Befehl. Ein einmal aus Versehen bestaetigter Absender liess sich nie zuruecknehmen. *Ort:* die Liste steht im Postfach-Dialog, weil sie sachlich zur Quelle gehoert; der Bild-Browser verweist mit der Zahl der freigegebenen Absender darauf, weil dort freigegeben wird. Beides allein waere unvollstaendig — im Dialog allein faende den Zusammenhang niemand, im Browser allein stuende eine Quelleneigenschaft am falschen Platz. *Vorhandene Fotos:* beim Entfernen wird gefragt, ob sie zurueck in die Quarantaene sollen. Stilles Verschwinden aus der laufenden Show waere die schlechtere Ueberraschung, stilles Bleiben nicht immer gewollt; bei einem Absender ohne Fotos entfaellt die Frage. *Vergleich* ohne Gross- und Kleinschreibung wie `is_allowed`, sonst zaehlte die Rueckfrage zu wenige Bilder. |
| E-33 | Rueckmeldung des Verbindungstests | **In die Fusszeile, mit der Zahl ungelesener Nachrichten** | Am Geraet nachgestellt: „Verbindung testen“ sitzt in der festen Fusszeile, die Meldung stand am Ende des scrollbaren Rumpfs. Wer beim Passwortfeld tippte, sah nichts geschehen und tippte erneut. Die Meldung steht jetzt neben ihrem Ausloeser; lange Servertexte schrumpfen per `flex`, statt die Schaltflaechen aus dem Dialog zu schieben. *Zahl:* `test_connection` liefert die ungelesenen Nachrichten und begruendet im eigenen Kommentar, warum sie zaehlen — sie belegt, dass nicht nur die Anmeldung stimmt, sondern auch der Ordner. `test_source` warf sie weg und schrieb sie nur ins Protokoll, wo sie niemand sieht. Sie steht jetzt in der Meldung; andere Quellenarten liefern `None` und bleiben bei der schlichten Fassung. |
| E-34 | Auch gelesene Nachrichten holen | **Schalter je Postfach, zweistufiger Abruf, Gelesen-Vermerk bleibt** | Anlass: zwei Rahmen an einem Postfach. Der erste markiert jede Mail als gelesen und nimmt sie dem zweiten weg. *Warum kein blosser Schalter:* der Abruf lud jede gefundene Nachricht vollstaendig herunter und fragte erst danach, ob er sie kennt. Bei `UNSEEN` geht das auf, weil der Gelesen-Vermerk Verarbeitetes aus der Suche nimmt; ueber den ganzen Ordner waere daraus eine Endlosschleife geworden — dieselben ersten 30 Mails bei jedem Lauf, die 31. nie. Deshalb zwei Stufen: `FETCH BODY.PEEK[HEADER.FIELDS (MESSAGE-ID)]` fuer alle (wenige Dutzend Bytes je Nachricht), voller Rumpf nur fuer die unbekannten. `PEEK`, damit schon das Nachsehen keinen Vermerk setzt. *Riskante Stelle:* beide Stufen muessen dieselbe Kennung bilden. Ein Test laesst `message_id_from_header` und `parse_mail` auf dieselbe Mail los und vergleicht — weichen sie ab (etwa um die spitzen Klammern), erkennt Stufe eins nie etwas wieder und der Rahmen legt alles doppelt ab, ohne Fehlermeldung. *Gelesen-Vermerk:* bleibt auch bei eingeschaltetem Schalter. **Bewusste Folge:** der Rahmen markiert dann den ganzen Ordner als gelesen, auch Post, die noch niemand gesehen hat. Der Hinweis im Formular empfiehlt deshalb einen eigenen Ordner statt der INBOX. *Voreinstellung* `false` mit `#[serde(default)]`, damit vorhandene Konfigurationen unveraendert weiterlaufen (NF-10). *Nebenbei geschlossen:* der Postfach-Abruf schrieb bislang nichts ins Protokoll. Jetzt eine Zeile je Lauf mit Ordnergroesse, bereits Bekanntem, Geholtem und Abgelegtem — ohne sie liesse sich nicht erklaeren, warum ein Durchgang ueber tausend Nachrichten drei Fotos brachte. |
| E-35 | Absender freigeben | **Tippen oeffnet die Wahl, Kachel nennt den Absender** | **Befund:** „Absender vertrauen" gab es im Rust-Backend seit jeher — `release_quarantine(trust_sender: true)` nimmt die Adresse auf und holt alle wartenden Fotos derselben Person mit. In der Bedienung wurde `release(entry, true)` von **nirgends** aufgerufen: ein Tipp gab immer nur das eine Bild frei. Die Freigabeliste aus F4 konnte sich also nie fuellen, und die Verwaltung aus E-32 verwaltete eine Liste, die leer bleiben musste. Aufgefallen erst auf Nachfrage; der Kommentar ueber der Funktion beschrieb den Knopf, als gaebe es ihn — dieselbe Verwechslung von Absicht und Umsetzung wie bei NF-15. *Bedienung:* ein Tipp gibt nicht mehr sofort frei, sondern zeigt Vorschau, Absender und Betreff und laesst zwischen „Nur dieses Bild" und „Alle von <Adresse>" waehlen. Gegen zwei kleine Ziele nebeneinander entschieden: ein Fehltipp gaebe dort dauerhaft alles frei. Die Schaltflaeche nennt die Adresse ausdruecklich, weil bei mehreren Wartenden sonst unklar bleibt, wem man gerade dauerhaft vertraut. *Kachel:* in der Quarantaene Absender **und** Datum, zweizeilig. Ohne den Absender soll jemand freigeben, ohne zu sehen, von wem — das Aufnahmedatum hilft bei dieser Entscheidung nicht. In den uebrigen Filtern bleibt es beim Datum allein. *Zuruecknehmen:* ein freigegebenes Bild wird ueber „Ausblenden" (FA-30) aus der Show genommen; die Diashow filtert `excluded` und `is_quarantined()` gleich. Ein einzelnes Foto zurueck in die Quarantaene zu schicken gibt es bewusst nicht — absenderweise leistet das E-32. |
| E-36 | Gedaechtnis fuer Nachrichten ohne Foto | **Eigene begrenzte Liste neben dem Cache-Index** | **Am Geraet im Abruf-Protokoll (F6) aufgefallen**, Stunde um Stunde dieselbe Zeile: „1 geholt · 0 neu · 2 bekannt". Der Doppelimport-Schutz (F2) erkennt eine Mail an ihrer Message-Id **im Cache-Index** — und der kennt nur Fotos. Eine Nachricht ohne brauchbaren Anhang hinterlaesst dort nichts und ist beim naechsten Lauf wieder unbekannt. *Warum es vorher niemand sah:* solange nur Ungelesenes geholt wurde, nahm der Gelesen-Vermerk sie aus der Suche. Mit „auch gelesene" (E-34) bleibt sie fuer immer darin und wird alle fuenfzehn Minuten erneut vollstaendig heruntergeladen — dieselbe Endlosschleife, die E-34 zu vermeiden glaubte, nur leiser: sie haengt nicht, sie kommt nur nie voran. Bei einem Postfach voller Rechnungen und Newsletter waere das jeder Lauf aufs Neue. *Loesung:* eine eigene Liste verarbeiteter Message-Id-Hashes neben dem Index, auf 5 000 Eintraege begrenzt (rund 80 KB). Gemerkt wird **jede** verarbeitete Nachricht, nicht nur die photolose — bei Fotos ist es ueberfluessig, aber harmlos, und die Unterscheidung waere eine Bedingung, die irgendwann falsch wird. *Stolperstelle:* die Suchmenge wird nicht mitgespeichert und muss beim Laden neu aufgebaut werden. Wer das vergisst, hat ein Gedaechtnis, das nichts erinnert — und merkt es nie, weil der Abruf trotzdem laeuft. Ein Test haelt genau das fest. *Uebergabe:* als Buendel von Rueckrufen (`MailMemory`), damit `mail::sync` nichts vom Anwendungszustand weiss und beide Seiten im Test einsetzbar bleiben. |
| E-37 | Weitere Bildformate | **BMP, TIFF, ICO und GIF (erstes Einzelbild)** | Alle vier liegen im `image`-Crate in reinem Rust vor und kosten je ein Feature — kein C-Cross-Compile, damit kein Widerspruch zu E-02 und E-04. *GIF:* stand bis hierher in der Sperrliste, weil E-07 Bewegtbild ausschliesst. Ein GIF hat aber ein erstes Einzelbild, und das ist ein Bild wie jedes andere; die Bewegung bleibt draussen. Ein Test kodiert ein zweibildriges GIF (rot, dann gruen) und prueft, dass **rot** ankommt — sonst zeigte die Diashow etwas anderes als die Vorschau im Mailprogramm. *Nicht aufgenommen:* HEIC/HEIF (libheif plus HEVC, kein produktionsreifer Rust-Decoder bekannt) und AVIF (Dekodieren ueber `dav1d` in C). Beide bleiben uebersprungen, E-04 gilt unveraendert. *Nebenbefund, am Geraet gemessen:* **Android-Bewegtbilder liefen schon immer.** Ein Pixel- oder Samsung-Motion-Photo ist eine gewoehnliche JPEG-Datei mit angehaengtem MP4; jeder JPEG-Decoder liest das Standbild und ueberspringt den Rest. 105 der 597 Fotos auf dem Referenzgeraet sind solche Dateien. Der Videoteil faellt beim Skalieren weg — fuer einen Bilderrahmen genau richtig. *Getestet* wird mit echten Bilddaten statt nur mit Endungen: aktiviert werden die Formate in `Cargo.toml`, und ein vergessenes Feature faellt sonst erst am Geraet auf. |
| E-38 | Vordergrunddienst und Rechte | **Dienst entfaellt, Manifest auf zwei Rechte** | **Kehrt E-24 um.** Der Dienst hob die Prozesspriorität, damit Android die App bei Speicherdruck nicht abraeumt — das greift aber nur bei *nicht sichtbarer* Activity, und ein Rahmen laeuft mit dauerhaft sichtbarer Diashow. Ob der Fall je eintrat, ist nie gemessen worden; der Siebentagetest wird es zeigen. *Was mitentfaellt:* `FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_SPECIAL_USE` und `POST_NOTIFICATIONS`. Damit faellt die **einzelne Google-Pruefung** fuer `specialUse` weg, die vor einer Veroeffentlichung sonst manuell und mit offenem Ausgang anstuende. *Ausserdem entfernt:* `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`. Es stand seit jeher im Manifest, wurde aber **nirgends im Code angefordert** — ein ungenutztes heikles Recht ist im Play Store ein leichter Ablehnungsgrund. Uebrig bleiben `INTERNET` und `ACCESS_NETWORK_STATE`. *Bewusst in Kauf genommen:* Wird der Rahmen doch einmal abgeraeumt, gibt es kein Netz darunter — `START_STICKY` legte den Dienst bisher neu an. NF-02 faellt damit auf den Stand vor E-24 zurueck; der Weckversuch aus `onStartCommand` scheiterte seit Android 10 ohnehin fast immer. *Nachgezogen:* `patch-android.mjs` fuehrt jetzt eine Verbotsliste und **entfernt** die drei Rechte aus dem generierten Manifest, statt nur hinzuzufuegen. `gen/` ist generiert, aber nicht bei jedem Lauf frisch — ohne diesen Schritt truege es die alten Rechte weiter, unsichtbar, weil niemand eine generierte Datei liest. Die Gegenprobe hat das beim ersten Lauf sofort gefunden. |
| E-39 | Postfach absichern ohne Keystore | **App-Passwort und eigenes Postfach empfehlen, im Formular** | Zu E-17 gehoerig, aber an anderer Stelle wirksam: der Keystore schuetzt den **Schluessel**, diese beiden Hinweise begrenzen, **was ein gestohlenes Passwort ueberhaupt oeffnet**. Bei einem Postfach zaehlt das zweite mehr. *App-Passwort:* einzeln widerrufbar, ohne das Kontopasswort zu aendern, und bei den meisten Anbietern nur fuer den Mailzugang gueltig. *Eigenes Postfach:* dort liegen ausschliesslich Fotos — ein verlorenes Passwort kostet keine Korrespondenz. Dieselbe Logik wie die bestehende NAS-Empfehlung („eigenes Konto mit Nur-Lese-Rechten"), die es fuer Postfaecher bislang nicht gab: der Hinweis am Passwortfeld erschien **nur** bei WebDAV und Nextcloud. *Ort:* im Formular, an den Feldern, wo die Entscheidung faellt — nicht in einer Anleitung, die niemand aufschlaegt. Beim Bearbeiten hat „leer lassen behaelt das gespeicherte" Vorrang; dort ist es die dringendere Auskunft. *Verworfen:* Schluessel aus einer PIN ableiten statt ihn zu speichern. Nach jedem Stromausfall (R-08) stuende der Abruf still, bis jemand zum Rahmen geht — und zwar lautlos, weil die Diashow aus dem Cache weiterlaeuft. Zudem schlechte Oekonomie: die beiden Hinweise senken den Wert des Geheimnisses, eine PIN erhoeht den Preis, es zu schuetzen. Wer mehr will als E-39, ist mit dem Keystore (E-17, Option B) guenstiger bedient — er kostet keine Bedienung. |
| E-40 | Sicherung: Datei statt Zwischenablage | **Export und Import ueber einen SAF-Dateidialog** | Ausloeser war ein Fehler, der lange unbemerkt blieb, weil er nur die Haelfte betraf: `navigator.clipboard.writeText` erlaubt Androids WebView, `readText` nicht — die Permissions-API kann `clipboard-read` dort nicht gewaehren, es gibt keinen Dialog zum Bestaetigen. Der Export meldete also Erfolg, der Import scheiterte ausnahmslos an „Read permission denied“. Eine Sicherung, die sich schreiben, aber nie zurueckspielen laesst, sieht funktionierend aus; der Beweis kommt erst, wenn man sie braucht. Aufgefallen beim Test des ersten signierten Release-Builds am 31.08.2026. *Warum Datei und nicht nur die Sperre umgehen:* Die Zwischenablage ueberlebt weder Neustart noch Werksreset noch Geraetewechsel — also genau die drei Faelle, fuer die man sichert. Der Zwischenablage-Weg waere auch mit nativem Lesen eine Sicherung geblieben, die nichts sichert. *Kosten:* gering, weil `tauri-plugin-android-fs` bereits fuer die Ordnerwahl (FA-20) eingebunden ist und `showSaveFilePicker`/`showOpenFilePicker` mitbringt — kein neuer Kotlin-Code, keine neue Abhaengigkeit, vier Eintraege in `capabilities/mobile.json`. *Verworfen:* Tauri-Plugin clipboard-manager (behebt den Fehler, nicht die Fluechtigkeit) und ein Einfuegefeld in der Oberflaeche (kein nativer Code, aber ein manueller Schritt und dieselbe Fluechtigkeit). *Schreibtisch:* dort bleibt die Zwischenablage, weil sie vollstaendig funktioniert und es keinen SAF-Dialog gibt (Nebenprodukt, Abschnitt 1.3). *Offen geblieben:* Nach dem Einspielen fehlen den lokalen Quellen die SAF-Freigaben — Android knuepft sie an die Installation, nicht an den Paketnamen. Die Eintraege kehren zurueck, das Leserecht nicht. Die Oberflaeche sagt das bisher nicht. |
| E-17 | Umsetzungsstufe NF-05 | **Noch offen** – siehe Abschnitt 10 | Umgesetzt ist AES-256-GCM (reines Rust) mit Schlüsseldatei im App-privaten Verzeichnis. Die Keystore-Bindung fehlt noch; die Trennlinie dafür ist der `KeyProvider`-Trait in `src-tauri/src/secrets.rs`. |

## 10. Offene Punkte

### 10.1 Entscheidungsbedarf

**E-10 – Referenz-Tablet: entschieden.** Das Xiaomi Pad 6 ist die Referenz. Damit entfällt der letzte Blocker vor M1: der Dauertest über 7 Tage, der Speichertest mit 5 000 Bildern und der Neustart-Test können beginnen, und die Play-Store-Screenshots kommen vom Zielgerät. Die Befunde des ersten Laufs stehen in 11.4.

**E-17 – Wie weit soll NF-05 gehen?** Die Anforderung lautet „Android Keystore oder gleichwertige Verschlüsselung". Umgesetzt ist die Verschlüsselung, nicht die Keystore-Bindung. Drei Optionen:

| Option | Aufwand | Schutzwirkung |
|---|---|---|
| **A – bleibt wie es ist** | keiner | Schützt gegen Gerätebackup und versehentliches Mitkopieren der Konfiguration. Nicht gegen Root-Zugriff auf ein entsperrtes Gerät. Mit der Empfehlung aus R-12 (eigenes NAS-Konto, nur lesend) für ein Wohnzimmergerät vertretbar. |
| **B – eigenes Kotlin-Plugin für den Android Keystore** | 1–2 Tage, neuer nativer Code | Schlüssel verlässt die Hardware nicht. Erhöht die Menge an eigenem Kotlin-Code, die laut R-01 ohnehin schon der Risikoschwerpunkt ist. |
| **C – Stronghold-Plugin** | ~½ Tag | Bringt keine Keystore-Anbindung mit (siehe Abschnitt 8), die Schlüsselableitung bliebe selbst zu lösen. Ergäbe gegenüber A wenig Gewinn bei zusätzlicher Abhängigkeit. |

Empfehlung zur Diskussion: A für M2, B als Ausbauoption nach dem Dauertest — die Entscheidung ist reversibel, weil die Aufrufstellen hinter `KeyProvider` unverändert bleiben.

**Drittlizenzen: erledigt, mit einem Befund.** Die Übersicht aus 5.1 und RB-05 liegt als [docs/third-party-licenses.md](docs/third-party-licenses.md) vor und wird von `npm run licenses` aus `cargo metadata` und den installierten npm-Paketen erzeugt — eine handgepflegte Liste wäre beim nächsten `cargo update` falsch, ohne dass es jemand merkt. Erfasst sind 334 Rust-Kisten (gefiltert auf `aarch64-linux-android`, nicht die 547 des vollen Baums) und 36 npm-Pakete. Die Schriften sind darin aufgenommen: SIL OFL 1.1, Apache-2.0-verträglich, unverändert gebündelt.

**Befund zu RB-05.** RB-05 verlangt „permissive Lizenzen". Fünf Kisten sind es nicht ganz: `cssparser`, `cssparser-macros`, `dtoa-short`, `selectors` und `option-ext` stehen unter der MPL-2.0, also unter schwachem Copyleft. Alle fünf kommen **transitiv über Tauri selbst** — die ersten vier über `dom_query` → `tauri-utils`, `option-ext` über `dirs` → `tauri` —, sind also ohne Verzicht auf Tauri nicht vermeidbar. Praktisch entsteht daraus keine Auflage über den Lizenzhinweis hinaus: das Copyleft der MPL-2.0 wirkt je Datei, die Kisten werden unverändert von crates.io eingebunden, und die Apache-2.0-Lizenzierung von Slowshow bleibt unberührt. Vorschlag: RB-05 auf „permissiv oder schwaches Copyleft ohne Auswirkung auf das Gesamtwerk" präzisieren, statt eine Einschränkung stehen zu lassen, die das Projekt faktisch nicht einhält.

### 10.2 Aus der Umsetzung entstanden

- **MQTT nachgereicht.** FA-55 nennt „REST-Endpunkte **oder** MQTT"; inzwischen sind beide da. MQTT bringt drei Dinge, die REST nicht kann: das Tablet braucht keine feste Adresse mehr (es verbindet sich zum Broker statt umgekehrt), Zustandsänderungen kommen ohne Polling an, und über das „letzte Wort" sieht Home Assistant einen Ausfall sofort. Mit Discovery meldet sich der Rahmen von allein als Gerät mit zwölf Entitäten an. Beide Wege laufen über dieselben Aktionen in `control.rs` und können deshalb nicht auseinanderlaufen.
- **Foreground-Service umgesetzt (E-24).** `SlowshowService` hebt die Prozesspriorität an und wird von Android nach einem Abschuss neu angelegt. Was er *nicht* leistet, steht in E-24: ein Rust-Panic reißt ihn mit, und aus dem Hintergrund darf er die Activity meist nicht mehr starten. Ob das über 7 Tage trägt, muss weiterhin der Dauertest zeigen.
- **Watchdog (NF-02) weiterhin nur teilweise.** Umgesetzt sind Selbstheilung der Datenhaltung (defekter Index, fehlende Cache-Dateien, unlesbare Konfiguration führen zu Standardwerten statt zum Startabbruch), ein Panic-Hook und seit E-24 der Vordergrunddienst. Der häufige Fall — Abschuss wegen Speicherdruck — ist damit abgedeckt, der seltene — Absturz des Prozesses — nicht. Ein vollständiger Neustart bräuchte einen zweiten Prozess oder einen Kiosk-Launcher.

- **NF-15 war falsch als umgesetzt geführt.** Der Thumbnail-Index stand in der Statustabelle, existierte im Code aber nicht — aufgefallen erst beim Planen des Bild-Browsers. Seit E-25 gibt es ihn wirklich. Lehre für die Tabelle: Sammelzeilen über mehrere Anforderungen („NF-12 bis NF-16") verstecken einzelne Lücken.

## 11. Umsetzungsstand

Stand 30. August 2026. Geprüft durch 232 Rust- und 89 Frontend-Tests sowie Clippy ohne Warnungen. Seit 11.4 laufen die Angaben zum Verhalten auf einem echten Tablet statt auf einem Telefon; ein Gerät nach RB-02 ist es weiterhin nicht (E-10).

### 11.1 Ergebnis der Code-Durchsicht

Eine systematische Durchsicht des fertigen Codes fand zehn Fehler, die alle behoben und — wo möglich — durch Tests abgesichert wurden. Vier davon hätten sich im Dauerbetrieb erst spät gezeigt und sind hier festgehalten, weil sie erklären, worauf bei ähnlichen Änderungen zu achten ist:

| Fehler | Wirkung | Absicherung |
|---|---|---|
| Neue entfernte Quelle bekam zwei verschiedene Kennungen — eine für die Quelle, eine für das Passwort | Nach dem ersten Bearbeiten der Quelle lief jede Anmeldung mit leerem Passwort. Wäre erst beim nächsten nächtlichen Sync aufgefallen. | Kennung wird einmal erzeugt und für beides verwendet |
| Zufallsreihenfolge wurde bei jedem Sync neu gemischt | Bilder wiederholten sich, andere fielen aus, obwohl FA-28 nur „neue Bilder ohne Neustart" verlangt | Reihenfolge entsteht jetzt durch Sortieren nach einem seed-abhängigen Hash statt durch Mischen; Test `zufallsreihenfolge_bleibt_beim_hinzufuegen_stabil` |
| Abendliche Absenkung endete um Mitternacht | Der Rahmen leuchtete die halbe Nacht auf voller Helligkeit — gegen FA-53 und NF-06 | Absenkung läuft bis zum Beginn der Aktivzeit; Test `brightness_bleibt_ueber_mitternacht_abgesenkt` |
| Abdunkelung lag über der Nachtuhr | Bei 1 % Helligkeit lag ein 99-Prozent-Overlay über der Uhr aus FA-54 — nicht von einem schwarzen Bildschirm zu unterscheiden | Im Nachtmodus wird nicht zusätzlich abgedunkelt |

Die übrigen sechs: `exclude_image` sprang an den Anfang statt weiterzulaufen (FA-30); die Schaltflächen in der Diashow lösten zusätzlich Gesten aus (FA-41); die Sync-Sperre überlebte einen Panic nicht; lokale Quellen meldeten jede Aktualisierung als Neuzugang; `layoutInDisplayCutoutMode` wurde ohne Zurückschreiben gesetzt; und die Heimnetz-Steuerung übernahm den Schalter erst nach einem App-Neustart.

### 11.2 Erster Lauf auf echter Hardware

Testgerät: Pixel 9a, Android 17 (SDK 37). Kein Zielgerät nach RB-02, aber das
erste echte Android. Drei Befunde, die kein Test gefunden hätte:

| Befund | Ursache | Behoben durch |
|---|---|---|
| **Keine Quelle ließ sich speichern** | `#[serde(rename_all)]` am Enum benennt nur die *Variantennamen* um, nicht die Felder der Varianten. Rust erwartete `saf_uri`, das Frontend schickte `safUri` — `add_source` scheiterte beim Deserialisieren. | `rename_all` zusätzlich an jeder Variante; vier Tests mit dem exakten JSON, das das Frontend erzeugt |
| Dazu: **keine Fehlermeldung** | `SourcesPane.onSave` fing die Ausnahme nicht ab — der Dialog blieb stumm offen stehen | Fehler wird abgefangen und im Dialog angezeigt |
| **Panic in der Helligkeitsbrücke** | `ndk_context::android_context()` paniert, wenn Tauri den Kontext nicht initialisiert hat — was es nicht tut. Mit `panic = "abort"` hätte das den Release-Build beendet. | Die Activity meldet sich in `onCreate` selbst beim Rust-Backend an; jeder Fehlerfall endet als No-op |
| **Rust-Logs waren unsichtbar** | Ohne Logger verschwinden alle `log::`-Ausgaben. Damit war FA-09 („übersprungene HEIC-Dateien werden im Log vermerkt") faktisch nicht erfüllt und der Dauertest aus 5.2 nicht auswertbar. | `tauri-plugin-log`, Ausgabe nach logcat und in eine Datei im App-Verzeichnis |

**Auf dem Gerät bestätigt:** App startet, läuft stabil, `dumpsys` zeigt
`fl=KEEP_SCREEN_ON` — FA-50 ist damit nicht mehr nur behauptet. Das
JNI-Symbol der Helligkeitsbrücke ist im Binärcode vorhanden und die
Registrierung wirft nicht; ob die Beleuchtung tatsächlich reagiert, ist noch
zu prüfen.

### 11.3 Aus der Bedienung am Gerät

Vier weitere Punkte, die erst beim Benutzen auffielen:

| Punkt | Befund | Änderung |
|---|---|---|
| **Quellen nicht löschbar** | `SourceCard` deklarierte ein `remove`-Ereignis, das nie ausgelöst wurde — es gab schlicht keinen Knopf. Toter Code, der wie eine fertige Funktion aussah. | Löschen sitzt jetzt im Bearbeiten-Dialog; die Liste bleibt nach Entwurf ruhig, und ein destruktiver Schritt liegt eine Ebene tiefer (FA-43) |
| **Kein Sync-Fortschritt sichtbar** | Entfernte Quellen liefen komplett in Rust durch, ohne Rückkanal. Bei tausend Bildern war die Oberfläche minutenlang stumm — nicht unterscheidbar von „hängt". | `SyncProgress`-Ereignis nach jeder Datei, Zähler und Balken auf der Quellenkarte |
| **Bilder erst nach vollständigem Sync** | Die Playlist wurde erst *nach* Abschluss neu gebaut. Bis dahin blieb der Rahmen leer, obwohl längst Bilder im Cache lagen — im Widerspruch zum Sinn von FA-01 und FA-28. | Playlist wächst während des Syncs mit: das erste Bild startet die Diashow sofort, danach in Schritten von 25. Gilt für entfernte Quellen wie für lokale Ordner. |
| **Schriften fehlten** | `fetch-fonts.mjs` war nie gelaufen; die WebView bekam HTML statt woff2 und meldete `OTS parsing error`. Der Rückfall auf Systemschrift funktionierte, aber das Schriftbild wich vom Entwurf ab. | Schriften geholt und im Repository abgelegt (49 KB, SIL OFL, RB-05) |

**Inzwischen erledigt:** Der erste vollständige Durchlauf mit einer echten
Quelle — Ordner wählen, synchronisieren, Diashow — ist am Xiaomi Pad 6
gelaufen; siehe 11.4.

**Nachträglich geschlossen:** Die native Hälfte von FA-53 hatte keinen Aufrufer — `MainActivity.setScreenBrightness` war geschrieben, aber nie verbunden. Die Brücke liegt jetzt in `src-tauri/src/brightness.rs` (JNI, nur für Android übersetzt) und wird bei jeder Änderung des Anzeigezustands aufgerufen. Sie übersetzt für das Android-Ziel; die Wirkung auf dem Display ist erst am Referenzgerät prüfbar (E-10).

### 11.4 Erster Lauf auf einem Tablet

Testgerät: Xiaomi Pad 6 (`pipa`), Android 14, HyperOS `OS2.0.16.0.UMZMIXM`,
2880 × 1800 bei 400 dpi. Seit E-10 ist es **das Referenzgerät**, nicht mehr nur
das nächstbeste zur Hand.

| Befund | Ursache | Behoben durch |
|---|---|---|
| **Installation per ADB abgewiesen** | `INSTALL_FAILED_USER_RESTRICTED`. Kein Fehler der App, sondern die HyperOS-Sperre für Installationen über USB — sie verlangt ein angemeldetes Mi-Konto. | Entwickleroption „USB-Installation" freigeschaltet; keine Codeänderung. Für Geräte ohne Mi-Konto bleibt der Weg über den Dateimanager. |
| **Schwarzer Streifen am oberen Rand** | `body` trug `padding: env(safe-area-inset-*)` mit dem Kommentar „Vollbild bis in die Aussparungen" — die Polsterung bewirkt das Gegenteil. Trotz Immersive-Modus meldet die WebView weiterhin die Höhe der Statusleiste als oberen Inset (60 px). Das Foto saß dadurch 60 px zu tief: gemessen 549 px Rand oben gegen 489 px unten. Verstößt gegen FA-01. | Polsterung aus `body` entfernt; die Einstellungen polstern sich selbst, damit Bedienelemente nicht unter eine Aussparung geraten. Am Gerät nachgemessen. |
| **Kamera-Dienst beim Start** | Die WebView-Initialisierung zählt Kameras auf (`Start proc com.android.camera … caller=dev.kerker.slowshow`), was `Long monitor contention … onTorchStatusChanged for 705ms` auslöst. Chromiums Verhalten, nicht unseres. | Nicht behoben — nur festgehalten. Rund 0,7 s zusätzliche Startzeit; im Dauerbetrieb ohne Bedeutung, beim Entwickeln störend. |

**Auf dem Gerät bestätigt:** Diashow läuft mit einer echten Nextcloud-Quelle
über WebDAV, Sync und Cache greifen, Uhr und Datumszeile stehen wie im Entwurf.
Damit ist der in 11.3 als offen vermerkte erste vollständige Durchlauf — Ordner
wählen, synchronisieren, Diashow — nachgeholt. Ebenfalls am Gerät geprüft: das
Umschalten zwischen Ziffern und Zeigern (E-20) greift zur Laufzeit, und das
dauerhafte Pausen-Abzeichen (E-21) bleibt stehen, solange die Pause gilt.

**Bauzeit und APK-Größe.** Gradle ruft den Rust-Build einmal *je ABI* auf. Ein
Debug-APK für alle vier wiegt 1 441 MB, weil vier ungestrippte
Rust-Debug-Bibliotheken darin liegen; nur für `aarch64` sind es 325 MB bei
einem Viertel der Übersetzungszeit. `deploy-android.ps1 -dev -arm64`
(`npm run android:deploy:arm64`) ist deshalb der Weg für Entwicklungsrunden.
Für ein Release in den Play Store werden weiterhin alle ABIs gebraucht.

Dabei fiel auf, dass die APK-Datei größer sein kann als ihr Inhalt: gemessen
633 MB bei 325 MB tatsächlichen Einträgen, die Differenz exakt eine weitere
Kopie der Rust-Bibliothek. Gradles `zipflinger` schreibt inkrementell in das
bestehende Archiv und lässt verdrängte Blöcke physisch stehen. Installiert wird
nur der gültige Teil, `adb install` überträgt aber die ganze Datei — deshalb
räumt `deploy-android.ps1` das alte APK vor dem Bauen weg.

| Bereich | Anforderungen | Stand | Ort im Code |
|---|---|---|---|
| Diashow und Anzeige | FA-01 bis FA-08, FA-10 | umgesetzt | `src/views/SlideshowView.vue`, `src/components/SlideStage.vue`, `src-tauri/src/playlist.rs` |
| HEIC überspringen | FA-09 | umgesetzt | `src-tauri/src/decode.rs` (Endung und Magic Bytes) |
| Lokale Quelle über SAF | FA-20 | umgesetzt | `src/lib/saf.ts`, `commands::ingest_image` |
| WebDAV, Nextcloud | FA-21, FA-23 | umgesetzt | `src-tauri/src/sources/` |
| Mehrere Quellen, Cache, Ringpuffer, Prefetch | FA-25 bis FA-27, FA-31 | umgesetzt | `src-tauri/src/cache/` |
| Sync, Filter, Ausschlussliste | FA-28 bis FA-30 | umgesetzt | `src-tauri/src/sync.rs` |
| Bedienung, Persistenz, Import/Export | FA-40 bis FA-43, FA-45 | umgesetzt (Wischen **und** Tippzonen, E-18) | `src/composables/useGestures.ts`, `src-tauri/src/config.rs` |
| Bildschirm an, Zeitplan, Helligkeit, Nachtmodus | FA-50, FA-52 bis FA-54 | umgesetzt | `src-tauri/android-src/MainActivity.kt`, `src-tauri/src/schedule.rs` |
| Heimnetz-Steuerung | FA-55 | umgesetzt (REST **und** MQTT) | `src-tauri/src/remote.rs`, `src-tauri/src/mqtt/`, gemeinsame Aktionen in `control.rs` |
| Stabilität, Selbstheilung | NF-01, NF-02 | teilweise – Vordergrunddienst seit E-24, Neustart nach Absturz weiterhin offen | `android-src/SlowshowService.kt` |
| Performance, Datenschutz | NF-03, NF-04, NF-06 | umgesetzt | Dekodierung in Rust, keine Drittanbieter-Aufrufe |
| Zugangsdaten | NF-05 | teilweise – siehe E-17 | `src-tauri/src/secrets.rs` |
| Einbrennschutz, i18n, Barrierefreiheit | NF-07, NF-09, NF-11 | umgesetzt | `src/composables/usePixelShift.ts`, `src/locales/` |
| APK-Größe | NF-10 | **erfüllt, gemessen: 11,5 MB** (arm64, Release) | davon 7,7 MB Rust-Bibliothek, 2,1 MB `classes.dex`. Puffer zu den geforderten 30 MB ist reichlich; die CI wacht zusätzlich über die Größe des Frontend-Bündels. |
| Cache-Ablage, Rust-Dekodierung, Delta-Sync, GPU-Übergänge | NF-12, NF-13, NF-14, NF-16 | umgesetzt | `decode.rs`, `cache/index.rs`, `SlideStage.vue` |
| Thumbnail-Index | NF-15 | umgesetzt (E-25) — **war zuvor fälschlich als umgesetzt geführt** | `decode::thumbnail`, `cache::read_or_make_thumb`, `panes/ImagesPane.vue` |
| Akku-Telemetrie | R-08 | umgesetzt (E-23) | `battery.rs`, `MainActivity.batteryState` |
| Vordergrunddienst | NF-01, teilweise NF-02 | umgesetzt (E-24) — Grenzen siehe dort | `android-src/SlowshowService.kt` |
| Hochformat-Montage | RB-02, FA-08 | umgesetzt (E-26), Entwurf offen | `orientation.rs`, `playlist.rs`, `SlideStage.vue` |
| CI-Build | Maßnahme zu R-11 und R-13 | umgesetzt | `.github/workflows/ci.yml` |
| Play-Store-Unterlagen | 5.1, RB-03 | vorbereitet | `docs/store-listing.md`, `docs/privacy-policy.md` |
| Drittlizenz-Übersicht | 5.1, RB-05 | umgesetzt – Befund zu RB-05 in 10.1 | `docs/third-party-licenses.md`, erzeugt von `scripts/third-party-licenses.mjs` |

**Noch nicht begonnen:** Dauertest über 7 Tage (5.2), Speichertest mit 5 000 Bildern (5.2), Neustart-Test auf echter Hardware. Mit E-10 sind alle drei jetzt durchführbar — das Referenzgerät steht fest und die App läuft darauf.

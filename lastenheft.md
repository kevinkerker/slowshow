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
| RB-02 | Zielplattform: Android 10 oder neuer, Tablets mit min. 2 GB RAM; Ziel-Formfaktor Querformat 7–13 Zoll. |
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
| FA-08 | SOLL | Bei Hochformatfotos auf Querformat-Display: zwei Hochformatbilder nebeneinander anzeigen (Paar-Modus). |
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
| E-13 | Design & App-Icon | **Galerie-minimal; Icon „Rahmen & Horizont"** | Designsystem: Tiefschwarz #0A0A0A (OLED-freundlich, stützt NF-07), Off-White #F2EFE9, Akzent Messing #C2A878; Instrument Sans (UI) + Cormorant Garamond (Wortmarke/Bildunterschriften). App-Icon: weißer Rahmen mit Messing-Horizont und -Sonne auf Schwarz; unter 48 px entfällt die Sonne. Mockups im Design-Canvas „Slowshow App-Design". |
| E-14 | Format der Cache-Ablage | **JPEG (Qualität 85) statt WebP** | NF-12 nennt WebP als Beispiel („z. B."). Ein verlustbehafteter WebP-Encoder existiert in Rust nur als Bindung an libwebp (C) und müsste für Android cross-kompiliert werden – dieselbe Aufwandsklasse, die bei HEIC (E-04) und SMB (E-02) bewusst gemieden wurde. Der JPEG-Encoder des `image`-Crates ist reines Rust. WebP-**Dekodierung** bleibt erhalten, es ist also weiterhin ein zulässiges Quellformat (FA-04). |
| E-15 | Ablage des Cache-Index | **JSON im Speicher statt SQLite** | Bei der Zielgröße aus 5.2 (5 000 Bilder) sind das wenige MB, die einmal beim Start geladen werden. Erspart die Cross-Kompilierung von libsqlite3 für Android. Der Index wird atomar geschrieben und beim Start gegen die vorhandenen Dateien abgeglichen (NF-02). |
| E-16 | Ablage des nativen Android-Codes | **Versioniert in `src-tauri/android-src/`, per Skript nach `gen/` gespiegelt** | `src-tauri/gen/` ist generiert und gitignored; handgeschriebener Code dort ginge bei `tauri android init` unbemerkt verloren. Für Slowshow kritisch, weil FA-01, FA-50 und FA-53 genau dort liegen. `scripts/patch-android.mjs` spielt den Code ein, ergänzt das Manifest und prüft das Ergebnis; es läuft vor jedem Android-Build und in der CI. |
| E-19 | Schaltflaechen in der Diashow | **Zahnrad und durchgestrichenes Auge, einzeln abschaltbar** | Der Entwurf sieht im Artboard „Diashow“ keine Schaltflaechen vor. Zwei sind trotzdem sinnvoll: ein kurzer Weg in die Einstellungen (FA-40) und das Ausblenden des laufenden Bildes (FA-30) — letzteres ist nur im Moment des Anschauens praktisch. Beide sind wie Uhr und Datum einzeln abschaltbar (FA-07); wer den Rahmen puristisch will, blendet sie aus und nutzt den langen Druck. Zuvor trugen sie das „System“-Symbol aus der Navigation (Kreis mit Strahlen) und ein Minus — beides las sich falsch, das eine als Helligkeit, das andere als gar nichts. |
| E-18 | Bedienung der Diashow | **Tippzonen zusätzlich zum Wischen** | FA-41 verlangt „Wischgesten für vor/zurück, Tippen für Pause/Weiter". Wischen bleibt; ergänzt werden drei Tippzonen: linkes Drittel zurück, Mitte Pause, rechtes Drittel weiter. Auf einem an der Wand hängenden Rahmen ist ein kurzer Tipp bequemer als eine Wischbewegung — und die großzügige Mitte fängt Fehlgriffe auf die harmlose Aktion. Drittel, weil das die verbreitete Aufteilung ist (E-Book-Leser) und damit am wenigsten überrascht. Langer Druck öffnet weiterhin die Einstellungen (FA-43). |
| E-20 | Analoguhr | **Getrennt schaltbar, Strichindex, ohne Sekundenzeiger** | Drei Teilfragen, einzeln entschieden. *Ort:* Diashow (FA-07) und Nachtmodus (FA-54) bekommen je einen eigenen Schalter — analog nachts neben digital tagsüber ist eine sinnvolle Kombination, kein Widerspruch. *Stil:* dünner Ring mit zwölf Marken, die auf zwölf/drei/sechs/neun länger; kein Zifferblatt mit Ziffern. Ziffern in der Display-Serife wären die auffälligste Variante gewesen — auf einem Rahmen, dessen Fotos der einzige helle Bereich sein sollen, ist das zu viel. *Sekundenzeiger:* keiner. `useNow` taktet bewusst nur auf die volle Minute (NF-06); ein Sekundenzeiger hielte die WebView rund um die Uhr im Sekundentakt am Zeichnen. Der Stundenzeiger wandert dafür stufenlos mit der Minute mit, sonst stünde er bei einer Uhr ohne Ziffern schlicht falsch. Zum Einbrennen (NF-07): eine Analoguhr ist nicht automatisch besser als eine digitale — die Zeiger rotieren zwar, Ring und Marken stehen aber dauerhaft. Der Pixel-Shift gilt unverändert. |
| E-21 | Anzeige der Pause | **Dauerhaftes Abzeichen statt kurzer Einblendung** | Bisher erschien „Pausiert" für gut zwei Sekunden und verschwand. Ein Rahmen, der stehenbleibt, sieht danach aus wie einer, der hängt — der Hinweis war genau dann weg, wenn später jemand davorstand. Das Abzeichen steht oben in der Mitte, solange die Pause gilt, in Messing statt Off-White: es meldet einen Zustand, keine Meldung. Nicht im Nachtmodus, dort soll der Schirm dunkel bleiben (FA-54). Es wandert wie die übrigen Einblendungen (NF-07) — eine Pause kann Tage dauern. |
| E-22 | Gerätegesteuerte Helligkeit | **Zusätzliche Option, die die Regelung vollständig abgibt — auch nachts** | FA-53 sah nur die Steuerung *durch* die App vor. Wer die Helligkeitsautomatik des Geräts bevorzugt, schaltet sie nun ein; die App setzt dann in **keinem** Zustand mehr eine Fensterhelligkeit (`BRIGHTNESS_OVERRIDE_NONE`) — weder tagsüber, noch nachts, noch auf einen Schlafbefehl aus dem Heimnetz. Eine Ausnahme „nur nachts doch" wäre nicht zu erklären: der Rahmen verhielte sich abends anders als morgens, ohne dass jemand etwas umgestellt hätte. Die abendliche Absenkung entfällt ebenfalls — sie würde gegen die Systemautomatik arbeiten — und die zugehörigen Regler werden ausgeblendet statt wirkungslos stehenzubleiben. **FA-52 bleibt trotzdem erfüllt:** außerhalb der Aktivzeit legt die Oberfläche den Schirm auf Schwarz (`dimOpacity` in `src/lib/dim.ts`). Geschwärzt wird der Inhalt, nur eben nicht zusätzlich die Hintergrundbeleuchtung. Technisch reist der Zustand als Wert `0` im vorhandenen Helligkeitsfeld statt als zweites Feld: die Helligkeit läuft über das Anzeige-Ereignis, REST (FA-55) und MQTT, und ein zusätzliches Feld wäre an jeder Stelle, die es übersieht, stumm wirkungslos. Der Schalter selbst ist fernsteuerbar — über REST als `deviceBrightness` und über MQTT als `cmd/device_brightness` mit eigener Discovery-Entität: wer die Helligkeit in Home Assistant automatisiert, muss die Automatik auch von dort umlegen können. Ein Helligkeitsbefehl über REST oder MQTT wird währenddessen gespeichert, bleibt aber wirkungslos, bis die Gerätesteuerung wieder aus ist — er schaltet sie **nicht** stillschweigend ab. Sonst entschiede eine Automatisierung über eine Einstellung, die niemand angeordnet hat. |
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
- **Foreground-Service noch nicht umgesetzt.** Abschnitt 8 nennt ihn für NF-01 und R-04. Aktuell decken `FLAG_KEEP_SCREEN_ON` und die angefragte Akku-Ausnahme den Fall ab. Ob das auf dem Referenzgerät über 7 Tage trägt, muss der Dauertest zeigen — das ist genau die Frage, die M1 beantworten soll.
- **Watchdog (NF-02) nur teilweise.** Umgesetzt sind Selbstheilung der Datenhaltung (defekter Index, fehlende Cache-Dateien, unlesbare Konfiguration führen zu Standardwerten statt zum Startabbruch) und ein Panic-Hook. Ein echter Neustart nach Prozessabbruch braucht den Foreground-Service aus dem vorigen Punkt.

## 11. Umsetzungsstand

Stand 30. August 2026. Geprüft durch 207 Rust- und 84 Frontend-Tests sowie Clippy ohne Warnungen. Seit 11.4 laufen die Angaben zum Verhalten auf einem echten Tablet statt auf einem Telefon; ein Gerät nach RB-02 ist es weiterhin nicht (E-10).

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
| Stabilität, Selbstheilung | NF-01, NF-02 | teilweise – siehe 10.2 | — |
| Performance, Datenschutz | NF-03, NF-04, NF-06 | umgesetzt | Dekodierung in Rust, keine Drittanbieter-Aufrufe |
| Zugangsdaten | NF-05 | teilweise – siehe E-17 | `src-tauri/src/secrets.rs` |
| Einbrennschutz, i18n, Barrierefreiheit | NF-07, NF-09, NF-11 | umgesetzt | `src/composables/usePixelShift.ts`, `src/locales/` |
| APK-Größe | NF-10 | **erfüllt, gemessen: 11,5 MB** (arm64, Release) | davon 7,7 MB Rust-Bibliothek, 2,1 MB `classes.dex`. Puffer zu den geforderten 30 MB ist reichlich; die CI wacht zusätzlich über die Größe des Frontend-Bündels. |
| Cache-Ablage, Rust-Dekodierung, Delta-Sync, GPU-Übergänge | NF-12 bis NF-16 | umgesetzt | `decode.rs`, `cache/index.rs`, `SlideStage.vue` |
| CI-Build | Maßnahme zu R-11 und R-13 | umgesetzt | `.github/workflows/ci.yml` |
| Play-Store-Unterlagen | 5.1, RB-03 | vorbereitet | `docs/store-listing.md`, `docs/privacy-policy.md` |
| Drittlizenz-Übersicht | 5.1, RB-05 | umgesetzt – Befund zu RB-05 in 10.1 | `docs/third-party-licenses.md`, erzeugt von `scripts/third-party-licenses.mjs` |

**Noch nicht begonnen:** Dauertest über 7 Tage (5.2), Speichertest mit 5 000 Bildern (5.2), Neustart-Test auf echter Hardware. Mit E-10 sind alle drei jetzt durchführbar — das Referenzgerät steht fest und die App läuft darauf.

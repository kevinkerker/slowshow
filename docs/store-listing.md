# Play-Store-Eintrag — Slowshow

Vorlage für die Google Play Console (Lastenheft RB-03, Lieferumfang 5.1).
Vor der ersten Veröffentlichung durchgehen und die offenen Punkte am Ende
abarbeiten.

---

## Grunddaten

| Feld | Wert |
| --- | --- |
| App-Name | Slowshow |
| Paketname | `dev.kerker.slowshow` |
| Kategorie | Foto & Video (alternativ: Personalisierung) |
| Inhaltsfreigabe | Ab 0 Jahren |
| Preis | kostenlos |
| Werbung | nein |
| In-App-Käufe | nein |

---

## Kurzbeschreibung (max. 80 Zeichen)

```text
Digitaler Bilderrahmen: Fotos vom NAS, aus der Cloud oder per E-Mail.
```

(69 Zeichen)

---

## Vollständige Beschreibung

```text
Slowshow verwandelt ein ausgedientes Android-Tablet in einen digitalen
Bilderrahmen — ohne Werbung, ohne Konto, ohne Abo.

Ihre Fotos bleiben, wo sie sind: auf dem Gerät, auf Ihrem NAS oder in Ihrer
eigenen Nextcloud. Slowshow holt sie sich von dort, legt verkleinerte Kopien
auf dem Tablet ab und zeigt sie als ruhige Endlos-Diashow.


BILDQUELLEN

• Lokale Ordner auf Tablet und SD-Karte
• NAS über WebDAV — Synology, QNAP, ownCloud und andere
• Nextcloud-Alben, mit serverseitigen Vorschaubildern
• Ein eigenes Postfach: Wer Ihnen ein Foto schickt, sieht es am Rahmen

Mehrere Quellen lassen sich gleichzeitig nutzen und einzeln ein- und
ausschalten.


FOTOS PER E-MAIL

Richten Sie ein Postfach ein, holt Slowshow die Bilder aus eingehenden
Nachrichten — ohne dass jemand eine App braucht. Großeltern bekommen so neue
Fotos, indem die Kinder sie einfach per Mail schicken.

• Fotos unbekannter Absender warten auf Ihre Freigabe am Rahmen
• Freigegebene Absender landen ohne Nachfrage in der Diashow
• Verarbeitete Nachrichten werden als gelesen markiert
• Empfohlen: ein eigenes Postfach und ein App-Passwort statt des
  Kontopassworts — die App weist im Formular darauf hin


FÜR DEN DAUERBETRIEB GEBAUT

• Läuft aus dem Zwischenspeicher weiter, auch wenn das Netz ausfällt
• Der Zwischenspeicher hat eine feste Obergrenze und ersetzt selbsttätig, was
  am längsten nicht zu sehen war
• Zeitplan: nachts dunkler Bildschirm oder eine gedimmte Uhr
• Einbrennschutz für OLED-Displays
• Bildschirm bleibt an, ohne dass Sie etwas einstellen müssen


RUHIG UND UNAUFDRINGLICH

• Weiche Überblendungen, wahlweise mit langsamem Zoom
• Uhrzeit, Datum und Bildunterschrift einzeln zuschaltbar
• Hoch- und Querformat ohne Verzerrung, Hochformatfotos wahlweise paarweise
• Einstellungen erst nach langem Druck — nichts verstellt sich versehentlich


IM SMART HOME

Auf Wunsch nimmt Slowshow Befehle aus dem Heimnetz entgegen. Ein
Bewegungsmelder kann den Rahmen wecken, Home Assistant ihn schlafen legen.


DATENSCHUTZ

Keine Datenerhebung, keine Analyse, keine Werbenetzwerke. Slowshow spricht nur
mit den Servern, die Sie selbst eintragen. Der gesamte Quellcode ist offen
einsehbar (Apache-Lizenz 2.0).


WAS SLOWSHOW NICHT KANN

• Keine HEIC-Fotos aus lokalen Ordnern oder vom NAS. Über Nextcloud
  funktionieren sie, weil der Server sie umwandelt.
• Keine Videos. Bewegte Bilder von iPhone und Pixel werden als Standbild
  gezeigt — das Foto darin liest Slowshow, den Film nicht.
• Von animierten GIFs wird das erste Einzelbild gezeigt.
• Kein Start nach einem Neustart des Tablets — die App muss dann von Hand
  geöffnet werden.

Gelesen werden JPEG, PNG, WebP, BMP, TIFF, ICO und GIF.
```

---

## Berechtigungen

Slowshow fordert genau zwei Rechte an:

| Berechtigung | Grund |
| --- | --- |
| `INTERNET` | Fotos von NAS, Nextcloud oder Postfach laden; Steuerung im Heimnetz |
| `ACCESS_NETWORK_STATE` | Erkennen, ob eine Verbindung besteht |

**Für keines davon verlangt Google eine gesonderte Erklärung.** Das ist das
Ergebnis von E-38: Hier standen früher ausformulierte Begründungen für
`FOREGROUND_SERVICE_SPECIAL_USE` und `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`.
Beide Rechte sind entfallen, weil der Vordergrunddienst entfallen ist.

`specialUse` löst eine gesonderte Prüfung durch Google aus, mit ungewissem
Ausgang und ungewisser Dauer. Wer die alten Texte aus Versehen doch einreicht,
holt sich diese Prüfung zurück — für eine Funktion, die die App nicht mehr hat.
Deshalb stehen sie hier nicht mehr, auch nicht als Rest.

Zwei weitere Rechte tauchen in der Installationsübersicht auf, ohne dass
Slowshow sie anfordert: `ACCESS_LOCAL_NETWORK` aus der Android-WebView und
`DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION` aus AndroidX. Beide kommen aus
Bibliotheken und werden von der App nicht verwendet.

Was der Wegfall kostet, steht in E-38: Android kann die App bei Speicherdruck
beenden, und sie startet nicht von selbst neu. Das gehört in die Beschreibung
(siehe „Was Slowshow nicht kann“), nicht in eine Rechtfertigung.

### Zugriff auf lokale Dateien

```text
Slowshow fordert keine Medienberechtigung an. Der Zugriff auf lokale Fotos
läuft ausschließlich über das Storage Access Framework: Nutzer wählen im
Systemdialog einen Ordner aus und geben ihn frei. Ein pauschaler Zugriff auf
die Mediathek findet nicht statt.
```

---

## Data Safety

| Frage | Antwort |
| --- | --- |
| Werden Daten erhoben? | Nein — nichts fließt zum Entwickler oder zu Dritten |
| Werden Daten geteilt? | Nein |
| Verschlüsselung bei der Übertragung? | IMAP immer über TLS; NAS und Nextcloud verschlüsselt, sofern der vom Nutzer eingetragene Server HTTPS anbietet |
| Können Nutzer Löschung beantragen? | Entfällt — alle Daten liegen auf dem Gerät und verschwinden mit der Deinstallation |

Google zählt als „Erhebung“, dass Daten das Gerät verlassen. Zugangsdaten
gehen ausschließlich an die Server, die der Nutzer selbst einträgt.

**Auf dem Gerät gespeichert** — im Formular nicht als Erhebung zu melden, aber
zu kennen: Zugangsdaten (AES-256-GCM verschlüsselt), verkleinerte Kopien der
Fotos sowie **Absenderadressen und Betreffzeilen** empfangener Nachrichten.
Letztere sind personenbezogene Daten Dritter — der Menschen, die dem Nutzer
schreiben.

Maßgeblich ist der Abschnitt „Angaben für das Data-Safety-Formular“ in
[privacy-policy.md](privacy-policy.md); diese Tabelle ist die Kurzfassung.
Verantwortlich für die Angaben im Formular ist, wer die App einreicht.

Die Datenschutzerklärung muss vor der Veröffentlichung unter einer
öffentlich erreichbaren Adresse liegen — Play verlangt eine URL, keine Datei.

---

## Grafiken

| Element | Format | Quelle |
| --- | --- | --- |
| App-Icon | 512 × 512 PNG | `docs/slowshow-icon-512.png` (aus `npm run icons`) |
| Feature-Grafik | 1024 × 500 PNG | noch zu erstellen |
| Screenshots Tablet | mind. 2, 16:10 quer | vom Referenzgerät, sobald es feststeht (E-10) |

Empfohlene Screenshots:

1. Diashow mit Uhr und Bildunterschrift (entspricht dem Artboard „Diashow")
2. Einstellungen mit drei eingerichteten Quellen
3. Nachtmodus
4. Zeitplan

---

## Vor der Veröffentlichung zu klären

- [x] Referenzgerät festgelegt (E-10): Xiaomi Pad 6, Android 15 (MIUI)
- [x] Signaturschlüssel erzeugt und außerhalb des Repositories gesichert —
      siehe [signing.md](signing.md). `keystore.properties` liegt unter
      `src-tauri/gen/` und ist gitignored; die Vorlage überlebt in
      [keystore.properties.example](keystore.properties.example).
- [x] Signiertes AAB gebaut und gegengeprüft (`jarsigner -verify`)
- [x] Target-API-Level: `targetSdk = 36` — gegen die dann gültige Vorgabe
      erneut prüfen, wenn die Einreichung ansteht (R-13)
- [ ] Datenschutzerklärung öffentlich erreichbar machen (GitHub Pages)
- [ ] Play-Console-Konto anlegen (einmalige Gebühr, RB-04 erlaubt das)
- [ ] Screenshots vom Referenzgerät erstellen
- [ ] Feature-Grafik erstellen (1024 × 500)
- [ ] Interner Test mit dem Referenzgerät vor der offenen Veröffentlichung

## Interner Test

Beschlossen: **zuerst interner Test, dann offene Veröffentlichung.** Grund
steht unter „Nicht geprüft“ — der Dauerbetrieb ist die Kernzusage der App und
nie gemessen worden.

**Was hochgeladen wird**

`src-tauri/gen/android/app/build/outputs/bundle/universalRelease/app-universal-release.aab`

Neu bauen mit `npx tauri android build` (ohne `--apk`). Vorher prüfen, dass
`scripts/patch-android.mjs` „Signaturschlüssel gefunden“ meldet — sonst ist das
Bundle unsigniert, und das fällt sonst erst beim Hochladen auf.

**Weg in der Play Console**

1. Testen → Interner Test → Neuen Release erstellen
2. AAB hochladen
3. Tester über eine E-Mail-Liste hinzufügen
4. Opt-in-Link an die Tester geben

**Was Play vorher verlangt**

Der Abschnitt „App-Inhalte“ muss ausgefüllt sein, bevor sich überhaupt ein
Release anlegen lässt: Datenschutzerklärung (als **URL**, nicht als Datei),
Data Safety, Inhaltsfreigabe, Zielgruppe, Werbung. Die Vorlagen dafür stehen
weiter oben in dieser Datei.

Wie viel von der Store-Präsenz (Grafiken, Screenshots) schon für den internen
Test verlangt wird, unterscheidet sich je nach Kontostand und ändert sich
gelegentlich — die Console führt eine eigene Aufgabenliste, die maßgeblich ist.

**Vor der Produktion beachten**

Google verlangt von **neuen persönlichen Entwicklerkonten** vor dem Zugang zur
Produktion einen **geschlossenen** Test mit einer Mindestzahl an Testern über
einen zusammenhängenden Zeitraum. Ein *interner* Test erfüllt das nicht. Die
genauen Zahlen und die Frage, ob es für dieses Konto gilt, zeigt die Console —
das ist vor der Planung zu klären, weil es Wochen kostet, nicht Stunden.

---

## Nicht geprüft

Drei Zusagen des Lastenhefts sind nie gemessen worden. Sie stehen hier, damit
niemand sie für geprüft hält:

- **Dauerbetrieb über sieben Tage.** Ohne Vordergrunddienst (E-38) kann Android
  die App bei Speicherdruck beenden. Wie oft das geschieht, weiß niemand.
- **Bestand mit 5000 Fotos.** Getestet wurde mit 597.
- **Verhalten nach einem Neustart des Geräts.** Die Beschreibung sagt zu, dass
  die App dann von Hand gestartet werden muss — gemessen ist auch das nicht.

Die ersten beiden sprechen dafür, mit einem **internen Test** zu beginnen und
nicht mit der offenen Veröffentlichung.

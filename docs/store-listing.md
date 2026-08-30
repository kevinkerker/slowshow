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
Ihr Tablet als digitaler Bilderrahmen. Fotos vom NAS, aus der Cloud, vom Gerät.
```

(78 Zeichen)

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

Mehrere Quellen lassen sich gleichzeitig nutzen und einzeln ein- und
ausschalten.


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
• Keine Videos.
• Kein Start nach einem Neustart des Tablets — die App muss dann von Hand
  geöffnet werden.
```

---

## Begründungen für die Prüfung

Google verlangt für einzelne Berechtigungen eine Erklärung. Vorformuliert:

### `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`

```text
Slowshow ist ein digitaler Bilderrahmen, der auf einem fest installierten,
dauerhaft mit Strom versorgten Tablet läuft. Die Kernfunktion ist die
ununterbrochene Anzeige über Stunden und Tage hinweg.

Die Batterieoptimierung vieler Hersteller beendet oder drosselt langlaufende
Apps und würde die Diashow unbemerkt anhalten. Die App fragt die Ausnahme
einmalig an; sie ist nicht erzwungen, und die Nutzung ohne Ausnahme bleibt
möglich.
```

### `FOREGROUND_SERVICE_SPECIAL_USE`

Der Dienst-Typ `specialUse` verlangt eine gesonderte Erklärung im
Play-Console-Formular. Sie muss zu der `<property>` im Manifest passen (E-24):

```text
Slowshow ist ein digitaler Bilderrahmen. Die Kernfunktion ist eine Diashow, die
über Tage hinweg ohne Nutzerinteraktion weiterläuft, auf einem fest
installierten und dauerhaft mit Strom versorgten Tablet.

Ohne Vordergrunddienst stuft Android die App bei Speicherdruck als entbehrlich
ein und beendet sie; der Bildschirm bleibt dann schwarz, bis jemand die App von
Hand neu startet. Keiner der vordefinierten Dienst-Typen trifft zu: Slowshow
gibt keine Medien wieder, ortet nicht, misst nichts und synchronisiert nicht im
Hintergrund als Selbstzweck — die Bildanzeige selbst ist der Zweck.

Der Dienst hält keine Wakelocks über die Anzeige hinaus und startet keine
Netzwerkaktivität von sich aus.
```

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
| Werden Daten erhoben? | Nein |
| Werden Daten geteilt? | Nein |
| Verschlüsselung bei der Übertragung? | Ja, sofern der vom Nutzer eingetragene Server HTTPS anbietet |
| Können Nutzer Löschung beantragen? | Entfällt — es werden keine Daten übermittelt |

Datenschutzerklärung: [privacy-policy.md](privacy-policy.md), vor der
Veröffentlichung unter einer öffentlichen Adresse ablegen.

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

- [ ] Referenzgerät festlegen (E-10) — ohne echte Screenshots kein Store-Eintrag
- [ ] Datenschutzerklärung öffentlich erreichbar machen (GitHub Pages)
- [ ] Signaturschlüssel erzeugen und **außerhalb des Repositories** sichern
- [ ] Play-Console-Konto anlegen (einmalige Gebühr, RB-04 erlaubt das)
- [ ] Target-API-Level gegen die dann gültige Vorgabe prüfen (R-13)
- [ ] Feature-Grafik erstellen
- [ ] Interner Test mit dem Referenzgerät vor der offenen Veröffentlichung

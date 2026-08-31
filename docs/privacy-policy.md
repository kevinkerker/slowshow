# Datenschutzerklärung — Slowshow

**Stand:** 31. August 2026
**Verantwortlich:** Kevin Kerker (privates Open-Source-Projekt)

Diese Erklärung ist für die Veröffentlichung im Google Play Store nötig
(Lastenheft RB-03). Sie muss vor dem Store-Eintrag unter einer öffentlich
erreichbaren Adresse liegen — üblicherweise als GitHub Pages des Projekts.

---

## Kurzfassung

Slowshow sammelt keine Daten über Sie. Es gibt keine Konten, keine Analyse,
keine Werbung, keine Weitergabe an Dritte und keine Verbindung zu Servern des
Entwicklers. Die App spricht ausschließlich mit den Servern, die Sie selbst
eintragen.

Wenn Sie ein Postfach einrichten, liest die App darin Nachrichten und
**markiert sie als gelesen**. Das ist eine Veränderung an Ihrem Postfach; der
Abschnitt „Fotos per E-Mail" beschreibt sie genau.

---

## Welche Daten die App verarbeitet

Alle genannten Daten bleiben auf Ihrem Gerät.

| Daten | Zweck | Ablage |
| --- | --- | --- |
| Ihre Fotos | Anzeige als Diashow | verkleinerte Kopien im App-eigenen Cache |
| Ordnerfreigabe (Android SAF) | Zugriff auf den gewählten lokalen Ordner | Konfigurationsdatei der App |
| Adresse und Benutzername Ihres NAS bzw. Ihrer Nextcloud | Verbindungsaufbau | Konfigurationsdatei der App |
| Adresse und Benutzername Ihres Postfachs | Anmeldung per IMAP | Konfigurationsdatei der App |
| Passwörter zu NAS, Nextcloud und Postfach | Anmeldung an Ihren Servern | verschlüsselt (AES-256-GCM) im App-eigenen Verzeichnis |
| **Absenderadresse und Betreff** empfangener Nachrichten | Zuordnung der Fotos, Freigabe je Absender, Anzeige im Bild-Browser | Cache-Index der App |
| Liste freigegebener Absender | Fotos dieser Personen ohne Nachfrage anzeigen | Konfigurationsdatei der App |
| Kennungen verarbeiteter Nachrichten | verhindert, dass dieselbe Mail zweimal geholt wird | eigene Datei im App-Verzeichnis, auf 5 000 Einträge begrenzt |
| Protokoll der letzten 50 Abrufe | Fehlersuche: wann lief der Abruf, was kam an | eigene Datei im App-Verzeichnis |
| Ihre Einstellungen | Verhalten der Diashow | Konfigurationsdatei der App |

Absenderadressen und Betreffzeilen sind **personenbezogene Daten anderer
Menschen** — der Personen, die Ihnen schreiben. Sie verlassen Ihr Gerät nicht,
aber sie liegen darauf. Wer Zugriff auf Ihr entsperrtes Tablet hat, kann im
Bild-Browser sehen, wer Ihnen Fotos geschickt hat.

Die App fordert **keinen** pauschalen Zugriff auf Ihre Medien an. Lokale Ordner
werden ausschließlich über den Android-Systemdialog freigegeben; Sie wählen
selbst, welcher Ordner sichtbar wird.

Automatische Cloud-Sicherungen sind für die App abgeschaltet
(`android:allowBackup="false"`), damit Ihre Zugangsdaten das Gerät nicht über
ein Google-Backup verlassen.

---

## Fotos per E-Mail

Richten Sie ein Postfach ein, meldet die App sich in regelmäßigen Abständen
per IMAP an und sieht nach, ob neue Nachrichten da sind.

**Was die App liest.** In der Voreinstellung nur *ungelesene* Nachrichten im
eingestellten Ordner. Von jeder Nachricht wertet sie Absender, Betreff, Datum
und die Bildanhänge aus. Den Text der Nachricht wertet sie nur so weit aus, wie
nötig ist, um eingebettete Bilder zu finden; er wird nicht gespeichert.

**Was die App verändert.** Verarbeitete Nachrichten werden **als gelesen
markiert**. Das geschieht in Ihrem Postfach und ist von anderen Mailprogrammen
aus sichtbar. Die App löscht keine Nachrichten, verschiebt keine und verschickt
keine.

**Wenn Sie „Auch gelesene Nachrichten" einschalten**, sieht die App bei jedem
Lauf den *gesamten* Ordner durch — also auch Nachrichten, die nichts mit Fotos
zu tun haben, und auch solche, die Sie noch nicht gelesen haben. Sie werden
dabei ebenfalls als gelesen markiert. Diese Einstellung ist für den Fall
gedacht, dass zwei Rahmen an demselben Postfach hängen.

**Empfehlung.** Richten Sie ein **eigenes Postfach nur für den Rahmen** ein und
verwenden Sie ein **App-Passwort** statt Ihres Kontopassworts. Dann liegen dort
ausschließlich Fotos, das Passwort lässt sich einzeln widerrufen, und keine der
oben beschriebenen Wirkungen betrifft Ihre eigentliche Post. Die App weist im
Einrichtungsformular an beiden Stellen darauf hin.

**Fotos unbekannter Absender** werden nicht sofort angezeigt, sondern warten
auf Ihre Freigabe am Rahmen. Erst wenn Sie einem Absender ausdrücklich
vertrauen, gehen dessen Fotos ohne Nachfrage in die Diashow.

---

## Netzwerkverbindungen

Slowshow verbindet sich ausschließlich mit:

1. **Ihrem NAS bzw. Ihrer Nextcloud** — nur, wenn Sie eine solche Quelle
   eingerichtet haben, und nur mit der von Ihnen eingetragenen Adresse.
2. **Ihrem Mailserver** — nur, wenn Sie ein Postfach eingerichtet haben. Die
   Verbindung läuft über IMAP mit TLS-Verschlüsselung (üblicherweise Port 993).
3. **Ihrem MQTT-Broker** — nur, wenn Sie MQTT eingeschaltet haben.
4. **Geräten in Ihrem Heimnetz** — nur, wenn Sie die Steuerung im Heimnetz
   eingeschaltet haben. Dann nimmt die App auf dem eingestellten Port Befehle
   entgegen (etwa von Home Assistant). Die Funktion ist standardmäßig aus.

Es gibt keine Verbindung zu Servern des Entwicklers, zu Analysediensten, zu
Werbenetzwerken oder zu Schriftarten-Diensten. Die verwendeten Schriften sind
Teil der App.

---

## Berechtigungen und warum sie gebraucht werden

| Berechtigung | Grund |
| --- | --- |
| `INTERNET` | Fotos von Ihrem NAS, Ihrer Nextcloud oder Ihrem Postfach laden; Steuerung im Heimnetz |
| `ACCESS_NETWORK_STATE` | Erkennen, ob eine Verbindung besteht — ohne Netz zeigt die App weiter Bilder aus dem Zwischenspeicher |

Mehr fordert die App nicht an. Insbesondere **kein** Zugriff auf Kamera,
Mikrofon, Standort, Kontakte oder Ihre Mediensammlung.

Beim Bau kommen zwei Berechtigungen aus verwendeten Bibliotheken hinzu, die in
der Installationsübersicht auftauchen: `ACCESS_LOCAL_NETWORK` (aus der
Android-WebView) und eine app-eigene Kennung für interne Rundrufe
(`DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION`, aus AndroidX). Beide werden von
Slowshow selbst nicht verwendet.

---

## Diagnosebericht und Sicherung

**Diagnosebericht.** Auf Wunsch erzeugt die App unter „System" einen Bericht
für Fehlermeldungen. Er wird Ihnen vor jeder Weitergabe angezeigt und ist
anonymisiert: Er enthält **keine** Mailadressen, Servernamen, Quellennamen oder
Dateinamen. Absender erscheinen als „Absender A", „Absender B"; Fehlermeldungen
des Servers werden auf die erste Zeile und 200 Zeichen gekürzt. Der Bericht
wird nirgendwohin gesendet — Sie entscheiden, ob und wohin Sie ihn kopieren.

**Sicherung der Einstellungen.** Der Export schreibt eine JSON-Datei in einen
Ordner, den Sie im Systemdialog selbst wählen. Sie enthält Ihre Einstellungen
und Quellen — darunter Serveradressen, Benutzernamen und die Liste
freigegebener Absenderadressen —, aber **keine Passwörter**. Wo die Datei
danach liegt und wer sie lesen kann, entscheiden Sie: legen Sie sie in einen
Cloud-Ordner, verlässt sie Ihr Gerät. Nach dem Einspielen müssen Sie die
Zugangsdaten neu eingeben und die lokalen Ordner erneut freigeben.

---

## Angaben für das Data-Safety-Formular

Diese Angaben sind ein Vorschlag auf Grundlage des Quellcodes. Verantwortlich
für die Angaben im Formular ist, wer die App einreicht.

- **Erhobene oder geteilte Daten:** keine. Google zählt als „Erhebung", dass
  Daten das Gerät verlassen. Slowshow überträgt Zugangsdaten ausschließlich an
  die Server, die Sie selbst eintragen, und an niemanden sonst. Zum Entwickler
  oder zu Dritten fließt nichts.
- **Auf dem Gerät gespeichert:** Zugangsdaten, Absenderadressen und
  Betreffzeilen empfangener Nachrichten, verkleinerte Kopien Ihrer Fotos.
- **Verschlüsselung bei der Übertragung:** IMAP läuft ausschließlich über TLS.
  Für NAS und Nextcloud gilt: verschlüsselt, sofern Ihr Server HTTPS anbietet.
  Unverschlüsseltes HTTP ist zusätzlich möglich, weil viele NAS-Geräte im
  Heimnetz nur so erreichbar sind — die Verbindung verlässt Ihr Netz dabei
  nicht.
- **Verschlüsselung im Ruhezustand:** Zugangsdaten ja (AES-256-GCM). Fotos,
  Absenderadressen und Betreffzeilen liegen unverschlüsselt im App-eigenen
  Verzeichnis, das Android gegen andere Apps abschottet.
- **Löschung der Daten:** durch Deinstallation der App. Alle Daten liegen im
  App-eigenen Verzeichnis und werden dabei mit entfernt.

---

## Grenzen der Verschlüsselung

Die Zugangsdaten zu NAS, Nextcloud und Postfach werden mit AES-256-GCM
verschlüsselt gespeichert. Der Schlüssel liegt im App-eigenen Verzeichnis, das
Android gegen andere Apps abschottet. Er ist **nicht** an den Android Keystore
gebunden. Wer vollen Zugriff auf ein entsperrtes und gerootetes Gerät hat, kann
die Zugangsdaten daher auslesen — **auch das Passwort Ihres Postfachs**.

Ein Mailpasswort wiegt schwerer als ein NAS-Passwort im Heimnetz: Damit ließe
sich Ihre gesamte Korrespondenz lesen. Deshalb noch einmal ausdrücklich:

- Verwenden Sie ein **App-Passwort**, kein Kontopasswort. Es lässt sich
  einzeln widerrufen und gilt bei den meisten Anbietern nur für den Mailzugang.
- Richten Sie ein **eigenes Postfach für den Rahmen** ein. Dann liegen dort
  ausschließlich Fotos.
- Für NAS und Nextcloud: ein eigenes Konto mit **Nur-Lese-Rechten** auf die
  Fotoordner.

Diese drei Maßnahmen wirken an der Stelle, die zählt — sie begrenzen, was ein
gestohlenes Passwort überhaupt öffnet.

---

## Ihre Rechte

Da keine personenbezogenen Daten an den Entwickler übermittelt werden, gibt es
beim Entwickler keine Daten, über die Auskunft erteilt oder die gelöscht werden
könnten. Ihre Daten liegen ausschließlich bei Ihnen.

Wenn Sie Fotos per E-Mail empfangen, verarbeiten Sie damit Daten anderer
Personen auf Ihrem Gerät — Adressen und Betreffzeilen derer, die Ihnen
schreiben. Sie können jederzeit einzelne Fotos ausblenden, Absender von der
Freigabeliste nehmen und deren Fotos zurück in die Quarantäne schicken; die
Einstellungen der Quelle bieten beides an.

---

## Quellcode

Slowshow ist Open Source unter der Apache-Lizenz 2.0. Der vollständige
Quellcode ist einsehbar; alle hier gemachten Angaben lassen sich darin
nachprüfen.

## Kontakt

Fragen zum Datenschutz: über die Issues des Projekt-Repositories.

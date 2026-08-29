# Datenschutzerklärung — Slowshow

**Stand:** 29. August 2026
**Verantwortlich:** Kevin Kerker (privates Open-Source-Projekt)

Diese Erklärung ist für die Veröffentlichung im Google Play Store nötig
(Lastenheft RB-03). Sie muss vor dem Store-Eintrag unter einer öffentlich
erreichbaren Adresse liegen — üblicherweise als GitHub Pages des Projekts.

---

## Kurzfassung

Slowshow sammelt keine Daten. Es gibt keine Konten, keine Analyse, keine
Werbung, keine Weitergabe an Dritte. Die App spricht ausschließlich mit den
Servern, die Sie selbst eintragen.

---

## Welche Daten die App verarbeitet

Alle genannten Daten bleiben auf Ihrem Gerät.

| Daten | Zweck | Ablage |
| --- | --- | --- |
| Ihre Fotos | Anzeige als Diashow | verkleinerte Kopien im App-eigenen Cache |
| Ordnerfreigabe (Android SAF) | Zugriff auf den gewählten lokalen Ordner | Konfigurationsdatei der App |
| Adresse und Benutzername Ihres NAS bzw. Ihrer Nextcloud | Verbindungsaufbau | Konfigurationsdatei der App |
| Passwort Ihres NAS bzw. Ihrer Nextcloud | Anmeldung an Ihrem Server | verschlüsselt (AES-256-GCM) im App-eigenen Verzeichnis |
| Ihre Einstellungen | Verhalten der Diashow | Konfigurationsdatei der App |

Die App fordert **keinen** pauschalen Zugriff auf Ihre Medien an. Lokale Ordner
werden ausschließlich über den Android-Systemdialog freigegeben; Sie wählen
selbst, welcher Ordner sichtbar wird.

Automatische Cloud-Sicherungen sind für die App abgeschaltet
(`android:allowBackup="false"`), damit Ihre Zugangsdaten das Gerät nicht über
ein Google-Backup verlassen.

---

## Netzwerkverbindungen

Slowshow verbindet sich ausschließlich mit:

1. **Ihrem NAS bzw. Ihrer Nextcloud** — nur, wenn Sie eine solche Quelle
   eingerichtet haben, und nur mit der von Ihnen eingetragenen Adresse.
2. **Geräten in Ihrem Heimnetz** — nur, wenn Sie die Steuerung im Heimnetz
   eingeschaltet haben. Dann nimmt die App auf dem eingestellten Port Befehle
   entgegen (etwa von Home Assistant). Die Funktion ist standardmäßig aus.

Es gibt keine Verbindung zu Servern des Entwicklers, zu Analysediensten, zu
Werbenetzwerken oder zu Schriftarten-Diensten. Die verwendeten Schriften sind
Teil der App.

---

## Berechtigungen und warum sie gebraucht werden

| Berechtigung | Grund |
| --- | --- |
| `INTERNET` | Fotos von Ihrem NAS oder Ihrer Nextcloud laden; Steuerung im Heimnetz |
| `ACCESS_NETWORK_STATE` | Erkennen, ob eine Verbindung besteht — ohne Netz zeigt die App weiter Bilder aus dem Zwischenspeicher |
| `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` | Ohne die Ausnahme beenden viele Hersteller-Systeme dauerhaft laufende Apps. Ein Bilderrahmen, der nachts still ausgeht, erfüllt seinen Zweck nicht. Die App fragt die Ausnahme an; Sie können sie ablehnen. |

---

## Angaben für das Data-Safety-Formular

- **Erhobene Daten:** keine
- **Geteilte Daten:** keine
- **Verschlüsselung bei der Übertragung:** ja, sofern Ihr Server HTTPS
  anbietet. Unverschlüsseltes HTTP ist zusätzlich möglich, weil viele NAS-Geräte
  im Heimnetz nur so erreichbar sind — die Verbindung verlässt Ihr Netz dabei
  nicht.
- **Löschung der Daten:** durch Deinstallation der App. Alle Daten liegen im
  App-eigenen Verzeichnis und werden dabei mit entfernt.

---

## Grenzen der Verschlüsselung

Die Zugangsdaten zu Ihrem NAS oder Ihrer Nextcloud werden mit AES-256-GCM
verschlüsselt gespeichert. Der Schlüssel liegt im App-eigenen Verzeichnis, das
Android gegen andere Apps abschottet. Er ist derzeit **nicht** an den Android
Keystore gebunden. Wer vollen Zugriff auf ein entsperrtes und gerootetes Gerät
hat, kann die Zugangsdaten daher auslesen.

Empfehlung: Legen Sie für die App ein eigenes Konto auf Ihrem NAS an und geben
Sie ihm nur Leserechte auf die Fotoordner.

---

## Ihre Rechte

Da keine personenbezogenen Daten an den Entwickler übermittelt werden, gibt es
keine Daten, über die Auskunft erteilt oder die gelöscht werden könnten. Ihre
Daten liegen ausschließlich bei Ihnen.

---

## Quellcode

Slowshow ist Open Source unter der Apache-Lizenz 2.0. Der vollständige
Quellcode ist einsehbar; alle hier gemachten Angaben lassen sich darin
nachprüfen.

## Kontakt

Fragen zum Datenschutz: über die Issues des Projekt-Repositories.

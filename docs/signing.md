# Signaturschlüssel einrichten

Debug-Builds signiert Gradle selbst — zum Testen ist nichts zu tun:

```powershell
.\deploy-android.ps1 -dev
```

Ein **Release**-Build braucht dagegen einen eigenen Schlüssel. Ohne ihn heißt
das Ergebnis `app-universal-release-unsigned.apk` und lässt sich weder
installieren noch im Play Store veröffentlichen (RB-03).

---

## Einmalig: Schlüssel erzeugen

```powershell
& "$env:JAVA_HOME\bin\keytool.exe" -genkey -v `
  -keystore $env:USERPROFILE\slowshow-release.jks `
  -keyalg RSA -keysize 2048 -validity 10000 `
  -alias slowshow
```

> **Diesen Schlüssel sichern**, zusammen mit dem Passwort, außerhalb des
> Rechners. Wie schlimm ein Verlust ist, hängt davon ab, wie verteilt wird —
> siehe den nächsten Abschnitt.

Der Schlüssel gehört **nicht** ins Repository. `.gitignore` schließt `*.jks`,
`*.keystore` und `keystore.properties` bereits aus.

## Ein Schlüssel oder mehrere?

Android erkennt eine App an **Paketname plus Signatur**: Ein Update wird nur
angenommen, wenn es mit demselben Schlüssel signiert ist wie die installierte
Fassung. Der Schlüssel hängt also an der App-Identität, nicht an einem Konto.

Denselben Schlüssel für mehrere Apps zu verwenden, ist erlaubt und verbreitet.
Getrennte Schlüssel sind sauberer — wird einer kompromittiert, betrifft es nur
eine App —, bei einem einzelnen Projekt ist das aber eine theoretische
Unterscheidung.

## Was ein Verlust bedeutet

Hier unterscheiden sich die Verteilwege, und der Unterschied ist groß.

**Über den Play Store** gibt es seit 2021 zwei Schlüssel:

| Schlüssel | Wer hält ihn | Wozu |
| --- | --- | --- |
| **Upload-Schlüssel** | du | signiert das AAB, das du hochlädst |
| **App-Signaturschlüssel** | Google | signiert, was auf den Geräten ankommt |

Google prüft deine Signatur, entfernt sie und signiert neu. Geht dein
**Upload**-Schlüssel verloren, lässt er sich über den Google-Support
zurücksetzen: Du erzeugst einen neuen und meldest ihn an. Der
App-Signaturschlüssel bleibt unverändert, die Geräte merken nichts davon.

**Außerhalb des Play Store** — direktes APK, GitHub Releases, F-Droid —
signierst du selbst, und was auf den Geräten ankommt, trägt deine Signatur.
Geht der Schlüssel verloren, gibt es **keinen Weg zurück**: Nutzer müssten
deinstallieren und neu installieren und verlören dabei Einstellungen und
Zwischenspeicher.

Wer beide Wege gehen will, nimmt denselben Schlüssel — bei Play ist er dann der
Upload-Schlüssel, außerhalb der eigentliche Signaturschlüssel.

## Gradle bekanntmachen

`src-tauri/gen/android/app/keystore.properties` anlegen:

```properties
storeFile=C:/Users/DEINNAME/slowshow-release.jks
storePassword=...
keyAlias=slowshow
keyPassword=...
```

Die `signingConfig` in `build.gradle.kts` trägt **nicht** Tauri ein, sondern
`scripts/patch-android.mjs` — zusammen mit dem übrigen nativen Code, vor jedem
Android-Bau. Das ist nötig, weil `gen/` generiert und gitignored ist: ein von
Hand eingefügter Block wäre beim nächsten `init` verschwunden.

> Ohne diesen Abschnitt liest **niemand** `keystore.properties`. Gradle meldet
> dann trotzdem Erfolg und legt eine Datei mit dem Zusatz `-unsigned` ab — der
> einzige Hinweis darauf, dass etwas fehlt. Das Einspielskript sagt seit diesem
> Fund im Klartext, ob ein Schlüssel gefunden wurde.

Danach liefert

```powershell
.\deploy-android.ps1
```

ein installierbares `app-universal-release.apk`.

Die Datei liegt unter `src-tauri/gen/` und ist damit gitignored — nach einem
`tauri android init` muss sie neu angelegt werden. Das ist Absicht: ein
Schlüsselpfad mit Passwort hat im Repository nichts verloren.

## Für den Play Store

Der Store nimmt kein APK, sondern ein App Bundle:

```powershell
npx tauri android build
```

Das Ergebnis liegt unter
`src-tauri/gen/android/app/build/outputs/bundle/universalRelease/`.

Weiter mit der Checkliste in [store-listing.md](store-listing.md).

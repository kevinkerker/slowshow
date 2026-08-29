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

> **Diesen Schlüssel sichern.** Geht er verloren, lässt sich die App im Play
> Store nie wieder aktualisieren — Google akzeptiert nur Updates mit demselben
> Schlüssel. Eine Kopie außerhalb des Rechners aufbewahren, zusammen mit dem
> Passwort.

Der Schlüssel gehört **nicht** ins Repository. `.gitignore` schließt `*.jks`,
`*.keystore` und `keystore.properties` bereits aus.

## Gradle bekanntmachen

`src-tauri/gen/android/app/keystore.properties` anlegen:

```properties
storeFile=C:/Users/DEINNAME/slowshow-release.jks
storePassword=...
keyAlias=slowshow
keyPassword=...
```

Tauri erzeugt die passende `signingConfig` in `build.gradle.kts`, sobald die
Datei vorhanden ist. Danach liefert

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

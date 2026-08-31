# Baut das APK und installiert es per ADB auf dem angeschlossenen Geraet.
# Voraussetzung: Geraet per USB verbunden, USB-Debugging aktiv.
#
# Verwendung:
#   .\deploy-android.ps1                # Release-Build (braucht einen Signaturschluessel)
#   .\deploy-android.ps1 -dev           # Debug-Build fuer alle vier ABIs
#   .\deploy-android.ps1 -dev -arm64    # nur fuers Tablet  <- schnellster Weg zum Testen
#
# Zum Ausprobieren ist -dev der richtige Weg: Debug-APKs signiert Gradle
# selbst, ein Release-APK ohne eingerichteten Schluessel laesst sich nicht
# installieren.
#
# -arm64 baut nur fuer aarch64 statt fuer alle vier ABIs. Gradle ruft den
# Rust-Build einmal *je ABI* auf, es entfaellt also drei Viertel der
# Uebersetzung; und das Debug-APK schrumpft deutlich, weil die ungestrippten
# Rust-Debug-Bibliotheken den Grossteil davon ausmachen (gemessen: 1,4 GB fuer
# alle vier). Fuer jedes Tablet und Telefon der letzten Jahre reicht aarch64.
# Nicht geeignet fuer den Emulator (meist x86_64) und nicht fuer ein Release,
# das in den Play Store soll -- dort werden alle ABIs gebraucht.

param(
    [switch]$dev,
    [switch]$arm64
)

$ErrorActionPreference = "Stop"

$adb = if (Get-Command adb -ErrorAction SilentlyContinue) {
    "adb"
} else {
    "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe"
}

if (-not (Test-Path $adb)) {
    Write-Error "adb nicht gefunden. Android-SDK-Platform-Tools installieren oder adb in den PATH legen."
    exit 1
}

# Ist ueberhaupt ein Geraet da? Sonst laeuft der Build minutenlang ins Leere.
$devices = & $adb devices | Select-Object -Skip 1 | Where-Object { $_ -match "\sdevice$" }
if (-not $devices) {
    Write-Warning "Kein Geraet verbunden. USB-Debugging aktiv? (adb devices zeigt nichts)"
    Write-Warning "Der Build laeuft trotzdem, die Installation wird uebersprungen."
}

# Nativen Kotlin-Code aus src-tauri/android-src/ in das generierte Projekt spiegeln.
# Muss vor jedem Build laufen, da `tauri android init` gen/ ueberschreiben kann.
node scripts/patch-android.mjs
if ($LASTEXITCODE -ne 0) { exit 1 }

$outputs = "src-tauri\gen\android\app\build\outputs\apk"

# Als Array aufgebaut und mit @() uebergeben: eine zusammengesetzte
# Zeichenkette wuerde von PowerShell als *ein* Argument an npx gereicht, und
# tauri saehe "--apk --debug" als einen einzigen Schalter.
$buildArgs = @("tauri", "android", "build", "--apk")
if ($dev) {
    $buildArgs += "--debug"
    $variant = "debug"
} else {
    $variant = "release"
}
if ($arm64) {
    $buildArgs += @("--target", "aarch64")
    Write-Host "Baue nur fuer aarch64 - drei Viertel der Uebersetzung entfallen."
}

# Vorhandenes APK wegraeumen, bevor Gradle baut.
#
# Gradles zipflinger schreibt inkrementell in das bestehende Archiv und laesst
# verdraengte Bloecke physisch darin stehen -- gemessen: 633 MB Datei bei
# 325 MB tatsaechlichem Inhalt, also noch einmal die komplette alte
# Rust-Bibliothek als Leiche. Installiert wird nur der gueltige Teil, aber
# `adb install` uebertraegt die ganze Datei.
Get-ChildItem -Path "$outputs\*\$variant\*.apk" -ErrorAction SilentlyContinue |
    Remove-Item -Force -ErrorAction SilentlyContinue

npx @buildArgs
if ($LASTEXITCODE -ne 0) { exit 1 }

# Den tatsaechlich erzeugten Dateinamen suchen statt ihn zu raten:
# ohne Signaturschluessel heisst das Release-APK "…-release-unsigned.apk",
# mit Schluessel "…-release.apk".
#
# Das Muster gehoert in den Pfad, nicht in -Filter: in Windows PowerShell 5.1
# liefert `Get-ChildItem -Path "…\*\debug" -Filter "*.apk"` nichts. Loest der
# Pfad selbst auf ein Verzeichnis auf, prueft -Filter den Verzeichnisnamen
# gegen das Muster statt dessen Inhalt — "debug" ist kein "*.apk", also bleibt
# das Ergebnis leer. Ohne -ErrorAction faellt das auf; mit ihm sah es aus, als
# haette der Build kein APK erzeugt.
$apk = Get-ChildItem -Path "$outputs\*\$variant\*.apk" -ErrorAction SilentlyContinue |
       Sort-Object LastWriteTime -Descending |
       Select-Object -First 1

if (-not $apk) {
    Write-Error "Kein APK unter $outputs\*\$variant gefunden."
    exit 1
}

Write-Host ""
Write-Host ("Gebaut: {0} ({1:N1} MB)" -f $apk.Name, ($apk.Length / 1MB))

if ($apk.Name -like "*unsigned*") {
    Write-Warning "Dieses APK ist nicht signiert und laesst sich nicht installieren."
    Write-Warning "Zum Testen: .\deploy-android.ps1 -dev"
    Write-Warning "Fuer Release-Builds einen Schluessel einrichten - siehe notes/signing.md"
    exit 1
}

# Ohne Geraet gibt es nichts zu installieren. Das ist kein Fehler des Baus,
# aber auch kein Erfolg des Deploys -- der Rueckgabewert muss es sagen.
#
# Hier stand bis heute ein zweiter, gleichlautender Block mit `exit 0` davor.
# Der Zusatz war damit unerreichbar: toter Code, der wie eine Absicherung
# aussah. Aufgefallen erst, als wirklich kein Geraet angeschlossen war.
if (-not $devices) {
    Write-Warning "APK gebaut, aber nicht installiert: kein Geraet verbunden."
    exit 2
}

Write-Host "Installiere..."
& $adb install -r $apk.FullName
if ($LASTEXITCODE -ne 0) {
    # Vorher endete das Skript hier still mit 0. Wer den Rueckgabewert als
    # Erfolgsmerkmal nahm -- ein Skript, eine Pipeline, ein Agent --, hielt
    # einen abgezogenen USB-Stecker fuer einen geglueckten Deploy.
    Write-Error "Installation fehlgeschlagen. Geraet noch angeschlossen und entsperrt?"
    exit 1
}

Write-Host "Starte App..."
& $adb shell am start -n "dev.kerker.slowshow/.MainActivity"
if ($LASTEXITCODE -ne 0) {
    Write-Error "App liess sich nicht starten."
    exit 1
}

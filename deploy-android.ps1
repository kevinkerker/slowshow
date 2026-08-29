# Baut das APK und installiert es per ADB auf dem angeschlossenen Geraet.
# Voraussetzung: Geraet per USB verbunden, USB-Debugging aktiv.
#
# Verwendung:
#   .\deploy-android.ps1           # Release-Build (braucht einen Signaturschluessel)
#   .\deploy-android.ps1 -dev      # Debug-Build, mit Chrome DevTools  <- zum Testen
#
# Zum Ausprobieren ist -dev der richtige Weg: Debug-APKs signiert Gradle
# selbst, ein Release-APK ohne eingerichteten Schluessel laesst sich nicht
# installieren.

param(
    [switch]$dev
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

if ($dev) {
    npx tauri android build --apk --debug
    if ($LASTEXITCODE -ne 0) { exit 1 }
    $variant = "debug"
} else {
    npx tauri android build --apk
    if ($LASTEXITCODE -ne 0) { exit 1 }
    $variant = "release"
}

# Den tatsaechlich erzeugten Dateinamen suchen statt ihn zu raten:
# ohne Signaturschluessel heisst das Release-APK "…-release-unsigned.apk",
# mit Schluessel "…-release.apk".
$apk = Get-ChildItem -Path "$outputs\*\$variant" -Filter "*.apk" -ErrorAction SilentlyContinue |
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
    Write-Warning "Fuer Release-Builds einen Schluessel einrichten - siehe docs/signing.md"
    exit 1
}

if (-not $devices) {
    Write-Host "Kein Geraet verbunden - Installation uebersprungen."
    exit 0
}

Write-Host "Installiere..."
& $adb install -r $apk.FullName
if ($LASTEXITCODE -eq 0) {
    Write-Host "Starte App..."
    & $adb shell am start -n "dev.kerker.slowshow/.MainActivity"
}

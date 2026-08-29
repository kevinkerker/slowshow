// Spielt den handgeschriebenen nativen Code in das generierte Android-Projekt ein.
//
// ## Warum es dieses Skript gibt
//
// `src-tauri/gen/` wird von `tauri android init` erzeugt und ist gitignored.
// Wer die MainActivity dort direkt bearbeitet, verliert die Änderungen beim
// nächsten `init` — und weil das Verzeichnis ignoriert ist, merkt es niemand.
// Genau diese Falle steckt im Schwesterprojekt Equity-Cove: dessen
// MainActivity.kt trägt handgeschriebene Zeilen, ist aber nicht versioniert.
//
// Für Slowshow ist das kritisch, weil gleich drei MUSS-Anforderungen dort
// landen: Vollbild (FA-01), Bildschirm an (FA-50) und Helligkeit (FA-53).
//
// Deshalb: Quelle der Wahrheit ist `src-tauri/android-src/`, dieses Skript
// spiegelt sie nach `gen/`. Es läuft automatisch vor jedem Android-Build.
//
//     node scripts/patch-android.mjs
//
// Ohne generiertes Projekt beendet es sich mit einem Hinweis, nicht mit einem
// Fehler — so lässt sich `npm run android:build` auch auf einem frischen
// Rechner ohne vorheriges `init` aufrufen.

import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const srcDir = resolve(root, 'src-tauri/android-src')
const genDir = resolve(root, 'src-tauri/gen/android')
const PACKAGE_PATH = 'dev/kerker/slowshow'

if (!existsSync(genDir)) {
  console.log('Kein generiertes Android-Projekt gefunden.')
  console.log('Zuerst einmalig: npx tauri android init')
  process.exit(0)
}

// ── 1. MainActivity ──────────────────────────────────────────────────────────

const activityTarget = resolve(genDir, `app/src/main/java/${PACKAGE_PATH}/MainActivity.kt`)
mkdirSync(dirname(activityTarget), { recursive: true })
copyFileSync(resolve(srcDir, 'MainActivity.kt'), activityTarget)
console.log('MainActivity.kt eingespielt (FA-01, FA-50, FA-53)')

// ── 2. AndroidManifest ───────────────────────────────────────────────────────

const manifestPath = resolve(genDir, 'app/src/main/AndroidManifest.xml')
if (!existsSync(manifestPath)) {
  console.error('AndroidManifest.xml nicht gefunden — ist `tauri android init` durchgelaufen?')
  process.exit(1)
}

const additions = readFileSync(resolve(srcDir, 'AndroidManifest.additions.xml'), 'utf8')
let manifest = readFileSync(manifestPath, 'utf8')

/** Zieht die `uses-permission`- und `uses-feature`-Zeilen aus der Vorlage. */
function extractElements(source, tag) {
  const pattern = new RegExp(`<${tag}[^>]*/>`, 'g')
  return source.match(pattern) ?? []
}

/** Liest die Attribut-Vorgaben eines Abschnitts als Name/Wert-Paare. */
function extractAttributes(source, section) {
  const block = source.match(new RegExp(`<${section}>([\\s\\S]*?)</${section}>`))
  if (!block) return []
  const pattern = /<attribute\s+name="([^"]+)"\s+value="([^"]+)"\s*\/>/g
  const result = []
  let match
  while ((match = pattern.exec(block[1])) !== null) {
    result.push({ name: match[1], value: match[2] })
  }
  return result
}

/** Fügt ein Element vor `</manifest>` ein, falls es noch fehlt. */
function ensureElement(xml, element) {
  const nameMatch = element.match(/android:name="([^"]+)"/)
  if (nameMatch && xml.includes(nameMatch[1])) return xml
  return xml.replace('</manifest>', `    ${element}\n</manifest>`)
}

/** Setzt oder ersetzt ein Attribut im angegebenen Start-Tag. */
function setAttribute(xml, tagName, attr, value) {
  const tagPattern = new RegExp(`<${tagName}\\b[^>]*>`)
  const tag = xml.match(tagPattern)
  if (!tag) {
    console.warn(`  <${tagName}> nicht gefunden — ${attr} übersprungen`)
    return xml
  }

  const existing = new RegExp(`\\s${attr.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}="[^"]*"`)
  const updated = existing.test(tag[0])
    ? tag[0].replace(existing, ` ${attr}="${value}"`)
    : tag[0].replace(/>$/, ` ${attr}="${value}">`)

  return xml.replace(tagPattern, updated)
}

for (const permission of extractElements(additions, 'uses-permission')) {
  manifest = ensureElement(manifest, permission)
}
for (const feature of extractElements(additions, 'uses-feature')) {
  manifest = ensureElement(manifest, feature)
}
for (const { name, value } of extractAttributes(additions, 'application-attributes')) {
  manifest = setAttribute(manifest, 'application', name, value)
}
for (const { name, value } of extractAttributes(additions, 'activity-attributes')) {
  manifest = setAttribute(manifest, 'activity', name, value)
}

writeFileSync(manifestPath, manifest, 'utf8')
console.log('AndroidManifest.xml ergänzt (Rechte, Querformat, kein Backup)')

// ── 3. Kontrolle ─────────────────────────────────────────────────────────────

const checks = [
  ['android.permission.INTERNET', 'Netzzugriff für FA-21/FA-23'],
  ['REQUEST_IGNORE_BATTERY_OPTIMIZATIONS', 'Akku-Ausnahme für R-04'],
  ['android:allowBackup="false"', 'kein Cloud-Backup der Zugangsdaten (NF-05)'],
  ['sensorLandscape', 'Querformat (RB-02)'],
]

let missing = 0
for (const [needle, why] of checks) {
  if (!manifest.includes(needle)) {
    console.error(`  fehlt: ${needle} — ${why}`)
    missing++
  }
}

if (missing > 0) {
  console.error(`\n${missing} Manifest-Eintrag/-Einträge fehlen.`)
  process.exit(1)
}

// Sicherung gegen versehentliches Übernehmen aus einer Tresor-App:
// FLAG_SECURE würde Screenshots und die App-Übersicht schwärzen — für einen
// Bilderrahmen funktional falsch.
//
// Kommentarzeilen ausnehmen: die MainActivity erklärt in ihrem Kopfkommentar
// genau, warum das Flag fehlt, und darf sich daran nicht selbst aufhängen.
const activityCode = readFileSync(activityTarget, 'utf8')
  .split('\n')
  .filter((line) => {
    const trimmed = line.trimStart()
    return !trimmed.startsWith('//') && !trimmed.startsWith('*') && !trimmed.startsWith('/*')
  })
  .join('\n')

if (activityCode.includes('FLAG_SECURE')) {
  console.error('FLAG_SECURE in MainActivity.kt — für einen Bilderrahmen falsch.')
  process.exit(1)
}

// Gegenprobe: die drei Anforderungen müssen tatsächlich im Code stehen.
for (const [needle, why] of [
  ['FLAG_KEEP_SCREEN_ON', 'Bildschirm bleibt an (FA-50)'],
  ['systemBars', 'Vollbild ohne Systemleisten (FA-01)'],
  ['screenBrightness', 'Displayhelligkeit (FA-53)'],
]) {
  if (!activityCode.includes(needle)) {
    console.error(`  fehlt in MainActivity.kt: ${needle} — ${why}`)
    process.exit(1)
  }
}

console.log('Nativer Code ist auf Stand.')

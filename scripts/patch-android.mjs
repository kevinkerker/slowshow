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

import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  writeFileSync,
} from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { SIGN_MARKER, keystoreIssue, withSigningConfig } from './lib/android-signing.mjs'

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

// Alle Kotlin-Dateien, nicht nur die MainActivity: mit dem Vordergrunddienst
// (E-24) gibt es eine zweite, und eine namentliche Liste hier waere die Art
// Stelle, die man beim Hinzufuegen der dritten vergisst.
const javaDir = resolve(genDir, `app/src/main/java/${PACKAGE_PATH}`)
mkdirSync(javaDir, { recursive: true })

const kotlinFiles = readdirSync(srcDir).filter((f) => f.endsWith('.kt'))
if (kotlinFiles.length === 0) {
  console.error('Keine .kt-Datei in src-tauri/android-src/ gefunden.')
  process.exit(1)
}
for (const file of kotlinFiles) {
  copyFileSync(resolve(srcDir, file), resolve(javaDir, file))
}
const activityTarget = resolve(javaDir, 'MainActivity.kt')
console.log(`${kotlinFiles.length} Kotlin-Datei(en) eingespielt: ${kotlinFiles.join(', ')}`)

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

/**
 * Liest die vollstaendigen `<service>`-Bloecke aus der Vorlage.
 *
 * Anders als Rechte und Merkmale sind das keine leeren Elemente: der
 * Vordergrunddienst traegt eine `<property>` mit der Begruendung fuer
 * `specialUse` (E-24), die mitkopiert werden muss.
 */
function extractServices(source) {
  const block = source.match(/<services>([\s\S]*?)<\/services>/)
  if (!block) return []
  return block[1].match(/<service[\s\S]*?<\/service>/g) ?? []
}

/**
 * Fuegt einen Dienst in `<application>` ein, falls er noch fehlt.
 *
 * Vor `</application>` und nicht vor `</manifest>`: ein `<service>` ausserhalb
 * von `<application>` laesst Gradle den Build mit einer Meldung abbrechen, die
 * nicht auf dieses Skript zeigt.
 */
function ensureService(xml, service) {
  const nameMatch = service.match(/android:name="([^"]+)"/)
  if (nameMatch && xml.includes(`android:name="${nameMatch[1]}"`)) return xml
  const indented = service
    .split(/\r?\n/)
    .map((line) => (line.trim() ? `        ${line.trim()}` : line))
    .join('\n')
  return xml.replace('</application>', `${indented}\n    </application>`)
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
/**
 * Rechte, die **nicht** mehr im Manifest stehen duerfen (E-38).
 *
 * Der Vordergrunddienst ist entfallen, und mit ihm drei Rechte. Ein
 * versehentlich wieder eingefuegtes waere im Play Store ein Pruefpunkt, den
 * niemand mehr erwartet — und in einer generierten Datei faellt es keinem auf.
 */
const forbidden = [
  ['FOREGROUND_SERVICE', 'Vordergrunddienst entfaellt seit E-38'],
  ['POST_NOTIFICATIONS', 'ohne Dienst gibt es keine Benachrichtigung (E-38)'],
  ['REQUEST_IGNORE_BATTERY_OPTIMIZATIONS', 'nie im Code verwendet, entfernt mit E-38'],
]

for (const service of extractServices(additions)) {
  manifest = ensureService(manifest, service)
}

/**
 * Entfernt Rechte und Dienste, die frueher einmal eingetragen wurden (E-38).
 *
 * `gen/` ist generiert, aber nicht bei jedem Lauf frisch: was dieses Skript
 * einmal hineingeschrieben hat, bleibt dort stehen. Ohne diesen Schritt truege
 * das Manifest die Rechte des Vordergrunddienstes weiter — unsichtbar, weil
 * niemand eine generierte Datei liest, und im Play Store ein Pruefpunkt, den
 * niemand mehr erwartet.
 */
function removeElement(xml, needle) {
  return xml
    .split(/\r?\n/)
    .filter((line) => !line.includes(needle))
    .join('\n')
}

for (const [needle] of forbidden) {
  manifest = removeElement(manifest, needle)
}
// Der Dienst selbst samt seiner <property>-Begruendung.
manifest = manifest.replace(/\s*<service[\s\S]*?SlowshowService[\s\S]*?<\/service>/g, '')

writeFileSync(manifestPath, manifest, 'utf8')
console.log('AndroidManifest.xml ergänzt (Rechte, Querformat, kein Backup)')

// ── 3. App-Icon ──────────────────────────────────────────────────────────────
//
// `tauri icon` legt den fertigen Android-Satz unter src-tauri/icons/android/ ab,
// aber `tauri android init` ueberschreibt gen/.../res/ mit Tauris Standardlogo.
// Wer nur `tauri icon` laufen laesst, hat deshalb weiterhin das Tauri-Logo auf
// dem Geraet -- genau so ist es hier passiert und erst am Startbildschirm
// aufgefallen. Also gehoert auch das Icon in dieses Skript.

const iconSrc = resolve(root, 'src-tauri/icons/android')
const resDir = resolve(genDir, 'app/src/main/res')

/** Kopiert einen Verzeichnisbaum rekursiv. */
function copyTree(from, to) {
  let count = 0
  for (const item of readdirSync(from, { withFileTypes: true })) {
    const src = resolve(from, item.name)
    const dst = resolve(to, item.name)
    if (item.isDirectory()) {
      mkdirSync(dst, { recursive: true })
      count += copyTree(src, dst)
    } else {
      mkdirSync(dirname(dst), { recursive: true })
      copyFileSync(src, dst)
      count++
    }
  }
  return count
}

if (existsSync(iconSrc)) {
  const copied = copyTree(iconSrc, resDir)

  // Die Hintergrundfarbe des adaptiven Icons schreibt dieses Skript selbst,
  // statt sie mitzukopieren: `tauri icon` setzt #fff, der Entwurf verlangt
  // Tiefschwarz (E-13). Eine von Hand korrigierte Datei waere beim naechsten
  // `npm run icons` wieder weiss.
  const background = resolve(resDir, 'values/ic_launcher_background.xml')
  mkdirSync(dirname(background), { recursive: true })
  writeFileSync(
    background,
    [
      '<?xml version="1.0" encoding="utf-8"?>',
      '<!-- Erzeugt von scripts/patch-android.mjs. Farbe aus E-13. -->',
      '<resources>',
      '  <color name="ic_launcher_background">#0A0A0A</color>',
      '</resources>',
      '',
    ].join('\n'),
    'utf8',
  )
  console.log(`App-Icon eingespielt (${copied} Dateien, Hintergrund #0A0A0A)`)
} else {
  console.warn('src-tauri/icons/android/ fehlt — einmalig `npm run icons` ausfuehren')
}

// ── 4. Signaturschlüssel ─────────────────────────────────────────────────
//
// Tauri legt in `build.gradle.kts` **keine** `signingConfig` an. Ohne diesen
// Abschnitt liest niemand `keystore.properties`, und der Release-Bau endet als
// `app-universal-release-unsigned.apk` — ohne Fehlermeldung: Gradle meldet
// Erfolg, die Datei lässt sich nur weder installieren noch hochladen (RB-03).
// Genau so ist der erste signierte Bau hier ausgegangen.
//
// Der Eingriff gehört hierher und nicht in die generierte Datei: `gen/` ist
// gitignored, ein dort von Hand eingefügter Block wäre beim nächsten
// `tauri android init` weg — und weil das Verzeichnis ignoriert ist, ohne dass
// es jemand bemerkt.

const gradlePath = resolve(genDir, 'app/build.gradle.kts')
if (!existsSync(gradlePath)) {
  console.error('app/build.gradle.kts fehlt — ist `tauri android init` durchgelaufen?')
  process.exit(1)
}

let result
try {
  result = withSigningConfig(readFileSync(gradlePath, 'utf8'))
} catch (error) {
  console.error(`  ${error.message}`)
  process.exit(1)
}

if (result.changed) {
  writeFileSync(gradlePath, result.gradle, 'utf8')
  console.log('build.gradle.kts um signingConfig ergänzt')
} else {
  console.log(`Signaturkonfiguration steht bereits in build.gradle.kts (${SIGN_MARKER})`)
}

const keystorePath = resolve(genDir, 'app/keystore.properties')
const issue = keystoreIssue(
  existsSync(keystorePath) ? readFileSync(keystorePath, 'utf8') : null,
  existsSync,
)
if (issue) {
  console.warn(`  ${issue}`)
  console.warn('  Release-Builds bleiben unsigniert. Vorlage:')
  console.warn('  docs/keystore.properties.example — Anleitung: docs/signing.md')
} else {
  console.log('Signaturschlüssel gefunden — Release-Builds werden signiert')
}

// ── 5. Kontrolle ─────────────────────────────────────────────────────────────

const checks = [
  ['android.permission.INTERNET', 'Netzzugriff für FA-21/FA-23'],
  ['android:allowBackup="false"', 'kein Cloud-Backup der Zugangsdaten (NF-05)'],
  ['sensorLandscape', 'Querformat (RB-02)'],
]

// `forbidden` steht oben, wo das Manifest zusammengebaut wird — dort wird es
// zum Aufraeumen gebraucht, hier nur noch zur Gegenprobe.
for (const [needle, why] of forbidden) {
  if (manifest.includes(needle)) {
    console.error(`${needle} steht wieder im Manifest — ${why}`)
    process.exit(1)
  }
}

// Gegenprobe fuers Icon: liegt in gen/ noch Tauris Standardlogo, faellt das
// sonst erst auf dem Startbildschirm des Geraets auf.
const launcher = resolve(resDir, 'mipmap-xxxhdpi/ic_launcher.png')
if (existsSync(launcher) && existsSync(iconSrc)) {
  const own = readFileSync(resolve(iconSrc, 'mipmap-xxxhdpi/ic_launcher.png'))
  if (!readFileSync(launcher).equals(own)) {
    console.error('App-Icon in gen/ weicht von src-tauri/icons/android/ ab.')
    process.exit(1)
  }
}

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

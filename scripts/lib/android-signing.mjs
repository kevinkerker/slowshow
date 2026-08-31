// Traegt die Signaturkonfiguration in ein generiertes build.gradle.kts ein.
//
// Eigenes Modul, weil `patch-android.mjs` beim Import sofort losarbeitet: es
// kopiert Dateien und ruft `process.exit`. Ein Test koennte es nicht laden,
// ohne das generierte Projekt zu veraendern. Hier steht nur Textumformung --
// rein, ohne Dateisystem, damit pruefbar.

/** Erkennungszeile im erzeugten Gradle-Skript. Macht den Lauf wiederholbar. */
export const SIGN_MARKER = 'slowshow-signing'

const LOADER = `// ${SIGN_MARKER}: eingespielt von scripts/patch-android.mjs
val keystoreProperties = Properties().apply {
    val f = file("keystore.properties")
    if (f.exists()) f.inputStream().use { load(it) }
}

android {`

// Vor `buildTypes`: die Kotlin-DSL wertet den Block der Reihe nach aus. Ein
// erst danach deklarierter signingConfig waere beim Zugriff noch nicht da.
const CONFIGS = `    signingConfigs {
        create("release") {
            val storePath = keystoreProperties.getProperty("storeFile")
            if (storePath != null && file(storePath).exists()) {
                storeFile = file(storePath)
                storePassword = keystoreProperties.getProperty("storePassword") ?: ""
                keyAlias = keystoreProperties.getProperty("keyAlias") ?: ""
                keyPassword = keystoreProperties.getProperty("keyPassword") ?: ""
            }
        }
    }

    buildTypes {`

// Zugewiesen nur, wenn ein Schluessel eingetragen ist: sonst braeche Gradle mit
// "storeFile not set" ab, und ein frisch geklontes Projekt liesse sich
// ueberhaupt nicht mehr bauen -- auch kein Debug-Bau.
const ASSIGN = `        getByName("release") {
            if (keystoreProperties.getProperty("storeFile") != null) {
                signingConfig = signingConfigs.getByName("release")
            }`

/**
 * Ergaenzt `build.gradle.kts` um Schluessel-Loader, signingConfig und Zuweisung.
 *
 * Tauri legt von sich aus **keine** signingConfig an. Fehlt sie, liest niemand
 * `keystore.properties`, und der Release-Bau endet als `-unsigned.apk` -- ohne
 * Fehlermeldung. Genau so ist der erste signierte Bau hier ausgegangen.
 *
 * @param {string} gradle Inhalt der generierten Datei
 * @returns {{ gradle: string, changed: boolean }}
 * @throws {Error} wenn die Tauri-Vorlage nicht mehr passt
 */
export function withSigningConfig(gradle) {
  if (gradle.includes(SIGN_MARKER)) return { gradle, changed: false }

  for (const anchor of ['android {', '    buildTypes {', '        getByName("release") {']) {
    if (!gradle.includes(anchor)) {
      throw new Error(`build.gradle.kts: "${anchor.trim()}" nicht gefunden -- Tauri-Vorlage geaendert?`)
    }
  }

  let out = gradle.replace('android {', LOADER)
  out = out.replace('    buildTypes {', CONFIGS)
  out = out.replace('        getByName("release") {', ASSIGN)
  return { gradle: out, changed: true }
}

/**
 * Beurteilt `keystore.properties`, bevor gebaut wird.
 *
 * Ohne gueltige Angaben bleibt der Release-Bau unsigniert, und das faellt sonst
 * erst auf, wenn der Play Store das Bundle ablehnt.
 *
 * @param {string|null} propsText Inhalt der Datei, `null` wenn sie fehlt
 * @param {(path: string) => boolean} exists Pruefung auf die Schluesseldatei
 * @returns {string|null} Klartext-Beanstandung, `null` wenn alles passt
 */
export function keystoreIssue(propsText, exists) {
  if (propsText === null) return 'keystore.properties fehlt'

  const line = propsText.match(/^storeFile=(.*)$/m)
  const store = line ? line[1].trim() : ''
  if (!store) return 'storeFile fehlt oder ist leer'
  if (!exists(store)) return `Schluesseldatei nicht gefunden: ${store}`

  for (const key of ['storePassword', 'keyAlias', 'keyPassword']) {
    const match = propsText.match(new RegExp(`^${key}=(.*)$`, 'm'))
    if (!match || match[1].trim() === '') return `${key} fehlt oder ist leer`
  }
  return null
}

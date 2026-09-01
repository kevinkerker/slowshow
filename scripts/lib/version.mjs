// Rechnet und ersetzt Versionsnummern (E-44).
//
// ## Warum es dieses Modul gibt
//
// Die Nummer steht an fuenf Stellen, und keine davon ist entbehrlich:
//
//  - `src-tauri/Cargo.toml` speist `CARGO_PKG_VERSION` — die Anzeige in den
//    Einstellungen und im Diagnosebericht.
//  - `src-tauri/tauri.conf.json` speist `versionName` und `versionCode` im APK.
//  - `package.json` und die beiden Sperrdateien fuehren sie mit; bleibt eine
//    stehen, meldet `npm ci` eine Sperrdatei, die nicht zu ihrem Paket passt.
//
// Das Konfigurationsschema der Tauri-CLI sagt zwar „If removed the version
// number from `Cargo.toml` is used". Am Geraet gemessen stimmt das fuer Android
// **nicht**: ohne das Feld schreibt `tauri android build` die Datei
// `gen/android/app/tauri.properties` gar nicht erst, und `build.gradle.kts`
// faellt auf seine eigenen Vorgaben zurueck — `versionName "1.0"` und
// `versionCode 1`. Ein so gebautes Bundle waere im Play Store unbrauchbar.
// Deshalb bleibt das Feld stehen, und dieses Skript ist der einzige Schreiber.
//
// Eigenes Modul wie bei der Signatur (`android-signing.mjs`): `bump.mjs`
// schreibt Dateien und ruft `git`, ein Test koennte es nicht laden, ohne beides
// zu tun. Hier steht nur Textumformung — rein, ohne Dateisystem.
//
// ## Warum Textersatz und kein JSON-Rundlauf
//
// `JSON.parse` + `JSON.stringify` ueber `package-lock.json` schreibt die Datei
// vollstaendig um: npm setzt andere Zeilenenden, und aus einem Zweizeiler wuerde
// ein Diff ueber 4 000 Zeilen. Ersetzt wird deshalb gezielt an einem Anker, und
// jede Ersetzung prueft, wie oft sie zugetroffen hat.

/** Die Stufen, um die sich heben laesst. */
export const STUFEN = ['major', 'minor', 'patch']

/**
 * Obergrenze des Play Store fuer `versionCode`.
 * @see https://developer.android.com/studio/publish/versioning
 */
export const VERSION_CODE_MAX = 2_100_000_000

/** Zerlegt "x.y.z". Wirft bei allem anderen — eine krumme Version faellt sonst
 *  erst im Play Store auf, und dort ist die Nummer schon verbraucht. */
export function parseVersion(text) {
  const treffer = /^(\d+)\.(\d+)\.(\d+)$/.exec(String(text).trim())
  if (!treffer) throw new Error(`Keine dreistellige Version: "${text}"`)
  const [major, minor, patch] = treffer.slice(1).map(Number)
  return { major, minor, patch }
}

export function formatVersion({ major, minor, patch }) {
  return `${major}.${minor}.${patch}`
}

/**
 * Die naechste Version.
 *
 * @param {string} current heutige Version
 * @param {string} kind `major`, `minor`, `patch` oder eine ausgeschriebene
 *   Version wie `2.0.0`
 */
export function nextVersion(current, kind) {
  const v = parseVersion(current)

  if (STUFEN.includes(kind)) {
    // Die kleineren Stellen fallen auf null zurueck — 1.0.9 minor ist 1.1.0,
    // nicht 1.1.9.
    if (kind === 'major') return formatVersion({ major: v.major + 1, minor: 0, patch: 0 })
    if (kind === 'minor') return formatVersion({ major: v.major, minor: v.minor + 1, patch: 0 })
    return formatVersion({ ...v, patch: v.patch + 1 })
  }

  // Ausgeschriebene Version: erlaubt, aber nur nach oben. Der Play Store nimmt
  // keinen kleineren `versionCode` an; eine gesenkte Nummer waere also eine
  // Fassung, die sich nie hochladen laesst.
  const ziel = parseVersion(kind)
  if (androidVersionCode(ziel) <= androidVersionCode(v)) {
    throw new Error(`${formatVersion(ziel)} liegt nicht ueber ${formatVersion(v)}`)
  }
  return formatVersion(ziel)
}

/**
 * Der `versionCode`, den Tauri aus der Version rechnet.
 *
 * Formel aus dem Konfigurationsschema der Tauri-CLI:
 * `major * 1000000 + minor * 1000 + patch`.
 */
export function androidVersionCode(version) {
  const v = typeof version === 'string' ? parseVersion(version) : version
  return v.major * 1_000_000 + v.minor * 1_000 + v.patch
}

/**
 * Was an dieser Version dem Play Store nicht passt — oder `null`.
 *
 * Der Ueberlauf ist die eigentliche Falle: 1.0.1000 ergaebe denselben
 * `versionCode` wie 1.1.0. Ein Upload wuerde abgewiesen, und die Ursache
 * stuende in keiner Fehlermeldung.
 */
export function versionCodeIssue(version) {
  const v = typeof version === 'string' ? parseVersion(version) : version
  if (v.minor > 999) return `minor ${v.minor} ueberlaeuft in die major-Stelle des versionCode`
  if (v.patch > 999) return `patch ${v.patch} ueberlaeuft in die minor-Stelle des versionCode`
  const code = androidVersionCode(v)
  if (code > VERSION_CODE_MAX) {
    return `versionCode ${code} ueberschreitet die Grenze des Play Store (${VERSION_CODE_MAX})`
  }
  return null
}

/**
 * Ersetzt und prueft dabei, wie oft der Anker zugetroffen hat.
 *
 * Gezaehlt wird ueber eine eigens globale Fassung des Musters: `match` ohne `g`
 * liefert den Treffer **samt Fanggruppen** zurueck, und `.length` waere damit
 * die Zahl der Gruppen statt der Fundstellen.
 */
function ersetze(text, muster, ersatz, erwartet, was) {
  const flags = muster.flags.includes('g') ? muster.flags : `${muster.flags}g`
  const anzahl = [...text.matchAll(new RegExp(muster.source, flags))].length
  if (anzahl !== erwartet) {
    throw new Error(`${was}: ${anzahl} Fundstelle(n) statt ${erwartet} — Aufbau geaendert?`)
  }
  return text.replace(muster, ersatz)
}

/**
 * Die Version im `[package]`-Abschnitt von Cargo.toml — die massgebliche Zeile.
 *
 * Der Anker ist der Paketname darueber: `version = "…"` steht in einer
 * Cargo.toml auch bei jeder Abhaengigkeit.
 */
export function withCargoVersion(toml, alt, neu) {
  return ersetze(
    toml,
    new RegExp(`(name = "slowshow"\\s*\\r?\\n(?:[^\\n]*\\r?\\n)*?version = )"${alt}"`),
    `$1"${neu}"`,
    1,
    'Cargo.toml',
  )
}

/**
 * Die Version eines JSON auf der aeusseren Ebene (zwei Leerzeichen Einzug).
 *
 * Der Einzug ist der Anker: eine Abhaengigkeit, die zufaellig auf dieselbe
 * Nummer festgelegt ist, steht tiefer und darf nicht mitgehoben werden.
 */
function withTopLevelJsonVersion(json, alt, neu, was) {
  return ersetze(json, new RegExp(`(^  "version":\\s*)"${alt}"`, 'm'), `$1"${neu}"`, 1, was)
}

/** Die Version in package.json. */
export function withPackageVersion(json, alt, neu) {
  return withTopLevelJsonVersion(json, alt, neu, 'package.json')
}

/**
 * Die Version in tauri.conf.json — sie wird zu `versionName` und `versionCode`.
 *
 * Ohne dieses Feld schreibt `tauri android build` keine `tauri.properties`, und
 * das APK bekommt aus `build.gradle.kts` die Vorgaben `1.0` und `1`. Am Geraet
 * nachgemessen, entgegen der Beschreibung im Konfigurationsschema.
 */
export function withTauriConfVersion(json, alt, neu) {
  return withTopLevelJsonVersion(json, alt, neu, 'tauri.conf.json')
}

/**
 * Beide Stellen in package-lock.json.
 *
 * npm fuehrt die Version des eigenen Pakets doppelt: einmal an der Wurzel und
 * einmal unter `packages[""]`. Bleibt eine stehen, meldet `npm ci` eine
 * Sperrdatei, die nicht zu ihrem Paket passt.
 */
export function withPackageLockVersion(lock, alt, neu) {
  return ersetze(
    lock,
    new RegExp(`("name":\\s*"slowshow",\\s*\\r?\\n\\s*"version":\\s*)"${alt}"`, 'g'),
    `$1"${neu}"`,
    2,
    'package-lock.json',
  )
}

/** Der Eintrag des eigenen Pakets in Cargo.lock. */
export function withCargoLockVersion(lock, alt, neu) {
  return ersetze(
    lock,
    new RegExp(`(name = "slowshow"\\s*\\r?\\nversion = )"${alt}"`),
    `$1"${neu}"`,
    1,
    'Cargo.lock',
  )
}

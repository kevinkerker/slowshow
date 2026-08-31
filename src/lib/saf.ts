/**
 * Ordnerauswahl über das Storage Access Framework (FA-20).
 *
 * Hier steht nur noch, was zwingend im Frontend passieren muss: den
 * Systemdialog öffnen und die Freigabe sichern. Das eigentliche Einlesen läuft
 * im Rust-Backend (`sources::local`) — dort hat das Plugin ebenfalls eine API,
 * und die Bilddaten müssen so gar nicht erst durch die WebView (R-03, NF-13).
 */

import type { AndroidFs as AndroidFsType, AndroidFsUri } from 'tauri-plugin-android-fs-api'

/** Lazy laden: das Plugin existiert nur in der Android-App. */
async function saf(): Promise<typeof AndroidFsType | null> {
  try {
    const m = await import(/* @vite-ignore */ 'tauri-plugin-android-fs-api')
    return m.AndroidFs
  } catch {
    return null
  }
}

export async function isAvailable(): Promise<boolean> {
  return (await saf()) !== null
}

export function uriToString(uri: AndroidFsUri): string {
  return JSON.stringify(uri)
}

export function uriFromString(s: string): AndroidFsUri | null {
  if (!s) return null
  try {
    return JSON.parse(s) as AndroidFsUri
  } catch {
    return null
  }
}

/**
 * Öffnet den Android-Ordnerdialog und sichert die Freigabe dauerhaft (FA-20:
 * „die App merkt sich die Freigaben dauerhaft").
 */
export async function pickFolder(): Promise<{ uri: string; name: string } | null> {
  const fs = await saf()
  if (!fs) return null

  const uri = await fs.showOpenDirPicker({ localOnly: false })
  if (!uri) return null

  await fs.persistPickerUriPermission(uri)
  const name = await fs.getName(uri).catch(() => 'Ordner')
  return { uri: uriToString(uri), name }
}

/** Besteht die Freigabe noch? Nach einem Android-Update kann sie wegfallen. */
export async function checkAccess(storedUri: string): Promise<boolean> {
  const fs = await saf()
  const uri = uriFromString(storedUri)
  if (!fs || !uri) return false
  try {
    return await fs.checkPersistedPickerUriPermission(uri, 'Read')
  } catch {
    return false
  }
}

/**
 * Vorschlag fuer den Dateinamen einer Sicherung.
 *
 * Datum im Namen, damit mehrere Staende nebeneinander liegen koennen und die
 * Dateiliste sich von selbst sortiert. ISO-Reihenfolge, nicht die deutsche:
 * `2026-08-31` sortiert richtig, `31.08.2026` nicht.
 */
export function backupFileName(now: Date = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, '0')
  const tag = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`
  return `slowshow-sicherung-${tag}.json`
}

/**
 * Schreibt Text in eine vom Nutzer gewaehlte Datei (FA-45).
 *
 * Ersetzt den Weg ueber die Zwischenablage. Der hat nur halb funktioniert:
 * `navigator.clipboard.writeText` erlaubt Androids WebView, `readText` nicht —
 * die Permissions-API kann `clipboard-read` dort nicht gewaehren, und es gibt
 * keinen Dialog, den man bestaetigen koennte. Eine Sicherung, die sich
 * schreiben, aber nie zurueckspielen laesst, ist keine.
 *
 * @returns Name der geschriebenen Datei, `null` wenn abgebrochen wurde
 */
export async function saveTextFile(name: string, content: string): Promise<string | null> {
  const fs = await saf()
  if (!fs) return null

  const uri = await fs.showSaveFilePicker(name, 'application/json')
  if (!uri) return null

  await fs.writeTextFile(uri, content)
  return await fs.getName(uri).catch(() => name)
}

/**
 * Liest Text aus einer vom Nutzer gewaehlten Datei (FA-45).
 *
 * `mimeTypes` ist bewusst weit gefasst und schliesst `text/*` ein. Ein enger
 * Filter auf `application/json` waere riskant: nicht jeder Dateianbieter meldet
 * fuer `.json` diesen Typ, und der Dialog blendet dann genau die Datei aus, die
 * man sucht. Auf dem Testgeraet (MIUI) zeigt der Dialog ohnehin alle Dateien —
 * der Filter ist dort nur ein Vorschlag, kein Ausschluss.
 *
 * @returns Inhalt und Dateiname, `null` wenn abgebrochen wurde
 */
export async function openTextFile(): Promise<{ content: string; name: string } | null> {
  const fs = await saf()
  if (!fs) return null

  const uris = await fs.showOpenFilePicker({ mimeTypes: ['application/json', 'text/*'] })
  const uri = uris?.[0]
  if (!uri) return null

  const content = await fs.readTextFile(uri)
  const name = await fs.getName(uri).catch(() => 'Sicherung')
  return { content, name }
}

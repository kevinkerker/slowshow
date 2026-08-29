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

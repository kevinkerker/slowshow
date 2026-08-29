/**
 * Einordnung einer Datei anhand ihrer Endung.
 *
 * Spiegelt `decode::classify` aus dem Backend. Doppelt vorhanden, weil der
 * SAF-Durchlauf (FA-20) im Frontend läuft und dort schon vor dem Lesen
 * entscheiden muss — sonst würde jede Videodatei erst über die Brücke
 * geschoben, um dann verworfen zu werden.
 *
 * Die Listen müssen mit `src-tauri/src/decode.rs` übereinstimmen; der Test
 * `file-class.test.ts` hält sie fest.
 */

/** Unterstützte Bildformate (FA-04). */
export const IMAGE_EXTENSIONS = ['jpg', 'jpeg', 'png', 'webp'] as const

/**
 * Bewusst ausgeschlossene Formate.
 * HEIC/HEIF und AVIF per E-04, Videodateien per E-07.
 */
export const SKIP_EXTENSIONS = [
  'heic',
  'heif',
  'avif',
  'mp4',
  'mov',
  'avi',
  'mkv',
  'webm',
  'm4v',
  '3gp',
  'gif',
] as const

export type FileClass = 'image' | 'skipped' | 'irrelevant'

export function classify(fileName: string): FileClass {
  const dot = fileName.lastIndexOf('.')
  if (dot <= 0) return 'irrelevant'

  const ext = fileName.slice(dot + 1).toLowerCase()
  if ((IMAGE_EXTENSIONS as readonly string[]).includes(ext)) return 'image'
  if ((SKIP_EXTENSIONS as readonly string[]).includes(ext)) return 'skipped'
  return 'irrelevant'
}

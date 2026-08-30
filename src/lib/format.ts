/**
 * Formatierungshilfen für Uhr, Bildunterschrift und Einstellungen.
 *
 * Bewusst als reine Funktionen ohne Vue-Bezug — so lassen sie sich einzeln
 * testen, und die Komponenten bleiben frei von Formatierungslogik.
 */

/** Uhrzeit "21:47" (FA-07). */
export function formatClock(date: Date): string {
  const h = String(date.getHours()).padStart(2, '0')
  const m = String(date.getMinutes()).padStart(2, '0')
  return `${h}:${m}`
}

/** Zeigerstellung der Analoguhr in Grad, im Uhrzeigersinn ab zwölf (E-20). */
export interface ClockAngles {
  hour: number
  minute: number
}

/**
 * Zeigerwinkel aus einer Uhrzeit (E-20).
 *
 * Der Stundenzeiger wandert mit der Minute mit (30° je Stunde plus 0,5° je
 * Minute) statt stündlich zu springen. Ohne das stünde er um 11:59 noch exakt
 * auf der Elf — bei einer Uhr ohne Ziffern ist das nicht ablesbar, sondern
 * schlicht falsch.
 *
 * Einen Sekundenzeiger gibt es bewusst nicht: `useNow` taktet nur auf die
 * volle Minute, damit die App im Dauerbetrieb nicht sekündlich aufwacht
 * (NF-06).
 */
export function clockAngles(date: Date): ClockAngles {
  const minutes = date.getMinutes()
  return {
    hour: (date.getHours() % 12) * 30 + minutes * 0.5,
    minute: minutes * 6,
  }
}

/**
 * Datumszeile im Stil des Entwurfs: "Samstag · 29. August".
 *
 * `locale` kommt aus der Spracheinstellung (NF-09), damit die Zeile bei
 * Englisch nicht deutsch bleibt.
 */
export function formatDateLine(date: Date, locale: string): string {
  const weekday = date.toLocaleDateString(locale, { weekday: 'long' })
  const day = date.toLocaleDateString(locale, { day: 'numeric', month: 'long' })
  return `${weekday} · ${day}`
}

/** Aufnahmedatum als "Juni 2025" für die Bildunterschrift (FA-07). */
export function formatTakenAt(unixSeconds: number | null, locale: string): string {
  if (unixSeconds == null) return ''
  const d = new Date(unixSeconds * 1000)
  if (Number.isNaN(d.getTime())) return ''
  return d.toLocaleDateString(locale, { month: 'long', year: 'numeric' })
}

/** Zeitpunkt des letzten Syncs als "vor 12 Min" (Entwurf: Quellenliste). */
export function formatRelativeTime(
  unixSeconds: number | null,
  now: Date,
  t: (key: string, params?: Record<string, unknown>) => string,
): string {
  if (unixSeconds == null) return t('sources.neverSynced')

  const seconds = Math.floor(now.getTime() / 1000) - unixSeconds
  if (seconds < 60) return t('time.justNow')

  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return t('time.minutesAgo', { n: minutes })

  const hours = Math.floor(minutes / 60)
  if (hours < 24) return t('time.hoursAgo', { n: hours })

  return t('time.daysAgo', { n: Math.floor(hours / 24) })
}

/** Dateigröße mit einer Nachkommastelle: "1,4 GB". */
export function formatBytes(bytes: number, locale = 'de-DE'): string {
  if (bytes < 1024) return `${bytes} B`

  const units = ['KB', 'MB', 'GB', 'TB']
  let value = bytes / 1024
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit++
  }
  const digits = value >= 100 ? 0 : 1
  return `${value.toLocaleString(locale, {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  })} ${units[unit]}`
}

/**
 * Anzeigeintervall lesbar machen (FA-02).
 * Unter einer Minute in Sekunden, darüber in Minuten.
 */
export function formatInterval(
  seconds: number,
  t: (key: string, params?: Record<string, unknown>) => string,
): string {
  if (seconds < 60) return t('time.seconds', { n: seconds })
  const minutes = Math.round(seconds / 60)
  return t('time.minutes', { n: minutes })
}

/** Bildmaße als "1920 × 1080". */
export function formatDimensions(width: number, height: number): string {
  return `${width} × ${height}`
}

/** Dateiname ohne Endung — im Entwurf steht die Endung nicht in der Unterschrift. */
export function stripExtension(fileName: string): string {
  const dot = fileName.lastIndexOf('.')
  return dot > 0 ? fileName.slice(0, dot) : fileName
}

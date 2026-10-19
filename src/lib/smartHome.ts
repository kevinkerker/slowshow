/**
 * Smart-Home-Ansicht: Hilfen für die Einstellungen (FA-46, E-74, E-76).
 *
 * Die Regeln selbst — welche Adresse gilt, welche Spanne der Timeout hat,
 * wann der Tipp öffnet — stehen in `src-tauri/src/smart_home.rs`. Hier nur,
 * was die Oberfläche zum Anzeigen braucht.
 */
import type { AppConfig, ButtonCorner } from './types'

/**
 * Auswahl für „Zurück zur Diashow nach“ (E-76, vorläufig).
 *
 * Eine Liste statt eines freien Felds: wer einen Rahmen einrichtet, will keine
 * Sekunden tippen. Erster und letzter Wert sind die Grenzen aus
 * `smart_home::TIMEOUT_MIN_SECS` und `TIMEOUT_MAX_SECS`; die Voreinstellung
 * von zwei Minuten steht mittendrin.
 */
export const TIMEOUT_CHOICES = [30, 60, 120, 300, 600, 1800] as const

/** Voreinstellung wie `smart_home::TIMEOUT_DEFAULT_SECS`. */
export const TIMEOUT_DEFAULT = 120

/** Reihenfolge der Ecken in der Auswahl — wie man sie auf dem Rahmen liest. */
export const CORNERS: ButtonCorner[] = ['topLeft', 'topRight', 'bottomLeft', 'bottomRight']

/**
 * Die Auswahl samt dem gespeicherten Wert.
 *
 * Über REST oder eine von Hand bearbeitete Datei kann ein Wert ankommen, der
 * nicht in der Liste steht. Ohne ihn zeigte die Auswahl ein leeres Feld, und
 * ein versehentlicher Tipp setzte einen anderen Wert.
 */
export function timeoutChoices(current: number): number[] {
  const all: number[] = [...TIMEOUT_CHOICES]
  if (Number.isFinite(current) && current > 0 && !all.includes(current)) all.push(current)
  return all.sort((a, b) => a - b)
}

/** Beschriftung einer Dauer: ganze Minuten als Minuten, sonst Sekunden. */
export function timeoutLabel(seconds: number): {
  key: 'system.smartHomeSeconds' | 'system.smartHomeMinutes'
  n: number
} {
  if (seconds >= 60 && seconds % 60 === 0) return { key: 'system.smartHomeMinutes', n: seconds / 60 }
  return { key: 'system.smartHomeSeconds', n: seconds }
}

/**
 * Hat das Backend die eingegebene Adresse verworfen?
 *
 * `AppConfig::clamp` leert alles, was nicht mit http oder https beginnt.
 * Ohne Rückmeldung sähe das aus, als hätte das Feld die Eingabe vergessen.
 */
export function urlWasRejected(entered: string, stored: string): boolean {
  return entered.trim() !== '' && stored === ''
}

/** Ist die Ansicht eingerichtet? Ohne Adresse bleibt alles wie bisher (E-73). */
export function isSmartHomeEnabled(config: AppConfig | null | undefined): boolean {
  return (config?.smartHome?.url ?? '') !== ''
}

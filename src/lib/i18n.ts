/**
 * Mehrsprachigkeit (NF-09).
 *
 * Oberflächensprache ist Deutsch; die Architektur lässt weitere Sprachen zu.
 * Englisch liegt als zweite Sprache bei, damit die Struktur nicht nur
 * theoretisch mehrsprachig ist — und weil der Play-Store-Eintrag international
 * ist (RB-03).
 */

import { createI18n } from 'vue-i18n'
import de from '../locales/de.json'
import en from '../locales/en.json'

export type Language = 'auto' | 'de' | 'en'

/** Löst 'auto' anhand der Systemsprache auf. */
export function resolveLanguage(lang: Language): 'de' | 'en' {
  if (lang !== 'auto') return lang
  const system = (navigator.language || 'de').split('-')[0]
  return system === 'de' ? 'de' : 'en'
}

/** BCP-47-Kennung für `toLocaleDateString` in `format.ts`. */
export function localeTag(lang: Language): string {
  return resolveLanguage(lang) === 'de' ? 'de-DE' : 'en-GB'
}

export const i18n = createI18n({
  legacy: false,
  locale: resolveLanguage('auto'),
  // Deutsch ist die Referenzsprache: fehlt ein englischer Schlüssel, ist die
  // deutsche Beschriftung immer noch besser als der rohe Schlüsselname.
  fallbackLocale: 'de',
  messages: { de, en },
})

export function applyLanguage(lang: Language): void {
  i18n.global.locale.value = resolveLanguage(lang)
}

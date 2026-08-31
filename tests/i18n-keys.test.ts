import { describe, expect, it } from 'vitest'
import de from '../src/locales/de.json'
import en from '../src/locales/en.json'

/**
 * Wächter gegen Beschriftungen, die es nicht gibt.
 *
 * Anlass: im Postfach-Formular stand am Gerät wörtlich `sourceForm.user` —
 * vue-i18n gibt bei einem unbekannten Schlüssel den Schlüssel selbst zurück,
 * ohne Fehler und ohne Warnung im Produktionsbau. Weder `vue-tsc` noch die
 * übrigen Tests konnten das sehen, weil der Schlüssel nur eine Zeichenkette
 * ist. Aufgefallen ist es erst beim Durchscrollen des Formulars auf dem
 * Telefon; genau das soll hier nicht mehr nötig sein.
 *
 * Grenze: zusammengesetzte Schlüssel (`t(\`clock.${style}\`)`) erfasst der
 * Test nicht — dafür müsste er den möglichen Werten von `style` folgen. Diese
 * Fälle stehen bewusst als Aufzählung im Code und fallen beim Umbenennen einer
 * Aufzählung durch den Typprüfer auf.
 */

/** Alle Blattpfade eines Sprachbaums, z. B. `sourceForm.mailHost`. */
function leafKeys(node: unknown, prefix = ''): string[] {
  if (typeof node !== 'object' || node === null) return [prefix]
  return Object.entries(node as Record<string, unknown>).flatMap(([k, v]) =>
    leafKeys(v, prefix ? `${prefix}.${k}` : k),
  )
}

/**
 * Quelltexte als Zeichenketten.
 *
 * Ueber Vites Glob statt `node:fs`: das Projekt fuehrt kein `@types/node`,
 * und eine Abhaengigkeit nur fuer das Einlesen von Dateien in einem Test
 * waere ein schlechter Tausch.
 */
const FILES: Record<string, string> = {
  ...import.meta.glob('../src/**/*.vue', { query: '?raw', import: 'default', eager: true }),
  ...import.meta.glob('../src/**/*.ts', { query: '?raw', import: 'default', eager: true }),
}

/**
 * `t('…')` und `$t('…')` mit festem Schlüssel.
 *
 * Die Rückschau auf Wort- und Punktzeichen hält `emit('cancel')` heraus — das
 * endet ebenfalls auf `t(` und lieferte sonst `cancel` als angeblichen
 * Übersetzungsschlüssel.
 */
const CALL = /(?<![\w.$])\$?t\(\s*'([^']+)'/g

function usedKeys(): Map<string, string> {
  const found = new Map<string, string>()
  for (const [file, text] of Object.entries(FILES)) {
    if (file.endsWith('.test.ts')) continue
    for (const [, key] of text.matchAll(CALL)) {
      if (!found.has(key)) found.set(key, file)
    }
  }
  return found
}

describe('Übersetzungsschlüssel', () => {
  it('sind für jeden festen Aufruf im Code hinterlegt', () => {
    const known = new Set(leafKeys(de))
    const missing = [...usedKeys()]
      .filter(([key]) => !known.has(key))
      .map(([key, file]) => `${key} (${file})`)

    expect(missing, 'Beschriftungen ohne Eintrag in de.json').toEqual([])
  })

  it('greift überhaupt — der Test findet die Aufrufe', () => {
    // Gegenprobe: fände das Muster nichts, wäre der Test oben immer gruen und
    // damit wertlos.
    const used = usedKeys()
    expect(used.size).toBeGreaterThan(100)
    expect(used.has('sourceForm.mailHost')).toBe(true)
  })

  it('stehen in beiden Sprachen', () => {
    // Deutsch ist die Referenz, Englisch faellt darauf zurueck (i18n.ts). Ein
    // fehlender englischer Schluessel ist deshalb kein Absturz, aber eine
    // deutsche Beschriftung mitten in einer englischen Oberflaeche — und das
    // faellt bei einem Geraet an der Wand niemandem auf.
    const deKeys = leafKeys(de).sort()
    const enKeys = leafKeys(en).sort()
    expect(enKeys.filter((k) => !deKeys.includes(k)), 'nur in en.json').toEqual([])
    expect(deKeys.filter((k) => !enKeys.includes(k)), 'nur in de.json').toEqual([])
  })
})

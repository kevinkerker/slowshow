import { describe, expect, it } from 'vitest'
import {
  allSelected,
  dirOn,
  hasOwnDecision,
  selectionOf,
  setDir,
  summaryOf,
  within,
  type FolderSelection,
} from './subfolders'

/**
 * Dieselben Fälle wie `src-tauri/src/sources/filter.rs`. Weicht eine Seite
 * ab, zeigt der Dialog etwas anderes an, als der Abgleich holt.
 */
function sel(include: string[], exclude: string[], includeRoot: boolean, includeNew: boolean): FolderSelection {
  return { include, exclude, includeRoot, includeNew }
}

describe('Regel (wie filter.rs)', () => {
  it('ohne Entscheidung zählt alles', () => {
    const s = allSelected()
    expect(dirOn(s, '')).toBe(true)
    expect(dirOn(s, 'Urlaub/2025')).toBe(true)
  })

  it('WhatsApp ohne Sent und Private (E-70)', () => {
    const s = sel([], ['Sent', 'Private'], true, true)
    expect(dirOn(s, '')).toBe(true)
    expect(dirOn(s, 'Sent')).toBe(false)
    expect(dirOn(s, 'Status')).toBe(true)
  })

  it('nächstgelegene Entscheidung gewinnt', () => {
    const s = sel(['Familie/2025'], ['Familie'], true, true)
    expect(dirOn(s, 'Familie')).toBe(false)
    expect(dirOn(s, 'Familie/2024')).toBe(false)
    expect(dirOn(s, 'Familie/2025')).toBe(true)
    expect(dirOn(s, 'Familie/2025/Sommer')).toBe(true)
  })

  it('ohne Automatik zählt nur das Angeschaltete', () => {
    const s = sel(['Urlaub'], [], false, false)
    expect(dirOn(s, 'Urlaub')).toBe(true)
    expect(dirOn(s, 'Neu')).toBe(false)
    expect(dirOn(s, '')).toBe(false)
  })

  it('die Wurzel ist kein Ahne', () => {
    const s = sel([], [], false, true)
    expect(dirOn(s, '')).toBe(false)
    expect(dirOn(s, 'Urlaub')).toBe(true)
  })

  it('vergleicht mit Trennzeichen', () => {
    expect(within('Urlaubsfotos', 'Urlaub')).toBe(false)
    expect(within('Urlaub/2025', 'Urlaub')).toBe(true)
    expect(dirOn(sel(['Urlaub'], [], false, false), 'Urlaubsfotos')).toBe(false)
  })
})

describe('selectionOf', () => {
  it('alte Positivliste behält ihre Bedeutung (FA-29)', () => {
    const s = selectionOf({
      subfolders: ['Urlaub', '/Familie/2025/'],
      excludedSubfolders: [],
      includeRoot: null,
      includeNewSubfolders: null,
    })
    expect(s).toEqual(sel(['Urlaub', 'Familie/2025'], [], false, false))
  })

  it('ohne alten Filter ist alles an', () => {
    const s = selectionOf({ subfolders: [], excludedSubfolders: [], includeRoot: null, includeNewSubfolders: null })
    expect(s).toEqual(allSelected())
  })

  it('übernimmt gespeicherte Werte', () => {
    const s = selectionOf({ subfolders: [], excludedSubfolders: ['Sent'], includeRoot: true, includeNewSubfolders: false })
    expect(s).toEqual(sel([], ['Sent'], true, false))
  })
})

describe('setDir', () => {
  it('schaltet einen Ordner ab und merkt es sich', () => {
    const s = setDir(allSelected(), 'Sent', false)
    expect(s.exclude).toEqual(['Sent'])
    expect(dirOn(s, 'Sent')).toBe(false)
  })

  it('räumt Entscheidungen darunter weg', () => {
    // „Familie" abschalten meint die ganze Familie.
    const s = setDir(sel(['Familie/2025'], ['Familie/2024'], true, true), 'Familie', false)
    expect(s).toEqual(sel([], ['Familie'], true, true))
  })

  it('lässt keine Entscheidung stehen, die dem Geerbten entspricht', () => {
    const aus = setDir(allSelected(), 'Sent', false)
    const wieder = setDir(aus, 'Sent', true)
    expect(wieder).toEqual(allSelected())
    expect(hasOwnDecision(wieder, 'Sent')).toBe(false)
  })

  it('schaltet einen Unterordner in einem abgeschalteten an', () => {
    const s = setDir(sel([], ['Familie'], true, true), 'Familie/2025', true)
    expect(s).toEqual(sel(['Familie/2025'], ['Familie'], true, true))
    expect(dirOn(s, 'Familie/2025')).toBe(true)
    expect(dirOn(s, 'Familie/2024')).toBe(false)
  })

  it('schaltet die Bilder direkt im Ordner getrennt', () => {
    const s = setDir(allSelected(), '', false)
    expect(s.includeRoot).toBe(false)
    expect(dirOn(s, 'Urlaub')).toBe(true)
  })

  it('verändert die übergebene Auswahl nicht', () => {
    const vorher = sel([], [], true, true)
    setDir(vorher, 'Sent', false)
    expect(vorher.exclude).toEqual([])
  })
})

describe('summaryOf (Karte, E-70)', () => {
  it('sagt nichts, wenn nichts gefiltert ist', () => {
    expect(summaryOf(allSelected())).toBeNull()
  })

  it('zählt mit Automatik, was aus ist', () => {
    expect(summaryOf(sel([], ['Sent', 'Private'], true, true))).toEqual({ kind: 'excluded', n: 2 })
    expect(summaryOf(sel([], [], false, true))).toEqual({ kind: 'excluded', n: 1 })
  })

  it('zählt ohne Automatik, was an ist', () => {
    expect(summaryOf(sel(['Urlaub', 'Familie/2025'], [], false, false))).toEqual({ kind: 'selected', n: 2 })
    expect(summaryOf(sel([], [], true, false))).toEqual({ kind: 'selected', n: 1 })
  })
})

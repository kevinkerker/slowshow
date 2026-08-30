import { describe, expect, it } from 'vitest'
import { isCopyleft } from '../../scripts/third-party-licenses.mjs'

/**
 * Die einzige Stelle im Lizenz-Skript, an der etwas beurteilt wird.
 *
 * Ein falsches Ja waere harmlos -- ein Paket zu viel in der Auflagenliste.
 * Ein falsches Nein waere es nicht: dann behauptet die Uebersicht aus RB-05,
 * es gaebe keine Copyleft-Auflagen, obwohl es welche gibt.
 */
describe('isCopyleft', () => {
  it('erkennt reines Copyleft', () => {
    expect(isCopyleft('MPL-2.0')).toBe(true)
    expect(isCopyleft('GPL-3.0-only')).toBe(true)
    expect(isCopyleft('AGPL-3.0')).toBe(true)
  })

  it('laesst permissive Lizenzen durch', () => {
    expect(isCopyleft('MIT')).toBe(false)
    expect(isCopyleft('Apache-2.0')).toBe(false)
    expect(isCopyleft('MIT OR Apache-2.0')).toBe(false)
    expect(isCopyleft('Unicode-3.0')).toBe(false)
  })

  it('zaehlt eine permissive Alternative als Ausweg', () => {
    // r-efi aus dem Android-Baum: LGPL steht drin, ist aber waehlbar neben MIT.
    // Ohne diese Unterscheidung stuende die Kiste faelschlich in der Auflagenliste.
    expect(isCopyleft('MIT OR Apache-2.0 OR LGPL-2.1-or-later')).toBe(false)
  })

  it('meldet Copyleft, wenn jede Alternative copyleft ist', () => {
    expect(isCopyleft('MPL-2.0 OR GPL-2.0-only')).toBe(true)
  })

  it('ist gegen Gross- und Kleinschreibung unempfindlich', () => {
    // Die SPDX-Angaben kommen aus fremden package.json und Cargo.toml.
    expect(isCopyleft('mpl-2.0')).toBe(true)
    expect(isCopyleft('mit or apache-2.0')).toBe(false)
  })
})

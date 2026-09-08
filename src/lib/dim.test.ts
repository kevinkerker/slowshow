import { describe, expect, it } from 'vitest'
import { dimOpacity } from './dim'
import type { DisplayState } from './types'

function state(over: Partial<DisplayState> = {}): DisplayState {
  return { slideshowActive: true, showNightClock: false, brightness: 100, ...over }
}

describe('dimOpacity', () => {
  it('dunkelt bei voller Helligkeit nicht ab', () => {
    expect(dimOpacity(state())).toBe(0)
  })

  it('dimmt tags nie, die Helligkeit ist Sache des Backlights (E-51)', () => {
    // Vorher dimmten Overlay und Backlight zugleich: 50 am Regler liessen ein
    // Viertel des Lichts durch. Wer aus Home Assistant die Helligkeit stellt,
    // stellt die Beleuchtung — nicht ein Bild, das ein Overlay abdunkelt.
    expect(dimOpacity(state({ brightness: 40 }))).toBe(0)
    expect(dimOpacity(state({ brightness: 25 }))).toBe(0)
    expect(dimOpacity(state({ brightness: 1 }))).toBe(0)
  })

  it('dunkelt vor dem ersten Laden nicht ab', () => {
    // Sonst waere der Start von einem Fehler nicht zu unterscheiden.
    expect(dimOpacity(null)).toBe(0)
  })

  it('laesst die Nachtuhr frei (FA-54)', () => {
    // Ein Overlay darueber machte sie unsichtbar — der Nachtmodus waere dann
    // nicht von einem schwarzen Bildschirm zu unterscheiden.
    expect(dimOpacity(state({ slideshowActive: false, showNightClock: true, brightness: 1 }))).toBe(0)
  })

  it('schwaerzt nachts ohne Nachtuhr vollstaendig (FA-52)', () => {
    expect(dimOpacity(state({ slideshowActive: false, brightness: 1 }))).toBe(1)
  })

  it('schwaerzt nachts auch bei geraetegesteuerter Helligkeit (E-22)', () => {
    // Die App senkt die Beleuchtung dann nicht mehr, also muss das Overlay
    // den Schirm schwaerzen. Sonst stuende die ganze Nacht das letzte Foto
    // auf dem Rahmen.
    expect(dimOpacity(state({ slideshowActive: false, brightness: 0 }))).toBe(1)
  })

  it('haelt sich tagsueber aus geraetegesteuerter Helligkeit heraus (E-22)', () => {
    expect(dimOpacity(state({ brightness: 0 }))).toBe(0)
  })
})

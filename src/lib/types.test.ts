import { describe, expect, it } from 'vitest'
import { DEVICE_CONTROLLED_BRIGHTNESS, EVENTS, slideIds, type Slide } from './types'

describe('slideIds', () => {
  it('liefert eine Id für ein Einzelbild', () => {
    const slide: Slide = { kind: 'single', id: 'abc' }
    expect(slideIds(slide)).toEqual(['abc'])
  })

  it('liefert beide Ids im Paar-Modus (FA-08)', () => {
    const slide: Slide = { kind: 'pair', left: 'l', right: 'r' }
    expect(slideIds(slide)).toEqual(['l', 'r'])
  })

  it('liefert für nichts eine leere Liste', () => {
    // Der leere Zustand tritt vor dem ersten Sync auf und darf nicht werfen.
    expect(slideIds(null)).toEqual([])
  })
})

describe('DEVICE_CONTROLLED_BRIGHTNESS', () => {
  // Steht doppelt: hier und als `schedule::DEVICE_CONTROLLED` in Rust. Laufen
  // die Werte auseinander, legt das Frontend bei gerätegesteuerter Helligkeit
  // ein fast vollständig schwarzes Overlay über das Bild (E-22).
  it('entspricht dem Sentinel aus schedule.rs', () => {
    expect(DEVICE_CONTROLLED_BRIGHTNESS).toBe(0)
  })

  it('liegt ausserhalb des gueltigen Helligkeitsbereichs', () => {
    // Nur deshalb kann der Wert nichts anderes bedeuten als "App regelt nicht".
    expect(DEVICE_CONTROLLED_BRIGHTNESS).toBeLessThan(1)
  })
})

describe('EVENTS', () => {
  // Die Namen stehen doppelt: hier und in src-tauri/src/state.rs (`mod events`).
  // Läuft eines auseinander, bekommt das Frontend stumm keine Aktualisierungen
  // mehr — ohne Fehlermeldung. Deshalb hier festgehalten.
  it('entspricht den Namen aus state::events', () => {
    expect(EVENTS.slide).toBe('slowshow://slide')
    expect(EVENTS.sync).toBe('slowshow://sync')
    expect(EVENTS.display).toBe('slowshow://display')
    expect(EVENTS.config).toBe('slowshow://config')
  })
})

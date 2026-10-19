import { describe, expect, it } from 'vitest'
import {
  CORNERS,
  TIMEOUT_CHOICES,
  TIMEOUT_DEFAULT,
  isSmartHomeEnabled,
  timeoutChoices,
  timeoutLabel,
  urlWasRejected,
} from './smartHome'
import type { AppConfig } from './types'

describe('Timeout-Auswahl (E-76)', () => {
  it('reicht von 30 Sekunden bis 30 Minuten wie smart_home.rs', () => {
    // Die Grenzen stehen doppelt: hier und als TIMEOUT_MIN_SECS/MAX_SECS.
    // Ein Wert ausserhalb wuerde vom Backend still geklemmt, und die Auswahl
    // zeigte danach etwas anderes als gewaehlt.
    expect(TIMEOUT_CHOICES[0]).toBe(30)
    expect(TIMEOUT_CHOICES[TIMEOUT_CHOICES.length - 1]).toBe(1800)
  })

  it('enthaelt die Voreinstellung', () => {
    expect(TIMEOUT_DEFAULT).toBe(120)
    expect(TIMEOUT_CHOICES).toContain(TIMEOUT_DEFAULT)
  })

  it('nimmt einen gespeicherten Wert ausserhalb der Liste auf', () => {
    expect(timeoutChoices(90)).toEqual([30, 60, 90, 120, 300, 600, 1800])
  })

  it('verdoppelt keinen Wert der Liste', () => {
    expect(timeoutChoices(120)).toEqual([...TIMEOUT_CHOICES])
  })

  it('ignoriert Unsinn', () => {
    expect(timeoutChoices(Number.NaN)).toEqual([...TIMEOUT_CHOICES])
    expect(timeoutChoices(0)).toEqual([...TIMEOUT_CHOICES])
  })
})

describe('timeoutLabel', () => {
  it('nennt ganze Minuten als Minuten', () => {
    expect(timeoutLabel(120)).toEqual({ key: 'system.smartHomeMinutes', n: 2 })
    expect(timeoutLabel(60)).toEqual({ key: 'system.smartHomeMinutes', n: 1 })
  })

  it('nennt den Rest in Sekunden', () => {
    expect(timeoutLabel(30)).toEqual({ key: 'system.smartHomeSeconds', n: 30 })
    expect(timeoutLabel(90)).toEqual({ key: 'system.smartHomeSeconds', n: 90 })
  })
})

describe('CORNERS', () => {
  it('kennt die vier Ecken aus model::ButtonCorner', () => {
    expect(CORNERS).toEqual(['topLeft', 'topRight', 'bottomLeft', 'bottomRight'])
  })
})

describe('urlWasRejected', () => {
  it('meldet eine eingegebene, aber verworfene Adresse', () => {
    expect(urlWasRejected('homeassistant.local:8123', '')).toBe(true)
  })

  it('schweigt beim bewussten Leeren', () => {
    expect(urlWasRejected('', '')).toBe(false)
    expect(urlWasRejected('   ', '')).toBe(false)
  })

  it('schweigt bei einer uebernommenen Adresse', () => {
    expect(urlWasRejected('http://ha.local:8123', 'http://ha.local:8123')).toBe(false)
  })
})

describe('isSmartHomeEnabled', () => {
  it('ist ohne Konfiguration oder ohne Block aus', () => {
    expect(isSmartHomeEnabled(null)).toBe(false)
    expect(isSmartHomeEnabled({} as AppConfig)).toBe(false)
  })

  it('haengt allein an der Adresse (E-73)', () => {
    const mit = { smartHome: { url: 'http://ha.local:8123' } } as AppConfig
    const ohne = { smartHome: { url: '' } } as AppConfig
    expect(isSmartHomeEnabled(mit)).toBe(true)
    expect(isSmartHomeEnabled(ohne)).toBe(false)
  })
})

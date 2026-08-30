import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import AnalogClock from './AnalogClock.vue'

/**
 * Die Uhr rechnet nicht selbst — die Winkel kommen aus `clockAngles`, das
 * eigene Tests hat. Geprüft wird deshalb, dass die Winkel unverfälscht als
 * Drehung an den richtigen Zeigern ankommen: eine vertauschte Zuordnung fiele
 * sonst erst am Gerät auf, und dort nur jemandem, der genau hinsieht.
 */
function dial(date: Date) {
  return mount(AnalogClock, { props: { date } })
}

describe('AnalogClock', () => {
  it('dreht Stunden- und Minutenzeiger getrennt', () => {
    // 3:30 Uhr: Minutenzeiger auf sechs, Stundenzeiger zwischen drei und vier.
    const w = dial(new Date(2026, 7, 30, 3, 30))
    expect(w.get('.hour').attributes('transform')).toBe('rotate(105 50 50)')
    expect(w.get('.minute').attributes('transform')).toBe('rotate(180 50 50)')
  })

  it('macht den Minutenzeiger laenger als den Stundenzeiger', () => {
    // Sind die Laengen vertauscht, laesst sich die Uhr nicht mehr ablesen.
    const w = dial(new Date(2026, 7, 30, 3, 30))
    const hour = Number(w.get('.hour').attributes('y2'))
    const minute = Number(w.get('.minute').attributes('y2'))
    expect(minute).toBeLessThan(hour)
  })

  it('zeichnet zwoelf Marken, vier davon lang', () => {
    const w = dial(new Date(2026, 7, 30, 3, 30))
    expect(w.findAll('.mark')).toHaveLength(12)
    expect(w.findAll('.mark.major')).toHaveLength(4)
  })

  it('setzt die langen Marken auf zwoelf, drei, sechs und neun', () => {
    const w = dial(new Date(2026, 7, 30, 3, 30))
    const angles = w.findAll('.mark.major').map((m) => m.attributes('transform'))
    expect(angles).toEqual([
      'rotate(0 50 50)',
      'rotate(90 50 50)',
      'rotate(180 50 50)',
      'rotate(270 50 50)',
    ])
  })

  it('haelt die Uhrzeit fuer Screenreader lesbar', () => {
    // Zeiger sind nicht vorlesbar — ohne Beschriftung waere die Uhr stumm.
    const w = dial(new Date(2026, 7, 30, 21, 47))
    expect(w.get('svg').attributes('aria-label')).toBe('21:47')
  })

  it('folgt einer neuen Uhrzeit', async () => {
    const w = dial(new Date(2026, 7, 30, 3, 30))
    await w.setProps({ date: new Date(2026, 7, 30, 3, 31) })
    expect(w.get('.minute').attributes('transform')).toBe('rotate(186 50 50)')
  })
})

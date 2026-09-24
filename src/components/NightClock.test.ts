import { beforeAll, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import NightClock from './NightClock.vue'
import { i18n } from '@/lib/i18n'

/**
 * Die Zeile unter der Nachtuhr (FA-54).
 *
 * Anlass: legte Home Assistant den Rahmen schlafen, während kein Zeitplan
 * eingeschaltet war, stand dort „Ruhemodus bis 07:00" — eine Zeit, zu der
 * nichts geschieht, denn der Fernbefehl gilt bis zum Gegenbefehl.
 */

beforeAll(() => {
  i18n.global.locale.value = 'de'
})

function night(resumeAt: string | null) {
  return mount(NightClock, {
    props: { resumeAt, clockStyle: 'digital', pixelShift: false },
    global: { plugins: [i18n] },
  })
}

describe('NightClock', () => {
  it('nennt die Aufwachzeit, wenn der Zeitplan sie bestimmt', () => {
    expect(night('07:00').get('.label').text()).toBe('Ruhemodus bis 07:00')
  })

  it('nennt keine Zeit, wenn es keine gibt', () => {
    const label = night(null).get('.label').text()
    expect(label).toBe('Ruhemodus')
    expect(label).not.toContain('bis')
  })
})

import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import SsFeedback from './SsFeedback.vue'

/**
 * Anzeige einer Rückmeldung (E-62). Die Zeitsteuerung prüft
 * `useFeedback.test.ts`; hier geht es um das, was zu sehen und zu hören ist.
 */

describe('SsFeedback', () => {
  it('hält den Live-Bereich auch ohne Meldung bereit', () => {
    // Screenreader kündigen nur Änderungen in einem Bereich an, der schon da
    // war. Entstünde er erst mit der Meldung, bliebe sie stumm.
    const w = mount(SsFeedback, { props: { feedback: null } })
    expect(w.attributes('aria-live')).toBe('polite')
    expect(w.find('.text').exists()).toBe(false)
  })

  it('zeigt Erfolg höflich an', () => {
    const w = mount(SsFeedback, { props: { feedback: { kind: 'ok', text: 'Gespeichert', id: 1 } } })
    const text = w.get('.text')
    expect(text.text()).toBe('Gespeichert')
    expect(text.classes()).toContain('ok')
    expect(text.attributes('role')).toBe('status')
  })

  it('meldet Fehler sofort', () => {
    const w = mount(SsFeedback, {
      props: { feedback: { kind: 'error', text: 'Verbindung fehlgeschlagen', id: 2 } },
    })
    const text = w.get('.text')
    expect(text.classes()).toContain('error')
    expect(text.attributes('role')).toBe('alert')
  })
})

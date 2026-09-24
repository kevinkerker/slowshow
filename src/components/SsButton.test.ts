import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import SsButton from './SsButton.vue'
import SsIconButton from './SsIconButton.vue'

/**
 * Das Knopfsystem (E-58).
 *
 * Vorher baute jeder Bereich seine Knöpfe selbst, und dieselbe Variante sah
 * je nach Ort anders aus — oder gar nicht: „Aufräumen" war nicht rot, der
 * Primärknopf im Diagnosebericht ungestaltet. Hier wird festgehalten, was
 * alle Knöpfe gemeinsam haben.
 */

describe('SsButton', () => {
  it('trägt seine Variante als Klasse', () => {
    for (const variant of ['primary', 'secondary', 'danger', 'ghost', 'link'] as const) {
      const w = mount(SsButton, { props: { variant }, slots: { default: 'Los' } })
      expect(w.classes()).toContain(`ss-btn--${variant}`)
    }
  })

  it('ist ohne Angabe sekundär und kein Absendeknopf', () => {
    // Ein `type="submit"` in einem Formular schickte es beim Tippen ab.
    const w = mount(SsButton, { slots: { default: 'Los' } })
    expect(w.classes()).toContain('ss-btn--secondary')
    expect(w.attributes('type')).toBe('button')
  })

  it('zeigt beim Arbeiten einen Kreisel vor der Aufschrift', () => {
    const w = mount(SsButton, { props: { busy: true }, slots: { default: 'Prüfe …' } })
    expect(w.find('.spinner').exists()).toBe(true)
    expect(w.text()).toBe('Prüfe …')
    expect(w.attributes('aria-busy')).toBe('true')
    // Beschäftigt ist nicht gesperrt: der Knopf wird nicht blass.
    expect(w.attributes('disabled')).toBeUndefined()
  })

  it('zeigt ohne Arbeit keinen Kreisel', () => {
    const w = mount(SsButton, { slots: { default: 'Los' } })
    expect(w.find('.spinner').exists()).toBe(false)
    expect(w.attributes('aria-busy')).toBeUndefined()
  })

  it('reicht Klicks durch und nimmt sie gesperrt nicht an', async () => {
    const aktiv = mount(SsButton, { slots: { default: 'Los' } })
    await aktiv.trigger('click')
    expect(aktiv.emitted('click')).toHaveLength(1)

    const gesperrt = mount(SsButton, { props: { disabled: true }, slots: { default: 'Los' } })
    expect(gesperrt.attributes('disabled')).toBeDefined()
  })
})

describe('SsIconButton', () => {
  it('trägt seine Beschriftung für Screenreader und als Tooltip', () => {
    const w = mount(SsIconButton, { props: { label: 'Bearbeiten' }, slots: { default: '<svg />' } })
    expect(w.attributes('aria-label')).toBe('Bearbeiten')
    expect(w.attributes('title')).toBe('Bearbeiten')
  })

  it('dreht beim Arbeiten sein Symbol', () => {
    // So meldet der Sync-Knopf der Quellenkarte einen laufenden Abgleich.
    const w = mount(SsIconButton, {
      props: { label: 'Jetzt synchronisieren', busy: true },
      slots: { default: '<svg />' },
    })
    expect(w.find('.glyph').classes()).toContain('spinning')
    expect(w.classes()).toContain('busy')
  })
})

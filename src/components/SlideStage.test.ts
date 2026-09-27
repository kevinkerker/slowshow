import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import SlideStage from './SlideStage.vue'
import { imageUrl } from '@/lib/api'
import type { Slide } from '@/lib/types'

/**
 * Livebild eines Kamerastroms auf der Bühne (E-67).
 *
 * Ein neues Einzelbild darf nur die Quelle des Bildes tauschen. Legte es eine
 * neue Ebene an, blendete die Bühne fünfmal je Sekunde über — auf einem alten
 * Tablet ein Dauerflimmern, und jedes Mal liefe die Überblendung von vorn.
 */

const LIVE = { kind: 'single', id: 'x_1' } as Slide

function buehne() {
  return mount(SlideStage, {
    props: {
      slide: LIVE,
      fitMode: 'contain',
      transitionEnabled: true,
      transitionMs: 800,
      kenBurns: false,
      liveFrame: null,
    },
  })
}

describe('SlideStage — Livebild (E-67)', () => {
  it('laedt das neue Einzelbild in dasselbe Bildelement', async () => {
    const w = buehne()
    await w.vm.$nextTick()
    const vorher = w.get('.layer.visible img').element

    await w.setProps({ liveFrame: { id: 'x_1', frame: 3 } })
    const img = w.get('.layer.visible img')
    expect(img.element, 'neues Bildelement heisst neue Ebene').toBe(vorher)
    expect(img.attributes('src')).toBe(`${imageUrl('x_1')}?f=3`)
    expect(w.findAll('.layer.visible')).toHaveLength(1)
  })

  it('zeigt Fotos der Sammlung unveraendert', async () => {
    // Auch wenn noch ein Einzelbild eines beendeten Stroms im Umlauf ist.
    const w = buehne()
    await w.setProps({
      slide: { kind: 'single', id: 'ea9c' } as Slide,
      liveFrame: { id: 'x_1', frame: 3 },
    })
    const srcs = w.findAll('img').map((i) => i.attributes('src'))
    expect(srcs).toContain(imageUrl('ea9c'))
    expect(srcs).not.toContain(`${imageUrl('ea9c')}?f=3`)
  })
})

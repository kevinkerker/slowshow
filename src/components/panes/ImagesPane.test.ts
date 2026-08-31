import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { mount } from '@vue/test-utils'
import ImagesPane from './ImagesPane.vue'
import { i18n } from '@/lib/i18n'
import * as api from '@/lib/api'
import type { CacheEntry } from '@/lib/types'

/**
 * Freigeben wartender Fotos (F4, E-35).
 *
 * Anlass: „Absender vertrauen" gab es im Rust-Backend seit jeher, in der
 * Bedienung aber nie. `release(entry, true)` wurde von nirgends aufgerufen —
 * ein Tipp gab immer nur das eine Bild frei, und die Freigabeliste konnte
 * sich niemals füllen. Aufgefallen ist es erst, als jemand danach fragte;
 * der Kommentar über der Funktion beschrieb den Knopf, als gäbe es ihn.
 *
 * Deshalb prüfen diese Tests nicht nur, *dass* etwas passiert, sondern mit
 * welchem Argument — daran hing der ganze Fehler.
 */

beforeAll(() => {
  i18n.global.locale.value = 'de'
})

function wartend(id: string, sender: string, subject = 'Gruesse'): CacheEntry {
  return {
    id,
    sourceId: 'post',
    relPath: `${id}.jpg`,
    fileName: `${id}.jpg`,
    etag: null,
    remoteSize: 1000,
    remoteMtime: null,
    takenAt: 1_700_000_000,
    width: 100,
    height: 100,
    bytes: 1000,
    addedAt: 1_700_000_000,
    lastShown: null,
    showCount: 0,
    excluded: false,
    thumbBytes: 100,
    mail: { sender, subject, messageId: id, quarantined: true },
  } as unknown as CacheEntry
}

const SEITE = {
  entries: [wartend('a', 'oma@example.org'), wartend('b', 'werbung@shop.de')],
  total: 2,
}

beforeEach(() => {
  setActivePinia(createPinia())
  vi.restoreAllMocks()
  vi.spyOn(api, 'imagePage').mockResolvedValue(SEITE as never)
  vi.spyOn(api, 'allowedSenders').mockResolvedValue([])
})

async function pane() {
  const w = mount(ImagesPane, { global: { plugins: [i18n] } })
  await new Promise((r) => setTimeout(r, 0))
  await w.vm.$nextTick()
  // Auf den Quarantaene-Filter umschalten: dort spielt sich alles ab.
  // Die Filterleiste nutzt `.ss-segment` aus den gemeinsamen Stilen, nicht
  // eine eigene Klasse — der letzte Eintrag ist „Warten auf Freigabe".
  const filter = w.findAll('.ss-segment')
  expect(filter.length, 'Filterleiste nicht gefunden').toBeGreaterThan(0)
  await filter[filter.length - 1].trigger('click')
  await new Promise((r) => setTimeout(r, 0))
  await w.vm.$nextTick()
  return w
}

describe('ImagesPane — wartende Fotos freigeben', () => {
  it('gibt beim Tippen nicht sofort frei, sondern fragt', async () => {
    const release = vi.spyOn(api, 'releaseQuarantine').mockResolvedValue(1)
    const w = await pane()

    await w.findAll('.cell')[0].trigger('click')
    await w.vm.$nextTick()

    expect(release, 'ein Tipp darf noch nichts freigeben').not.toHaveBeenCalled()
    expect(w.find('.release').exists()).toBe(true)
  })

  it('zeigt vor der Entscheidung, von wem das Bild kommt', async () => {
    const w = await pane()
    await w.findAll('.cell')[0].trigger('click')
    await w.vm.$nextTick()

    const text = w.get('.release').text()
    expect(text).toContain('oma@example.org')
    expect(text).toContain('Gruesse')
  })

  it('gibt auf Wunsch nur das eine Bild frei', async () => {
    const release = vi.spyOn(api, 'releaseQuarantine').mockResolvedValue(1)
    const w = await pane()
    await w.findAll('.cell')[0].trigger('click')
    await w.vm.$nextTick()

    await w.get('.release .primary').trigger('click')
    await new Promise((r) => setTimeout(r, 0))

    expect(release).toHaveBeenCalledWith('a', false)
  })

  it('vertraut auf Wunsch dem ganzen Absender', async () => {
    // Der Aufruf, den es vier Monate lang nicht gab.
    const release = vi.spyOn(api, 'releaseQuarantine').mockResolvedValue(2)
    const w = await pane()
    await w.findAll('.cell')[0].trigger('click')
    await w.vm.$nextTick()

    await w.get('.release .secondary').trigger('click')
    await new Promise((r) => setTimeout(r, 0))

    expect(release).toHaveBeenCalledWith('a', true)
  })

  it('nennt den Absender auf der Schaltflaeche', async () => {
    // „Alle von oma@example.org" statt „Alle von diesem Absender": bei zwei
    // wartenden Personen hintereinander ist sonst nicht klar, wen man gerade
    // dauerhaft freigibt.
    const w = await pane()
    await w.findAll('.cell')[1].trigger('click')
    await w.vm.$nextTick()

    expect(w.get('.release .secondary').text()).toContain('werbung@shop.de')
  })

  it('laesst sich abbrechen, ohne etwas freizugeben', async () => {
    const release = vi.spyOn(api, 'releaseQuarantine').mockResolvedValue(1)
    const w = await pane()
    await w.findAll('.cell')[0].trigger('click')
    await w.vm.$nextTick()

    await w.get('.release .ghost').trigger('click')
    await w.vm.$nextTick()

    expect(release).not.toHaveBeenCalled()
    expect(w.find('.release').exists()).toBe(false)
  })

  it('schreibt den Absender auf die wartende Kachel', async () => {
    // In der Quarantaene ist „von wem" die Frage, die zur Entscheidung
    // fuehrt; das Aufnahmedatum allein half dort nicht weiter.
    const w = await pane()
    expect(w.findAll('.cell')[0].get('.sender').text()).toBe('oma@example.org')
    expect(w.findAll('.cell')[0].find('.taken').exists()).toBe(true)
  })
})

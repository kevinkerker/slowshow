import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import SourcesPane from './SourcesPane.vue'
import { CANCEL, confirmRequest, settle } from '@/composables/useConfirm'
import { i18n } from '@/lib/i18n'
import { useConfigStore } from '@/stores/config'
import type { AppConfig, Source, SyncReport } from '@/lib/types'

/**
 * Quellenliste: Abgleich von Hand und Entfernen.
 *
 * Zwei Umbauten aus v1.2: das Ergebnis eines Abgleichs steht in der Karte der
 * Quelle statt unter der Liste (E-62), und vor dem Entfernen fragt der eigene
 * Dialog statt `window.confirm()` (E-59).
 */

beforeAll(() => {
  i18n.global.locale.value = 'de'
})

afterEach(() => settle(CANCEL))

const NAS: Source = {
  id: 'nas',
  name: 'NAS · Fotoarchiv',
  kind: { type: 'webDav', url: 'https://nas.local/dav', username: 'k', passwordRef: 'nas', allowInsecureTls: false },
  enabled: true,
  subfolders: [],
  minWidth: 0,
  minHeight: 0,
  syncIntervalMinutes: 360,
  lastSync: null,
}

const REPORT: SyncReport = {
  sourceId: 'nas',
  added: 12,
  updated: 3,
  unchanged: 0,
  removed: 0,
  skipped: 0,
  failed: 0,
  evicted: 0,
  truncated: false,
  error: null,
}

let store: ReturnType<typeof useConfigStore>

beforeEach(() => {
  setActivePinia(createPinia())
  store = useConfigStore()
  store.config = { language: 'de', sources: [NAS] } as unknown as AppConfig
})

function pane() {
  return mount(SourcesPane, { global: { plugins: [i18n] } })
}

describe('SourcesPane', () => {
  it('meldet das Ergebnis des Abgleichs in der Karte der Quelle', async () => {
    vi.spyOn(store, 'syncSource').mockResolvedValue(REPORT)
    const w = pane()
    await w.get('.action-sync').trigger('click')
    await flushPromises()

    expect(w.get('.card .ss-feedback .ok').text()).toBe('12 neu, 3 aktualisiert, 0 entfernt')
  })

  it('meldet einen gescheiterten Abgleich als Fehler', async () => {
    vi.spyOn(store, 'syncSource').mockResolvedValue({ ...REPORT, error: 'Anmeldung abgelehnt' })
    const w = pane()
    await w.get('.action-sync').trigger('click')
    await flushPromises()

    const meldung = w.get('.card .ss-feedback .error')
    expect(meldung.text()).toContain('Anmeldung abgelehnt')
  })

  it('fragt vor dem Entfernen im eigenen Dialog und nennt die Quelle', async () => {
    const remove = vi.spyOn(store, 'removeSource').mockResolvedValue()
    const w = pane()
    await w.get('.action-edit').trigger('click')
    const knopf = w.findAll('button').find((b) => b.text() === 'Quelle entfernen')
    await knopf!.trigger('click')
    await flushPromises()

    const frage = confirmRequest.value
    expect(frage, 'keine Rückfrage offen').toBeTruthy()
    expect(frage!.title).toBe('Quelle „NAS · Fotoarchiv“ entfernen?')
    expect(frage!.body).toContain('an der Quelle bleibt alles unverändert')
    expect(remove).not.toHaveBeenCalled()

    settle('confirm')
    await flushPromises()
    expect(remove).toHaveBeenCalledWith('nas')
  })

  it('entfernt nichts, wenn abgebrochen wird', async () => {
    const remove = vi.spyOn(store, 'removeSource').mockResolvedValue()
    const w = pane()
    await w.get('.action-edit').trigger('click')
    const knopf = w.findAll('button').find((b) => b.text() === 'Quelle entfernen')
    await knopf!.trigger('click')
    await flushPromises()
    settle(CANCEL)
    await flushPromises()

    expect(remove).not.toHaveBeenCalled()
  })
})

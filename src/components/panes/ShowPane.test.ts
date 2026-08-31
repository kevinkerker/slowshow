import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { mount } from '@vue/test-utils'
import ShowPane from './ShowPane.vue'
import { i18n } from '@/lib/i18n'
import * as api from '@/lib/api'
import { useConfigStore } from '@/stores/config'
import type { AppConfig, PlaybackStats } from '@/lib/types'

/**
 * Durchlauf und Statistik (Wartung F1–F3).
 *
 * Nach E-31 stehen sie bei der Wiedergabe statt in einem eigenen Bereich.
 * Geprüft wird vor allem das Zurücksetzen der Historie: es ist die einzige
 * destruktive Handlung auf dieser Seite, und ein versehentlicher Griff kostet
 * die Anzeigehistorie des gesamten Bestands.
 */

beforeAll(() => {
  i18n.global.locale.value = 'de'
})

const STATS: PlaybackStats = {
  total: 597,
  eligible: 214,
  neverShown: 12,
  bagRemaining: 88,
  cycles: 3,
  mostShown: [
    { id: 'a', fileName: 'oma.jpg', showCount: 9, lastShown: 1_700_000_000 },
    { id: 'b', fileName: 'opa.jpg', showCount: 4, lastShown: 1_700_000_000 },
  ],
  longestUnseen: [{ id: 'c', fileName: 'alt.jpg', showCount: 1, lastShown: 1_600_000_000 }],
}

/**
 * Kleinste Konfiguration, mit der die Ansicht rendert.
 *
 * Sie haengt an `v-if="cfg"` — ohne geladene Konfiguration bleibt die Seite
 * leer, und jede Zusicherung darunter liefe ins Nichts.
 */
const CONFIG = {
  intervalSeconds: 30,
  order: 'smart',
  fitMode: 'contain',
  transition: { enabled: false, durationMs: 1200 },
  overlays: {
    showClock: false,
    showDate: false,
    showFileName: false,
    showTakenAt: false,
    pixelShift: true,
    showSettingsButton: false,
    showExcludeButton: true,
    showQuarantineHint: true,
    clockStyle: 'analog',
  },
  schedule: {
    enabled: false,
    activeFrom: '07:00',
    activeTo: '22:00',
    nightClock: true,
    nightClockStyle: 'digital',
  },
  brightness: { level: 40, autoDim: false, dimFrom: '20:00', dimLevel: 15, deviceControlled: false },
  cache: { maxBytes: 1, prefetchCount: 5, targetWidth: 2560, targetHeight: 1600, jpegQuality: 85 },
  remote: { enabled: false, port: 8127, token: '' },
  mqtt: {
    enabled: false,
    host: '',
    port: 1883,
    username: '',
    baseTopic: 'slowshow',
    discovery: true,
    discoveryPrefix: 'homeassistant',
  },
  pairMode: false,
  kenBurns: false,
  protectSettings: false,
  orientation: 'auto',
  playback: { newBoost: true, leastRecentlyShown: true, clusterFilter: true, newestFirst: false },
  filter: { time: { type: 'all' }, senders: [], includeUndated: true },
  language: 'auto',
  sources: [],
} as unknown as AppConfig

beforeEach(() => {
  setActivePinia(createPinia())
  vi.restoreAllMocks()
  vi.spyOn(api, 'playbackStats').mockResolvedValue(STATS)
  vi.spyOn(api, 'filterFacets').mockResolvedValue({ years: [], senders: [] } as never)
})

async function pane() {
  useConfigStore().config = CONFIG
  const w = mount(ShowPane, { global: { plugins: [i18n] } })
  await new Promise((r) => setTimeout(r, 0))
  await w.vm.$nextTick()
  return w
}

describe('ShowPane — Durchlauf und Statistik', () => {
  it('zeigt Bestand, nie gezeigte und den Durchlauf', async () => {
    const text = (await pane()).text()
    expect(text).toContain('597')
    expect(text).toContain('214')
    expect(text).toContain('12')
    // „noch 88 von 214" — die Bezugsgroesse ist der spielbare Bestand, nicht
    // der ganze Cache; sonst erreichte der Fortschritt nie null.
    expect(text).toContain('88')
  })

  it('fuehrt beide Bestenlisten mit Dateinamen', async () => {
    const text = (await pane()).text()
    expect(text).toContain('oma.jpg')
    expect(text).toContain('alt.jpg')
  })

  it('startet den Durchlauf ohne Rueckfrage neu', async () => {
    // F2 ist nicht destruktiv: nur die Urne wird geleert. Eine Rueckfrage
    // waere hier bloss im Weg.
    const restart = vi.spyOn(api, 'restartCycle').mockResolvedValue()
    const confirmSpy = vi.fn(() => true)
    vi.stubGlobal('confirm', confirmSpy)

    const w = await pane()
    const knopf = w.findAll('button').find((b) => b.text() === 'Durchlauf neu starten')
    expect(knopf, 'Schaltflaeche nicht gefunden').toBeTruthy()
    await knopf!.trigger('click')
    await new Promise((r) => setTimeout(r, 0))

    expect(restart).toHaveBeenCalled()
    expect(confirmSpy).not.toHaveBeenCalled()
    vi.unstubAllGlobals()
  })

  it('fragt vor dem Zuruecksetzen der Historie und nennt die Zahl', async () => {
    const reset = vi.spyOn(api, 'resetHistory').mockResolvedValue(214)
    const confirmSpy = vi.fn((_text?: string) => true)
    vi.stubGlobal('confirm', confirmSpy)

    const w = await pane()
    const knopf = w.findAll('button').find((b) => b.text() === 'Anzeige-Historie zurücksetzen')
    await knopf!.trigger('click')
    await new Promise((r) => setTimeout(r, 0))

    expect(confirmSpy).toHaveBeenCalled()
    // Die Zahl gehoert in die Frage: sonst muesste jemand raten, wie viel er
    // gerade verwirft.
    expect(String(confirmSpy.mock.calls[0]?.[0])).toContain('214')
    expect(reset).toHaveBeenCalled()
    vi.unstubAllGlobals()
  })

  it('setzt nichts zurueck, wenn die Rueckfrage verneint wird', async () => {
    const reset = vi.spyOn(api, 'resetHistory').mockResolvedValue(0)
    vi.stubGlobal('confirm', vi.fn(() => false))

    const w = await pane()
    const knopf = w.findAll('button').find((b) => b.text() === 'Anzeige-Historie zurücksetzen')
    await knopf!.trigger('click')
    await new Promise((r) => setTimeout(r, 0))

    expect(reset).not.toHaveBeenCalled()
    vi.unstubAllGlobals()
  })

  it('kommt ohne Statistik aus, statt die Seite zu verlieren', async () => {
    // Die Zahlen sind eine Beigabe. Faellt der Aufruf aus, muessen die
    // Einstellungen darunter trotzdem bedienbar bleiben.
    vi.spyOn(api, 'playbackStats').mockRejectedValue(new Error('kaputt'))
    const w = await pane()
    // Der Statistik-Abschnitt faellt weg, die Einstellungen bleiben.
    expect(w.find('.stats').exists()).toBe(false)
    expect(w.text()).toContain('Anzeigedauer')
  })

  it('nennt bei den laengsten Wartenden den Zeitpunkt, nicht die Zahl', async () => {
    // Am Geraet stand dort „0×" neben jedem Eintrag: `showCount` ist juenger
    // als der Bestand, vorhandene Bilder tragen einen Zeitpunkt ohne Zaehler.
    // Und die Liste ist nach dem Zeitpunkt sortiert — der gehoert daneben.
    const w = await pane()
    const spalten = w.findAll('.top')
    const laengst = spalten[spalten.length - 1]
    expect(laengst.text()).toContain('alt.jpg')
    expect(laengst.text()).not.toContain('0×')
    expect(laengst.text()).not.toContain('1×')
  })

  it('nennt bei den meistgezeigten weiterhin die Zahl', async () => {
    const w = await pane()
    expect(w.findAll('.top')[0].text()).toContain('9×')
  })
})

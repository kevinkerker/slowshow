import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import App from './App.vue'
import { router } from './router'
import { i18n } from './lib/i18n'
import * as api from './lib/api'
import { BOOT_RETRY_DELAYS_MS } from './lib/retry'
import type { AppConfig, CacheStats } from './lib/types'

/**
 * Startet die ganze App mit einem nachgestellten Backend.
 *
 * Anlass: Nach einem Release-Build blieb der Rahmen schwarz — App.vue rendert
 * die Ansicht erst, wenn `store.load()` durch ist, und ein Fehler oder Haenger
 * in dieser Kette ist von aussen nicht von einem toten WebView zu
 * unterscheiden. Dieser Test faehrt genau diese Kette im jsdom hoch: Laden,
 * Ausrichtung, Diashow-Start, erstes Bild. Faellt er, ist es das Frontend;
 * besteht er, liegt es an der Bruecke oder am Geraet.
 */

const CONFIG = {
  intervalSeconds: 30,
  order: 'smart',
  fitMode: 'contain',
  transition: { enabled: true, durationMs: 800 },
  overlays: {
    showClock: true,
    showDate: false,
    showFileName: false,
    showTakenAt: false,
    pixelShift: true,
    showQuarantineHint: true,
    showExcludeButton: false,
    clockStyle: 'digital',
  },
  schedule: { enabled: false, activeFrom: '07:00', activeTo: '22:00', nightClock: true, nightClockStyle: 'digital' },
  brightness: { level: 100, autoDim: false, dimFrom: '20:00', dimLevel: 40, deviceControlled: false },
  cache: { maxBytes: 2_000_000_000, prefetchCount: 3, targetWidth: 2560, targetHeight: 1600, jpegQuality: 85 },
  remote: { enabled: false, port: 8127, token: '' },
  mqtt: { enabled: false, host: '', port: 1883, username: '', baseTopic: 'slowshow', discovery: true, discoveryPrefix: 'homeassistant' },
  upnp: { enabled: true, port: 8128, udn: 'uuid:test', friendlyName: 'Slowshow' },
  pairMode: true,
  kenBurns: false,
  protectSettings: false,
  orientation: 'landscape',
  playback: {},
  filter: { senders: [], years: [], albums: [] },
  language: 'de',
  sources: [],
} as unknown as AppConfig

const STATS = { images: 2, bytes: 10, maxBytes: 2_000_000_000, excluded: 0, thumbBytes: 1 } as CacheStats

beforeEach(() => {
  setActivePinia(createPinia())
  vi.restoreAllMocks()
  // jsdom kennt kein matchMedia; App.vue fragt damit die Ausrichtung ab (E-26).
  window.matchMedia = vi.fn().mockReturnValue({
    matches: false,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  } as unknown as MediaQueryList)
  vi.spyOn(api, 'getConfig').mockResolvedValue(CONFIG)
  vi.spyOn(api, 'getDisplayState').mockResolvedValue({
    slideshowActive: true,
    showNightClock: false,
    brightness: 100,
  })
  vi.spyOn(api, 'cacheStats').mockResolvedValue(STATS)
  vi.spyOn(api, 'sourceCounts').mockResolvedValue({})
  vi.spyOn(api, 'applyOrientation').mockResolvedValue()
  vi.spyOn(api, 'reportDisplaySize').mockResolvedValue()
  vi.spyOn(api, 'setFrameOrientation').mockResolvedValue()
  vi.spyOn(api, 'currentSlide').mockResolvedValue({ kind: 'single', id: 'abc123' })
  vi.spyOn(api, 'isPlaying').mockResolvedValue(true)
  vi.spyOn(api, 'prefetchWindow').mockResolvedValue(['abc123'])
  vi.spyOn(api, 'imageInfo').mockResolvedValue(null)
  vi.spyOn(api, 'quarantineCount').mockResolvedValue(0)
  vi.spyOn(api, 'nextSlide').mockResolvedValue({ kind: 'single', id: 'def456' })
})

describe('App-Start', () => {
  it('faehrt bis zur Diashow mit dem ersten Bild hoch', async () => {
    const fehler = vi.spyOn(console, 'error').mockImplementation(() => {})
    await router.push('/')
    await router.isReady()

    const wrapper = mount(App, {
      global: { plugins: [router, i18n] },
      attachTo: document.body,
    })
    await flushPromises()
    await flushPromises()

    // App.vue: erst nach `store.load()` rendert die Ansicht ueberhaupt.
    expect(wrapper.find('.slideshow').exists(), 'Diashow-Ansicht fehlt — store.load() nicht durch').toBe(true)
    // Das erste Bild haengt an der Buehne.
    const bilder = wrapper.findAll('img')
    expect(bilder.length, 'kein Bild auf der Buehne').toBeGreaterThan(0)
    expect(bilder[0].attributes('src')).toContain('abc123')
    // Tags dimmt nichts (E-51).
    expect(wrapper.find('.dim').exists()).toBe(false)
    expect(fehler).not.toHaveBeenCalled()
    wrapper.unmount()
  })

  it('kommt hoch, auch wenn der erste get_config noch vor manage() scheitert (E-54)', async () => {
    // Der Befund vom Geraet: Tauri legt das Fenster vor dem setup-Hook an,
    // die WebView ruft `get_config`, bevor Rust den Zustand angemeldet hat,
    // und ohne Wiederholung blieb der Rahmen dunkel. Der Rust-Teil beseitigt
    // das Rennen; dieser Test sichert die zweite Schicht: das Frontend gibt
    // nach einem Fehlschlag nicht auf.
    const fehler = vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(api, 'getConfig')
      .mockRejectedValueOnce(
        new Error(
          'state not managed for field `state` on command `get_config`. You must call `.manage()` before using this command',
        ),
      )
      .mockResolvedValue(CONFIG)
    await router.push('/')
    await router.isReady()

    const wrapper = mount(App, {
      global: { plugins: [router, i18n] },
      attachTo: document.body,
    })
    await flushPromises()

    // Vor der Wiederholung nur die Huelle — genau das Bild vom Geraet.
    expect(wrapper.find('.slideshow').exists()).toBe(false)

    // Die erste Wiederholung kommt nach dem ersten Abstand aus der Liste.
    await new Promise((r) => setTimeout(r, BOOT_RETRY_DELAYS_MS[0] + 200))
    await flushPromises()
    await flushPromises()

    expect(
      wrapper.find('.slideshow').exists(),
      'Diashow-Ansicht fehlt — der Boot wurde nicht wiederholt',
    ).toBe(true)
    expect(wrapper.findAll('img').length, 'kein Bild auf der Buehne').toBeGreaterThan(0)
    // Der Fehlschlag steht im Log, mit seiner Nummer — sonst verriete der
    // Rahmen nie, dass etwas war.
    expect(fehler).toHaveBeenCalledTimes(1)
    expect(String(fehler.mock.calls[0][0])).toContain('Versuch 1')
    wrapper.unmount()
  })

  it('weiterschalten von Hand geht ueber den Geste-Pfad (E-52)', async () => {
    await router.push('/')
    await router.isReady()
    const wrapper = mount(App, { global: { plugins: [router, i18n] } })
    await flushPromises()
    await flushPromises()

    const { useSlideshowStore } = await import('./stores/slideshow')
    const show = useSlideshowStore()
    await show.next()
    expect(api.nextSlide).toHaveBeenCalledWith(true)
    expect(wrapper.findAll('img').some((i) => i.attributes('src')?.includes('def456'))).toBe(true)
    wrapper.unmount()
  })
})

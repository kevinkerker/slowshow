import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useConfigStore } from './config'
import * as api from '@/lib/api'
import type { AppConfig, CacheStats } from '@/lib/types'

/**
 * Wann die Cache-Statistik erneuert wird.
 *
 * Am Gerät gemeldet: die maximale Cachegröße ließ sich unter „System"
 * umstellen, in der Fußzeile der Quellenliste stand aber weiter der alte
 * Wert. Die Fußzeile liest `stats.maxBytes`, und `patch` hat die Statistik nie
 * nachgeladen — `addSource` und `removeSource` schon, weshalb die Zahl
 * irgendwann doch umsprang und der Fehler nach Zufall aussah.
 *
 * Geprüft wird beides: dass eine geänderte Grenze nachlädt, und dass ein
 * beliebiger anderer Schalter es nicht tut. `cacheStats` läuft über den
 * ganzen Index; bei jedem Schalter wäre das verschenkte Arbeit.
 */

const CONFIG = {
  intervalSeconds: 30,
  language: 'auto',
  cache: { maxBytes: 2_147_483_648, prefetchCount: 5, targetWidth: 2560, targetHeight: 1600, jpegQuality: 85 },
  overlays: { showClock: false },
  sources: [],
} as unknown as AppConfig

const STATS = { images: 1, bytes: 10, maxBytes: 2_147_483_648, excluded: 0, thumbBytes: 1 } as CacheStats

function mitConfig(cfg: AppConfig) {
  vi.spyOn(api, 'setConfig').mockResolvedValue(cfg)
  vi.spyOn(api, 'getDisplayState').mockResolvedValue({} as never)
  vi.spyOn(api, 'sourceCounts').mockResolvedValue({} as never)
  return vi.spyOn(api, 'cacheStats').mockResolvedValue(STATS)
}

beforeEach(() => {
  setActivePinia(createPinia())
  vi.restoreAllMocks()
})

describe('configStore.patch', () => {
  it('laedt die Statistik nach, wenn die Cache-Grenze sich aendert', async () => {
    const kleiner = { ...CONFIG, cache: { ...CONFIG.cache, maxBytes: 1_073_741_824 } }
    const stats = mitConfig(kleiner)

    const store = useConfigStore()
    store.config = CONFIG
    await store.patch((d) => (d.cache.maxBytes = 1_073_741_824))

    expect(stats).toHaveBeenCalled()
  })

  it('laedt sie nicht nach, wenn etwas anderes umgestellt wird', async () => {
    // `cacheStats` laeuft ueber den ganzen Index. Bei jedem Schalter waere
    // das verschenkte Arbeit — und bei 5 000 Bildern spuerbar.
    const gleich = { ...CONFIG, intervalSeconds: 60 }
    const stats = mitConfig(gleich)

    const store = useConfigStore()
    store.config = CONFIG
    await store.patch((d) => (d.intervalSeconds = 60))

    expect(stats).not.toHaveBeenCalled()
  })

  it('richtet sich nach dem Wert des Backends, nicht nach dem Entwurf', async () => {
    // Das Backend klemmt Werte ab. Verglichen wird deshalb mit dem, was
    // zurueckkommt — sonst laedt die Oberflaeche nach, obwohl sich nichts
    // geaendert hat, oder umgekehrt.
    const geklemmt = { ...CONFIG, cache: { ...CONFIG.cache, maxBytes: CONFIG.cache.maxBytes } }
    const stats = mitConfig(geklemmt)

    const store = useConfigStore()
    store.config = CONFIG
    await store.patch((d) => (d.cache.maxBytes = 99_999_999_999))

    expect(stats, 'das Backend hat den Wert nicht uebernommen').not.toHaveBeenCalled()
  })
})

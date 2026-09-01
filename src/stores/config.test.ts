import { beforeEach, describe, expect, it, vi } from 'vitest'
import { listen } from '@tauri-apps/api/event'
import { createPinia, setActivePinia } from 'pinia'
import { useConfigStore } from './config'
import * as api from '@/lib/api'
import { EVENTS, type AppConfig, type CacheStats, type SyncProgress, type SyncReport } from '@/lib/types'

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

/**
 * Zwei Quellen gleichzeitig (E-43).
 *
 * Vorher gab es im Backend eine Sperre ueber *alle* Quellen und im Store ein
 * einzelnes `syncing`. Ein „Jetzt abgleichen" waehrend eines laufenden Laufs
 * fiel damit lautlos aus — `syncSource` kehrte sofort mit `null` zurueck, ohne
 * das Backend ueberhaupt zu fragen.
 */

const QUELLEN = [
  { id: 'a', name: 'A', enabled: true },
  { id: 'b', name: 'B', enabled: true },
] as unknown as AppConfig['sources']

function mitZweiQuellen() {
  const store = useConfigStore()
  store.config = { ...CONFIG, sources: QUELLEN }
  vi.spyOn(api, 'cacheStats').mockResolvedValue(STATS)
  vi.spyOn(api, 'sourceCounts').mockResolvedValue({})
  return store
}

/** Ein von aussen aufloesbares Versprechen. */
function offen<T>() {
  let loese!: (v: T) => void
  const p = new Promise<T>((r) => (loese = r))
  return { p, loese }
}

describe('configStore.syncSource', () => {
  it('laesst zwei verschiedene Quellen gleichzeitig laufen', async () => {
    const a = offen<SyncReport[]>()
    const b = offen<SyncReport[]>()
    const sync = vi
      .spyOn(api, 'syncNow')
      .mockImplementationOnce(() => a.p)
      .mockImplementationOnce(() => b.p)

    const store = mitZweiQuellen()
    const laufA = store.syncSource('a')
    const laufB = store.syncSource('b')

    expect(sync).toHaveBeenCalledTimes(2)
    expect(store.isSyncing('a')).toBe(true)
    expect(store.isSyncing('b')).toBe(true)

    a.loese([])
    b.loese([])
    await Promise.all([laufA, laufB])
    expect(store.isSyncing('a')).toBe(false)
    expect(store.isSyncing('b')).toBe(false)
  })

  it('startet dieselbe Quelle nicht zweimal', async () => {
    const a = offen<SyncReport[]>()
    const sync = vi.spyOn(api, 'syncNow').mockImplementation(() => a.p)

    const store = mitZweiQuellen()
    const lauf = store.syncSource('a')
    expect(await store.syncSource('a')).toBeNull()
    expect(sync).toHaveBeenCalledTimes(1)

    a.loese([])
    await lauf
  })
})

describe('configStore: Fortschritt mehrerer Quellen', () => {
  /** Meldet den Rueckruf, den der Store fuer dieses Ereignis angemeldet hat. */
  function rueckruf(name: string) {
    const treffer = vi.mocked(listen).mock.calls.find((c) => c[0] === name)
    if (!treffer) throw new Error(`Kein Zuhoerer fuer ${name}`)
    return treffer[1] as (e: { payload: unknown }) => void
  }

  async function geladen() {
    vi.spyOn(api, 'getConfig').mockResolvedValue({ ...CONFIG, sources: QUELLEN })
    vi.spyOn(api, 'getDisplayState').mockResolvedValue({} as never)
    vi.spyOn(api, 'cacheStats').mockResolvedValue(STATS)
    vi.spyOn(api, 'sourceCounts').mockResolvedValue({})
    const store = useConfigStore()
    await store.load()
    return store
  }

  function fortschritt(sourceId: string, done: number): SyncProgress {
    return { sourceId, sourceName: sourceId, done, total: 10, stored: done, current: 'x.jpg' }
  }

  it('haelt die Zwischenstaende beider Quellen auseinander', async () => {
    const store = await geladen()
    const melde = rueckruf(EVENTS.syncProgress)

    melde({ payload: fortschritt('a', 3) })
    melde({ payload: fortschritt('b', 7) })

    expect(store.progressFor('a')?.done).toBe(3)
    expect(store.progressFor('b')?.done).toBe(7)
  })

  it('raeumt beim Abschluss nur die fertige Quelle weg', async () => {
    // Vorher setzte jeder eingehende Bericht den einen Zwischenstand auf null
    // — der Balken der zweiten, noch laufenden Quelle verschwand mit.
    const store = await geladen()
    rueckruf(EVENTS.syncProgress)({ payload: fortschritt('a', 3) })
    rueckruf(EVENTS.syncProgress)({ payload: fortschritt('b', 7) })

    rueckruf(EVENTS.sync)({ payload: { sourceId: 'a', error: null } as SyncReport })

    expect(store.progressFor('a')).toBeNull()
    expect(store.progressFor('b')?.done, 'b laeuft weiter').toBe(7)
  })

  it('zeigt auch einen Hintergrundlauf als laufend an', async () => {
    // `syncing` kennt nur selbst ausgeloeste Laeufe. Ohne den Rueckfall auf den
    // Fortschritt liefe ein Abgleich des Zeitgebers unsichtbar.
    const store = await geladen()
    expect(store.isSyncing('a')).toBe(false)

    rueckruf(EVENTS.syncProgress)({ payload: fortschritt('a', 1) })
    expect(store.isSyncing('a')).toBe(true)
  })
})

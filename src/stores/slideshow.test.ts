import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { listen } from '@tauri-apps/api/event'
import { createPinia, setActivePinia } from 'pinia'
import { useSlideshowStore } from './slideshow'
import * as api from '@/lib/api'
import { EVENTS, type LiveFrame, type Slide } from '@/lib/types'

/**
 * Der Taktgeber der Diashow.
 *
 * Am Gerät gemeldet: nach langer Laufzeit blieb der Rahmen stehen. Der Grund
 * lag nicht im Backend, sondern hier — `restartTimer` rief `next()` ohne
 * Auffangen, und `next` setzt den Takt erst an seinem Ende neu. Ein einziger
 * fehlgeschlagener Aufruf über die Brücke nahm den Zeitgeber damit endgültig
 * mit. Auf einem Rahmen, der wochenlang läuft, ist das kein Randfall (NF-02).
 */

const SLIDE = { kind: 'single', id: 'a' } as Slide

/** Ein Takt in Sekunden — kurz, damit die Zeitgeber im Test überschaubar sind. */
const TAKT = 1

beforeEach(() => {
  setActivePinia(createPinia())
  vi.restoreAllMocks()
  vi.useFakeTimers()
})

afterEach(() => {
  vi.useRealTimers()
})

/** Startet den Store mit einer Brücke, die nur das Nötigste beantwortet. */
async function starte() {
  const store = useSlideshowStore()
  vi.spyOn(api, 'currentSlide').mockResolvedValue(SLIDE)
  vi.spyOn(api, 'isPlaying').mockResolvedValue(true)
  vi.spyOn(api, 'imageInfo').mockResolvedValue(null)
  vi.spyOn(api, 'prefetchWindow').mockResolvedValue([])
  await store.start(
    () => TAKT,
    () => true,
  )
  return store
}

describe('slideshowStore: Taktgeber', () => {
  it('laeuft nach einem fehlgeschlagenen Bildwechsel weiter', async () => {
    const next = vi
      .spyOn(api, 'nextSlide')
      .mockRejectedValueOnce(new Error('Brücke weg'))
      .mockResolvedValue(SLIDE)
    vi.spyOn(console, 'error').mockImplementation(() => {})

    const store = await starte()

    await vi.advanceTimersByTimeAsync(TAKT * 1000)
    expect(next).toHaveBeenCalledTimes(1)

    // Der entscheidende Takt: ohne das Auffangen in `restartTimer` bliebe es
    // für immer bei diesem einen Aufruf.
    await vi.advanceTimersByTimeAsync(TAKT * 1000)
    expect(next).toHaveBeenCalledTimes(2)

    store.dispose()
  })

  it('taktet weiter, ohne zu schalten, solange pausiert ist', async () => {
    const next = vi.spyOn(api, 'nextSlide').mockResolvedValue(SLIDE)
    const store = await starte()
    vi.spyOn(api, 'setPlaying').mockResolvedValue()
    await store.togglePlaying()

    await vi.advanceTimersByTimeAsync(TAKT * 3000)
    expect(next).not.toHaveBeenCalled()

    // Nach dem Fortsetzen muss der Takt sofort wieder greifen und nicht erst
    // nach einem vollen Zyklus Anlauf brauchen.
    await store.togglePlaying()
    await vi.advanceTimersByTimeAsync(TAKT * 1000)
    expect(next).toHaveBeenCalledTimes(1)

    store.dispose()
  })

  it('nimmt einen Fehler beim Nachladen der Metadaten nicht mit', async () => {
    // `refreshInfo` läuft über die Brücke und kann genauso scheitern. Stünde
    // es vor `restartTimer`, hinge daran wieder der ganze Takt.
    const next = vi.spyOn(api, 'nextSlide').mockResolvedValue(SLIDE)
    const store = await starte()
    vi.spyOn(api, 'imageInfo').mockRejectedValue(new Error('Index gesperrt'))
    vi.spyOn(console, 'error').mockImplementation(() => {})

    await vi.advanceTimersByTimeAsync(TAKT * 1000)
    await vi.advanceTimersByTimeAsync(TAKT * 1000)
    expect(next).toHaveBeenCalledTimes(2)

    store.dispose()
  })
})

/**
 * Livebild eines Kamerastroms (E-67).
 *
 * Das Backend meldet je Einzelbild nur Id und Nummer; die Bühne lädt das Bild
 * selbst. Der Store reicht die Nummer nur weiter, solange genau dieser Strom
 * an der Wand hängt.
 */
describe('slideshowStore: Livebild', () => {
  /** Der zuletzt angemeldete Zuhörer für ein Ereignis. */
  function zuhoerer<T>(name: string) {
    const treffer = vi.mocked(listen).mock.calls.filter((c) => c[0] === name).pop()
    if (!treffer) throw new Error(`Kein Zuhoerer fuer ${name}`)
    return treffer[1] as (e: { payload: T }) => void
  }

  const LIVE = { kind: 'single', id: 'x_1' } as Slide

  async function mitLivebild() {
    const store = await starte()
    // Home Assistant hat die Kamera geschickt: das Fremdbild haengt.
    zuhoerer<Slide | null>(EVENTS.slide)({ payload: LIVE })
    await vi.advanceTimersByTimeAsync(0)
    return store
  }

  it('reicht die Nummer des neuesten Einzelbilds weiter', async () => {
    const store = await mitLivebild()
    zuhoerer<LiveFrame>(EVENTS.liveFrame)({ payload: { id: 'x_1', frame: 4 } })
    expect(store.liveFrame).toEqual({ id: 'x_1', frame: 4 })
    zuhoerer<LiveFrame>(EVENTS.liveFrame)({ payload: { id: 'x_1', frame: 5 } })
    expect(store.liveFrame).toEqual({ id: 'x_1', frame: 5 })
    store.dispose()
  })

  it('schaltet dafuer nicht weiter und laedt keine Metadaten', async () => {
    // Fuenf Einzelbilder je Sekunde, jedes ein Aufruf ueber die Bruecke —
    // das waere die Last, die das Livebild gerade vermeiden soll.
    const store = await mitLivebild()
    const info = vi.mocked(api.imageInfo)
    const vorher = info.mock.calls.length
    zuhoerer<LiveFrame>(EVENTS.liveFrame)({ payload: { id: 'x_1', frame: 1 } })
    await vi.advanceTimersByTimeAsync(0)
    expect(info.mock.calls.length).toBe(vorher)
    expect(store.slide).toEqual(LIVE)
    store.dispose()
  })

  it('vergisst das Livebild, sobald wieder die Diashow laeuft', async () => {
    const store = await mitLivebild()
    zuhoerer<LiveFrame>(EVENTS.liveFrame)({ payload: { id: 'x_1', frame: 4 } })
    // Stop, Geste oder Zeitlimit: das Backend meldet das naechste Foto.
    zuhoerer<Slide | null>(EVENTS.slide)({ payload: SLIDE })
    await vi.advanceTimersByTimeAsync(0)
    expect(store.liveFrame).toBeNull()
    store.dispose()
  })

  it('ignoriert ein verspaetetes Einzelbild eines beendeten Stroms', async () => {
    const store = await starte()
    zuhoerer<LiveFrame>(EVENTS.liveFrame)({ payload: { id: 'x_1', frame: 9 } })
    expect(store.liveFrame).toBeNull()
    store.dispose()
  })
})

/**
 * Smart-Home-Ansicht (FA-46).
 *
 * Solange sie über der Diashow liegt, hält die Diashow an; danach läuft sie
 * weiter. Die Pause des Nutzers ist davon getrennt und darf nicht verloren
 * gehen — wer vorher angehalten hatte, findet danach ein angehaltenes Bild vor.
 */
describe('slideshowStore: Smart-Home-Ansicht', () => {
  function zuhoerer(name: string) {
    const treffer = vi.mocked(listen).mock.calls.filter((c) => c[0] === name).pop()
    if (!treffer) throw new Error(`Kein Zuhoerer fuer ${name}`)
    return treffer[1] as (e: { payload: unknown }) => void
  }

  it('schaltet nicht weiter, solange die Ansicht offen ist', async () => {
    const next = vi.spyOn(api, 'nextSlide').mockResolvedValue(SLIDE)
    vi.spyOn(api, 'openSmartHome').mockResolvedValue(true)
    const store = await starte()

    expect(await store.openSmartHome('Diashow')).toBe(true)
    expect(api.openSmartHome).toHaveBeenCalledWith('Diashow')
    expect(store.suspended).toBe(true)

    await vi.advanceTimersByTimeAsync(TAKT * 5000)
    expect(next).not.toHaveBeenCalled()
    store.dispose()
  })

  it('laeuft nach dem Schliessen mit voller Anzeigedauer weiter', async () => {
    const next = vi.spyOn(api, 'nextSlide').mockResolvedValue(SLIDE)
    vi.spyOn(api, 'openSmartHome').mockResolvedValue(true)
    const store = await starte()
    await store.openSmartHome('Diashow')

    zuhoerer(EVENTS.smartHomeClosed)({ payload: null })
    expect(store.suspended).toBe(false)

    await vi.advanceTimersByTimeAsync(TAKT * 1000 - 1)
    expect(next).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(1)
    expect(next).toHaveBeenCalledTimes(1)
    store.dispose()
  })

  it('laesst die Pause des Nutzers unangetastet', async () => {
    const next = vi.spyOn(api, 'nextSlide').mockResolvedValue(SLIDE)
    const setPlaying = vi.spyOn(api, 'setPlaying').mockResolvedValue()
    vi.spyOn(api, 'openSmartHome').mockResolvedValue(true)
    const store = await starte()
    await store.togglePlaying()
    setPlaying.mockClear()

    await store.openSmartHome('Diashow')
    zuhoerer(EVENTS.smartHomeClosed)({ payload: null })

    // Weder beim Oeffnen noch beim Schliessen ein Wort ans Backend: die
    // Pause bleibt die des Nutzers, und Home Assistant sieht keinen Wechsel.
    expect(setPlaying).not.toHaveBeenCalled()
    expect(store.playing).toBe(false)
    await vi.advanceTimersByTimeAsync(TAKT * 3000)
    expect(next).not.toHaveBeenCalled()
    store.dispose()
  })

  it('haelt nicht an, wenn das Backend nicht oeffnet', async () => {
    // Nachts (E-79), ohne Adresse oder auf dem Desktop: der Aufrufer schaltet
    // dann die Pause um, und die Diashow darf nicht zusaetzlich stehen.
    vi.spyOn(api, 'openSmartHome').mockResolvedValue(false)
    const store = await starte()
    expect(await store.openSmartHome('Diashow')).toBe(false)
    expect(store.suspended).toBe(false)
    store.dispose()
  })

  it('wirft nicht, wenn die Bruecke scheitert', async () => {
    vi.spyOn(api, 'openSmartHome').mockRejectedValue(new Error('Brücke weg'))
    vi.spyOn(console, 'error').mockImplementation(() => {})
    const store = await starte()
    expect(await store.openSmartHome('Diashow')).toBe(false)
    expect(store.suspended).toBe(false)
    store.dispose()
  })

  it('laeuft weiter, wenn das Schliessen-Ereignis verloren ging', async () => {
    // Die Oberflaeche ruft `resumeFromSmartHome` bei jeder Beruehrung, die
    // die Diashow erreicht — dann liegt die Ansicht nicht mehr darueber.
    const next = vi.spyOn(api, 'nextSlide').mockResolvedValue(SLIDE)
    vi.spyOn(api, 'openSmartHome').mockResolvedValue(true)
    const store = await starte()
    await store.openSmartHome('Diashow')

    store.resumeFromSmartHome()
    expect(store.suspended).toBe(false)
    await vi.advanceTimersByTimeAsync(TAKT * 1000)
    expect(next).toHaveBeenCalledTimes(1)
    store.dispose()
  })
})

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useSlideshowStore } from './slideshow'
import * as api from '@/lib/api'
import type { Slide } from '@/lib/types'

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

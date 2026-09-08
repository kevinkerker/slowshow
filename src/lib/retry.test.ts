import { describe, expect, it, vi } from 'vitest'
import { BOOT_RETRY_DELAYS_MS, retryDelay, retryUntilResolved } from './retry'

/** Schlaf-Attrappe: merkt sich die Wartezeiten, wartet aber nicht. */
function fakeSleep() {
  const waited: number[] = []
  const sleep = vi.fn(async (ms: number) => {
    waited.push(ms)
  })
  return { sleep, waited }
}

describe('retryDelay', () => {
  it('folgt der Liste und bleibt danach beim letzten Wert', () => {
    const delays = [300, 1000, 2000, 5000]
    expect(retryDelay(delays, 1)).toBe(300)
    expect(retryDelay(delays, 2)).toBe(1000)
    expect(retryDelay(delays, 4)).toBe(5000)
    // Ab hier waechst nichts mehr: ein Backend, das Minuten braucht, wird
    // alle fuenf Sekunden gefragt, nicht seltener.
    expect(retryDelay(delays, 5)).toBe(5000)
    expect(retryDelay(delays, 50)).toBe(5000)
  })

  it('wartet ohne Liste gar nicht und klemmt unsinnige Nummern', () => {
    expect(retryDelay([], 3)).toBe(0)
    expect(retryDelay([700], 0)).toBe(700)
    expect(retryDelay([700], -4)).toBe(700)
  })

  it('startet beim App-Start schnell und deckelt bei fuenf Sekunden (E-54)', () => {
    // Der erste Fehlschlag ist meist nur der Wimpernschlag, in dem Rust den
    // Index noch liest — deshalb sofort wieder, nicht erst nach Sekunden.
    expect(BOOT_RETRY_DELAYS_MS[0]).toBeLessThanOrEqual(500)
    expect(Math.max(...BOOT_RETRY_DELAYS_MS)).toBe(5000)
  })
})

describe('retryUntilResolved', () => {
  it('gibt beim ersten Erfolg sofort zurueck, ohne zu warten', async () => {
    const { sleep, waited } = fakeSleep()
    const task = vi.fn().mockResolvedValue('ok')

    await expect(retryUntilResolved(task, { sleep })).resolves.toBe('ok')

    expect(task).toHaveBeenCalledTimes(1)
    expect(waited).toEqual([])
  })

  it('wiederholt nach den Wartezeiten, bis es gelingt', async () => {
    const { sleep, waited } = fakeSleep()
    const task = vi
      .fn()
      .mockRejectedValueOnce(new Error('state not managed'))
      .mockRejectedValueOnce(new Error('state not managed'))
      .mockRejectedValueOnce(new Error('state not managed'))
      .mockResolvedValue({ intervalSeconds: 60 })

    const result = await retryUntilResolved(task, { sleep, delaysMs: [300, 1000, 2000, 5000] })

    expect(result).toEqual({ intervalSeconds: 60 })
    expect(task).toHaveBeenCalledTimes(4)
    expect(waited).toEqual([300, 1000, 2000])
  })

  it('meldet jeden Fehlschlag mit seiner Nummer (fuer das Log)', async () => {
    const { sleep } = fakeSleep()
    const onError = vi.fn()
    const first = new Error('eins')
    const second = new Error('zwei')
    const task = vi.fn().mockRejectedValueOnce(first).mockRejectedValueOnce(second).mockResolvedValue(1)

    await retryUntilResolved(task, { sleep, onError })

    expect(onError).toHaveBeenCalledTimes(2)
    expect(onError).toHaveBeenNthCalledWith(1, first, 1)
    expect(onError).toHaveBeenNthCalledWith(2, second, 2)
  })

  it('gibt nach der Obergrenze den letzten Fehler weiter', async () => {
    const { sleep, waited } = fakeSleep()
    const task = vi.fn().mockRejectedValue(new Error('dauerhaft'))

    await expect(
      retryUntilResolved(task, { sleep, maxAttempts: 3, delaysMs: [10, 20] }),
    ).rejects.toThrow('dauerhaft')

    expect(task).toHaveBeenCalledTimes(3)
    // Nach dem letzten erlaubten Versuch wird nicht mehr gewartet.
    expect(waited).toEqual([10, 20])
  })

  it('wiederholt ohne Obergrenze auch weit ueber die Liste hinaus', async () => {
    const { sleep, waited } = fakeSleep()
    let calls = 0
    const task = vi.fn(async () => {
      calls++
      if (calls < 12) throw new Error('noch nicht')
      return 'endlich'
    })

    await expect(retryUntilResolved(task, { sleep, delaysMs: [1, 2] })).resolves.toBe('endlich')

    expect(task).toHaveBeenCalledTimes(12)
    expect(waited).toEqual([1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2])
  })
})

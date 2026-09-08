/**
 * Wiederholen, bis es gelingt (NF-01, NF-02).
 *
 * Anlass (E-54): Beim Start rief das Frontend `get_config`, während Rust den
 * Zustand noch lud. Der Aufruf kam mit „state not managed" zurück, und weil
 * nichts ihn wiederholte, blieb der Rahmen dunkel — bis jemand die App von
 * Hand neu startete. Für einen Bilderrahmen ist ein einzelner gescheiterter
 * Aufruf kein Grund aufzugeben: er versucht es mit wachsendem Abstand wieder,
 * und zwar ohne Obergrenze, denn die Alternative wäre ein schwarzer Schirm
 * ohne jede Aussicht auf Besserung.
 *
 * Als reine Funktion mit austauschbarem `sleep`, damit die Abfolge der
 * Wartezeiten ohne echte Uhr prüfbar ist.
 */

export interface RetryOptions {
  /**
   * Wartezeiten vor dem zweiten, dritten, … Versuch. Der letzte Wert gilt für
   * alle weiteren — die Abstände wachsen also bis zu einer Obergrenze.
   */
  delaysMs?: readonly number[]
  /**
   * Wird nach jedem gescheiterten Versuch gerufen, mit dem Fehler und der
   * Nummer des Versuchs (ab 1). Gedacht fürs Log: ein Rahmen, der sich still
   * wieder fängt, verrät sonst nie, dass etwas war.
   */
  onError?: (error: unknown, attempt: number) => void
  /** Obergrenze der Versuche. Ohne Angabe unbegrenzt. */
  maxAttempts?: number
  /** Warten — im Test durch eine Attrappe ersetzbar. */
  sleep?: (ms: number) => Promise<void>
}

/**
 * Abstände für den App-Start: schnell wieder, wenn es nur ein Wimpernschlag
 * war, aber höchstens alle fünf Sekunden, wenn das Backend länger braucht.
 */
export const BOOT_RETRY_DELAYS_MS: readonly number[] = [300, 1000, 2000, 5000]

/** Wartezeit nach dem `attempt`-ten Fehlschlag (ab 1); der letzte Wert gilt weiter. */
export function retryDelay(delaysMs: readonly number[], attempt: number): number {
  if (delaysMs.length === 0) return 0
  const index = Math.min(Math.max(attempt, 1), delaysMs.length) - 1
  return delaysMs[index]
}

function defaultSleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

/**
 * Führt `task` aus und wiederholt bei einem Fehler nach den Wartezeiten aus
 * `delaysMs`, bis ein Versuch gelingt oder `maxAttempts` erreicht ist. Dann
 * wird der letzte Fehler weitergegeben.
 */
export async function retryUntilResolved<T>(
  task: () => Promise<T>,
  options: RetryOptions = {},
): Promise<T> {
  const delays = options.delaysMs ?? BOOT_RETRY_DELAYS_MS
  const sleep = options.sleep ?? defaultSleep

  for (let attempt = 1; ; attempt++) {
    try {
      return await task()
    } catch (error) {
      options.onError?.(error, attempt)
      if (options.maxAttempts !== undefined && attempt >= options.maxAttempts) {
        throw error
      }
      await sleep(retryDelay(delays, attempt))
    }
  }
}

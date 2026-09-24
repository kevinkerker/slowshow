/**
 * Rückmeldung neben ihrem Auslöser (E-62, Vorlage `Vorschlag-Rueckmeldung`).
 *
 * Eine Regel für alle Bereiche:
 *  - Erfolg verschwindet nach `--ss-feedback-duration` (6 s) von selbst.
 *  - Ein Fehler bleibt stehen, bis ein Versuch gelingt oder er ersetzt wird.
 *
 * Vorher hatte jeder Bereich seine eigene Fassung: Quellen 6 s gedimmt,
 * Diashow 6 s in Messing, System 5 s gedimmt — und im System stand obendrein
 * jede Meldung ganz unten, auch die zum MQTT-Passwort weit darüber.
 *
 * Bewusst ohne Vue-Lebenszyklus nutzbar: die Quellenliste legt je Quelle eine
 * eigene Rückmeldung an, und zwar erst beim ersten Abgleich, also außerhalb
 * von `setup`. Innerhalb eines Effektbereichs räumt sich der Zeitgeber selbst
 * weg; außerhalb ruft der Besitzer `dispose()`.
 */
import { getCurrentScope, onScopeDispose, readonly, ref } from 'vue'

export type FeedbackKind = 'ok' | 'error'

export interface FeedbackState {
  kind: FeedbackKind
  text: string
  /** Wechselt mit jeder Meldung — auch dieselbe Meldung zweimal ist zweimal neu. */
  id: number
}

/** Entspricht `--ss-feedback-duration` in `tokens.css`. */
export const FEEDBACK_DURATION_MS = 6000

let nextId = 0

export function useFeedback(durationMs = FEEDBACK_DURATION_MS) {
  const state = ref<FeedbackState | null>(null)
  let timer: ReturnType<typeof setTimeout> | undefined

  function stopTimer() {
    if (timer !== undefined) clearTimeout(timer)
    timer = undefined
  }

  function show(kind: FeedbackKind, text: string) {
    stopTimer()
    if (!text) {
      state.value = null
      return
    }
    state.value = { kind, text, id: ++nextId }
    if (kind === 'ok') {
      timer = setTimeout(() => {
        state.value = null
        timer = undefined
      }, durationMs)
    }
  }

  const ok = (text: string) => show('ok', text)
  const error = (text: string) => show('error', text)

  function clear() {
    stopTimer()
    state.value = null
  }

  if (getCurrentScope()) onScopeDispose(stopTimer)

  return { feedback: readonly(state), ok, error, clear, dispose: stopTimer }
}

export type Feedback = ReturnType<typeof useFeedback>

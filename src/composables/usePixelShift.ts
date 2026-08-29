import { computed, onBeforeUnmount, ref, type ComputedRef, type Ref } from 'vue'

/** Wie oft die Einblendungen ihre Position wechseln. */
const SHIFT_INTERVAL_MS = 90_000
/** Maximale Auslenkung in Pixeln. */
const AMPLITUDE = 8

/**
 * Einbrennschutz für statische Einblendungen (NF-07).
 *
 * Uhr und Bildunterschrift stehen im Dauerbetrieb monatelang an derselben
 * Stelle — auf einem OLED-Display brennt genau das ein. Die Positionen wandern
 * deshalb langsam über ein kleines Rechteck.
 *
 * Bewusst über `transform` und nicht über `left`/`bottom`: eine Verschiebung
 * per `transform` löst kein Layout aus und stört die Überblendung nicht (NF-16).
 *
 * Die Schrittfolge ist ein festes Muster statt Zufall — so ist die Abdeckung
 * gleichmäßig, und im Test lässt sie sich prüfen.
 */
export function usePixelShift(enabled: Ref<boolean> | ComputedRef<boolean>) {
  const step = ref(0)
  const timer = setInterval(() => {
    step.value = (step.value + 1) % SHIFT_STEPS.length
  }, SHIFT_INTERVAL_MS)

  onBeforeUnmount(() => clearInterval(timer))

  const transform = computed(() => {
    if (!enabled.value) return 'none'
    const [x, y] = SHIFT_STEPS[step.value]
    return `translate3d(${x * AMPLITUDE}px, ${y * AMPLITUDE}px, 0)`
  })

  return { transform, step }
}

/**
 * Positionen als Vielfache der Amplitude.
 * Läuft ein Rechteck ab und geht durch die Mitte zurück — dadurch verteilt
 * sich die Belastung gleichmäßig auf die Fläche.
 */
export const SHIFT_STEPS: ReadonlyArray<readonly [number, number]> = [
  [0, 0],
  [1, 0],
  [1, -1],
  [0, -1],
  [-1, -1],
  [-1, 0],
  [-1, 1],
  [0, 1],
  [1, 1],
]

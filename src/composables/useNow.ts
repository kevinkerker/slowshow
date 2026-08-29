import { onBeforeUnmount, ref, type Ref } from 'vue'

/**
 * Aktuelle Uhrzeit als reaktiver Wert.
 *
 * Taktet auf die volle Minute statt in festen Abständen: eine Uhr, die die
 * Minute erst zwanzig Sekunden zu spät weiterschaltet, fällt auf einem
 * Bilderrahmen auf. Zwischen den Minuten schläft der Zeitgeber — im
 * Dauerbetrieb zählt jede eingesparte Aufwachphase (NF-06).
 */
export function useNow(): Ref<Date> {
  const now = ref(new Date())
  let timer: ReturnType<typeof setTimeout> | null = null

  function scheduleNextMinute() {
    const current = new Date()
    now.value = current
    const msToNextMinute = 60_000 - (current.getSeconds() * 1000 + current.getMilliseconds())
    timer = setTimeout(scheduleNextMinute, msToNextMinute + 50)
  }

  scheduleNextMinute()

  onBeforeUnmount(() => {
    if (timer) clearTimeout(timer)
  })

  return now
}

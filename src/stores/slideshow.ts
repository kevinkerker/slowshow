/**
 * Ablauf der Diashow: Taktgeber, Vorausladen und Metadaten des aktuellen Bildes.
 *
 * Das Vorausladen (FA-31) gehört bewusst hierher und nicht ins Backend: nur die
 * WebView selbst kann ihren eigenen Bilddekoder vorwärmen. Das Backend liefert
 * lediglich die Liste der als Nächstes fälligen Ids und garantiert, dass deren
 * Dateien im Cache liegen und displaygerecht sind (NF-12).
 */

import { defineStore } from 'pinia'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { computed, ref } from 'vue'
import * as api from '@/lib/api'
import { EVENTS, slideIds, type CacheEntry, type Slide } from '@/lib/types'

export const useSlideshowStore = defineStore('slideshow', () => {
  const slide = ref<Slide | null>(null)
  const playing = ref(true)
  /** Metadaten der gerade sichtbaren Bilder — für die Einblendungen (FA-07). */
  const info = ref<Record<string, CacheEntry>>({})
  /** Anzahl Bilder in der Playlist; 0 heißt „noch keine Quelle eingerichtet". */
  const hasImages = ref(false)

  let timer: ReturnType<typeof setTimeout> | null = null
  let unlisten: UnlistenFn[] = []

  /**
   * Taktparameter aus `start()`, damit auch `next()` und `prev()` den
   * Zeitgeber neu setzen können.
   *
   * Ohne das liefe nach einer Wischgeste der alte Zeitgeber weiter: wer eine
   * Sekunde vor dem regulären Wechsel weiterwischt, bekäme eine Sekunde später
   * gleich das übernächste Bild (FA-41 gegen FA-02).
   */
  let intervalGetter: () => number = () => 30
  let activeGetter: () => boolean = () => true

  /**
   * Vorgeladene Bilder festhalten.
   *
   * Ohne diese Referenzen dürfte die WebView die dekodierten Bilder sofort
   * wieder verwerfen — dann wäre das Vorausladen wirkungslos. Die Zahl ist
   * durch `prefetchCount` begrenzt (NF-03, R-03).
   */
  const preloaded = new Map<string, HTMLImageElement>()

  const currentIds = computed(() => slideIds(slide.value))

  async function start(intervalSeconds: () => number, active: () => boolean) {
    intervalGetter = intervalSeconds
    activeGetter = active

    slide.value = await api.currentSlide()
    playing.value = await api.isPlaying()
    await refreshInfo()

    unlisten.push(
      await listen<Slide | null>(EVENTS.slide, async (e) => {
        slide.value = e.payload
        // Vor dem Nachladen der Metadaten: die Anzeigedauer läuft ab dem
        // Bildwechsel, und ein Fehler beim Nachladen darf den Takt nicht
        // mitnehmen.
        restartTimer()
        await refreshInfo()
      }),
    )
    // Waehrend eines Syncs: sobald das erste Bild im Cache liegt, anzeigen,
    // statt bis zum Ende des Laufs zu warten.
    unlisten.push(
      await listen(EVENTS.syncProgress, async () => {
        if (slide.value) return
        slide.value = await api.currentSlide()
        if (slide.value) {
          await refreshInfo()
          await prefetch()
          restartTimer()
        }
      }),
    )
    // Nach einem Sync können neue Bilder dazugekommen sein (FA-28).
    unlisten.push(
      await listen(EVENTS.sync, async () => {
        if (!slide.value) {
          slide.value = await api.currentSlide()
          await refreshInfo()
          restartTimer()
        }
        await prefetch()
      }),
    )

    await prefetch()
    restartTimer()
  }

  function dispose() {
    if (timer) clearTimeout(timer)
    timer = null
    unlisten.forEach((fn) => fn())
    unlisten = []
    preloaded.clear()
  }

  /**
   * Setzt den Taktgeber neu.
   *
   * `setTimeout` statt `setInterval`: eine geänderte Anzeigedauer greift so
   * beim nächsten Wechsel, und es kann sich kein Rückstand aufstauen, wenn ein
   * Bildwechsel einmal länger dauert.
   */
  function restartTimer() {
    if (timer) clearTimeout(timer)
    timer = setTimeout(
      async () => {
        if (!playing.value || !activeGetter()) {
          // Pausiert oder außerhalb der Aktivzeit: weiter takten, aber nicht
          // weiterschalten — sonst müsste beim Fortsetzen erst ein voller
          // Zyklus abgewartet werden.
          restartTimer()
          return
        }
        try {
          await next()
        } catch (e) {
          // Ohne dieses Auffangen nimmt ein einzelner fehlgeschlagener Aufruf
          // den Taktgeber mit: `next` setzt ihn erst am Ende neu, und die
          // Diashow stünde ab da endgültig still. Auf einem Rahmen, der
          // wochenlang läuft, ist ein Fehlschlag kein Randfall (NF-02) —
          // aussehen würde es wie ein hängender Rahmen.
          console.error('Bildwechsel fehlgeschlagen', e)
          restartTimer()
        }
      },
      Math.max(1, intervalGetter()) * 1000,
    )
  }

  async function next() {
    slide.value = await api.nextSlide()
    await refreshInfo()
    // Nach einem Wechsel von Hand beginnt die volle Anzeigedauer neu.
    restartTimer()
    await prefetch()
  }

  async function prev() {
    slide.value = await api.prevSlide()
    await refreshInfo()
    restartTimer()
    await prefetch()
  }

  async function togglePlaying() {
    playing.value = !playing.value
    await api.setPlaying(playing.value)
  }

  /** Aktuelles Bild aus der Diashow nehmen (FA-30). */
  async function excludeCurrent(): Promise<boolean> {
    const ids = currentIds.value
    if (ids.length === 0) return false
    await api.excludeImage(ids[0])
    return true
  }

  async function refreshInfo() {
    const ids = currentIds.value
    hasImages.value = ids.length > 0
    if (ids.length === 0) {
      info.value = {}
      return
    }
    const entries = await Promise.all(ids.map((id) => api.imageInfo(id)))
    const next: Record<string, CacheEntry> = {}
    entries.forEach((entry) => {
      if (entry) next[entry.id] = entry
    })
    info.value = next
  }

  /**
   * Lädt die nächsten Bilder und lässt sie dekodieren (FA-31).
   *
   * `decode()` ist der entscheidende Teil: ohne den Aufruf hätte die WebView
   * die Datei zwar geholt, würde sie aber erst beim Einblenden dekodieren —
   * und genau das erzeugt die sichtbare Ladepause, die NF-03 ausschließt.
   */
  async function prefetch() {
    let ids: string[]
    try {
      ids = await api.prefetchWindow()
    } catch {
      return
    }
    if (ids.length === 0) {
      preloaded.clear()
      return
    }

    // Was nicht mehr im Fenster liegt, darf die WebView vergessen.
    const wanted = new Set(ids)
    for (const id of [...preloaded.keys()]) {
      if (!wanted.has(id)) preloaded.delete(id)
    }

    await Promise.all(
      ids.map(async (id) => {
        if (preloaded.has(id)) return
        const img = new Image()
        img.src = api.imageUrl(id)
        try {
          await img.decode()
          preloaded.set(id, img)
        } catch {
          // Ein fehlendes Bild ist kein Grund anzuhalten — der Ringpuffer kann
          // es zwischenzeitlich verdrängt haben (FA-27).
        }
      }),
    )
  }

  return {
    slide,
    playing,
    info,
    hasImages,
    currentIds,
    start,
    dispose,
    next,
    prev,
    togglePlaying,
    excludeCurrent,
    prefetch,
    restartTimer,
  }
})

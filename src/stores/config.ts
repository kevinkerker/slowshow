/**
 * Konfiguration, Quellen und Cache-Kennzahlen.
 *
 * Die Wahrheit liegt im Backend — dieser Store spiegelt sie nur. Jede Änderung
 * geht über ein Kommando und kommt als Ergebnis zurück; damit gilt immer der
 * vom Backend geklemmte Wert (FA-02, FA-42) und nie eine Annahme der Oberfläche.
 */

import { defineStore } from 'pinia'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { ref, computed } from 'vue'
import * as api from '@/lib/api'
import { applyLanguage } from '@/lib/i18n'
import {
  EVENTS,
  type AppConfig,
  type CacheStats,
  type DisplayState,
  type Source,
  type SyncProgress,
  type SyncReport,
} from '@/lib/types'

export const useConfigStore = defineStore('config', () => {
  const config = ref<AppConfig | null>(null)
  const stats = ref<CacheStats | null>(null)
  const counts = ref<Record<string, number>>({})
  const display = ref<DisplayState | null>(null)
  /** Quellen, deren Abgleich diese Ansicht selbst angestoßen hat (E-43). */
  const syncing = ref<Set<string>>(new Set())
  const lastReport = ref<SyncReport | null>(null)
  /**
   * Zwischenstand je Quelle — treibt die Anzeige in der Quellenliste.
   *
   * Nach Quelle geschlüsselt, weil seit E-43 zwei Quellen gleichzeitig laufen
   * können. Ein einzelner Wert zeigte sonst abwechselnd die eine und die
   * andere, und beide Fortschrittsbalken zuckten durcheinander.
   */
  const progress = ref<Record<string, SyncProgress>>({})
  const error = ref<string | null>(null)

  const sources = computed(() => config.value?.sources ?? [])

  /**
   * Wird diese Quelle gerade synchronisiert — egal ob von Hand angestoßen oder
   * vom Zeitgeber im Backend (FA-28)?
   *
   * `syncing` kennt nur die selbst ausgelösten Läufe. Ohne den Rückfall auf den
   * Fortschritt liefe ein Hintergrund-Sync für den Nutzer unsichtbar, während
   * sich die Bilderzahl scheinbar von selbst ändert.
   */
  function isSyncing(id: string): boolean {
    return syncing.value.has(id) || progress.value[id] !== undefined
  }

  /** Zwischenstand dieser Quelle, falls einer vorliegt. */
  function progressFor(id: string): SyncProgress | null {
    return progress.value[id] ?? null
  }

  const ready = computed(() => config.value !== null)

  let unlisten: UnlistenFn[] = []

  async function load() {
    config.value = await api.getConfig()
    applyLanguage(config.value.language)
    display.value = await api.getDisplayState()
    await refreshStats()

    unlisten.push(
      await listen<AppConfig>(EVENTS.config, (e) => {
        config.value = e.payload
        applyLanguage(e.payload.language)
      }),
    )
    unlisten.push(
      await listen<DisplayState>(EVENTS.display, (e) => {
        display.value = e.payload
      }),
    )
    unlisten.push(
      await listen<SyncReport>(EVENTS.sync, (e) => {
        lastReport.value = e.payload
        // Nur den Zwischenstand dieser Quelle wegräumen: eine zweite kann
        // weiterlaufen, und ihr Balken darf davon nicht verschwinden (E-43).
        delete progress.value[e.payload.sourceId]
        void refreshStats()
      }),
    )
    unlisten.push(
      await listen<SyncProgress>(EVENTS.syncProgress, (e) => {
        progress.value[e.payload.sourceId] = e.payload
      }),
    )
  }

  function dispose() {
    unlisten.forEach((fn) => fn())
    unlisten = []
  }

  async function refreshStats() {
    stats.value = await api.cacheStats()
    counts.value = await api.sourceCounts()
  }

  /**
   * Ändert die Konfiguration.
   *
   * Der Rückgabewert des Backends ersetzt den lokalen Zustand — steht dort ein
   * geklemmter Wert, zeigt die Oberfläche sofort diesen an.
   */
  async function patch(change: (draft: AppConfig) => void) {
    if (!config.value) return
    const draft: AppConfig = JSON.parse(JSON.stringify(config.value))
    change(draft)

    // Vorher merken, um danach zu erkennen, ob die Cache-Grenze sich geaendert
    // hat. Am Geraet gemeldet: die maximale Cachegroesse liess sich unter
    // „System" umstellen, in der Fusszeile der Quellenliste stand aber weiter
    // der alte Wert — die Statistik traegt `maxBytes`, und `patch` hat sie nie
    // erneuert. `addSource` und `removeSource` taten es, deshalb sprang die
    // Zahl irgendwann doch um und der Fehler sah nach Zufall aus.
    const vorher = config.value.cache.maxBytes

    config.value = await api.setConfig(draft)
    applyLanguage(config.value.language)
    display.value = await api.getDisplayState()

    // Nur bei Bedarf: `cacheStats` laeuft ueber den ganzen Index, und `patch`
    // feuert bei jedem Schalter.
    if (config.value.cache.maxBytes !== vorher) {
      await refreshStats()
    }
  }

  async function addSource(source: Source, password?: string) {
    config.value = await api.addSource(source, password)
    await refreshStats()
  }

  async function updateSource(source: Source, password?: string) {
    config.value = await api.updateSource(source, password)
    await refreshStats()
  }

  async function removeSource(id: string) {
    config.value = await api.removeSource(id)
    await refreshStats()
  }

  /**
   * Synchronisiert eine Quelle (FA-28).
   *
   * Alle Quellenarten laufen über dasselbe Kommando — auch lokale Ordner, seit
   * der SAF-Durchlauf im Rust-Backend liegt. Das Frontend kennt den Unterschied
   * nicht mehr und bekommt für alle den gleichen Fortschritt.
   */
  async function syncSource(id: string): Promise<SyncReport | null> {
    const source = sources.value.find((s) => s.id === id)
    // Nur diese eine Quelle darf nicht doppelt laufen. Vorher blockierte jeder
    // laufende Abgleich jeden anderen — auch den einer ganz anderen Quelle
    // (E-43).
    if (!source || syncing.value.has(id)) return null

    syncing.value.add(id)
    delete progress.value[id]
    error.value = null
    try {
      const reports = await api.syncNow(id)
      lastReport.value = reports[0] ?? null
      return lastReport.value
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      return null
    } finally {
      syncing.value.delete(id)
      delete progress.value[id]
      await refreshStats()
    }
  }

  return {
    config,
    stats,
    counts,
    display,
    syncing,
    lastReport,
    progress,
    error,
    sources,
    isSyncing,
    progressFor,
    ready,
    load,
    dispose,
    refreshStats,
    patch,
    addSource,
    updateSource,
    removeSource,
    syncSource,
  }
})

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
  const syncing = ref<string | null>(null)
  const lastReport = ref<SyncReport | null>(null)
  /** Zwischenstand des laufenden Syncs — treibt die Anzeige in der Quellenliste. */
  const progress = ref<SyncProgress | null>(null)
  const error = ref<string | null>(null)

  const sources = computed(() => config.value?.sources ?? [])

  /**
   * Welche Quelle gerade synchronisiert wird — egal ob von Hand angestoßen
   * oder vom Zeitgeber im Backend (FA-28).
   *
   * `syncing` kennt nur die selbst ausgelösten Läufe. Ohne den Rückfall auf den
   * Fortschritt liefe ein Hintergrund-Sync für den Nutzer unsichtbar, während
   * sich die Bilderzahl scheinbar von selbst ändert.
   */
  const activeSyncSourceId = computed(() => syncing.value ?? progress.value?.sourceId ?? null)
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
        progress.value = null
        void refreshStats()
      }),
    )
    unlisten.push(
      await listen<SyncProgress>(EVENTS.syncProgress, (e) => {
        progress.value = e.payload
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
    config.value = await api.setConfig(draft)
    applyLanguage(config.value.language)
    display.value = await api.getDisplayState()
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
    if (!source || syncing.value) return null

    syncing.value = id
    progress.value = null
    error.value = null
    try {
      const reports = await api.syncNow(id)
      lastReport.value = reports[0] ?? null
      return lastReport.value
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      return null
    } finally {
      syncing.value = null
      progress.value = null
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
    activeSyncSourceId,
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

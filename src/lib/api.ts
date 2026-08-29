/**
 * Typisierte Hülle um die Tauri-Kommandos.
 *
 * Einziger Ort im Frontend, an dem `invoke` vorkommt. Das hält die Grenze zum
 * Backend an einer Stelle nachvollziehbar und erleichtert das Nachziehen, wenn
 * sich `src-tauri/src/commands.rs` ändert.
 */

import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import type {
  Album,
  AppConfig,
  CacheEntry,
  CacheStats,
  DisplayState,
  MqttStatus,
  Slide,
  Source,
  SyncReport,
} from './types'

/**
 * URL eines Bildes aus dem Cache.
 *
 * Bilder laufen ausschließlich über dieses Protokoll — nie über IPC. Ein
 * 5-MB-Foto als JSON-Array durch die Brücke zu schieben würde den
 * WebView-Speicher sprengen (R-03, NF-13).
 */
export function imageUrl(id: string): string {
  return convertFileSrc(id, 'slowshow')
}

// ── Konfiguration ────────────────────────────────────────────────────────────

export const getConfig = (): Promise<AppConfig> => invoke('get_config')

export const setConfig = (config: AppConfig): Promise<AppConfig> =>
  invoke('set_config', { config })

export const exportConfig = (): Promise<string> => invoke('export_config')

export const importConfig = (json: string): Promise<AppConfig> =>
  invoke('import_config', { json })

// ── Anzeige ──────────────────────────────────────────────────────────────────

export const getDisplayState = (): Promise<DisplayState> => invoke('get_display_state')

export const currentSlide = (): Promise<Slide | null> => invoke('current_slide')

export const nextSlide = (): Promise<Slide | null> => invoke('next_slide')

export const prevSlide = (): Promise<Slide | null> => invoke('prev_slide')

export const setPlaying = (playing: boolean): Promise<void> =>
  invoke('set_playing', { playing })

export const isPlaying = (): Promise<boolean> => invoke('is_playing')

/** Die als Nächstes anzuzeigenden Bild-Ids (FA-31). */
export const prefetchWindow = (): Promise<string[]> => invoke('prefetch_window')

/** Metadaten für die Einblendungen (FA-07). */
export const imageInfo = (id: string): Promise<CacheEntry | null> =>
  invoke('image_info', { id })

// ── Ausschlussliste (FA-30) ──────────────────────────────────────────────────

export const excludeImage = (id: string): Promise<void> => invoke('exclude_image', { id })

export const includeImage = (id: string): Promise<void> => invoke('include_image', { id })

export const excludedImages = (): Promise<CacheEntry[]> => invoke('excluded_images')

// ── Cache und Quellen ────────────────────────────────────────────────────────

export const cacheStats = (): Promise<CacheStats> => invoke('cache_stats')

export const sourceCounts = (): Promise<Record<string, number>> => invoke('source_counts')

export const addSource = (source: Source, password?: string): Promise<AppConfig> =>
  invoke('add_source', { source, password: password ?? null })

export const updateSource = (source: Source, password?: string): Promise<AppConfig> =>
  invoke('update_source', { source, password: password ?? null })

export const removeSource = (id: string): Promise<AppConfig> => invoke('remove_source', { id })

export const testSource = (source: Source, password: string): Promise<void> =>
  invoke('test_source', { source, password })

export const listNextcloudAlbums = (
  url: string,
  username: string,
  password: string,
  allowInsecureTls: boolean,
): Promise<Album[]> =>
  invoke('list_nextcloud_albums', { url, username, password, allowInsecureTls })

// ── MQTT (FA-55) ─────────────────────────────────────────────────────────────

/** Legt das Broker-Passwort verschlüsselt ab. Leer entfernt es. */
export const setMqttPassword = (password: string): Promise<void> =>
  invoke('set_mqtt_password', { password })

export const hasMqttPassword = (): Promise<boolean> => invoke('has_mqtt_password')

export const mqttStatus = (): Promise<MqttStatus> => invoke('mqtt_status')

/** Verbindet neu — nach einer korrigierten Adresse. */
export const mqttReconnect = (): Promise<MqttStatus> => invoke('mqtt_reconnect')

// ── Synchronisierung ─────────────────────────────────────────────────────────

export const syncNow = (sourceId?: string): Promise<SyncReport[]> =>
  invoke('sync_now', { sourceId: sourceId ?? null })

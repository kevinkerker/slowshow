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
  ImageFilter,
  ImagePage,
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

/**
 * Präfix, an dem das Rust-Backend ein Vorschaubild erkennt (E-25).
 *
 * Muss mit `THUMB_PREFIX` in `src-tauri/src/lib.rs` übereinstimmen — die
 * Grenze prüft kein Compiler, deshalb steht sie in `api.test.ts`.
 */
export const THUMB_PREFIX = 't_'

/**
 * URL des Vorschaubilds (E-25).
 *
 * Ein Präfix statt eines eigenen Pfadsegments: `convertFileSrc` steckt die Id
 * per `encodeURIComponent` in *ein* Segment, aus `thumb/<id>` würde also
 * `thumb%2F<id>`. Buchstabe und Unterstrich überstehen die Kodierung
 * unverändert.
 */
export function thumbUrl(id: string): string {
  return convertFileSrc(THUMB_PREFIX + id, 'slowshow')
}

// ── Konfiguration ────────────────────────────────────────────────────────────

/** Ein Ausschnitt des Cache-Index für den Bild-Browser (E-25). */
export const imagePage = (
  offset: number,
  limit: number,
  filter: ImageFilter,
): Promise<ImagePage> => invoke('image_page', { offset, limit, filter })

/**
 * Meldet dem Backend, wie der Rahmen gerade hängt (E-26).
 *
 * Nötig nur bei `orientation: 'auto'` — dann weiß allein die Oberfläche, was
 * der Lagesensor ergeben hat. Wirkt sich ausschließlich auf die Paarbildung
 * aus (FA-08).
 */
export const setFrameOrientation = (portrait: boolean): Promise<void> =>
  invoke('set_frame_orientation', { portrait })

/**
 * Setzt die eingestellte Ausrichtung am Fenster durch (E-26).
 *
 * Muss vom Frontend kommen: Tauris `setup()` läuft auf einem eigenen Thread und
 * kann die JNI-Brücke noch nicht erreichen. Steht die WebView, steht sie.
 */
export const applyOrientation = (): Promise<void> => invoke('apply_orientation')

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

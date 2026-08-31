/**
 * Typisierte Hülle um die Tauri-Kommandos.
 *
 * Einziger Ort im Frontend, an dem `invoke` vorkommt. Das hält die Grenze zum
 * Backend an einer Stelle nachvollziehbar und erleichtert das Nachziehen, wenn
 * sich `src-tauri/src/commands.rs` ändert.
 */

import { invoke, convertFileSrc } from '@tauri-apps/api/core'
import { EVENTS } from './types'
import type {
  Album,
  AppConfig,
  CacheEntry,
  CacheStats,
  DisplayState,
  FilterFacets,
  ImageFilter,
  ImagePage,
  MqttStatus,
  Slide,
  Source,
  SyncReport,
  PlaybackStats,
  FetchLogEntry,
  ResyncProgress,
  StorageBreakdown,
  DatabaseCheck,
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

/**
 * Gibt ein Foto aus der Quarantäne frei (F4, E-31).
 *
 * `trustSender` nimmt den Absender dauerhaft in die Freigabeliste und holt
 * alle seine wartenden Fotos mit. Liefert die Anzahl freigegebener Bilder.
 */
export const releaseQuarantine = (id: string, trustSender: boolean): Promise<number> =>
  invoke('release_quarantine', { id, trustSender })

/** Jahre und Absender mit Anzahl, für die Filterauswahl (F5). */
export const filterFacets = (): Promise<FilterFacets> => invoke('filter_facets')

/** Wie viele Fotos auf Freigabe warten — Grundlage des Hinweises (E-31). */
export const quarantineCount = (): Promise<number> => invoke('quarantine_count')

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

/**
 * Prüft eine Quelle. Bei einem Postfach kommt die Zahl der ungelesenen
 * Nachrichten zurück, sonst `null` — die Zahl belegt beim Einrichten, dass
 * auch der richtige Ordner gewählt ist.
 */
export const testSource = (source: Source, password: string): Promise<number | null> =>
  invoke('test_source', { source, password })

/**
 * Gleicht ein Postfach vollständig neu ab (Wartung F8).
 *
 * Läuft in Stapeln à 50 mit Pause dazwischen, damit die Diashow weiterläuft.
 * Vorhandene Fotos bleiben unangetastet. Liefert die Zahl der neu abgelegten.
 */
export const resyncMailbox = (sourceId: string): Promise<number> =>
  invoke('resync_mailbox', { sourceId })

/**
 * Hört auf den Fortschritt eines Neuabgleichs (Wartung F8).
 *
 * Der Aufruf von `resyncMailbox` läuft minutenlang und könnte am Ende nur
 * „fertig" oder „fehlgeschlagen" melden. Der Zwischenstand kommt deshalb als
 * Ereignis. Gibt die Abmeldefunktion zurück.
 */
export const onResyncProgress = async (
  handler: (p: ResyncProgress) => void,
): Promise<() => void> => {
  const { listen } = await import('@tauri-apps/api/event')
  return listen<ResyncProgress>(EVENTS.resync, (e) => handler(e.payload))
}

/** Bricht einen laufenden Neuabgleich ab (Wartung F8). */
export const cancelResync = (): Promise<void> => invoke('cancel_resync')

/** Belegung nach Jahr und Absender (Wartung F9). */
export const storageBreakdown = (): Promise<StorageBreakdown> => invoke('storage_breakdown')

/** Vergleicht Index und Dateibestand. Ändert nichts (Wartung F10). */
export const checkDatabase = (): Promise<DatabaseCheck> => invoke('check_database')

/**
 * Räumt auf, was die Prüfung findet (Wartung F10).
 *
 * Prüft im Backend erneut, statt dem angezeigten Ergebnis zu vertrauen —
 * dazwischen kann ein Sync gelaufen sein. Liefert die freigewordenen Bytes.
 */
export const repairDatabase = (): Promise<number> => invoke('repair_database')

/**
 * Baut den anonymisierten Diagnosebericht (Wartung F11).
 *
 * Enthält keine Mailadressen, Servernamen, Quellennamen oder Dateinamen;
 * Absender erscheinen als „Absender A". Serverfehler sind auf die erste Zeile
 * und 200 Zeichen gekürzt.
 */
export const diagnosticReport = (
  androidRelease: string,
  deviceModel: string,
): Promise<string> => invoke('diagnostic_report', { androidRelease, deviceModel })

/** Die letzten Postfach-Abrufe, neueste zuerst (Wartung F6). */
export const fetchLog = (): Promise<FetchLogEntry[]> => invoke('fetch_log')

/**
 * Stand des letzten Abrufs einer Quelle (Wartung F5).
 *
 * `null`, solange noch nie abgerufen wurde.
 */
export const lastFetch = (sourceId: string): Promise<FetchLogEntry | null> =>
  invoke('last_fetch', { sourceId })

/** Statistik der Zufallswiedergabe (Wartung F1). */
export const playbackStats = (): Promise<PlaybackStats> => invoke('playback_stats')

/**
 * Beginnt den Durchlauf von vorn (Wartung F2).
 *
 * Nicht destruktiv: nur die Urne wird geleert. Anzeigezähler und Zeitpunkte
 * bleiben — dafür gibt es `resetHistory`.
 */
export const restartCycle = (): Promise<void> => invoke('restart_cycle')

/**
 * Setzt Anzeigezeitpunkt und -zähler des aktuellen Bestands zurück (F3).
 *
 * Liefert die Zahl der geänderten Einträge. Destruktiv — die Oberfläche fragt
 * vorher nach.
 */
export const resetHistory = (): Promise<number> => invoke('reset_history')

/** Ein freigegebener Absender samt Zahl seiner Fotos (F4, E-32). */
export interface AllowedSender {
  address: string
  photoCount: number
}

export const allowedSenders = (sourceId: string): Promise<AllowedSender[]> =>
  invoke('allowed_senders', { sourceId })

/**
 * Nimmt einen Absender von der Freigabeliste.
 *
 * `requarantine` entscheidet über die vorhandenen Fotos: nur künftige Mails
 * wieder prüfen, oder auch die alten Bilder erneut warten lassen (E-32).
 * Liefert die Zahl der Fotos, die zurück in die Quarantäne gegangen sind.
 */
export const removeAllowedSender = (
  sourceId: string,
  sender: string,
  requarantine: boolean,
): Promise<number> => invoke('remove_allowed_sender', { sourceId, sender, requarantine })

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

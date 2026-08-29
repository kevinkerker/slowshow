/**
 * Spiegelbild des Rust-Datenmodells (`src-tauri/src/model.rs`).
 *
 * Wird eine Struktur dort geändert, muss sie hier nachgezogen werden — der
 * Compiler kann diese Grenze nicht prüfen. Die Tests in `types.test.ts` decken
 * deshalb die Standardwerte ab, die beide Seiten teilen.
 */

/** Reihenfolge der Diashow (FA-03). */
export type PlayOrder = 'random' | 'fileName' | 'takenAt' | 'modified'

/** Darstellung bei abweichendem Seitenverhältnis (FA-05). */
export type FitMode = 'contain' | 'cover'

export interface TransitionConfig {
  /** Weiche Überblendungen (FA-06). */
  enabled: boolean
  durationMs: number
}

/** Einblendungen, einzeln schaltbar (FA-07). */
export interface OverlayConfig {
  showClock: boolean
  showDate: boolean
  showFileName: boolean
  showTakenAt: boolean
  /** Einbrennschutz durch periodisches Verschieben (NF-07). */
  pixelShift: boolean
  /** Zahnrad oben rechts — kurzer Weg in die Einstellungen (FA-40). */
  showSettingsButton: boolean
  /** Durchgestrichenes Auge — Bild aus der Diashow nehmen (FA-30). */
  showExcludeButton: boolean
}

/** Aktivzeiten und Nachtmodus (FA-52, FA-54). */
export interface ScheduleConfig {
  enabled: boolean
  activeFrom: string
  activeTo: string
  nightClock: boolean
}

/** Helligkeitssteuerung (FA-53). */
export interface BrightnessConfig {
  level: number
  autoDim: boolean
  dimFrom: string
  dimLevel: number
}

/** Cache-Ringpuffer und Prefetch (FA-27, FA-31, NF-12). */
export interface CacheConfig {
  maxBytes: number
  prefetchCount: number
  targetWidth: number
  targetHeight: number
  jpegQuality: number
}

/** Heimnetz-Steuerung (FA-55). */
export interface RemoteConfig {
  enabled: boolean
  port: number
  token: string
}

/** MQTT-Anbindung an Home Assistant (FA-55). */
export interface MqttConfig {
  enabled: boolean
  host: string
  port: number
  username: string
  /** Wurzel aller Topics. Bei mehreren Rahmen je Gerät ein eigener. */
  baseTopic: string
  /** Entitäten in Home Assistant selbst anmelden. */
  discovery: boolean
  discoveryPrefix: string
}

/** Zustand der MQTT-Verbindung (Gegenstück zu mqtt::MqttStatus). */
export interface MqttStatus {
  /** Ist der Dienst gestartet? */
  running: boolean
  /** Steht die Verbindung zum Broker wirklich? */
  connected: boolean
  /** Letzter Fehler, solange keine Verbindung steht. */
  lastError: string | null
}

export type SourceKind =
  | { type: 'local'; safUri: string; displayPath: string }
  | {
      type: 'webDav'
      url: string
      username: string
      passwordRef: string
      allowInsecureTls: boolean
    }
  | {
      type: 'nextcloud'
      url: string
      username: string
      passwordRef: string
      album: string
      usePreviewApi: boolean
      allowInsecureTls: boolean
    }

export interface Source {
  id: string
  name: string
  kind: SourceKind
  /** Fließt die Quelle in die Diashow ein? (FA-25) */
  enabled: boolean
  /** Nur diese Unterordner (FA-29). */
  subfolders: string[]
  minWidth: number
  minHeight: number
  syncIntervalMinutes: number
  lastSync: number | null
}

export interface AppConfig {
  intervalSeconds: number
  order: PlayOrder
  fitMode: FitMode
  transition: TransitionConfig
  overlays: OverlayConfig
  schedule: ScheduleConfig
  brightness: BrightnessConfig
  cache: CacheConfig
  remote: RemoteConfig
  mqtt: MqttConfig
  /** Zwei Hochformatbilder nebeneinander (FA-08). */
  pairMode: boolean
  /** Langsames Zoomen/Schwenken (FA-10). */
  kenBurns: boolean
  /** Einstellungen erst nach langem Druck (FA-43). */
  protectSettings: boolean
  language: 'auto' | 'de' | 'en'
  sources: Source[]
}

/** Was gerade auf dem Schirm steht. */
export type Slide =
  | { kind: 'single'; id: string }
  | { kind: 'pair'; left: string; right: string }

/** Metadaten eines zwischengespeicherten Bildes. */
export interface CacheEntry {
  id: string
  sourceId: string
  relPath: string
  fileName: string
  etag: string | null
  remoteSize: number | null
  remoteMtime: number | null
  /** EXIF-Aufnahmedatum als Unix-Zeitstempel (FA-07). */
  takenAt: number | null
  width: number
  height: number
  bytes: number
  addedAt: number
  lastShown: number | null
  excluded: boolean
}

export interface CacheStats {
  images: number
  bytes: number
  maxBytes: number
  excluded: number
}

/** Anzeigezustand aus dem Zeitplan (FA-52–54). */
export interface DisplayState {
  slideshowActive: boolean
  showNightClock: boolean
  brightness: number
}

/** Zwischenstand eines laufenden Syncs (Gegenstueck zu sync::SyncProgress). */
export interface SyncProgress {
  sourceId: string
  sourceName: string
  done: number
  total: number
  stored: number
  current: string
}

export interface SyncReport {
  sourceId: string
  added: number
  updated: number
  unchanged: number
  removed: number
  skipped: number
  failed: number
  evicted: number
  truncated: boolean
  error: string | null
}

export interface Album {
  name: string
}

/** Ereignisnamen aus `state::events`. */
export const EVENTS = {
  slide: 'slowshow://slide',
  sync: 'slowshow://sync',
  syncProgress: 'slowshow://sync-progress',
  display: 'slowshow://display',
  config: 'slowshow://config',
  mqtt: 'slowshow://mqtt',
} as const

/** Alle Bild-Ids eines Slides — für Prefetch und Overlays. */
export function slideIds(slide: Slide | null): string[] {
  if (!slide) return []
  return slide.kind === 'single' ? [slide.id] : [slide.left, slide.right]
}

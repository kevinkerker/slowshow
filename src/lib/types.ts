/**
 * Spiegelbild des Rust-Datenmodells (`src-tauri/src/model.rs`).
 *
 * Wird eine Struktur dort geändert, muss sie hier nachgezogen werden — der
 * Compiler kann diese Grenze nicht prüfen. Die Tests in `types.test.ts` decken
 * deshalb die Standardwerte ab, die beide Seiten teilen.
 */

/** Reihenfolge der Diashow (FA-03, fortgeschrieben durch E-29). */
export type PlayOrder = 'smart' | 'random' | 'fileName' | 'chronological'

/** Zeitraum der Diashow-Auswahl (F5). */
export type TimeFilter =
  | { type: 'all' }
  | { type: 'last12Months' }
  | { type: 'thisYear' }
  | { type: 'years'; years: number[] }

/** Welche Bilder in die Diashow kommen (F5). */
export interface PlaybackFilter {
  time: TimeFilter
  /** Nur diese Absender. Leer = alle. */
  senders: string[]
  /** Bilder ohne Aufnahmedatum mitzeigen. */
  includeUndated: boolean
}

/** Jahre und Absender mit Anzahl, für die Filterauswahl (F5). */
export interface FilterFacets {
  /** `[Jahr, Anzahl]`, neueste zuerst. */
  years: [number, number][]
  /** `[Absender, Anzahl]`, häufigste zuerst. */
  senders: [string, number][]
  undated: number
}

/** Feineinstellungen der Wiedergabe (E-29). */
export interface PlaybackConfig {
  newBoost: boolean
  leastRecentlyShown: boolean
  clusterFilter: boolean
  /** Nur bei `chronological`: neueste zuerst. */
  newestFirst: boolean
}

/** Darstellung bei abweichendem Seitenverhältnis (FA-05). */
export type FitMode = 'contain' | 'cover'

/** Ausrichtung des Rahmens (E-26). Gegenstück zu `model::Orientation`. */
export type Orientation = 'landscape' | 'portrait' | 'auto'

/** Ziffern oder Zeiger (E-20). Gegenstück zu `model::ClockStyle`. */
export type ClockStyle = 'digital' | 'analog'

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
  /** Hinweis, wenn Fotos auf Freigabe warten (F4, E-31). */
  showQuarantineHint: boolean
  /** Ziffern oder Zeiger für die Uhr über dem Foto (E-20). */
  clockStyle: ClockStyle
}

/** Aktivzeiten und Nachtmodus (FA-52, FA-54). */
export interface ScheduleConfig {
  enabled: boolean
  activeFrom: string
  activeTo: string
  nightClock: boolean
  /** Ziffern oder Zeiger für die Nachtuhr (E-20). */
  nightClockStyle: ClockStyle
}

/** Helligkeitssteuerung (FA-53). */
export interface BrightnessConfig {
  level: number
  autoDim: boolean
  dimFrom: string
  dimLevel: number
  /** Das Gerät regelt die Helligkeit selbst (E-22). */
  deviceControlled: boolean
}

/**
 * `DisplayState.brightness`, wenn die App die Helligkeit nicht regelt (E-22).
 * Gegenstück zu `schedule::DEVICE_CONTROLLED`.
 */
export const DEVICE_CONTROLLED_BRIGHTNESS = 0

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
  | {
      /** Postfach, aus dem Fotos per Mail eintreffen (E-30). */
      type: 'mail'
      host: string
      port: number
      username: string
      passwordRef: string
      folder: string
      /** Absender, deren Fotos ohne Quarantäne durchgehen (F4). */
      allowedSenders: string[]
      /** Auch bereits gelesene Nachrichten holen (E-34). */
      includeSeen: boolean
      /** Auch bekannte Absender erst in Quarantäne legen. */
      quarantineAll: boolean
      maxAttachmentBytes: number
      maxMailsPerHour: number
      quality: MailQuality
    }

/** Ablagequalität für Mail-Fotos (E-30). */
export type MailQuality = 'frugal' | 'standard' | 'original'

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
  /** Ausrichtung des Rahmens (E-26). */
  orientation: Orientation
  /** Feineinstellungen der Wiedergabe (E-29). */
  playback: PlaybackConfig
  /** Auswahl, welche Bilder laufen (F5). */
  filter: PlaybackFilter
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
  /** Größe des Vorschaubilds, sobald eines erzeugt wurde (E-25). */
  thumbBytes: number | null
  /** Wie oft das Bild bereits gezeigt wurde (E-29). */
  showCount: number
  /** Herkunft, falls das Bild per Mail kam (E-30). */
  mail: MailMeta | null
}

/** Was der Bild-Browser anzeigt (E-25, erweitert durch E-31). */
export type ImageFilter =
  | 'all'
  | 'excluded'
  | 'included'
  | 'quarantine'
  /** Noch nie in der Diashow gewesen (Wartung F4). */
  | 'neverShown'

/** Wodurch ein Abruf ausgelöst wurde (Wartung F6). */
export type FetchTrigger = 'interval' | 'manual' | 'resync'

/** Fortschritt eines Postfach-Neuabgleichs (Wartung F8). */
/** Belegung einer Gruppe — Jahr oder Absender (Wartung F9). */
export interface StorageGroup {
  label: string
  count: number
  bytes: number
}

/** Aufschlüsselung des Speichers (Wartung F9). */
export interface StorageBreakdown {
  byYear: StorageGroup[]
  bySender: StorageGroup[]
}

/** Ergebnis der Datenbank-Prüfung (Wartung F10). */
export interface DatabaseCheck {
  /** Einträge im Index, zu denen keine Datei existiert. */
  missingFiles: string[]
  /** Dateien ohne Eintrag. */
  orphanFiles: string[]
  orphanThumbs: string[]
  reclaimableBytes: number
}

export interface ResyncProgress {
  done: number
  total: number
  added: number
}

/** Ein Eintrag im Abruf-Protokoll (Wartung F6). */
export interface FetchLogEntry {
  at: number
  sourceId: string
  trigger: FetchTrigger
  /** Nachrichten im Ordner zum Zeitpunkt des Laufs. */
  seenInFolder: number
  /** Davon bereits bekannt — Stufe eins des zweistufigen Abrufs (E-34). */
  alreadyKnown: number
  checked: number
  added: number
  quarantined: number
  skipped: number
  failed: number
  /** Klartext; `null` bei einem geglückten Lauf. */
  error: string | null
}

/** Ein Bild in einer der Bestenlisten der Statistik (Wartung F1). */
export interface TopEntry {
  id: string
  fileName: string
  showCount: number
  lastShown: number | null
}

/** Statistik der Zufallswiedergabe (Wartung F1). */
export interface PlaybackStats {
  /** Alle Bilder im Cache, auch ausgeblendete und wartende. */
  total: number
  /** Davon in der Diashow spielbar — Bezugsgröße des Durchlaufs. */
  eligible: number
  neverShown: number
  /** Im laufenden Durchlauf noch offen. */
  bagRemaining: number
  cycles: number
  mostShown: TopEntry[]
  longestUnseen: TopEntry[]
}

/** Herkunft eines per Mail eingetroffenen Fotos (E-30). */
export interface MailMeta {
  sender: string
  subject: string
  /** Hash der Message-ID. */
  messageId: string
  /** Wartet das Foto auf Freigabe? (F4) */
  quarantined: boolean
}

/** Ein Ausschnitt des Cache-Index für den Bild-Browser (E-25). */
export interface ImagePage {
  entries: CacheEntry[]
  /** Anzahl aller Einträge, die zum Filter passen. */
  total: number
}

export interface CacheStats {
  images: number
  bytes: number
  maxBytes: number
  excluded: number
  /** Belegung durch Vorschaubilder (E-25), getrennt ausgewiesen. */
  thumbBytes: number
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
  /** Fortschritt eines Postfach-Neuabgleichs (Wartung F8). */
  resync: 'slowshow://resync',
  display: 'slowshow://display',
  config: 'slowshow://config',
  mqtt: 'slowshow://mqtt',
} as const

/** Alle Bild-Ids eines Slides — für Prefetch und Overlays. */
export function slideIds(slide: Slide | null): string[] {
  if (!slide) return []
  return slide.kind === 'single' ? [slide.id] : [slide.left, slide.right]
}

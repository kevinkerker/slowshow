//! Datenmodell: Konfiguration und Quellen.
//!
//! Alle Typen werden 1:1 an das Frontend gereicht (`camelCase`) und in
//! `config.json` persistiert (FA-42). Neue Felder brauchen ein `#[serde(default)]`,
//! damit ältere Konfigurationsdateien nach einem Update weiterhin laden (NF-10).

use serde::{Deserialize, Serialize};

// ── Diashow ──────────────────────────────────────────────────────────────────

/// Reihenfolge der Diashow (FA-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PlayOrder {
    /// Zufällig. Nutzt einen pro Durchlauf neu gezogenen Seed.
    #[default]
    Random,
    /// Alphabetisch nach Dateiname.
    FileName,
    /// Aufnahmedatum (EXIF), Fallback Änderungsdatum.
    TakenAt,
    /// Änderungsdatum an der Quelle.
    Modified,
}

/// Darstellung bei abweichendem Seitenverhältnis (FA-05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum FitMode {
    /// Einpassen mit Hintergrund — das ganze Bild bleibt sichtbar.
    #[default]
    Contain,
    /// Formatfüllend mit Beschnitt.
    Cover,
}

/// Darstellung der Uhr (E-20).
///
/// Gilt getrennt für die Einblendung über dem Foto (FA-07) und für den
/// Nachtmodus (FA-54) — nachts darf eine andere Uhr stehen als tagsüber.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ClockStyle {
    /// Ziffern wie im Entwurf (Artboards „Diashow" und „Nachtmodus").
    #[default]
    Digital,
    /// Zeiger auf einem Zifferblatt mit Strichindex.
    Analog,
}

/// Weiche Überblendungen (FA-06).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionConfig {
    pub enabled: bool,
    /// Dauer in Millisekunden.
    pub duration_ms: u32,
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            duration_ms: 1200,
        }
    }
}

/// Einblendbare Zusatzinformationen, einzeln schaltbar (FA-07).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayConfig {
    pub show_clock: bool,
    pub show_date: bool,
    pub show_file_name: bool,
    pub show_taken_at: bool,
    /// Einbrennschutz: Overlays verschieben sich periodisch (NF-07).
    pub pixel_shift: bool,
    /// Zahnrad oben rechts — kurzer Weg in die Einstellungen (FA-40).
    #[serde(default = "default_true")]
    pub show_settings_button: bool,
    /// Durchgestrichenes Auge — Bild aus der Diashow nehmen (FA-30).
    #[serde(default = "default_true")]
    pub show_exclude_button: bool,
    /// Ziffern oder Zeiger für die Uhr über dem Foto (E-20).
    #[serde(default)]
    pub clock_style: ClockStyle,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            show_clock: true,
            show_date: true,
            show_file_name: false,
            show_taken_at: false,
            pixel_shift: true,
            show_settings_button: true,
            show_exclude_button: true,
            clock_style: ClockStyle::Digital,
        }
    }
}

// ── Dauerbetrieb ─────────────────────────────────────────────────────────────

/// Zeitplan mit Aktiv-Zeiten (FA-52) und Nachtmodus (FA-54).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleConfig {
    pub enabled: bool,
    /// Beginn der Aktivzeit als "HH:MM" in lokaler Zeit.
    pub active_from: String,
    /// Ende der Aktivzeit als "HH:MM". Ein Wert kleiner als `active_from`
    /// bedeutet einen über Mitternacht laufenden Zeitraum.
    pub active_to: String,
    /// Statt komplett schwarz eine gedimmte Uhr zeigen (FA-54).
    pub night_clock: bool,
    /// Ziffern oder Zeiger für die Nachtuhr (E-20).
    #[serde(default)]
    pub night_clock_style: ClockStyle,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            active_from: "07:00".into(),
            active_to: "22:00".into(),
            night_clock: true,
            night_clock_style: ClockStyle::Digital,
        }
    }
}

/// Helligkeitssteuerung (FA-53).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrightnessConfig {
    /// Zielhelligkeit in Prozent (1..=100).
    pub level: u8,
    /// Abendliche Absenkung aktiv.
    pub auto_dim: bool,
    /// Uhrzeit "HH:MM", ab der abgesenkt wird.
    pub dim_from: String,
    /// Abgesenkte Helligkeit in Prozent.
    pub dim_level: u8,
    /// Das Gerät regelt die Helligkeit selbst (E-22).
    ///
    /// Die App setzt dann keine Fensterhelligkeit mehr, sodass die
    /// Systemautomatik greift. Der Nachtmodus bleibt davon unberührt: FA-52
    /// ist ein MUSS und würde sonst nachts einen hell leuchtenden Rahmen
    /// stehen lassen.
    #[serde(default)]
    pub device_controlled: bool,
}

impl Default for BrightnessConfig {
    fn default() -> Self {
        Self {
            level: 100,
            auto_dim: false,
            dim_from: "20:00".into(),
            dim_level: 40,
            device_controlled: false,
        }
    }
}

/// Cache- und Prefetch-Parameter (FA-27, FA-31, NF-12).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheConfig {
    /// Obergrenze des rotierenden Caches in Bytes. Standard 2 GB.
    pub max_bytes: u64,
    /// Anzahl vorausgeladener Bilder (FA-31).
    pub prefetch_count: u8,
    /// Zielbreite der Cache-Ablage in Pixeln (NF-12).
    pub target_width: u32,
    /// Zielhöhe der Cache-Ablage in Pixeln.
    pub target_height: u32,
    /// JPEG-Qualität der Cache-Ablage (1..=100).
    pub jpeg_quality: u8,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_bytes: 2 * 1024 * 1024 * 1024,
            prefetch_count: 5,
            target_width: 2560,
            target_height: 1600,
            jpeg_quality: 85,
        }
    }
}

/// Heimnetz-Steuerung über REST (FA-55, E-09).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteConfig {
    pub enabled: bool,
    pub port: u16,
    /// Optionales Bearer-Token. Leer = keine Authentifizierung im Heimnetz.
    pub token: String,
}

impl Default for RemoteConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 8127,
            token: String::new(),
        }
    }
}

/// MQTT-Anbindung an Home Assistant (FA-55).
///
/// Das Passwort steht bewusst nicht hier, sondern verschluesselt in der
/// Zugangsdatenablage (NF-05) — sonst landete es beim Konfigurationsexport
/// aus FA-45 im Klartext.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MqttConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub username: String,
    /// Wurzel aller Topics. Bei mehreren Rahmen im Haus je Geraet ein eigener.
    pub base_topic: String,
    /// Entitaeten in Home Assistant selbst anmelden.
    pub discovery: bool,
    pub discovery_prefix: String,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: String::new(),
            port: 1883,
            username: String::new(),
            base_topic: "slowshow".into(),
            discovery: true,
            discovery_prefix: "homeassistant".into(),
        }
    }
}

// ── Gesamtkonfiguration ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// Anzeigedauer je Bild in Sekunden. Zulässig 5..=1800 (FA-02).
    #[serde(default = "default_interval")]
    pub interval_seconds: u32,
    #[serde(default)]
    pub order: PlayOrder,
    #[serde(default)]
    pub fit_mode: FitMode,
    #[serde(default)]
    pub transition: TransitionConfig,
    #[serde(default)]
    pub overlays: OverlayConfig,
    #[serde(default)]
    pub schedule: ScheduleConfig,
    #[serde(default)]
    pub brightness: BrightnessConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub remote: RemoteConfig,
    #[serde(default)]
    pub mqtt: MqttConfig,
    /// Zwei Hochformatbilder nebeneinander (FA-08).
    #[serde(default)]
    pub pair_mode: bool,
    /// Langsames Zoomen/Schwenken (FA-10).
    #[serde(default)]
    pub ken_burns: bool,
    /// Einstellungen erst nach langem Druck erreichbar (FA-43).
    #[serde(default = "default_true")]
    pub protect_settings: bool,
    /// Oberflächensprache: "auto" | "de" | "en" (NF-09).
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default)]
    pub sources: Vec<Source>,
}

fn default_interval() -> u32 {
    30
}

fn default_true() -> bool {
    true
}

fn default_language() -> String {
    "auto".into()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            interval_seconds: default_interval(),
            order: PlayOrder::default(),
            fit_mode: FitMode::default(),
            transition: TransitionConfig::default(),
            overlays: OverlayConfig::default(),
            schedule: ScheduleConfig::default(),
            brightness: BrightnessConfig::default(),
            cache: CacheConfig::default(),
            remote: RemoteConfig::default(),
            mqtt: MqttConfig::default(),
            pair_mode: false,
            ken_burns: false,
            protect_settings: true,
            language: default_language(),
            sources: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Erzwingt die im Lastenheft festgelegten Wertebereiche.
    ///
    /// Wird nach jedem Laden und vor jedem Schreiben angewandt, damit weder eine
    /// von Hand bearbeitete Datei noch ein REST-Aufruf (FA-55) unsinnige Werte setzt.
    pub fn clamp(&mut self) {
        // FA-02: mindestens 5 Sekunden bis 30 Minuten.
        self.interval_seconds = self.interval_seconds.clamp(5, 1800);
        self.transition.duration_ms = self.transition.duration_ms.clamp(100, 10_000);
        // NF-03: der Prefetch-Puffer darf den RAM nicht gefährden.
        self.cache.prefetch_count = self.cache.prefetch_count.clamp(1, 12);
        self.cache.jpeg_quality = self.cache.jpeg_quality.clamp(40, 100);
        self.cache.target_width = self.cache.target_width.clamp(640, 4096);
        self.cache.target_height = self.cache.target_height.clamp(480, 4096);
        // Unter 128 MB wäre der Ringpuffer nicht sinnvoll befüllbar.
        self.cache.max_bytes = self.cache.max_bytes.max(128 * 1024 * 1024);
        self.brightness.level = self.brightness.level.clamp(1, 100);
        self.brightness.dim_level = self.brightness.dim_level.clamp(1, 100);
        if self.mqtt.port == 0 {
            self.mqtt.port = 1883;
        }
        if self.mqtt.base_topic.trim().is_empty() {
            self.mqtt.base_topic = "slowshow".into();
        }
        for s in &mut self.sources {
            s.sync_interval_minutes = s.sync_interval_minutes.clamp(5, 10_080);
        }
    }
}

// ── Quellen ──────────────────────────────────────────────────────────────────

/// Quellentyp (FA-20, FA-21, FA-23).
///
/// SMB fehlt bewusst (E-02), Google Photos ebenfalls (E-08).
///
/// **`rename_all` steht bewusst zweimal hier.** Am Enum benennt es nur die
/// *Variantennamen* um (`WebDav` -> `webDav`), nicht die Felder innerhalb der
/// Varianten. Ohne die Attribute an jeder Variante erwartet Rust `saf_uri`,
/// während das Frontend `safUri` schickt — jedes Anlegen einer Quelle
/// scheiterte dann beim Deserialisieren. Die Tests unten halten beide
/// Richtungen fest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SourceKind {
    /// Lokaler Ordner über das Storage Access Framework (FA-20).
    ///
    /// Die Aufzählung läuft im Frontend über das SAF-Plugin, die Bytes kommen
    /// per `ingest_image` ins Rust-Backend — SAF ist nur über die Android-Brücke
    /// erreichbar (R-06).
    #[serde(rename_all = "camelCase")]
    Local {
        /// Serialisierte SAF-URI der dauerhaft freigegebenen Ordnerauswahl.
        saf_uri: String,
        /// Anzeigename des Ordners.
        display_path: String,
    },
    /// NAS über WebDAV (FA-21).
    #[serde(rename_all = "camelCase")]
    WebDav {
        url: String,
        username: String,
        /// Schlüssel in den verschlüsselt abgelegten Zugangsdaten (NF-05).
        password_ref: String,
        /// Selbstsignierte Zertifikate im Heimnetz akzeptieren.
        #[serde(default)]
        allow_insecure_tls: bool,
    },
    /// Nextcloud Photos-Album über den Photos-WebDAV-Endpunkt (FA-23, E-03).
    #[serde(rename_all = "camelCase")]
    Nextcloud {
        url: String,
        username: String,
        password_ref: String,
        /// Albumname unter `remote.php/dav/photos/{user}/albums/`.
        album: String,
        /// Bilder über die Preview-API in displaygerechter Größe holen.
        /// Entlastet NF-12 und löst HEIC serverseitig (FA-09, E-04).
        #[serde(default = "default_true")]
        use_preview_api: bool,
        #[serde(default)]
        allow_insecure_tls: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    pub name: String,
    pub kind: SourceKind,
    /// Fließt die Quelle in die Diashow ein? (FA-25)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Nur diese Unterordner berücksichtigen. Leer = alle (FA-29).
    #[serde(default)]
    pub subfolders: Vec<String>,
    /// Mindestauflösung in Pixeln; darunter wird verworfen (FA-29). 0 = aus.
    #[serde(default)]
    pub min_width: u32,
    #[serde(default)]
    pub min_height: u32,
    /// Sync-Intervall in Minuten (FA-28).
    #[serde(default = "default_sync_interval")]
    pub sync_interval_minutes: u64,
    /// Unix-Zeitstempel des letzten erfolgreichen Syncs.
    #[serde(default)]
    pub last_sync: Option<i64>,
}

fn default_sync_interval() -> u64 {
    360
}

impl Source {
    /// Nur entfernte Quellen holt der Rust-Sync selbst ab; lokale Ordner treibt
    /// das Frontend, weil SAF ausschließlich über die Android-Brücke erreichbar ist.
    pub fn is_remote(&self) -> bool {
        !matches!(self.kind, SourceKind::Local { .. })
    }

    /// Ist der nächste Sync fällig? (FA-28)
    pub fn is_sync_due(&self, now: i64) -> bool {
        match self.last_sync {
            None => true,
            Some(last) => now - last >= (self.sync_interval_minutes as i64) * 60,
        }
    }

    /// Erfüllt ein Bild die Mindestauflösung dieser Quelle? (FA-29)
    pub fn meets_min_resolution(&self, width: u32, height: u32) -> bool {
        width >= self.min_width && height >= self.min_height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn webdav_source() -> Source {
        Source {
            id: "a".into(),
            name: "A".into(),
            kind: SourceKind::WebDav {
                url: "https://nas.local/dav".into(),
                username: "u".into(),
                password_ref: "r".into(),
                allow_insecure_tls: false,
            },
            enabled: true,
            subfolders: vec![],
            min_width: 0,
            min_height: 0,
            sync_interval_minutes: 60,
            last_sync: None,
        }
    }

    #[test]
    fn clamp_haelt_intervall_in_den_grenzen_aus_fa_02() {
        let mut c = AppConfig {
            interval_seconds: 1,
            ..Default::default()
        };
        c.clamp();
        assert_eq!(c.interval_seconds, 5, "Untergrenze 5 Sekunden");

        let mut c = AppConfig {
            interval_seconds: 99_999,
            ..Default::default()
        };
        c.clamp();
        assert_eq!(c.interval_seconds, 1800, "Obergrenze 30 Minuten");
    }

    #[test]
    fn clamp_begrenzt_prefetch_wegen_nf_03() {
        let mut c = AppConfig::default();
        c.cache.prefetch_count = 200;
        c.clamp();
        assert_eq!(c.cache.prefetch_count, 12);
    }

    #[test]
    fn clamp_erzwingt_cache_mindestgroesse() {
        let mut c = AppConfig::default();
        c.cache.max_bytes = 1024;
        c.clamp();
        assert_eq!(c.cache.max_bytes, 128 * 1024 * 1024);
    }

    #[test]
    fn config_laedt_aus_teilweiser_json_dank_serde_default() {
        // Simuliert eine ältere config.json nach einem App-Update (NF-10).
        let json = r#"{ "intervalSeconds": 45 }"#;
        let c: AppConfig = serde_json::from_str(json).expect("Teilkonfiguration muss laden");
        assert_eq!(c.interval_seconds, 45);
        assert_eq!(
            c.cache.prefetch_count, 5,
            "fehlende Felder kommen aus Default"
        );
        assert!(c.sources.is_empty());
    }

    #[test]
    fn uhrstil_faellt_ohne_feld_auf_digital_zurueck_e_20() {
        // Eine config.json, die vor E-20 geschrieben wurde, kennt weder
        // `clockStyle` noch `nightClockStyle`. Beide müssen dann auf der
        // bisherigen Darstellung landen — ein Update darf das Aussehen des
        // Rahmens nicht von sich aus ändern (NF-10).
        let json = r#"{ "overlays": { "showClock": true, "showDate": true,
                                      "showFileName": false, "showTakenAt": false,
                                      "pixelShift": true },
                        "schedule": { "enabled": true, "activeFrom": "07:00",
                                      "activeTo": "22:00", "nightClock": true } }"#;
        let c: AppConfig = serde_json::from_str(json).expect("Altkonfiguration muss laden");
        assert_eq!(c.overlays.clock_style, ClockStyle::Digital);
        assert_eq!(c.schedule.night_clock_style, ClockStyle::Digital);
    }

    #[test]
    fn uhrstil_wird_als_camel_case_uebertragen_e_20() {
        // Das Frontend liest die Werte als String-Literale ('digital' |
        // 'analog') aus `types.ts`. Weicht die Schreibweise ab, greift dort
        // stumm der Digital-Zweig.
        let mut c = AppConfig::default();
        c.overlays.clock_style = ClockStyle::Analog;
        c.schedule.night_clock_style = ClockStyle::Analog;
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains(r#""clockStyle":"analog""#), "{json}");
        assert!(json.contains(r#""nightClockStyle":"analog""#), "{json}");
    }

    #[test]
    fn config_roundtrip_ueber_json() {
        let mut c = AppConfig::default();
        c.sources.push(webdav_source());
        let json = serde_json::to_string(&c).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.sources.len(), 1);
        assert!(back.sources[0].is_remote());
    }

    #[test]
    fn sync_faellig_wenn_nie_synchronisiert() {
        let s = webdav_source();
        assert!(s.is_sync_due(0));
        assert!(s.is_remote());

        let s2 = Source {
            last_sync: Some(1000),
            ..s
        };
        assert!(!s2.is_sync_due(1000 + 59 * 60));
        assert!(s2.is_sync_due(1000 + 60 * 60));
    }

    #[test]
    fn quelle_laedt_aus_dem_json_das_das_frontend_schickt() {
        // Genau die Form, die src/lib/types.ts erzeugt. Der Test existiert,
        // weil serdes `rename_all` am Enum nur die Variantennamen umbenennt,
        // nicht die Felder der Varianten — ohne Attribut an jeder Variante
        // erwartet Rust `saf_uri`, das Frontend schickt aber `safUri`, und
        // `add_source` scheitert beim Deserialisieren.
        let json = r#"{
            "id": "src-1",
            "name": "Tablet",
            "kind": { "type": "local", "safUri": "{}", "displayPath": "DCIM" },
            "enabled": true,
            "subfolders": [],
            "minWidth": 0,
            "minHeight": 0,
            "syncIntervalMinutes": 360,
            "lastSync": null
        }"#;
        let s: Source = serde_json::from_str(json).expect("lokale Quelle muss laden");
        match s.kind {
            SourceKind::Local {
                saf_uri,
                display_path,
            } => {
                assert_eq!(saf_uri, "{}");
                assert_eq!(display_path, "DCIM");
            }
            other => panic!("Local erwartet, war {other:?}"),
        }
    }

    #[test]
    fn webdav_quelle_laedt_aus_frontend_json() {
        let json = r#"{
            "id": "src-2",
            "name": "NAS",
            "kind": {
                "type": "webDav",
                "url": "https://nas.local/dav",
                "username": "u",
                "passwordRef": "src-2",
                "allowInsecureTls": true
            },
            "enabled": true,
            "subfolders": ["Urlaub"],
            "minWidth": 1024,
            "minHeight": 768,
            "syncIntervalMinutes": 60,
            "lastSync": null
        }"#;
        let s: Source = serde_json::from_str(json).expect("WebDAV-Quelle muss laden");
        match s.kind {
            SourceKind::WebDav {
                password_ref,
                allow_insecure_tls,
                ..
            } => {
                assert_eq!(password_ref, "src-2");
                assert!(allow_insecure_tls);
            }
            other => panic!("WebDav erwartet, war {other:?}"),
        }
    }

    #[test]
    fn nextcloud_quelle_laedt_aus_frontend_json() {
        let json = r#"{
            "id": "src-3",
            "name": "Cloud",
            "kind": {
                "type": "nextcloud",
                "url": "https://cloud.example.org",
                "username": "kevin",
                "passwordRef": "src-3",
                "album": "Sommer",
                "usePreviewApi": true,
                "allowInsecureTls": false
            },
            "enabled": true,
            "subfolders": [],
            "minWidth": 0,
            "minHeight": 0,
            "syncIntervalMinutes": 360,
            "lastSync": null
        }"#;
        let s: Source = serde_json::from_str(json).expect("Nextcloud-Quelle muss laden");
        match s.kind {
            SourceKind::Nextcloud {
                album,
                use_preview_api,
                ..
            } => {
                assert_eq!(album, "Sommer");
                assert!(use_preview_api);
            }
            other => panic!("Nextcloud erwartet, war {other:?}"),
        }
    }

    #[test]
    fn quelle_wird_so_geschrieben_wie_das_frontend_sie_liest() {
        // Gegenrichtung: get_config muss camelCase liefern, sonst kann die
        // Oberflaeche eine gespeicherte Quelle nicht mehr anzeigen.
        let s = Source {
            kind: SourceKind::Local {
                saf_uri: "{}".into(),
                display_path: "DCIM".into(),
            },
            ..webdav_source()
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"safUri\""), "war: {json}");
        assert!(json.contains("\"displayPath\""), "war: {json}");
        assert!(!json.contains("saf_uri"), "war: {json}");
    }

    #[test]
    fn lokale_quelle_ist_nicht_remote() {
        let s = Source {
            kind: SourceKind::Local {
                saf_uri: "{}".into(),
                display_path: "DCIM/Familie".into(),
            },
            ..webdav_source()
        };
        assert!(!s.is_remote());
    }

    #[test]
    fn mindestaufloesung_filtert_fa_29() {
        let s = Source {
            min_width: 1024,
            min_height: 768,
            ..webdav_source()
        };
        assert!(s.meets_min_resolution(1920, 1080));
        assert!(s.meets_min_resolution(1024, 768));
        assert!(!s.meets_min_resolution(800, 600));
        // Standardquelle ohne Filter lässt alles durch.
        assert!(webdav_source().meets_min_resolution(1, 1));
    }
}

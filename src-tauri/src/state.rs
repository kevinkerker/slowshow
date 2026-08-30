//! Gemeinsamer Anwendungszustand.
//!
//! Hält Konfiguration, Cache, Zugangsdaten und Playlist hinter je einem Mutex.
//! Die Aufteilung ist bewusst feingliedrig: ein Sync darf den Cache sperren,
//! ohne dass die Diashow beim Lesen der Konfiguration wartet.
//!
//! **Regel:** kein Guard darf über einen `await` gehalten werden. Alle
//! `lock()`-Aufrufe stehen deshalb in engen Blöcken.

use crate::cache::Cache;
use crate::config::ConfigStore;
use crate::model::{AppConfig, Orientation, Source};
use crate::playlist::{build_order, Playlist, Slide};
use crate::schedule::{self, DisplayState};
use crate::secrets::{FileKeyProvider, SecretStore};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// Ereignisnamen Richtung Frontend.
pub mod events {
    /// Neues Bild bzw. Bildpaar anzuzeigen.
    pub const SLIDE: &str = "slowshow://slide";
    /// Ein Sync-Lauf ist fertig.
    pub const SYNC: &str = "slowshow://sync";
    /// Zwischenstand eines laufenden Syncs.
    pub const SYNC_PROGRESS: &str = "slowshow://sync-progress";
    /// Zeitplan/Helligkeit haben sich geändert (FA-52–54).
    pub const DISPLAY: &str = "slowshow://display";
    /// Die Konfiguration wurde von außen geändert (FA-55).
    pub const CONFIG: &str = "slowshow://config";
    /// Die MQTT-Verbindung hat ihren Zustand gewechselt.
    pub const MQTT: &str = "slowshow://mqtt";
}

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub cache: Mutex<Cache>,
    pub secrets: Mutex<SecretStore>,
    pub playlist: Mutex<Playlist>,
    config_store: ConfigStore,
    /// Läuft die Diashow? Pause über Tippen (FA-41) oder Fernsteuerung (FA-55).
    playing: AtomicBool,
    /// Verhindert überlappende Sync-Läufe.
    syncing: AtomicBool,
    /// Hängt der Rahmen hochkant? (E-26)
    ///
    /// Beeinflusst nur die Paarbildung (FA-08). Die Einstellung liefert den
    /// Ausgangswert; bei `Orientation::Auto` kann nur die Oberfläche wissen,
    /// wie das Gerät gerade liegt, und meldet es über `set_frame_orientation`.
    frame_portrait: AtomicBool,
}

impl AppState {
    /// Öffnet alle Speicher unterhalb von `data_dir`.
    pub fn new(data_dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;

        let config_store = ConfigStore::new(data_dir).map_err(|e| e.to_string())?;
        let config = config_store.load();

        let cache_dir = image_cache_dir(data_dir);
        migrate_legacy_cache(data_dir, &cache_dir);
        let cache = Cache::open(&cache_dir).map_err(|e| e.to_string())?;
        let secrets = SecretStore::open(data_dir, &FileKeyProvider::new(data_dir))
            .map_err(|e| e.to_string())?;

        // Vor dem Verschieben in den Mutex ablesen.
        let starts_portrait = config.orientation == Orientation::Portrait;

        let state = Self {
            config: Mutex::new(config),
            cache: Mutex::new(cache),
            secrets: Mutex::new(secrets),
            playlist: Mutex::new(Playlist::new()),
            config_store,
            playing: AtomicBool::new(true),
            frame_portrait: AtomicBool::new(starts_portrait),
            syncing: AtomicBool::new(false),
        };
        state.rebuild_playlist();
        Ok(state)
    }

    // ── Konfiguration ───────────────────────────────────────────────────────

    pub fn config_snapshot(&self) -> AppConfig {
        self.config
            .lock()
            .expect("Konfigurations-Mutex vergiftet")
            .clone()
    }

    /// Ändert die Konfiguration und schreibt sie sofort (FA-42).
    pub fn update_config<F>(&self, f: F) -> Result<AppConfig, String>
    where
        F: FnOnce(&mut AppConfig),
    {
        let snapshot = {
            let mut guard = self.config.lock().map_err(|_| "Konfiguration gesperrt")?;
            f(&mut guard);
            guard.clamp();
            guard.clone()
        };
        self.config_store
            .save(&snapshot)
            .map_err(|e| e.to_string())?;
        Ok(snapshot)
    }

    pub fn source_by_id(&self, id: &str) -> Option<Source> {
        self.config
            .lock()
            .ok()?
            .sources
            .iter()
            .find(|s| s.id == id)
            .cloned()
    }

    // ── Diashow ─────────────────────────────────────────────────────────────

    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Relaxed)
    }

    /// Wie der Rahmen gerade hängt (E-26).
    pub fn frame_portrait(&self) -> bool {
        self.frame_portrait.load(Ordering::Relaxed)
    }

    pub fn set_frame_portrait(&self, portrait: bool) {
        self.frame_portrait.store(portrait, Ordering::Relaxed);
    }

    pub fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Relaxed);
    }

    /// Sperre gegen überlappende Sync-Läufe. Gibt `false`, wenn schon einer läuft.
    pub fn try_begin_sync(&self) -> bool {
        self.syncing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    pub fn end_sync(&self) {
        self.syncing.store(false, Ordering::SeqCst);
    }

    pub fn is_syncing(&self) -> bool {
        self.syncing.load(Ordering::SeqCst)
    }

    /// Baut die Anzeigereihenfolge neu auf (FA-03).
    ///
    /// Nach jedem Sync (FA-28) und jeder Änderung an Quellen oder Sortierung.
    /// Die aktuelle Position bleibt nach Möglichkeit erhalten.
    pub fn rebuild_playlist(&self) {
        let config = self.config_snapshot();
        let enabled: HashSet<String> = config
            .sources
            .iter()
            .filter(|s| s.enabled)
            .map(|s| s.id.clone())
            .collect();

        // Seed nur beim ersten Aufbau ziehen. Zusammen mit der stabilen
        // Hash-Sortierung in `build_order` heißt das: ein Sync oder eine
        // Konfigurationsänderung schiebt neue Bilder ein, ohne die laufende
        // Reihenfolge neu zu würfeln. Neu gemischt wird nur beim Umlauf
        // (siehe `Playlist::advance`).
        let seed = {
            let pl = self.playlist.lock().expect("Playlist-Mutex vergiftet");
            if pl.is_empty() {
                random_seed()
            } else {
                pl.seed()
            }
        };
        let order = {
            let cache = self.cache.lock().expect("Cache-Mutex vergiftet");
            build_order(cache.index(), &enabled, config.order, seed)
        };

        let len = order.len();
        self.playlist
            .lock()
            .expect("Playlist-Mutex vergiftet")
            .replace(order, seed);
        log::debug!("Playlist neu aufgebaut: {len} Bilder");
    }

    pub fn current_slide(&self) -> Option<Slide> {
        let pair_mode = self.config_snapshot().pair_mode;
        let portrait = self.frame_portrait();
        let cache = self.cache.lock().ok()?;
        self.playlist
            .lock()
            .ok()?
            .current(pair_mode, portrait, cache.index())
    }

    /// Schaltet weiter und merkt die Anzeige für den Ringpuffer (FA-27).
    pub fn advance(&self) -> Option<Slide> {
        self.step(true)
    }

    pub fn back(&self) -> Option<Slide> {
        self.step(false)
    }

    fn step(&self, forward: bool) -> Option<Slide> {
        let pair_mode = self.config_snapshot().pair_mode;
        let portrait = self.frame_portrait();
        let now = now_ts();

        let slide = {
            let cache = self.cache.lock().ok()?;
            let mut pl = self.playlist.lock().ok()?;
            if forward {
                pl.advance(pair_mode, portrait, cache.index(), random_seed())
            } else {
                pl.back(pair_mode, portrait, cache.index())
            }
        }?;

        // Anzeigezeitpunkt in einem eigenen Block: `mark_shown` braucht den
        // Cache schreibend, die Playlist-Sperre ist hier schon wieder frei.
        if let Ok(mut cache) = self.cache.lock() {
            for id in slide.ids() {
                cache.mark_shown(id, now);
            }
        }
        Some(slide)
    }

    /// Nimmt ein Bild aus der Diashow und schaltet auf das nächste (FA-30).
    ///
    /// Der Nachfolger wird *vor* dem Neuaufbau gemerkt: danach steht das
    /// ausgeschlossene Bild nicht mehr in der Reihenfolge, `Playlist::replace`
    /// fände seine alte Position nicht wieder und fiele auf 0 zurück — die
    /// Diashow spränge also an den Anfang statt weiterzulaufen.
    pub fn exclude_image(&self, id: &str) -> Result<(), String> {
        let successor = self
            .playlist
            .lock()
            .ok()
            .and_then(|pl| pl.window(2).into_iter().find(|other| other != id));

        {
            let mut cache = self.cache.lock().map_err(|_| "Cache gesperrt")?;
            if !cache.set_excluded(id, true) {
                return Err(format!("Unbekanntes Bild: {id}"));
            }
            cache.flush().map_err(|e| e.to_string())?;
        }

        self.rebuild_playlist();

        if let Some(next_id) = successor {
            if let Ok(mut pl) = self.playlist.lock() {
                pl.seek_to(&next_id);
            }
        }
        Ok(())
    }

    /// Ids, die der Ringpuffer nicht verdrängen darf: das aktuelle Bild und die
    /// vorgeladenen (FA-27, FA-31).
    pub fn protected_ids(&self) -> HashSet<String> {
        let n = self.config_snapshot().cache.prefetch_count as usize;
        self.playlist
            .lock()
            .map(|pl| pl.window(n).into_iter().collect())
            .unwrap_or_default()
    }

    /// Das Vorausladefenster für das Frontend (FA-31).
    pub fn prefetch_window(&self) -> Vec<String> {
        let n = self.config_snapshot().cache.prefetch_count as usize;
        self.playlist
            .lock()
            .map(|pl| pl.window(n))
            .unwrap_or_default()
    }

    // ── Zeitsteuerung ───────────────────────────────────────────────────────

    /// Aktueller Anzeigezustand laut Zeitplan (FA-52–54).
    pub fn display_state(&self) -> DisplayState {
        schedule::evaluate(&self.config_snapshot(), schedule::now_local_minutes())
    }

    /// Schreibt ausstehende Änderungen. Beim Pausieren und Beenden aufrufen.
    pub fn flush(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            if let Err(e) = cache.flush() {
                log::error!("Cache-Index nicht schreibbar: {e}");
            }
        }
    }
}

/// Verzeichnis der zwischengespeicherten Bilder.
///
/// **Bewusst nicht `cache/`.** Auf Android ist `<datenverzeichnis>/cache` das
/// Verzeichnis aus `Context.getCacheDir()`: Android leert es bei Speicherknappheit
/// selbsttätig, und der Nutzer kann es in den Systemeinstellungen unter
/// „Cache leeren" wegwerfen. FA-27 verlangt aber ausdrücklich einen
/// **permanenten** Cache, der App- und Geräteneustart übersteht — und FA-26
/// hängt daran, dass die Diashow bei Netzausfall weiterläuft.
///
/// `imagecache/` liegt daneben im App-Datenverzeichnis und wird von Android
/// nicht angefasst.
pub fn image_cache_dir(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("imagecache")
}

/// Verschiebt einen Cache aus dem alten, von Android verwalteten Verzeichnis.
///
/// Betrifft nur Installationen aus der Zeit vor dieser Korrektur. Schlägt das
/// Verschieben fehl, ist das kein Grund zum Abbruch — der Cache wird dann beim
/// nächsten Sync neu aufgebaut.
fn migrate_legacy_cache(data_dir: &Path, target: &Path) {
    let legacy = data_dir.join("cache");
    if target.exists() || !legacy.join("index.json").exists() {
        return;
    }
    match std::fs::rename(&legacy, target) {
        Ok(()) => log::info!("Bild-Cache aus dem System-Cache-Verzeichnis verschoben"),
        Err(e) => log::warn!("Bild-Cache konnte nicht verschoben werden: {e}"),
    }
}

/// Unix-Zeitstempel in Sekunden.
pub fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Zufälliger Seed für das Mischen der Diashow.
///
/// Fällt auf die Systemzeit zurück, falls die Entropiequelle nicht verfügbar
/// ist — für eine Bilderreihenfolge völlig ausreichend.
pub fn random_seed() -> u64 {
    let mut buf = [0u8; 8];
    match getrandom::getrandom(&mut buf) {
        Ok(()) => u64::from_le_bytes(buf),
        Err(_) => now_ts() as u64,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PlayOrder, SourceKind};
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let mut p = std::env::temp_dir();
            p.push(format!("slowshow-state-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn prepared(w: u32, h: u32) -> crate::decode::Prepared {
        crate::decode::Prepared {
            bytes: vec![1u8; 100],
            width: w,
            height: h,
            taken_at: None,
        }
    }

    fn add_source(state: &AppState, id: &str, enabled: bool) {
        state
            .update_config(|c| {
                c.sources.push(Source {
                    id: id.into(),
                    name: id.into(),
                    kind: SourceKind::Local {
                        saf_uri: "{}".into(),
                        display_path: "DCIM".into(),
                    },
                    enabled,
                    subfolders: vec![],
                    min_width: 0,
                    min_height: 0,
                    sync_interval_minutes: 60,
                    last_sync: None,
                });
            })
            .unwrap();
    }

    #[test]
    fn neuer_zustand_startet_spielbereit() {
        let dir = TempDir::new("new");
        let state = AppState::new(&dir.0).unwrap();
        assert!(
            state.is_playing(),
            "die Diashow beginnt ohne Interaktion (FA-51)"
        );
        assert!(!state.is_syncing());
        assert!(
            state.current_slide().is_none(),
            "ohne Bilder nichts anzuzeigen"
        );
    }

    #[test]
    fn update_config_schreibt_sofort_fa_42() {
        let dir = TempDir::new("cfgsave");
        {
            let state = AppState::new(&dir.0).unwrap();
            state.update_config(|c| c.interval_seconds = 90).unwrap();
        }
        let state2 = AppState::new(&dir.0).unwrap();
        assert_eq!(state2.config_snapshot().interval_seconds, 90);
    }

    #[test]
    fn update_config_erzwingt_die_wertebereiche() {
        let dir = TempDir::new("cfgclamp");
        let state = AppState::new(&dir.0).unwrap();
        let c = state.update_config(|c| c.interval_seconds = 1).unwrap();
        assert_eq!(c.interval_seconds, 5, "FA-02 Untergrenze");
    }

    #[test]
    fn playlist_enthaelt_nur_aktive_quellen_fa_25() {
        let dir = TempDir::new("playlist");
        let state = AppState::new(&dir.0).unwrap();
        add_source(&state, "an", true);
        add_source(&state, "aus", false);
        {
            let mut cache = state.cache.lock().unwrap();
            cache
                .store(
                    "an",
                    "1.jpg",
                    "1.jpg",
                    prepared(1920, 1080),
                    None,
                    None,
                    None,
                    0,
                )
                .unwrap();
            cache
                .store(
                    "aus",
                    "2.jpg",
                    "2.jpg",
                    prepared(1920, 1080),
                    None,
                    None,
                    None,
                    0,
                )
                .unwrap();
        }
        state.rebuild_playlist();

        assert_eq!(state.playlist.lock().unwrap().len(), 1);
    }

    #[test]
    fn advance_merkt_die_anzeige_fuer_den_ringpuffer_fa_27() {
        let dir = TempDir::new("advance");
        let state = AppState::new(&dir.0).unwrap();
        add_source(&state, "s", true);
        let ids: Vec<String> = {
            let mut cache = state.cache.lock().unwrap();
            (0..3)
                .map(|i| {
                    cache
                        .store(
                            "s",
                            &format!("{i}.jpg"),
                            "x.jpg",
                            prepared(1920, 1080),
                            None,
                            None,
                            None,
                            0,
                        )
                        .unwrap()
                        .id
                })
                .collect()
        };
        state
            .update_config(|c| c.order = PlayOrder::FileName)
            .unwrap();
        state.rebuild_playlist();

        assert!(state.advance().is_some());
        let cache = state.cache.lock().unwrap();
        let gezeigt = ids
            .iter()
            .filter(|id| cache.index().get(id).unwrap().last_shown.is_some());
        assert_eq!(
            gezeigt.count(),
            1,
            "genau das angezeigte Bild wird markiert"
        );
    }

    #[test]
    fn protected_ids_deckt_das_prefetch_fenster_ab_fa_31() {
        let dir = TempDir::new("protected");
        let state = AppState::new(&dir.0).unwrap();
        add_source(&state, "s", true);
        {
            let mut cache = state.cache.lock().unwrap();
            for i in 0..10 {
                cache
                    .store(
                        "s",
                        &format!("{i}.jpg"),
                        "x.jpg",
                        prepared(1920, 1080),
                        None,
                        None,
                        None,
                        0,
                    )
                    .unwrap();
            }
        }
        state.update_config(|c| c.cache.prefetch_count = 3).unwrap();
        state.rebuild_playlist();

        // Aktuelles Bild plus drei vorgeladene.
        assert_eq!(state.protected_ids().len(), 4);
        assert_eq!(state.prefetch_window().len(), 4);
    }

    #[test]
    fn sync_sperre_verhindert_ueberlappung() {
        let dir = TempDir::new("synclock");
        let state = AppState::new(&dir.0).unwrap();

        assert!(state.try_begin_sync());
        assert!(
            !state.try_begin_sync(),
            "zweiter Lauf muss abgewiesen werden"
        );
        assert!(state.is_syncing());

        state.end_sync();
        assert!(state.try_begin_sync(), "nach dem Ende wieder frei");
    }

    #[test]
    fn playpause_schaltet_um_fa_41() {
        let dir = TempDir::new("play");
        let state = AppState::new(&dir.0).unwrap();
        state.set_playing(false);
        assert!(!state.is_playing());
        state.set_playing(true);
        assert!(state.is_playing());
    }

    #[test]
    fn display_state_folgt_dem_zeitplan_fa_52() {
        let dir = TempDir::new("display");
        let state = AppState::new(&dir.0).unwrap();
        // Ohne Zeitplan ist die Diashow immer aktiv.
        assert!(state.display_state().slideshow_active);

        state
            .update_config(|c| {
                c.schedule.enabled = true;
                c.schedule.active_from = "00:00".into();
                c.schedule.active_to = "00:00".into();
            })
            .unwrap();
        assert!(
            state.display_state().slideshow_active,
            "gleiche Zeiten = ganztaegig"
        );
    }

    #[test]
    fn exclude_image_schaltet_auf_das_naechste_bild_fa_30() {
        let dir = TempDir::new("exclude");
        let state = AppState::new(&dir.0).unwrap();
        add_source(&state, "s", true);
        {
            let mut cache = state.cache.lock().unwrap();
            for i in 0..4 {
                cache
                    .store(
                        "s",
                        &format!("{i}.jpg"),
                        "x.jpg",
                        prepared(1920, 1080),
                        None,
                        None,
                        None,
                        0,
                    )
                    .unwrap();
            }
        }
        state
            .update_config(|c| c.order = PlayOrder::FileName)
            .unwrap();
        state.rebuild_playlist();

        // Zwei weiterschalten, damit ein Sprung an den Anfang auffiele.
        state.advance();
        state.advance();
        let aktuell = match state.current_slide().unwrap() {
            Slide::Single { id } => id,
            other => panic!("Einzelbild erwartet, war {other:?}"),
        };
        let erwarteter_nachfolger = state.prefetch_window()[1].clone();

        state.exclude_image(&aktuell).unwrap();

        match state.current_slide().unwrap() {
            Slide::Single { id } => assert_eq!(
                id, erwarteter_nachfolger,
                "die Diashow muss weiterlaufen, nicht an den Anfang springen"
            ),
            other => panic!("Einzelbild erwartet, war {other:?}"),
        }
    }

    #[test]
    fn exclude_image_meldet_unbekannte_id() {
        let dir = TempDir::new("exclude-unknown");
        let state = AppState::new(&dir.0).unwrap();
        assert!(state.exclude_image("gibtsnicht").is_err());
    }

    #[test]
    fn exclude_image_auf_dem_letzten_bild_paniciert_nicht() {
        let dir = TempDir::new("exclude-last");
        let state = AppState::new(&dir.0).unwrap();
        add_source(&state, "s", true);
        let id = {
            let mut cache = state.cache.lock().unwrap();
            cache
                .store(
                    "s",
                    "1.jpg",
                    "1.jpg",
                    prepared(1920, 1080),
                    None,
                    None,
                    None,
                    0,
                )
                .unwrap()
                .id
        };
        state.rebuild_playlist();

        state.exclude_image(&id).unwrap();
        assert!(
            state.current_slide().is_none(),
            "danach ist nichts mehr zu zeigen"
        );
    }

    #[test]
    fn rebuild_behaelt_den_seed_und_damit_die_reihenfolge() {
        // FA-28: neue Bilder ohne Neustart — aber die laufende Zufallsfolge
        // darf dabei nicht neu gewuerfelt werden.
        let dir = TempDir::new("seed");
        let state = AppState::new(&dir.0).unwrap();
        add_source(&state, "s", true);
        {
            let mut cache = state.cache.lock().unwrap();
            for i in 0..8 {
                cache
                    .store(
                        "s",
                        &format!("{i}.jpg"),
                        "x.jpg",
                        prepared(1920, 1080),
                        None,
                        None,
                        None,
                        0,
                    )
                    .unwrap();
            }
        }
        state.rebuild_playlist();
        let seed = state.playlist.lock().unwrap().seed();

        state.rebuild_playlist();
        assert_eq!(
            state.playlist.lock().unwrap().seed(),
            seed,
            "ein erneuter Aufbau darf keinen neuen Seed ziehen"
        );
    }

    #[test]
    fn bild_cache_liegt_nicht_im_system_cache_fa_27() {
        // Auf Android ist <datenverzeichnis>/cache das Verzeichnis, das das
        // System jederzeit leeren darf. Ein permanenter Cache (FA-27) darf
        // dort nicht landen.
        let dir = TempDir::new("cachedir");
        let cache_dir = image_cache_dir(&dir.0);
        assert_ne!(cache_dir, dir.0.join("cache"));
        assert!(cache_dir.starts_with(&dir.0));
    }

    #[test]
    fn alter_cache_wird_uebernommen() {
        let dir = TempDir::new("migrate");
        // Zustand einer Installation vor der Korrektur nachbauen.
        let legacy = dir.0.join("cache").join("images");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(dir.0.join("cache").join("index.json"), b"{}").unwrap();
        std::fs::write(legacy.join("abc.jpg"), b"bild").unwrap();

        let state = AppState::new(&dir.0).unwrap();
        drop(state);

        let moved = image_cache_dir(&dir.0).join("images").join("abc.jpg");
        assert!(moved.exists(), "die Bilder muessen mitkommen");
        assert!(!dir.0.join("cache").join("index.json").exists());
    }

    #[test]
    fn migration_laesst_vorhandenen_cache_in_ruhe() {
        let dir = TempDir::new("migrate-skip");
        // Beide Verzeichnisse vorhanden: der neue gewinnt, nichts wird ueberschrieben.
        let legacy = dir.0.join("cache");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("index.json"), b"{}").unwrap();

        let target = image_cache_dir(&dir.0).join("images");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("neu.jpg"), b"neu").unwrap();

        let state = AppState::new(&dir.0).unwrap();
        drop(state);

        assert!(target.join("neu.jpg").exists());
        assert!(
            legacy.join("index.json").exists(),
            "der alte Ordner bleibt unangetastet"
        );
    }

    #[test]
    fn random_seed_liefert_verschiedene_werte() {
        assert_ne!(random_seed(), random_seed());
    }

    #[test]
    fn source_by_id_findet_und_meldet_fehlschlag() {
        let dir = TempDir::new("srcid");
        let state = AppState::new(&dir.0).unwrap();
        add_source(&state, "nas", true);
        assert!(state.source_by_id("nas").is_some());
        assert!(state.source_by_id("gibtsnicht").is_none());
    }
}

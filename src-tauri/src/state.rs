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
use crate::model::{AppConfig, Orientation, PlayOrder, Source};
use crate::playlist::{build_order, Playlist, Slide};
use crate::schedule::{self, DisplayState};
use crate::scheduler::{Scheduler, SystemRandom};
use crate::secrets::{FileKeyProvider, SecretStore};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;

/// Wie viele Quellen zeitgleich abgeglichen werden (E-43).
///
/// Zwei und nicht mehr: `decode::prepare` haelt je Lauf ein dekodiertes
/// Vollbild im Speicher, und der Rahmen teilt sich den mit der WebView (R-03).
/// Zwei genuegen fuer den Gewinn, um den es geht — eine Quelle haengt am Netz,
/// die andere am Prozessor.
pub const MAX_PARALLEL_SOURCES: usize = 2;

/// Ereignisnamen Richtung Frontend.
pub mod events {
    /// Neues Bild bzw. Bildpaar anzuzeigen.
    pub const SLIDE: &str = "slowshow://slide";
    /// Ein Sync-Lauf ist fertig.
    pub const SYNC: &str = "slowshow://sync";
    /// Fortschritt eines Postfach-Neuabgleichs (Wartung F8).
    pub const RESYNC: &str = "slowshow://resync";
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
    /// Ablageort des Durchlaufs (E-29).
    playback_path: PathBuf,
    /// Protokoll der Postfach-Abrufe (Wartung F6).
    ///
    /// Eigene Datei wie der Durchlauf: es beschreibt, *was geschehen ist*,
    /// nicht *was da ist* — ein neu aufgebauter Cache soll die Vorgeschichte
    /// nicht mitreissen, gerade dann will man sie lesen.
    pub fetch_log: Mutex<crate::mail::log::FetchLog>,
    fetch_log_path: PathBuf,
    /// Bereits verarbeitete Nachrichten ohne Foto (E-36).
    ///
    /// Der Cache-Index kennt nur Bilder; eine Mail ohne brauchbaren Anhang
    /// waere sonst bei jedem Lauf wieder unbekannt und wuerde erneut geholt.
    pub seen_mails: Mutex<crate::mail::seen::SeenMails>,
    seen_mails_path: PathBuf,
    /// Läuft die Diashow? Pause über Tippen (FA-41) oder Fernsteuerung (FA-55).
    playing: AtomicBool,
    /// Quellen, die gerade abgeglichen werden oder darauf warten (E-43).
    ///
    /// Ersetzt die frühere Sperre „ein Lauf zur Zeit". Sie wies einen zweiten
    /// Aufruf mit einem Fehler ab, und der war damit verloren: ein „Jetzt
    /// abgleichen" waehrend des Hintergrundlaufs schlug fehl, und jeder Takt,
    /// der einen laufenden Sync antraf, uebersprang sich ganz.
    ///
    /// Die Menge dient nur der Entdopplung — dieselbe Quelle soll nicht
    /// zweimal gleichzeitig laufen. Wie viele *verschiedene* zugleich duerfen,
    /// entscheidet [`AppState::sync_slots`].
    sync_claimed: Mutex<HashSet<String>>,
    /// Wie viele Quellen zeitgleich abgeglichen werden duerfen (E-43).
    ///
    /// Ein Semaphor und keine Zahl in einer Schleife: die Grenze gilt ueber
    /// **alle** Ausloeser hinweg — Zeitgeber, Bedienung, MQTT und REST — und
    /// `acquire` reiht der Reihe nach ein, statt abzuweisen. Damit ist die
    /// Warteschlange genau diese Wartezeile.
    sync_slots: Arc<Semaphore>,
    /// Abbruchwunsch für den laufenden Neuabgleich (Wartung F8).
    ///
    /// Ein Neuabgleich über ein volles Postfach läuft Minuten. Ohne Abbruch
    /// bliebe nur, die App zu beenden — und der Rahmen hängt an der Wand.
    pub resync_cancel: AtomicBool,
    /// Urne der intelligenten Mischung (E-29).
    ///
    /// Neben der Playlist und nicht in ihr: die uebrigen Modi sind eine
    /// Reihenfolge mit Position, die intelligente Mischung eine Ziehung ohne
    /// feste Folge. Beides in einen Typ zu pressen haette jede Methode mit
    /// einer Fallunterscheidung belastet.
    pub scheduler: Mutex<Scheduler>,
    /// Der gerade gezeigte Slide der intelligenten Mischung.
    ///
    /// Muss gemerkt werden, weil `current_slide` nicht ziehen darf: die
    /// Ziehung hat Nebenwirkungen (Urne, Boost-Zaehler, Historie), und die
    /// Oberflaeche fragt den aktuellen Stand mehrfach ab.
    smart_slide: Mutex<Option<Slide>>,
    /// Die bereits gezogene, aber noch nicht gezeigte naechste Anzeige (FA-31).
    ///
    /// Eine Zufallsziehung kennt ihr naechstes Bild nicht — ohne diesen Puffer
    /// koennte das Frontend nichts vorwaermen, und `prefetch_window` lieferte
    /// eine Fensterposition, die in dieser Reihenfolge gar nicht weiterwandert.
    /// Genau eine Ziehung im Voraus: sie reicht fuer die Ueberblendung, und
    /// jede weitere haelt eine weitere dekodierte Bitmap in der WebView fest
    /// (R-03).
    smart_next: Mutex<Option<Slide>>,
    /// Hängt der Rahmen hochkant? (E-26)
    ///
    /// Beeinflusst nur die Paarbildung (FA-08). Die Einstellung liefert den
    /// Ausgangswert; bei `Orientation::Auto` kann nur die Oberfläche wissen,
    /// wie das Gerät gerade liegt, und meldet es über `set_frame_orientation`.
    frame_portrait: AtomicBool,
    /// Laengste Kante des Displays in echten Pixeln; 0 = noch nicht gemeldet.
    ///
    /// Nur die WebView kennt sie, deshalb meldet die Oberflaeche sie beim Start
    /// (wie die Ausrichtung, E-26). Sie deckelt die Zielgroesse beim
    /// Aufbereiten — siehe [`AppState::effective_cache_config`].
    display_edge_px: AtomicU32,
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
        let playback_path = data_dir.join("playback.json");
        let fetch_log_path = data_dir.join("fetchlog.json");
        let seen_mails_path = data_dir.join("seenmails.json");

        let state = Self {
            config: Mutex::new(config),
            cache: Mutex::new(cache),
            secrets: Mutex::new(secrets),
            playlist: Mutex::new(Playlist::new()),
            config_store,
            playing: AtomicBool::new(true),
            scheduler: Mutex::new(load_scheduler(&playback_path)),
            playback_path,
            fetch_log: Mutex::new(load_fetch_log(&fetch_log_path)),
            fetch_log_path,
            seen_mails: Mutex::new(load_seen_mails(&seen_mails_path)),
            seen_mails_path,
            smart_slide: Mutex::new(None),
            smart_next: Mutex::new(None),
            frame_portrait: AtomicBool::new(starts_portrait),
            display_edge_px: AtomicU32::new(0),
            sync_claimed: Mutex::new(HashSet::new()),
            sync_slots: Arc::new(Semaphore::new(MAX_PARALLEL_SOURCES)),
            resync_cancel: AtomicBool::new(false),
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

    /// Meldet die Displaygroesse in echten Pixeln (NF-12).
    pub fn set_display_size(&self, width: u32, height: u32) {
        self.display_edge_px
            .store(width.max(height), Ordering::Relaxed);
    }

    /// Cache-Parameter mit der Displayaufloesung als Obergrenze (NF-12, R-03).
    ///
    /// Die Voreinstellung 2560x1600 stammt aus einer Zeit, in der niemand
    /// wusste, worauf das laufen wuerde. Auf einem 1920x1200-Tablet sind das
    /// 16 MB je dekodiertem Bild statt 9 — Speicher, den die WebView haelt und
    /// der auf dem Schirm nicht ankommt. Genau daran hat Android den Renderer
    /// abgeschossen (R-03).
    ///
    /// Gedeckelt wird mit der **laengsten** Kante auf beiden Achsen, nicht
    /// achsweise: der Rahmen kann hochkant haengen (E-26), und ein quadratischer
    /// Deckel gilt fuer beide Lagen. `fit_within` behaelt das Seitenverhaeltnis
    /// bei, ein Bild wird dadurch nie verzerrt und nie groesser als noetig.
    ///
    /// Wirkt beim Aufbereiten, also erst fuer neu geholte Bilder. Was schon im
    /// Cache liegt, behaelt seine Groesse bis zum naechsten Abgleich.
    pub fn effective_cache_config(&self) -> crate::model::CacheConfig {
        let mut cache = self.config_snapshot().cache;
        let edge = self.display_edge_px.load(Ordering::Relaxed);
        if edge > 0 {
            cache.target_width = cache.target_width.min(edge);
            cache.target_height = cache.target_height.min(edge);
        }
        cache
    }

    pub fn set_playing(&self, playing: bool) {
        self.playing.store(playing, Ordering::Relaxed);
    }

    /// Meldet Quellen zum Abgleich an und gibt zurueck, welche davon neu sind
    /// (E-43).
    ///
    /// Was schon laeuft oder wartet, faellt heraus: sonst haette ein zweiter
    /// Ausloeser dieselbe Quelle ein zweites Mal in der Leitung, und beide
    /// Laeufe schrieben abwechselnd in denselben Cache-Eintrag. Der Aufrufer
    /// arbeitet **nur** die zurueckgegebenen Ids ab — eine leere Liste heisst
    /// „ist schon unterwegs", nicht „Fehler".
    pub fn claim_sources(&self, ids: &[String]) -> Vec<String> {
        let Ok(mut claimed) = self.sync_claimed.lock() else {
            return Vec::new();
        };
        ids.iter()
            .filter(|id| claimed.insert((*id).clone()))
            .cloned()
            .collect()
    }

    /// Gibt eine Quelle wieder frei. Gehoert in ein `Drop`, nicht ans Ende
    /// einer Funktion: sonst bliebe sie nach einem Panic bis zum Neustart
    /// angemeldet und wuerde nie wieder abgeglichen.
    pub fn release_source(&self, id: &str) {
        if let Ok(mut claimed) = self.sync_claimed.lock() {
            claimed.remove(id);
        }
    }

    /// Die Warteschlange der Abgleiche (E-43).
    pub fn sync_slots(&self) -> Arc<Semaphore> {
        Arc::clone(&self.sync_slots)
    }

    /// Laeuft oder wartet gerade ein Abgleich? Grundlage der Statusmeldung
    /// nach MQTT und REST (FA-55).
    pub fn is_syncing(&self) -> bool {
        self.sync_claimed
            .lock()
            .map(|c| !c.is_empty())
            .unwrap_or(false)
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
            build_order(
                cache.index(),
                &enabled,
                config.order,
                config.playback.newest_first,
                &config.filter,
                now_ts(),
                seed,
            )
        };

        let len = order.len();
        self.playlist
            .lock()
            .expect("Playlist-Mutex vergiftet")
            .replace(order, seed);

        // Eine vorgezogene Ziehung kann durch den Neuaufbau unzulaessig
        // geworden sein — abgeschaltete Quelle, geaenderter Filter, geloeschtes
        // Bild. Ungeprueft haenge es genau dann an der Wand, wenn jemand es
        // gerade ausgeschlossen hat.
        self.drop_smart_next_if_invalid();

        log::debug!("Playlist neu aufgebaut: {len} Bilder");
    }

    pub fn current_slide(&self) -> Option<Slide> {
        let config = self.config_snapshot();

        if config.order == PlayOrder::Smart {
            if let Some(slide) = self.smart_slide.lock().ok().and_then(|s| s.clone()) {
                return Some(slide);
            }
            // Erster Aufruf nach dem Start: einmal ziehen, damit ueberhaupt
            // etwas an der Wand haengt.
            return self.smart_step();
        }

        let portrait = self.frame_portrait();
        let cache = self.cache.lock().ok()?;
        self.playlist
            .lock()
            .ok()?
            .current(config.pair_mode, portrait, cache.index())
    }

    /// Schaltet die intelligente Mischung einen Schritt weiter (E-28, FA-31).
    ///
    /// Gezeigt wird die beim letzten Schritt vorgezogene Ziehung; im selben
    /// Zug wird die naechste vorgezogen. Dadurch kennt `prefetch_window` das
    /// kommende Bild, bevor es an der Wand haengt — anders als eine sortierte
    /// Reihenfolge gibt eine Urne das sonst nicht her.
    fn smart_step(&self) -> Option<Slide> {
        let now = now_ts();

        let slide = self.take_smart_next().or_else(|| self.draw_slide())?;
        self.remember_smart(&slide, now);
        self.fill_smart_next();
        Some(slide)
    }

    /// Nimmt die vorgezogene Ziehung heraus, ohne sie zu ersetzen.
    fn take_smart_next(&self) -> Option<Slide> {
        self.smart_next.lock().ok()?.take()
    }

    /// Zieht die uebernaechste Anzeige im Voraus (FA-31).
    fn fill_smart_next(&self) {
        let drawn = self.draw_slide();
        if let Ok(mut next) = self.smart_next.lock() {
            *next = drawn;
        }
    }

    /// Verwirft eine vorgezogene Ziehung, die nie zu sehen war.
    ///
    /// Das Bild wandert zurueck in die Urne: es war in diesem Durchlauf noch
    /// nicht an der Reihe, und ohne die Rueckgabe verbrauchte jeder Sync — der
    /// baut die Playlist neu auf — stillschweigend Bilder aus dem Durchlauf.
    /// `Scheduler::remove` raeumt es zugleich aus der Historie, damit das
    /// Zurueckwischen nicht auf ein nie gezeigtes Bild fuehrt (FA-41).
    fn discard_smart_next(&self) {
        let Some(slide) = self.take_smart_next() else {
            return;
        };
        let candidates = self
            .playlist
            .lock()
            .map(|pl| pl.ids_owned())
            .unwrap_or_default();

        let mut rng = SystemRandom;
        if let Ok(mut sched) = self.scheduler.lock() {
            for id in slide.ids() {
                sched.remove(id);
                // Zurueck in die Urne nur, wenn das Bild ueberhaupt noch
                // gezeigt werden darf: ausgeblendet oder aus einer
                // abgeschalteten Quelle gehoert es dort nicht wieder hinein.
                if candidates.iter().any(|c| c == id) {
                    sched.insert(id, &mut rng);
                }
            }
        }
    }

    /// Steht das Bild in der vorgezogenen Ziehung?
    fn smart_next_contains(&self, id: &str) -> bool {
        self.smart_next
            .lock()
            .ok()
            .and_then(|s| s.clone())
            .is_some_and(|slide| slide.ids().contains(&id))
    }

    /// Verwirft die vorgezogene Ziehung, wenn sie nach einem Neuaufbau der
    /// Playlist nicht mehr zulaessig waere (FA-28).
    fn drop_smart_next_if_invalid(&self) {
        let Some(slide) = self.smart_next.lock().ok().and_then(|s| s.clone()) else {
            return;
        };
        let still_valid = self
            .playlist
            .lock()
            .map(|pl| {
                let ids = pl.ids_owned();
                slide.ids().iter().all(|id| ids.iter().any(|c| c == id))
            })
            .unwrap_or(false);

        if !still_valid {
            self.discard_smart_next();
        }
    }

    /// Sichtbares Bild und vorgezogene Ziehung (FA-27, FA-31).
    ///
    /// Das ist in der intelligenten Mischung die Entsprechung zum Fenster der
    /// sortierten Reihenfolgen. Es liest nur ab und zieht selbst nichts: ein
    /// Aufruf zum Vorausladen oder zum Verdraengungsschutz darf die Reihenfolge
    /// nicht weiterschalten.
    fn smart_window(&self) -> Vec<String> {
        let mut ids: Vec<String> = Vec::new();
        for slot in [&self.smart_slide, &self.smart_next] {
            let Ok(guard) = slot.lock() else { continue };
            let Some(slide) = guard.as_ref() else {
                continue;
            };
            for id in slide.ids() {
                if !ids.iter().any(|k| k == id) {
                    ids.push(id.to_string());
                }
            }
        }
        ids
    }

    /// Eine Ziehung der intelligenten Mischung, inklusive Paarbildung (E-28).
    ///
    /// Die Ziehung liefert ein Bild; passt es nicht zum Rahmenformat und ist
    /// der Paar-Modus an, wird ein zweites dazugezogen. Findet sich keines,
    /// laeuft das erste allein.
    ///
    /// Vermerkt die Anzeige **nicht** — das tut erst [`Self::remember_smart`],
    /// wenn der Slide wirklich gezeigt wird. Sonst zaehlte das Vorausladen als
    /// Anzeige, und die Gewichtung aus E-29 rechnete mit Bildern, die niemand
    /// gesehen hat.
    fn draw_slide(&self) -> Option<Slide> {
        let config = self.config_snapshot();
        let portrait = self.frame_portrait();
        let now = now_ts();
        let mut rng = SystemRandom;

        let slide = {
            let cache = self.cache.lock().ok()?;
            let candidates = self.playlist.lock().ok()?.ids_owned();
            let mut sched = self.scheduler.lock().ok()?;

            let first = sched.draw(cache.index(), &candidates, &config.playback, now, &mut rng)?;

            let pairable = |e: &crate::cache::index::CacheEntry| e.is_portrait() != portrait;
            let first_pairable = cache.index().get(&first).is_some_and(pairable);

            if config.pair_mode && first_pairable {
                match sched.draw_partner(
                    cache.index(),
                    &candidates,
                    &config.playback,
                    now,
                    &mut rng,
                    &pairable,
                ) {
                    Some(second) => Slide::Pair {
                        left: first,
                        right: second,
                    },
                    None => Slide::Single { id: first },
                }
            } else {
                Slide::Single { id: first }
            }
        };

        Some(slide)
    }

    /// Schreibt den gezogenen Slide fort und vermerkt die Anzeige (FA-27, E-29).
    fn remember_smart(&self, slide: &Slide, now: i64) {
        if let Ok(mut cache) = self.cache.lock() {
            for id in slide.ids() {
                cache.mark_shown(id, now);
            }
        }
        if let Ok(mut cur) = self.smart_slide.lock() {
            *cur = Some(slide.clone());
        }
    }

    /// Schaltet weiter und merkt die Anzeige für den Ringpuffer (FA-27).
    pub fn advance(&self) -> Option<Slide> {
        self.step(true)
    }

    pub fn back(&self) -> Option<Slide> {
        self.step(false)
    }

    fn step(&self, forward: bool) -> Option<Slide> {
        let config = self.config_snapshot();

        if config.order == PlayOrder::Smart {
            if forward {
                return self.smart_step();
            }
            // Erst die vorgezogene Ziehung verwerfen: sie steht am Ende der
            // Historie, und `Scheduler::back` nimmt sonst *sie* fuer das
            // gerade gezeigte Bild — zurueckgewischt kaeme dann dasselbe Bild
            // noch einmal (FA-41).
            self.discard_smart_next();

            // Zurueck liefert immer ein Einzelbild: die Historie fuehrt Ids,
            // keine Paare. Ein Paar rueckwaerts wieder herzustellen hiesse,
            // die Paarbildung in der Historie mitzufuehren -- Aufwand fuer
            // einen Fall, der beim Zurueckwischen kaum auffaellt.
            let id = self.scheduler.lock().ok()?.back()?;
            let slide = Slide::Single { id };
            self.remember_smart(&slide, now_ts());
            return Some(slide);
        }

        let pair_mode = config.pair_mode;
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

        // Steht das Bild in der vorgezogenen Ziehung, muss die weg — sie waere
        // sonst genau das, was gleich gezeigt wird. Vor `Scheduler::remove`,
        // weil die Ruecknahme das Bild noch einmal in die Urne legt.
        if self.smart_next_contains(id) {
            self.discard_smart_next();
        }

        // Aus der laufenden Urne nehmen (E-29). `Scheduler::pool` zieht aus der
        // Urne und prueft dabei *nicht* gegen die Kandidatenliste — ein gerade
        // ausgeblendetes Bild koennte sonst bis zum Ende des Durchlaufs erneut
        // gezogen werden, und FA-30 hielte nur bis zur naechsten Ziehung.
        if let Ok(mut sched) = self.scheduler.lock() {
            sched.remove(id);
        }
        self.flush_scheduler();

        self.rebuild_playlist();

        // Die intelligente Mischung haelt den laufenden Slide zwischengespeichert
        // (`smart_slide`); die Playlist ist dort nur der Kandidatentopf, und ihre
        // Position sagt nichts darueber aus, was gerade haengt. Ohne eine neue
        // Ziehung gaebe `current_slide()` weiter das eben ausgeblendete Bild
        // zurueck — der Rahmen zeigte es bis zum naechsten Takt weiter, statt
        // sofort weiterzuschalten (FA-30).
        if self.config_snapshot().order == PlayOrder::Smart {
            if let Ok(mut cur) = self.smart_slide.lock() {
                *cur = None;
            }
            // Legt `smart_slide` neu an — oder laesst es leer, wenn das
            // ausgeblendete das letzte Bild war. Steht die vorgezogene Ziehung
            // noch, ist sie genau das naechste Bild und schon vorgewaermt.
            self.smart_step();
            return Ok(());
        }

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
        let config = self.config_snapshot();
        if config.order == PlayOrder::Smart {
            return self.smart_window().into_iter().collect();
        }
        let n = config.cache.prefetch_count as usize;
        self.playlist
            .lock()
            .map(|pl| pl.window(n).into_iter().collect())
            .unwrap_or_default()
    }

    /// Das Vorausladefenster für das Frontend (FA-31).
    pub fn prefetch_window(&self) -> Vec<String> {
        let config = self.config_snapshot();
        // Die Urne kennt nur eine Ziehung im Voraus (E-29). `prefetch_count`
        // gilt deshalb nur fuer die sortierten Reihenfolgen, in denen das
        // naechste Bild aus der Position folgt.
        if config.order == PlayOrder::Smart {
            return self.smart_window();
        }
        let n = config.cache.prefetch_count as usize;
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
        self.flush_scheduler();
    }

    /// Schreibt den Durchlauf der intelligenten Mischung (E-29).
    ///
    /// Eigene Datei neben der Konfiguration statt im Cache-Index: der Index
    /// beschreibt, welche Bilder es gibt, der Durchlauf, welche davon in
    /// dieser Runde schon dran waren. Beides hat verschiedene Lebensdauern —
    /// ein neu aufgebauter Cache soll den Durchlauf nicht mitreissen.
    pub fn flush_scheduler(&self) {
        let Ok(sched) = self.scheduler.lock() else {
            return;
        };
        match serde_json::to_vec(&*sched) {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(&self.playback_path, bytes) {
                    // Kein Grund zur Aufregung: geht der Durchlauf verloren,
                    // beginnt beim naechsten Start ein neuer.
                    log::warn!("Durchlauf nicht schreibbar: {e}");
                }
            }
            Err(e) => log::warn!("Durchlauf nicht serialisierbar: {e}"),
        }
    }

    /// Merkt eine verarbeitete Nachricht und schreibt das Gedaechtnis weg
    /// (E-36).
    pub fn remember_mail(&self, hash: String) {
        let bytes = {
            let Ok(mut seen) = self.seen_mails.lock() else {
                return;
            };
            seen.insert(hash);
            serde_json::to_vec(&*seen)
        };
        if let Ok(bytes) = bytes {
            if let Err(e) = std::fs::write(&self.seen_mails_path, bytes) {
                log::warn!("Mail-Gedaechtnis nicht schreibbar: {e}");
            }
        }
    }

    /// Nimmt einen Abruf ins Protokoll auf und schreibt es weg (F6).
    ///
    /// Sofort auf die Platte statt im Takt des Cache-Index: das Protokoll
    /// waechst um einen Eintrag je Viertelstunde, und wer es liest, tut das
    /// meist nach einem Neustart — genau dann waere ein ungeschriebener
    /// Eintrag der interessanteste.
    pub fn record_fetch(&self, entry: crate::mail::log::FetchLogEntry) {
        let bytes = {
            let Ok(mut log) = self.fetch_log.lock() else {
                return;
            };
            log.push(entry);
            serde_json::to_vec(&*log)
        };
        match bytes {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(&self.fetch_log_path, bytes) {
                    log::warn!("Abruf-Protokoll nicht schreibbar: {e}");
                }
            }
            Err(e) => log::warn!("Abruf-Protokoll nicht serialisierbar: {e}"),
        }
    }
}

/// Laedt das Gedaechtnis verarbeiteter Nachrichten (E-36).
///
/// `rebuild` ist zwingend: die Suchmenge wird nicht mitgespeichert, und ohne
/// sie erinnerte das Gedaechtnis nichts — der Fehler kaeme still zurueck.
fn load_seen_mails(path: &Path) -> crate::mail::seen::SeenMails {
    let mut seen: crate::mail::seen::SeenMails = match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            log::warn!("Mail-Gedaechtnis unlesbar, beginne neu: {e}");
            Default::default()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(e) => {
            log::warn!("Mail-Gedaechtnis nicht lesbar: {e}");
            Default::default()
        }
    };
    seen.rebuild();
    seen
}

/// Laedt das Abruf-Protokoll, falls eines gespeichert ist (F6).
///
/// Wie beim Durchlauf: ein unlesbarer Stand beginnt neu, statt den Start zu
/// verhindern (NF-02). Ein verlorenes Protokoll ist aergerlich, eine App, die
/// deswegen nicht startet, waere schlimmer.
fn load_fetch_log(path: &Path) -> crate::mail::log::FetchLog {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            log::warn!("Abruf-Protokoll unlesbar, beginne neu: {e}");
            crate::mail::log::FetchLog::default()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => crate::mail::log::FetchLog::default(),
        Err(e) => {
            log::warn!("Abruf-Protokoll nicht lesbar: {e}");
            crate::mail::log::FetchLog::default()
        }
    }
}

/// Laedt den Durchlauf, falls einer gespeichert ist (E-29).
///
/// Ein unlesbarer Stand fuehrt zu einem frischen Durchlauf, nicht zum
/// Startabbruch — dieselbe Haltung wie beim Cache-Index (NF-02).
fn load_scheduler(path: &Path) -> Scheduler {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_else(|e| {
            log::warn!("Durchlauf unlesbar, beginne neu: {e}");
            Scheduler::new()
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Scheduler::new(),
        Err(e) => {
            log::warn!("Durchlauf nicht lesbar: {e}");
            Scheduler::new()
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
    fn zielgroesse_folgt_dem_display_nf_12() {
        let dir = TempDir::new("displaygroesse");
        let state = AppState::new(&dir.0).unwrap();

        // Ohne Meldung bleibt die eingestellte Groesse stehen — ein Deckel von
        // null waere ein leeres Bild.
        let ohne = state.effective_cache_config();
        assert_eq!((ohne.target_width, ohne.target_height), (2560, 1600));

        // 1920x1200-Tablet: 2560 faellt auf 1920, 1600 bleibt darunter.
        state.set_display_size(1920, 1200);
        let mit = state.effective_cache_config();
        assert_eq!((mit.target_width, mit.target_height), (1920, 1600));

        // Hochkant gemeldet muss derselbe Deckel herauskommen: der Rahmen darf
        // gedreht haengen (E-26), das Foto wird davon nicht kleiner.
        state.set_display_size(1200, 1920);
        let hochkant = state.effective_cache_config();
        assert_eq!(
            (hochkant.target_width, hochkant.target_height),
            (1920, 1600)
        );
    }

    #[test]
    fn zielgroesse_wird_nie_hochgesetzt_nf_12() {
        // Ein grosses Display darf die Einstellung nicht ueberschreiben —
        // sonst waere aus einem Deckel eine Vorgabe geworden.
        let dir = TempDir::new("displaygroesse-gross");
        let state = AppState::new(&dir.0).unwrap();
        state.set_display_size(3840, 2160);

        let c = state.effective_cache_config();
        assert_eq!((c.target_width, c.target_height), (2560, 1600));
    }

    #[test]
    fn protected_ids_deckt_das_prefetch_fenster_ab_fa_31() {
        let dir = TempDir::new("protected");
        let state = state_mit_bildern(&dir, 10);
        // `prefetch_count` gilt nur fuer die sortierten Reihenfolgen; die
        // intelligente Mischung kennt nur eine Ziehung im Voraus.
        state
            .update_config(|c| {
                c.order = PlayOrder::FileName;
                c.cache.prefetch_count = 3;
            })
            .unwrap();
        state.rebuild_playlist();

        // Aktuelles Bild plus drei vorgeladene.
        assert_eq!(state.protected_ids().len(), 4);
        assert_eq!(state.prefetch_window().len(), 4);
    }

    #[test]
    fn prefetch_folgt_der_intelligenten_mischung_fa_31() {
        // Regression: die Playlist-Position wandert in `PlayOrder::Smart` nie
        // weiter, `window()` lieferte deshalb immer dieselben ersten Ids —
        // vorgewaermt wurden Bilder, die nie an der Wand hingen, waehrend das
        // sichtbare weder vorgeladen noch vor dem Ringpuffer geschuetzt war.
        let dir = TempDir::new("smart-prefetch");
        let state = state_mit_bildern(&dir, 12);

        for _ in 0..5 {
            let sichtbar = state.current_slide().unwrap();
            let fenster = state.prefetch_window();

            assert_eq!(
                fenster.first().map(String::as_str),
                Some(sichtbar.ids()[0]),
                "das Fenster beginnt beim sichtbaren Bild"
            );
            assert!(
                sichtbar
                    .ids()
                    .iter()
                    .all(|id| state.protected_ids().contains(*id)),
                "das sichtbare Bild darf der Ringpuffer nicht verdraengen"
            );

            // Das vorgezogene Bild ist genau das, was als Naechstes kommt.
            let vorgezogen = fenster.last().unwrap().clone();
            let naechstes = state.advance().unwrap();
            assert!(
                naechstes.ids().contains(&vorgezogen.as_str()),
                "vorgewaermt wurde {vorgezogen}, gezeigt wird {naechstes:?}"
            );
        }
    }

    #[test]
    fn prefetch_fenster_bleibt_klein_r_03() {
        // Jede vorgehaltene Id ist eine dekodierte Bitmap in der WebView.
        // Sechs davon waren der Grund, warum Android den Renderer abgeschossen
        // hat; mehr als ein Einzelbild plus einem Paar darf es nie werden.
        let dir = TempDir::new("smart-klein");
        let state = state_mit_bildern(&dir, 40);
        state
            .update_config(|c| c.cache.prefetch_count = 12)
            .unwrap();

        for _ in 0..10 {
            state.advance();
            assert!(
                state.prefetch_window().len() <= 4,
                "hoechstens zwei Slides, war {:?}",
                state.prefetch_window()
            );
        }
    }

    #[test]
    fn zurueckwischen_ueberspringt_die_vorgezogene_ziehung_fa_41() {
        // Die vorgezogene Ziehung steht am Ende der Historie. Ungeprueft nimmt
        // `Scheduler::back` sie fuer das gerade gezeigte Bild — zurueck fuehrte
        // dann auf dasselbe Bild zurueck.
        let dir = TempDir::new("smart-back");
        let state = state_mit_bildern(&dir, 12);

        state.current_slide();
        let vorher = state.advance().unwrap().ids()[0].to_string();
        let jetzt = state.advance().unwrap().ids()[0].to_string();

        let zurueck = state.back().expect("zwei Bilder in der Historie");
        assert!(
            !zurueck.ids().contains(&jetzt.as_str()),
            "zurueck darf nicht auf dem aktuellen Bild stehenbleiben"
        );
        assert!(
            zurueck.ids().contains(&vorher.as_str()),
            "zurueck fuehrt auf das davor gezeigte Bild, war {zurueck:?}"
        );
    }

    #[test]
    fn vorgezogene_ziehung_zaehlt_nicht_als_anzeige_e_29() {
        // Vorausladen ist kein Zeigen: wuerde die vorgezogene Ziehung als
        // Anzeige vermerkt, rechnete die Gewichtung mit Bildern, die niemand
        // gesehen hat, und der Verdraengungsschutz des Ringpuffers ebenso.
        let dir = TempDir::new("smart-shown");
        let state = state_mit_bildern(&dir, 12);

        let sichtbar = state.current_slide().unwrap();
        let vorgezogen: Vec<String> = state
            .prefetch_window()
            .into_iter()
            .filter(|id| !sichtbar.ids().contains(&id.as_str()))
            .collect();
        assert!(!vorgezogen.is_empty(), "es wird etwas vorgezogen");

        let cache = state.cache.lock().unwrap();
        for id in &vorgezogen {
            let e = cache.index().get(id).unwrap();
            assert_eq!(e.show_count, 0, "vorgezogen, aber noch nicht gezeigt: {id}");
            assert!(e.last_shown.is_none(), "kein Anzeigezeitpunkt fuer {id}");
        }
    }

    #[test]
    fn dieselbe_quelle_wird_nicht_zweimal_eingereiht_e_43() {
        // Zwei Laeufe ueber dieselbe Quelle schrieben abwechselnd in denselben
        // Cache-Eintrag. Vorher verhinderte das eine Sperre ueber *alle*
        // Quellen — die wies auch den Abgleich einer ganz anderen Quelle ab.
        let dir = TempDir::new("synclock");
        let state = AppState::new(&dir.0).unwrap();
        assert!(!state.is_syncing(), "am Anfang laeuft nichts");

        let erste = state.claim_sources(&["a".into(), "b".into()]);
        assert_eq!(erste, vec!["a".to_string(), "b".to_string()]);
        assert!(state.is_syncing());

        // Beide sind unterwegs: ein zweiter Ausloeser bekommt nichts zu tun.
        assert!(state.claim_sources(&["a".into(), "b".into()]).is_empty());

        // Eine dritte Quelle wird davon nicht aufgehalten — das ist der
        // Unterschied zur alten Sperre.
        assert_eq!(
            state.claim_sources(&["c".into()]),
            vec!["c".to_string()],
            "eine andere Quelle darf jederzeit dazukommen"
        );
    }

    #[test]
    fn freigegebene_quelle_kann_wieder_eingereiht_werden_e_43() {
        let dir = TempDir::new("syncfree");
        let state = AppState::new(&dir.0).unwrap();

        state.claim_sources(&["a".into()]);
        state.release_source("a");

        assert!(!state.is_syncing(), "nach dem Freigeben laeuft nichts mehr");
        assert_eq!(state.claim_sources(&["a".into()]), vec!["a".to_string()]);
    }

    #[test]
    fn nur_zwei_quellen_gleichzeitig_e_43() {
        // Der Semaphor ist die Warteschlange: die dritte Quelle wartet, statt
        // abgewiesen zu werden. Mehr als zwei gleichzeitig hiesse mehr als zwei
        // dekodierte Vollbilder im Speicher (R-03).
        let dir = TempDir::new("syncslots");
        let state = AppState::new(&dir.0).unwrap();
        let slots = state.sync_slots();

        assert_eq!(MAX_PARALLEL_SOURCES, 2);
        let _a = slots.clone().try_acquire_owned().expect("erster Platz");
        let _b = slots.clone().try_acquire_owned().expect("zweiter Platz");
        assert!(
            slots.clone().try_acquire_owned().is_err(),
            "der dritte muss warten"
        );

        drop(_a);
        assert!(
            slots.try_acquire_owned().is_ok(),
            "nach dem Freiwerden geht es weiter"
        );
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

    /// `n` Bilder in einer aktiven Quelle — die Grundlage der Ausblende-Tests.
    fn state_mit_bildern(dir: &TempDir, n: usize) -> AppState {
        let state = AppState::new(&dir.0).unwrap();
        add_source(&state, "s", true);
        let mut cache = state.cache.lock().unwrap();
        for i in 0..n {
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
        drop(cache);
        state.rebuild_playlist();
        state
    }

    #[test]
    fn exclude_image_zieht_in_der_intelligenten_mischung_neu_fa_30() {
        // Regression: in `PlayOrder::Smart` haengt der laufende Slide in
        // `smart_slide` und nicht an der Playlist-Position. Ohne neue Ziehung
        // meldete `current_slide()` nach dem Ausblenden weiter dasselbe Bild —
        // der Rahmen wechselte erst beim naechsten Takt.
        let dir = TempDir::new("exclude-smart");
        let state = state_mit_bildern(&dir, 4);
        assert_eq!(
            state.config_snapshot().order,
            PlayOrder::Smart,
            "der Test prueft die Voreinstellung"
        );

        let sichtbar = state.current_slide().unwrap();
        let ausgeblendet = sichtbar.ids()[0].to_string();

        state.exclude_image(&ausgeblendet).unwrap();

        let danach = state.current_slide().expect("es sind noch drei Bilder da");
        assert!(
            !danach.ids().contains(&ausgeblendet.as_str()),
            "das ausgeblendete Bild darf nicht stehenbleiben, war {danach:?}"
        );
    }

    #[test]
    fn exclude_image_nimmt_das_bild_aus_der_urne_fa_30() {
        // Ein Bild aus dem Browser ausblenden, waehrend ein anderes haengt:
        // `Scheduler::pool` zieht aus der Urne, *ohne* gegen die
        // Kandidatenliste zu pruefen. Ohne `Scheduler::remove` kaeme das
        // ausgeblendete Bild bis zum Ende des Durchlaufs wieder.
        let dir = TempDir::new("exclude-urne");
        let state = state_mit_bildern(&dir, 4);

        let sichtbar = state.current_slide().unwrap().ids()[0].to_string();
        let ausgeblendet = state
            .prefetch_window()
            .into_iter()
            .find(|id| *id != sichtbar)
            .expect("drei weitere Bilder in der Urne");

        state.exclude_image(&ausgeblendet).unwrap();

        for _ in 0..20 {
            let slide = state.advance().unwrap();
            assert!(
                !slide.ids().contains(&ausgeblendet.as_str()),
                "ausgeblendet heisst ausgeblendet, war {slide:?}"
            );
        }
    }

    #[test]
    fn exclude_image_raeumt_die_historie_fa_41() {
        // Zurueckwischen darf nicht auf das gerade ausgeblendete Bild fuehren —
        // die Historie der intelligenten Mischung fuehrt es sonst weiter.
        let dir = TempDir::new("exclude-historie");
        let state = state_mit_bildern(&dir, 4);

        state.current_slide();
        state.advance();
        let ausgeblendet = state.advance().unwrap().ids()[0].to_string();

        state.exclude_image(&ausgeblendet).unwrap();

        let zurueck = state.back().expect("zwei aeltere Bilder in der Historie");
        assert!(
            !zurueck.ids().contains(&ausgeblendet.as_str()),
            "das ausgeblendete Bild darf nicht zurueckkommen, war {zurueck:?}"
        );
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

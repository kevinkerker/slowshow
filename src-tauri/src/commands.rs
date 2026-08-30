//! Tauri-Kommandos — die gesamte Schnittstelle zwischen Frontend und Backend.
//!
//! Grundsatz: hier fließen ausschließlich Metadaten und Steuerbefehle.
//! Bilddaten gehen nur über das Asset-Protokoll `slowshow://img/<id>` ans
//! Frontend (NF-13, R-03) — und sie kommen auch nur über Rust herein, lokale
//! Ordner eingeschlossen (siehe `sources::local`).

use crate::cache::{CacheEntry, CacheStats};
use crate::model::{AppConfig, Source, SourceKind};
use crate::playlist::Slide;
use crate::schedule::DisplayState;
use crate::sources::{Album, NextcloudClient, RemoteClient};
use crate::state::{events, now_ts, AppState};
use crate::sync::{self, SyncReport};
use std::collections::HashSet;
use tauri::{AppHandle, Emitter, Manager, State};

type Res<T> = Result<T, String>;

// ── Konfiguration ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> AppConfig {
    state.config_snapshot()
}

/// Übernimmt eine vollständige Konfiguration aus der Einstellungsoberfläche.
///
/// Quellen werden bewusst *nicht* mitgeschrieben: die haben eigene Kommandos,
/// weil dort Passwörter im Spiel sind, die nicht durch jeden Speichervorgang
/// laufen sollen.
#[tauri::command]
pub fn set_config(app: AppHandle, state: State<'_, AppState>, config: AppConfig) -> Res<AppConfig> {
    let before = state.config_snapshot();

    let updated = state.update_config(|c| {
        let sources = std::mem::take(&mut c.sources);
        *c = config;
        c.sources = sources;
    })?;

    if updated.order != before.order {
        state.rebuild_playlist();
    }

    // Heimnetz-Steuerung sofort übernehmen (FA-55). Ohne das bliebe der
    // Schalter in den Einstellungen bis zum nächsten App-Start wirkungslos —
    // auf einem Gerät, das man selten neu startet, faktisch kaputt.
    let remote_changed = updated.remote.enabled != before.remote.enabled
        || updated.remote.port != before.remote.port
        || updated.remote.token != before.remote.token;
    if remote_changed {
        app.state::<crate::remote::RemoteServer>()
            .apply_config(&app);
    }

    // Wie bei der REST-Steuerung: sofort uebernehmen, sonst bliebe der
    // Schalter bis zum naechsten App-Start wirkungslos.
    let m = &updated.mqtt;
    let mb = &before.mqtt;
    if m.enabled != mb.enabled
        || m.host != mb.host
        || m.port != mb.port
        || m.username != mb.username
        || m.base_topic != mb.base_topic
        || m.discovery != mb.discovery
        || m.discovery_prefix != mb.discovery_prefix
    {
        app.state::<crate::mqtt::MqttService>().apply_config(&app);
    }
    let display = state.display_state();
    crate::brightness::apply(display.brightness);
    crate::orientation::apply(updated.orientation);
    let _ = app.emit(events::CONFIG, &updated);
    let _ = app.emit(events::DISPLAY, display);
    Ok(updated)
}

/// Konfiguration als JSON exportieren (FA-45). Ohne Zugangsdaten.
#[tauri::command]
pub fn export_config(state: State<'_, AppState>) -> Res<String> {
    serde_json::to_string_pretty(&state.config_snapshot()).map_err(|e| e.to_string())
}

/// Konfiguration aus JSON übernehmen (FA-45).
///
/// Quellen kommen mit, ihre Passwörter naturgemäß nicht — die müssen nach dem
/// Import neu gesetzt werden.
#[tauri::command]
pub fn import_config(app: AppHandle, state: State<'_, AppState>, json: String) -> Res<AppConfig> {
    let imported: AppConfig = serde_json::from_str(&json)
        .map_err(|e| format!("Datei ist keine gültige Konfiguration: {e}"))?;

    let updated = state.update_config(|c| *c = imported)?;
    state.rebuild_playlist();
    let _ = app.emit(events::CONFIG, &updated);
    Ok(updated)
}

// ── Anzeige und Zeitplan ─────────────────────────────────────────────────────

#[tauri::command]
pub fn get_display_state(state: State<'_, AppState>) -> DisplayState {
    state.display_state()
}

#[tauri::command]
pub fn current_slide(state: State<'_, AppState>) -> Option<Slide> {
    state.current_slide()
}

#[tauri::command]
pub fn next_slide(state: State<'_, AppState>) -> Option<Slide> {
    state.advance()
}

#[tauri::command]
pub fn prev_slide(state: State<'_, AppState>) -> Option<Slide> {
    state.back()
}

#[tauri::command]
pub fn set_playing(state: State<'_, AppState>, playing: bool) {
    state.set_playing(playing);
}

#[tauri::command]
pub fn is_playing(state: State<'_, AppState>) -> bool {
    state.is_playing()
}

/// Die als Nächstes anzuzeigenden Bilder — das Frontend lädt sie vor (FA-31).
#[tauri::command]
pub fn prefetch_window(state: State<'_, AppState>) -> Vec<String> {
    state.prefetch_window()
}

/// Metadaten eines Bildes für die Einblendungen (FA-07).
#[tauri::command]
pub fn image_info(state: State<'_, AppState>, id: String) -> Option<CacheEntry> {
    state.cache.lock().ok()?.index().get(&id).cloned()
}

/// Bild aus der Diashow nehmen, ohne es an der Quelle zu löschen (FA-30).
#[tauri::command]
pub fn exclude_image(app: AppHandle, state: State<'_, AppState>, id: String) -> Res<()> {
    state.exclude_image(&id)?;

    if let Some(slide) = state.current_slide() {
        let _ = app.emit(events::SLIDE, slide);
    }
    Ok(())
}

#[tauri::command]
pub fn include_image(state: State<'_, AppState>, id: String) -> Res<()> {
    let mut cache = state.cache.lock().map_err(|_| "Cache gesperrt")?;
    if !cache.set_excluded(&id, false) {
        return Err(format!("Unbekanntes Bild: {id}"));
    }
    cache.flush().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cache_stats(state: State<'_, AppState>) -> CacheStats {
    let max = state.config_snapshot().cache.max_bytes;
    state
        .cache
        .lock()
        .map(|c| c.stats(max))
        .unwrap_or(CacheStats {
            images: 0,
            bytes: 0,
            max_bytes: max,
            excluded: 0,
            thumb_bytes: 0,
        })
}

/// Setzt die eingestellte Ausrichtung am Fenster durch (E-26).
///
/// Wird vom Frontend beim Start gerufen und **nicht** in `setup()`: Tauris
/// `_start_app` läuft auf einem eigenen Thread, `MainActivity.onCreate` kehrt
/// sofort zurück, und `nativeRegisterActivity` rennt damit gegen `setup()`. Wer
/// die Ausrichtung dort setzt, gewinnt das Rennen mal und verliert es mal —
/// gemessen am Gerät blieb der Aufruf wirkungslos, ohne eine Spur im Log.
///
/// Wenn die WebView so weit ist, steht die Brücke sicher.
#[tauri::command]
pub fn apply_orientation(state: State<'_, AppState>) {
    crate::orientation::apply(state.config_snapshot().orientation);
}

/// Meldet, wie der Rahmen gerade hängt (E-26).
///
/// Nötig nur für `Orientation::Auto`: dann bestimmt der Lagesensor die
/// Ausrichtung, und allein die Oberfläche sieht das Ergebnis. Bei fester
/// Einstellung schickt sie denselben Wert, den die Konfiguration ohnehin
/// vorgibt — das ist billiger als eine Sonderbehandlung an beiden Enden.
///
/// Wirkt sich ausschließlich auf die Paarbildung aus (FA-08).
#[tauri::command]
pub fn set_frame_orientation(state: State<'_, AppState>, portrait: bool) {
    state.set_frame_portrait(portrait);
}

// ── Bild-Browser (E-25) ──────────────────────────────────────────────────────

/// Was der Browser anzeigen soll.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ImageFilter {
    #[default]
    All,
    /// Nur ausgeschlossene Bilder (FA-30).
    Excluded,
    /// Nur solche, die in der Diashow laufen.
    Included,
}

/// Ein Ausschnitt des Cache-Index für den Bild-Browser.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagePage {
    pub entries: Vec<CacheEntry>,
    /// Anzahl aller Einträge, die zum Filter passen — für die Kopfzeile und
    /// damit das Frontend weiß, wie weit es blättern kann.
    pub total: usize,
}

/// Obergrenze je Abruf.
///
/// Der Browser lädt nachrangig nach, während gescrollt wird. Ohne Grenze würde
/// ein Cache mit 5 000 Bildern rund anderthalb Megabyte JSON durch die
/// IPC-Brücke schieben — genau die Art Last, gegen die R-03 gerichtet ist.
const PAGE_LIMIT: usize = 200;

/// Liefert einen sortierten Ausschnitt des Cache-Index (E-25).
///
/// Sortiert nach Aufnahmedatum, neueste zuerst — dieselbe Reihenfolge, die
/// `PlayOrder::TakenAt` verwendet (FA-03), mit denselben Rückfällen. Wer im
/// Browser nach einem Bild sucht, sucht es dort, wo es zeitlich hingehört.
#[tauri::command]
pub fn image_page(
    state: State<'_, AppState>,
    offset: usize,
    limit: usize,
    filter: ImageFilter,
) -> ImagePage {
    let Ok(cache) = state.cache.lock() else {
        return ImagePage {
            entries: Vec::new(),
            total: 0,
        };
    };

    let mut matching: Vec<&CacheEntry> = cache
        .index()
        .values()
        .filter(|e| match filter {
            ImageFilter::All => true,
            ImageFilter::Excluded => e.excluded,
            ImageFilter::Included => !e.excluded,
        })
        .collect();

    // Zweiter Schlüssel ist die Id: bei gleichem Zeitstempel — etwa einer
    // Serie ohne EXIF — wäre die Reihenfolge sonst von der HashMap abhängig
    // und spränge zwischen zwei Abrufen derselben Seite.
    matching.sort_by(|a, b| b.sort_time().cmp(&a.sort_time()).then(a.id.cmp(&b.id)));

    let total = matching.len();
    let entries = matching
        .into_iter()
        .skip(offset)
        .take(limit.min(PAGE_LIMIT))
        .cloned()
        .collect();

    ImagePage { entries, total }
}

// ── Quellenverwaltung ────────────────────────────────────────────────────────

/// Anzahl zwischengespeicherter Bilder je Quelle — für die Quellenliste.
#[tauri::command]
pub fn source_counts(state: State<'_, AppState>) -> std::collections::HashMap<String, usize> {
    let config = state.config_snapshot();
    let Ok(cache) = state.cache.lock() else {
        return Default::default();
    };
    config
        .sources
        .iter()
        .map(|s| (s.id.clone(), cache.index().count_for_source(&s.id)))
        .collect()
}

/// Legt eine Quelle an (FA-25). Das Passwort wird verschlüsselt abgelegt (NF-05).
#[tauri::command]
pub fn add_source(
    app: AppHandle,
    state: State<'_, AppState>,
    source: Source,
    password: Option<String>,
) -> Res<AppConfig> {
    if let Some(pw) = password {
        let reference = password_ref(&source.kind).unwrap_or(&source.id).to_string();
        state
            .secrets
            .lock()
            .map_err(|_| "Zugangsdaten gesperrt")?
            .set(&reference, &pw)
            .map_err(|e| e.to_string())?;
    }

    let updated = state.update_config(|c| {
        c.sources.retain(|s| s.id != source.id);
        c.sources.push(source);
    })?;

    state.rebuild_playlist();
    let _ = app.emit(events::CONFIG, &updated);
    Ok(updated)
}

/// Aktualisiert eine Quelle. Ein leeres Passwort lässt das gespeicherte stehen.
#[tauri::command]
pub fn update_source(
    app: AppHandle,
    state: State<'_, AppState>,
    source: Source,
    password: Option<String>,
) -> Res<AppConfig> {
    if let Some(pw) = password.filter(|p| !p.is_empty()) {
        let reference = password_ref(&source.kind).unwrap_or(&source.id).to_string();
        state
            .secrets
            .lock()
            .map_err(|_| "Zugangsdaten gesperrt")?
            .set(&reference, &pw)
            .map_err(|e| e.to_string())?;
    }

    let enabled_changed = state
        .source_by_id(&source.id)
        .map(|old| old.enabled != source.enabled)
        .unwrap_or(true);

    let updated = state.update_config(|c| {
        if let Some(slot) = c.sources.iter_mut().find(|s| s.id == source.id) {
            *slot = source;
        }
    })?;

    if enabled_changed {
        state.rebuild_playlist();
    }
    let _ = app.emit(events::CONFIG, &updated);
    Ok(updated)
}

/// Entfernt eine Quelle samt zwischengespeicherter Bilder und Passwort.
#[tauri::command]
pub fn remove_source(app: AppHandle, state: State<'_, AppState>, id: String) -> Res<AppConfig> {
    let updated = state.update_config(|c| c.sources.retain(|s| s.id != id))?;

    {
        let mut cache = state.cache.lock().map_err(|_| "Cache gesperrt")?;
        let n = cache.remove_source(&id);
        let _ = cache.flush();
        log::info!("Quelle '{id}' entfernt, {n} Bilder aus dem Cache gelöscht");
    }

    // Verwaiste Passwörter aufräumen.
    // Das MQTT-Passwort gehoert zu keiner Quelle und muss beim Aufraeumen
    // ausdruecklich behalten werden.
    let mut keep: Vec<String> = updated
        .sources
        .iter()
        .map(|s| password_ref(&s.kind).unwrap_or(&s.id).to_string())
        .collect();
    keep.push(crate::mqtt::PASSWORD_REF.to_string());
    if let Ok(mut secrets) = state.secrets.lock() {
        let _ = secrets.retain_refs(&keep);
    }

    state.rebuild_playlist();
    let _ = app.emit(events::CONFIG, &updated);
    Ok(updated)
}

/// Legt das Broker-Passwort verschlüsselt ab (NF-05) und verbindet neu.
///
/// Eigenes Kommando, weil das Passwort nicht durch jeden Speichervorgang der
/// Konfiguration laufen soll — und weil es damit auch nicht im Export aus
/// FA-45 landet.
#[tauri::command]
pub fn set_mqtt_password(app: AppHandle, state: State<'_, AppState>, password: String) -> Res<()> {
    {
        let mut secrets = state.secrets.lock().map_err(|_| "Zugangsdaten gesperrt")?;
        if password.is_empty() {
            secrets.remove(crate::mqtt::PASSWORD_REF)
        } else {
            secrets.set(crate::mqtt::PASSWORD_REF, &password)
        }
        .map_err(|e| e.to_string())?;
    }
    app.state::<crate::mqtt::MqttService>().apply_config(&app);
    Ok(())
}

/// Ist ein Broker-Passwort hinterlegt? Die Oberfläche zeigt damit an, ob das
/// Feld leer gelassen werden darf.
#[tauri::command]
pub fn has_mqtt_password(state: State<'_, AppState>) -> bool {
    state
        .secrets
        .lock()
        .map(|s| s.get(crate::mqtt::PASSWORD_REF).is_some())
        .unwrap_or(false)
}

/// Zustand der MQTT-Verbindung — gestartet, verbunden, letzter Fehler.
#[tauri::command]
pub fn mqtt_status(app: AppHandle) -> crate::mqtt::MqttStatus {
    app.state::<crate::mqtt::MqttService>().status()
}

/// Verbindet neu.
///
/// Nach einem korrigierten Tippfehler in der Adresse spart das den Umweg,
/// den Schalter aus- und wieder einzuschalten.
#[tauri::command]
pub fn mqtt_reconnect(app: AppHandle) -> crate::mqtt::MqttStatus {
    let service = app.state::<crate::mqtt::MqttService>();
    service.apply_config(&app);
    service.status()
}

/// Prüft die Erreichbarkeit einer Quelle, bevor sie gespeichert wird.
#[tauri::command]
pub async fn test_source(app: AppHandle, source: Source, password: String) -> Res<()> {
    let client = RemoteClient::from_source(&app, &source, &password)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Für diese Quelle gibt es keinen Verbindungstest".to_string())?;
    client.test().await.map_err(|e| e.to_string())
}

/// Listet die Photos-Alben einer Nextcloud zur Auswahl (FA-23).
#[tauri::command]
pub async fn list_nextcloud_albums(
    url: String,
    username: String,
    password: String,
    allow_insecure_tls: bool,
) -> Res<Vec<Album>> {
    // Für das Auflisten ist noch kein Album gewählt — die Wurzel genügt.
    let client = NextcloudClient::new(&url, &username, &password, "", true, allow_insecure_tls)
        .map_err(|e| e.to_string())?;
    client.list_albums().await.map_err(|e| e.to_string())
}

// ── Synchronisierung ─────────────────────────────────────────────────────────

/// Stößt einen Sync an (FA-28). `source_id = None` synchronisiert alle Quellen.
#[tauri::command]
pub async fn sync_now(app: AppHandle, source_id: Option<String>) -> Res<Vec<SyncReport>> {
    run_sync(&app, source_id, false).await
}

/// Führt die Synchronisierung aus.
///
/// `only_due` beschränkt auf Quellen, deren Intervall abgelaufen ist — so
/// arbeitet der Hintergrund-Zeitgeber (FA-28).
pub async fn run_sync(
    app: &AppHandle,
    source_id: Option<String>,
    only_due: bool,
) -> Res<Vec<SyncReport>> {
    if !app.state::<AppState>().try_begin_sync() {
        return Err("Es läuft bereits eine Synchronisierung".into());
    }

    // Die Sperre wird über `Drop` gelöst, nicht am Ende der Funktion. Ein
    // Panic in einer der `expect`-Stellen des Sync-Pfads würde die Sperre sonst
    // dauerhaft gesetzt lassen und jede weitere Synchronisierung bis zum
    // Neustart der App verhindern — auf einem unbeaufsichtigten Gerät fiele das
    // niemandem auf.
    let _guard = SyncGuard(app.clone());
    run_sync_inner(app, source_id, only_due).await
}

/// Gibt die Sync-Sperre frei, sobald sie den Gültigkeitsbereich verlässt.
struct SyncGuard(AppHandle);

impl Drop for SyncGuard {
    fn drop(&mut self) {
        self.0.state::<AppState>().end_sync();
    }
}

async fn run_sync_inner(
    app: &AppHandle,
    source_id: Option<String>,
    only_due: bool,
) -> Res<Vec<SyncReport>> {
    let now = now_ts();
    let (sources, cfg) = {
        let state = app.state::<AppState>();
        let config = state.config_snapshot();
        let sources: Vec<Source> = config
            .sources
            .iter()
            .filter(|s| source_id.as_deref().map(|id| s.id == id).unwrap_or(true))
            .filter(|s| !only_due || s.is_sync_due(now))
            .cloned()
            .collect();
        (sources, config.cache)
    };

    log::info!(
        "Sync angestossen: {} Quelle(n) in Frage kommend (only_due={only_due})",
        sources.len()
    );

    let mut reports = Vec::new();
    for source in sources {
        let password = {
            let state = app.state::<AppState>();
            let reference = password_ref(&source.kind).unwrap_or(&source.id).to_string();
            let guard = state.secrets.lock().map_err(|_| "Zugangsdaten gesperrt")?;
            guard.get(&reference).unwrap_or_default().to_string()
        };

        let client = match RemoteClient::from_source(app, &source, &password) {
            Ok(Some(c)) => c,
            Ok(None) => {
                log::warn!("'{}': kein Client fuer diese Quellenart", source.name);
                continue;
            }
            Err(e) => {
                let mut r = SyncReport::for_source(&source.id);
                r.error = Some(e.to_string());
                reports.push(r);
                continue;
            }
        };

        let protected: HashSet<String> = app.state::<AppState>().protected_ids();
        let report = {
            let state = app.state::<AppState>();
            let progress_app = app.clone();
            sync::sync_source(
                &source,
                &client,
                &state.cache,
                &cfg,
                &protected,
                now,
                &move |p| {
                    let st = progress_app.state::<AppState>();

                    // Die Diashow soll nicht warten, bis der letzte von
                    // mehreren tausend Downloads durch ist. Sobald das erste
                    // Bild im Cache liegt, wird gezeigt; danach wächst die
                    // Playlist in Schritten mit.
                    //
                    // Der Schritt ist bewusst klein: bei 25 blieb die Playlist
                    // während eines Laufs über einen Ordner mit einem Dutzend
                    // Fotos bei genau einem Eintrag stehen — die Diashow zeigte
                    // dann minutenlang dasselbe Bild, weil sie im Kreis lief.
                    //
                    // Nicht bei jedem Bild neu aufbauen: `build_order`
                    // sortiert den gesamten Index, das wäre bei 5 000 Bildern
                    // pro Download verschwendete Rechenzeit (NF-06).
                    let empty = st.playlist.lock().map(|pl| pl.is_empty()).unwrap_or(false);
                    if p.stored > 0 && (empty || p.stored % 5 == 0) {
                        st.rebuild_playlist();
                        if empty {
                            if let Some(slide) = st.current_slide() {
                                let _ = progress_app.emit(events::SLIDE, slide);
                            }
                        }
                    }

                    let _ = progress_app.emit(events::SYNC_PROGRESS, &p);
                },
            )
            .await
        };

        // Zeitstempel nur bei Erfolg fortschreiben, sonst würde ein Ausfall
        // den nächsten Versuch um ein volles Intervall verschieben.
        if report.error.is_none() {
            let state = app.state::<AppState>();
            let _ = state.update_config(|c| {
                if let Some(s) = c.sources.iter_mut().find(|s| s.id == source.id) {
                    s.last_sync = Some(now);
                }
            });
        }

        if report.changed_anything() {
            app.state::<AppState>().rebuild_playlist();
        }
        let _ = app.emit(events::SYNC, &report);
        reports.push(report);
    }

    Ok(reports)
}

// ── Hilfen ───────────────────────────────────────────────────────────────────

/// Schlüssel, unter dem das Passwort einer Quelle liegt.
fn password_ref(kind: &SourceKind) -> Option<&str> {
    match kind {
        SourceKind::Local { .. } => None,
        SourceKind::WebDav { password_ref, .. } | SourceKind::Nextcloud { password_ref, .. } => {
            Some(password_ref)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_ref_liefert_fuer_lokale_quellen_nichts() {
        let local = SourceKind::Local {
            saf_uri: "{}".into(),
            display_path: "DCIM".into(),
        };
        assert_eq!(password_ref(&local), None);
    }

    #[test]
    fn password_ref_liest_beide_entfernten_quellen() {
        let dav = SourceKind::WebDav {
            url: "https://nas".into(),
            username: "u".into(),
            password_ref: "ref-nas".into(),
            allow_insecure_tls: false,
        };
        assert_eq!(password_ref(&dav), Some("ref-nas"));

        let nc = SourceKind::Nextcloud {
            url: "https://cloud".into(),
            username: "u".into(),
            password_ref: "ref-cloud".into(),
            album: "A".into(),
            use_preview_api: true,
            allow_insecure_tls: false,
        };
        assert_eq!(password_ref(&nc), Some("ref-cloud"));
    }
}

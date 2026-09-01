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

/// Konfiguration als JSON exportieren (FA-45, Wartung F12).
///
/// Ohne Zugangsdaten: die Passwoerter liegen im Schluesselspeicher, nicht in
/// der Konfiguration, und werden bewusst nicht mitgesichert. Nach dem
/// Einspielen muessen sie neu eingegeben werden — das sagt die Oberflaeche.
///
/// Mit `schemaVersion`, damit eine spaetere Fassung erkennen kann, womit sie
/// es zu tun hat.
#[tauri::command]
pub fn export_config(state: State<'_, AppState>) -> Res<String> {
    let backup = crate::maintenance::Backup {
        schema_version: crate::maintenance::SCHEMA_VERSION,
        created_at: now_ts(),
        config: state.config_snapshot(),
    };
    serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())
}

/// Konfiguration aus JSON übernehmen (FA-45).
///
/// Quellen kommen mit, ihre Passwörter naturgemäß nicht — die müssen nach dem
/// Import neu gesetzt werden.
#[tauri::command]
pub fn import_config(app: AppHandle, state: State<'_, AppState>, json: String) -> Res<AppConfig> {
    // Fassung zuerst pruefen: eine Sicherung aus einer neueren App wird
    // abgelehnt, statt sie halb zu uebernehmen (F12).
    let backup = crate::maintenance::parse_backup(&json).map_err(|e| e.to_string())?;
    let imported = backup.config;

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

    // Auch `None` wird gemeldet: war es das letzte Bild, muss der Rahmen es vom
    // Schirm nehmen. Vorher blieb genau dann das ausgeblendete Bild stehen.
    let _ = app.emit(events::SLIDE, state.current_slide());
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

/// Jahre und Absender, nach denen sich filtern lässt — mit Anzahl (F5).
///
/// Die Anzahl steht dabei, weil eine Jahresliste ohne sie nichts sagt: „1987"
/// mit drei Bildern ist eine andere Auswahl als „1987" mit vierhundert.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterFacets {
    /// Jahre mit Anzahl, neueste zuerst.
    pub years: Vec<(i32, usize)>,
    /// Absender mit Anzahl, häufigste zuerst.
    pub senders: Vec<(String, usize)>,
    /// Bilder ohne Aufnahmedatum.
    pub undated: usize,
}

#[tauri::command]
pub fn filter_facets(state: State<'_, AppState>) -> FilterFacets {
    use chrono::{Datelike, TimeZone, Utc};
    use std::collections::HashMap;

    let Ok(cache) = state.cache.lock() else {
        return FilterFacets {
            years: Vec::new(),
            senders: Vec::new(),
            undated: 0,
        };
    };

    let mut years: HashMap<i32, usize> = HashMap::new();
    let mut senders: HashMap<String, usize> = HashMap::new();
    let mut undated = 0;

    for e in cache.index().values() {
        if e.excluded || e.is_quarantined() {
            continue;
        }
        match e.taken_at {
            Some(ts) => {
                if let Some(d) = Utc.timestamp_opt(ts, 0).single() {
                    *years.entry(d.year()).or_insert(0) += 1;
                }
            }
            None => undated += 1,
        }
        if let Some(m) = &e.mail {
            *senders.entry(m.sender.clone()).or_insert(0) += 1;
        }
    }

    let mut years: Vec<(i32, usize)> = years.into_iter().collect();
    years.sort_by_key(|(year, _)| std::cmp::Reverse(*year));

    let mut senders: Vec<(String, usize)> = senders.into_iter().collect();
    senders.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    FilterFacets {
        years,
        senders,
        undated,
    }
}

/// Gibt ein Foto aus der Quarantäne frei (F4, E-31).
///
/// Mit `trust_sender` wandert der Absender dauerhaft in die Freigabeliste und
/// alle seine wartenden Fotos werden mit freigegeben — „einmal tippen und die
/// Tante ist bekannt", wie es das Papier vorsieht. Ohne das gilt die Freigabe
/// nur für dieses eine Bild.
#[tauri::command]
pub fn release_quarantine(
    state: State<'_, AppState>,
    id: String,
    trust_sender: bool,
) -> Res<usize> {
    let sender = {
        let cache = state.cache.lock().map_err(|_| "Cache gesperrt")?;
        cache
            .index()
            .get(&id)
            .and_then(|e| e.mail.as_ref())
            .map(|m| m.sender.clone())
    };
    let Some(sender) = sender else {
        return Err(format!("Unbekanntes oder quellenfremdes Bild: {id}"));
    };

    let mut freed = 0;
    {
        let mut cache = state.cache.lock().map_err(|_| "Cache gesperrt")?;
        if trust_sender {
            let ids: Vec<String> = cache
                .index()
                .values()
                .filter(|e| {
                    e.mail
                        .as_ref()
                        .is_some_and(|m| m.quarantined && m.sender == sender)
                })
                .map(|e| e.id.clone())
                .collect();
            for id in ids {
                if cache.release_quarantine(&id) {
                    freed += 1;
                }
            }
        } else if cache.release_quarantine(&id) {
            freed = 1;
        }
        cache.flush().map_err(|e| e.to_string())?;
    }

    if trust_sender {
        state.update_config(|c| {
            for s in c.sources.iter_mut() {
                if let SourceKind::Mail {
                    allowed_senders, ..
                } = &mut s.kind
                {
                    if !allowed_senders
                        .iter()
                        .any(|a| a.eq_ignore_ascii_case(&sender))
                    {
                        allowed_senders.push(sender.clone());
                    }
                }
            }
        })?;
    }

    state.rebuild_playlist();
    Ok(freed)
}

/// Freigegebene Absender eines Postfachs, mit der Zahl ihrer Fotos (F4).
///
/// Die Liste selbst steht in der Konfiguration, die das Frontend ohnehin hat —
/// die Zahl der Fotos nicht. Sie ist die Grundlage der Rueckfrage beim
/// Entfernen: ohne sie waere „Fotos zurueck in die Quarantaene?" eine Frage
/// ins Blaue.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowedSender {
    pub address: String,
    pub photo_count: usize,
}

#[tauri::command]
pub fn allowed_senders(state: State<'_, AppState>, source_id: String) -> Res<Vec<AllowedSender>> {
    let addresses = {
        let config = state.config_snapshot();
        let Some(source) = config.sources.iter().find(|s| s.id == source_id) else {
            return Err(format!("Unbekannte Quelle: {source_id}"));
        };
        match &source.kind {
            SourceKind::Mail {
                allowed_senders, ..
            } => allowed_senders.clone(),
            _ => return Err("Diese Quelle ist kein Postfach".into()),
        }
    };

    let cache = state.cache.lock().map_err(|_| "Cache gesperrt")?;
    Ok(addresses
        .into_iter()
        .map(|address| AllowedSender {
            photo_count: cache.sender_photo_count(&source_id, &address),
            address,
        })
        .collect())
}

/// Nimmt einen Absender von der Freigabeliste (F4).
///
/// Gegenstueck zu `release_quarantine(trust_sender: true)`. Ohne diesen Weg
/// waere die Liste eine Einbahnstrasse: ein einmal aus Versehen bestaetigter
/// Absender liesse sich nie wieder zuruecknehmen.
///
/// `requarantine` entscheidet ueber die bereits vorhandenen Fotos. Beides ist
/// vertretbar, deshalb fragt die Oberflaeche (E-32): nur kuenftige Mails
/// wieder pruefen, oder auch die alten Bilder erneut warten lassen.
///
/// Gibt zurueck, wie viele Fotos in die Quarantaene zurueckgegangen sind.
#[tauri::command]
pub fn remove_allowed_sender(
    state: State<'_, AppState>,
    source_id: String,
    sender: String,
    requarantine: bool,
) -> Res<usize> {
    let mut found = false;
    state.update_config(|c| {
        if let Some(source) = c.sources.iter_mut().find(|s| s.id == source_id) {
            if let SourceKind::Mail {
                allowed_senders, ..
            } = &mut source.kind
            {
                let before = allowed_senders.len();
                allowed_senders.retain(|a| !a.trim().eq_ignore_ascii_case(sender.trim()));
                found = allowed_senders.len() < before;
            }
        }
    })?;

    if !found {
        return Err(format!("'{sender}' steht nicht auf der Freigabeliste"));
    }

    let mut moved = 0;
    if requarantine {
        let mut cache = state.cache.lock().map_err(|_| "Cache gesperrt")?;
        moved = cache.quarantine_sender(&source_id, &sender);
        cache.flush().map_err(|e| e.to_string())?;
    }

    log::info!("Absender '{sender}' entfernt, {moved} Foto(s) zurueck in die Quarantaene");
    state.rebuild_playlist();
    Ok(moved)
}

/// Gehoert dieses Bild in die gewaehlte Ansicht?
///
/// Als eigene Funktion, weil `image_page` einen laufenden `State` braucht und
/// sich im Unit-Test nicht aufrufen laesst. Die Bedingungen selbst sollen
/// trotzdem geprueft sein — sie entscheiden, was der Nutzer zu sehen bekommt.
fn matches_image_filter(e: &CacheEntry, filter: ImageFilter) -> bool {
    match filter {
        ImageFilter::All => true,
        ImageFilter::Excluded => e.excluded,
        ImageFilter::Included => !e.excluded && !e.is_quarantined(),
        ImageFilter::Quarantine => e.is_quarantined(),
        // Wie `Included`, zusaetzlich ungezeigt: ein ausgeblendetes Bild wurde
        // zwar nie gezeigt, gehoert aber nicht in die Liste derer, die noch
        // drankommen sollen (Wartung F4).
        ImageFilter::NeverShown => !e.excluded && !e.is_quarantined() && e.show_count == 0,
    }
}

/// Gleicht ein Postfach vollständig neu ab (Wartung F8).
///
/// ## Warum kein zweiter Mechanismus
///
/// Der Neuabgleich ist inhaltlich das, was E-34 mit „auch gelesene" schon
/// tut: den ganzen Ordner durchsehen und über `message_id_hash` erkennen, was
/// bereits im Cache liegt. Neu ist nur die Form — Stapel mit Pause, Fortschritt
/// und Abbruch. Deshalb ruft er `fetch_mails` wiederholt auf, statt einen
/// eigenen Abrufweg zu bauen, der dann getrennt gepflegt und getrennt falsch
/// werden könnte.
///
/// Läuft, bis ein Stapel nichts Unbekanntes mehr findet. Vorhandene Fotos
/// bleiben unangetastet: Stufe eins erkennt sie an der Message-Id und lädt sie
/// gar nicht erst herunter.
#[tauri::command]
pub async fn resync_mailbox(app: AppHandle, source_id: String) -> Res<usize> {
    use std::sync::atomic::Ordering;

    let (source, password, cfg) = {
        let state = app.state::<AppState>();
        let config = state.config_snapshot();
        let Some(source) = config.sources.iter().find(|s| s.id == source_id).cloned() else {
            return Err(format!("Unbekannte Quelle: {source_id}"));
        };
        if !matches!(source.kind, SourceKind::Mail { .. }) {
            return Err("Diese Quelle ist kein Postfach".into());
        }
        let reference = password_ref(&source.kind).unwrap_or(&source.id).to_string();
        let password = state
            .secrets
            .lock()
            .map_err(|_| "Zugangsdaten gesperrt")?
            .get(&reference)
            .unwrap_or_default()
            .to_string();
        (source, password, state.effective_cache_config())
    };

    let Some(mut mailbox) = crate::mail::sync::mailbox_config(&source, &password) else {
        return Err("Postfach unvollständig konfiguriert".into());
    };
    // Der Neuabgleich sieht immer den ganzen Ordner durch, unabhaengig davon,
    // wie die Quelle sonst eingestellt ist — das ist ja sein Zweck.
    mailbox.include_seen = true;

    app.state::<AppState>()
        .resync_cancel
        .store(false, Ordering::Relaxed);

    let mut gesamt = 0usize;
    let mut geprueft = 0usize;

    loop {
        if app
            .state::<AppState>()
            .resync_cancel
            .load(Ordering::Relaxed)
        {
            log::info!("Neuabgleich abgebrochen nach {geprueft} Nachricht(en)");
            break;
        }

        let now = now_ts();
        let state = app.state::<AppState>();
        let report = crate::mail::sync::sync_mailbox_batch(
            &source,
            &mailbox,
            &state.cache,
            &cfg,
            now,
            crate::mail::sync::RESYNC_BATCH_SIZE,
            &crate::mail::sync::MailMemory {
                seen: &|h: &str| {
                    state
                        .seen_mails
                        .lock()
                        .map(|s| s.contains(h))
                        .unwrap_or(false)
                },
                remember: &|h: String| state.remember_mail(h),
            },
        )
        .await;

        if let Some(err) = report.error.clone() {
            app.state::<AppState>()
                .record_fetch(crate::mail::log::FetchLogEntry {
                    at: now,
                    source_id: source.id.clone(),
                    trigger: crate::mail::log::Trigger::Resync,
                    seen_in_folder: report.seen_in_folder,
                    already_known: report.already_known,
                    checked: report.checked,
                    added: report.added,
                    quarantined: report.quarantined,
                    skipped: report.skipped,
                    failed: report.failed,
                    error: Some(err.clone()),
                });
            return Err(err);
        }

        gesamt += report.added;
        geprueft += report.checked;

        let _ = app.emit(
            events::RESYNC,
            crate::mail::sync::ResyncProgress {
                done: geprueft + report.already_known,
                total: report.seen_in_folder,
                added: gesamt,
            },
        );

        // Nichts Neues mehr im Ordner: fertig.
        if report.checked == 0 {
            break;
        }

        tokio::time::sleep(std::time::Duration::from_millis(
            crate::mail::sync::RESYNC_BATCH_PAUSE_MS,
        ))
        .await;
    }

    let state = app.state::<AppState>();
    state.record_fetch(crate::mail::log::FetchLogEntry {
        at: now_ts(),
        source_id: source.id.clone(),
        trigger: crate::mail::log::Trigger::Resync,
        seen_in_folder: geprueft,
        already_known: 0,
        checked: geprueft,
        added: gesamt,
        quarantined: 0,
        skipped: 0,
        failed: 0,
        error: None,
    });

    if gesamt > 0 {
        state.rebuild_playlist();
    }
    log::info!("Neuabgleich fertig: {geprueft} geprueft, {gesamt} neu");
    Ok(gesamt)
}

/// Bricht einen laufenden Neuabgleich ab (Wartung F8).
#[tauri::command]
pub fn cancel_resync(state: State<'_, AppState>) {
    state
        .resync_cancel
        .store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Baut den anonymisierten Diagnosebericht (Wartung F11).
///
/// Sammelt hier ein, was `maintenance::diagnostic_report` braucht — die
/// Zusammenstellung selbst ist dort und ohne laufende App prüfbar. Das ist bei
/// dieser Funktion besonders wichtig: die Datei ist für ein öffentliches
/// Fehlerticket gedacht, und was darin steht, soll ein Test festhalten und
/// nicht das Gerät.
///
/// Gerät und Systemfassung kommen vom Aufrufer, weil sie im Frontend über die
/// Tauri-Plattform-API leichter zu bekommen sind als über JNI.
#[tauri::command]
pub fn diagnostic_report(
    state: State<'_, AppState>,
    android_release: String,
    device_model: String,
) -> Res<String> {
    let config = state.config_snapshot();

    let (remaining, cycles) = {
        let sched = state.scheduler.lock().map_err(|_| "Durchlauf gesperrt")?;
        (sched.remaining(), sched.cycles())
    };
    let enabled: std::collections::HashSet<String> = config
        .sources
        .iter()
        .filter(|s| s.enabled)
        .map(|s| s.id.clone())
        .collect();

    let fetch_log = state
        .fetch_log
        .lock()
        .map(|l| l.recent())
        .unwrap_or_default();

    let cache = state.cache.lock().map_err(|_| "Cache gesperrt")?;
    let stats = crate::maintenance::playback_stats(
        cache.index(),
        &|e| !e.excluded && !e.is_quarantined() && enabled.contains(&e.source_id),
        remaining,
        cycles,
    );
    let storage = crate::maintenance::storage_breakdown(cache.index(), &crate::playlist::year_of);
    let bilder = cache.image_ids_on_disk();
    let vorschau = cache.thumb_ids_on_disk();
    let check = crate::maintenance::check_database(cache.index(), &bilder, &vorschau, &|id| {
        cache.file_bytes(id)
    });
    let stats_cache = cache.stats(config.cache.max_bytes);

    Ok(crate::maintenance::diagnostic_report(
        &crate::maintenance::DiagnosticInput {
            app_version: env!("CARGO_PKG_VERSION"),
            android_release: &android_release,
            device_model: &device_model,
            config: &config,
            stats: &stats,
            storage: &storage,
            check: &check,
            fetch_log: &fetch_log,
            cache_bytes: stats_cache.bytes,
            cache_max_bytes: stats_cache.max_bytes,
        },
    ))
}

/// Belegung nach Jahr und Absender (Wartung F9).
#[tauri::command]
pub fn storage_breakdown(state: State<'_, AppState>) -> Res<crate::maintenance::StorageBreakdown> {
    let cache = state.cache.lock().map_err(|_| "Cache gesperrt")?;
    Ok(crate::maintenance::storage_breakdown(
        cache.index(),
        // Dieselbe Umrechnung wie im Jahresfilter — sonst meinten
        // Aufschluesselung und Filter verschiedene Jahre.
        &crate::playlist::year_of,
    ))
}

/// Vergleicht Index und Dateibestand (Wartung F10).
///
/// Aendert nichts. Das Aufraeumen ist ein eigener Befehl, damit die
/// Oberflaeche erst zeigen kann, worum es geht.
#[tauri::command]
pub fn check_database(state: State<'_, AppState>) -> Res<crate::maintenance::DatabaseCheck> {
    let cache = state.cache.lock().map_err(|_| "Cache gesperrt")?;
    let bilder = cache.image_ids_on_disk();
    let vorschau = cache.thumb_ids_on_disk();
    Ok(crate::maintenance::check_database(
        cache.index(),
        &bilder,
        &vorschau,
        &|id| cache.file_bytes(id),
    ))
}

/// Raeumt auf, was die Pruefung gefunden hat (Wartung F10).
///
/// Prueft **erneut**, statt das Ergebnis der Anzeige zu uebernehmen: zwischen
/// Anzeige und Tippen kann ein Sync gelaufen sein, und dann loeschte der
/// Aufraeumer Dateien, die inzwischen wieder dazugehoeren.
///
/// Gibt die freigewordenen Bytes zurueck.
#[tauri::command]
pub fn repair_database(app: AppHandle, state: State<'_, AppState>) -> Res<u64> {
    let frei = {
        let mut cache = state.cache.lock().map_err(|_| "Cache gesperrt")?;
        let bilder = cache.image_ids_on_disk();
        let vorschau = cache.thumb_ids_on_disk();
        let check = crate::maintenance::check_database(cache.index(), &bilder, &vorschau, &|id| {
            cache.file_bytes(id)
        });
        let frei = cache.repair(&check);
        cache.flush().map_err(|e| e.to_string())?;
        log::info!(
            "Datenbank aufgeraeumt (F10): {} verwaiste Datei(en), {} Eintrag/Eintraege ohne Datei, {frei} Bytes frei",
            check.orphan_files.len() + check.orphan_thumbs.len(),
            check.missing_files.len()
        );
        frei
    };

    state.rebuild_playlist();
    let _ = app.emit(events::CONFIG, &state.config_snapshot());
    Ok(frei)
}

/// Die letzten Postfach-Abrufe, neueste zuerst (Wartung F6).
///
/// Ohne diese Liste war „der Abruf laeuft nicht" nicht von „es wurde nichts
/// geschickt" zu unterscheiden — beides sieht am Rahmen gleich aus.
#[tauri::command]
pub fn fetch_log(state: State<'_, AppState>) -> Vec<crate::mail::log::FetchLogEntry> {
    state
        .fetch_log
        .lock()
        .map(|l| l.recent())
        .unwrap_or_default()
}

/// Stand des letzten Abrufs einer Quelle (Wartung F5).
///
/// Grundlage der Statuszeile „Zuletzt: vor 8 Min., 2 neue Fotos". `None`,
/// solange noch nie abgerufen wurde.
#[tauri::command]
pub fn last_fetch(
    state: State<'_, AppState>,
    source_id: String,
) -> Option<crate::mail::log::FetchLogEntry> {
    state
        .fetch_log
        .lock()
        .ok()
        .and_then(|l| l.last_for(&source_id).cloned())
}

/// Statistik der Zufallswiedergabe (Wartung F1).
///
/// Rechnet in `maintenance`, damit die Auswertung ohne laufende App pruefbar
/// bleibt; hier wird nur der Zustand eingesammelt.
#[tauri::command]
pub fn playback_stats(state: State<'_, AppState>) -> Res<crate::maintenance::PlaybackStats> {
    let (remaining, cycles) = {
        let sched = state.scheduler.lock().map_err(|_| "Durchlauf gesperrt")?;
        (sched.remaining(), sched.cycles())
    };

    let enabled: std::collections::HashSet<String> = state
        .config_snapshot()
        .sources
        .iter()
        .filter(|s| s.enabled)
        .map(|s| s.id.clone())
        .collect();

    let cache = state.cache.lock().map_err(|_| "Cache gesperrt")?;
    Ok(crate::maintenance::playback_stats(
        cache.index(),
        // Dieselbe Bedingung wie in `playlist::build_order` -- eine zweite,
        // abweichende Fassung waere eine Statistik ueber einen Bestand, den
        // es so nicht gibt.
        &|e| !e.excluded && !e.is_quarantined() && enabled.contains(&e.source_id),
        remaining,
        cycles,
    ))
}

/// Beginnt den Durchlauf von vorn (Wartung F2).
///
/// Nicht destruktiv: die Urne wird geleert und beim naechsten Zug neu
/// befuellt. Anzeigezaehler und Zeitpunkte bleiben — dafuer gibt es F3.
#[tauri::command]
pub fn restart_cycle(state: State<'_, AppState>) -> Res<()> {
    state
        .scheduler
        .lock()
        .map_err(|_| "Durchlauf gesperrt")?
        .restart();
    state.flush_scheduler();
    log::info!("Durchlauf neu gestartet (F2)");
    Ok(())
}

/// Setzt Anzeigezeitpunkt und -zaehler zurueck (Wartung F3).
///
/// Wirkt auf den **aktuellen Bestand**, nicht auf den ganzen Cache: wer die
/// Diashow gerade auf ein Jahr eingegrenzt hat, erwartet, dass sich das
/// Zuruecksetzen auf eben diese Bilder bezieht. Der Rueckgabewert nennt die
/// Zahl der geaenderten Eintraege, damit die Sicherheitsabfrage vorher sagen
/// kann, worum es geht.
#[tauri::command]
pub fn reset_history(app: AppHandle, state: State<'_, AppState>) -> Res<usize> {
    let enabled: std::collections::HashSet<String> = state
        .config_snapshot()
        .sources
        .iter()
        .filter(|s| s.enabled)
        .map(|s| s.id.clone())
        .collect();

    let mut cache = state.cache.lock().map_err(|_| "Cache gesperrt")?;
    let ids: Vec<String> = cache
        .index()
        .values()
        .filter(|e| !e.excluded && !e.is_quarantined() && enabled.contains(&e.source_id))
        .map(|e| e.id.clone())
        .collect();
    let n = cache.reset_history(&ids);
    cache.flush().map_err(|e| e.to_string())?;
    drop(cache);

    // Der Durchlauf muss mit: sonst blieben Bilder in der Urne, deren
    // Historie gerade geloescht wurde, und die Gewichtung rechnete mit
    // Zahlen, die es nicht mehr gibt.
    if let Ok(mut sched) = state.scheduler.lock() {
        sched.restart();
    }
    state.flush_scheduler();
    state.rebuild_playlist();
    let _ = app.emit(events::CONFIG, &state.config_snapshot());

    log::info!("Anzeige-Historie zurueckgesetzt (F3): {n} Eintraege");
    Ok(n)
}

/// Wie viele Fotos gerade auf Freigabe warten (F4, E-31).
///
/// Grundlage des Hinweises in der Diashow — der soll nicht bei jedem Bild eine
/// Liste durch die Brücke schieben, sondern nur eine Zahl.
#[tauri::command]
pub fn quarantine_count(state: State<'_, AppState>) -> usize {
    state
        .cache
        .lock()
        .map(|c| c.index().values().filter(|e| e.is_quarantined()).count())
        .unwrap_or(0)
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

/// Fassung der laufenden App (E-13, Wartung F11).
///
/// Aus `CARGO_PKG_VERSION` und damit aus derselben Quelle wie der
/// Diagnosebericht. Vorher stand in der Oberflaeche eine feste Zeichenkette,
/// die bei `0.1.0` stehengeblieben war, waehrend das Paket schon bei 1.0.0 war
/// — wer eine Fehlermeldung schickt, nennt darin die falsche Fassung.
#[tauri::command]
pub fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Meldet die Displaygroesse in echten Pixeln (NF-12, R-03).
///
/// Wie die Ausrichtung von der Oberflaeche gemeldet: nur die WebView kennt
/// `screen` und `devicePixelRatio`. Das Backend deckelt damit die Zielgroesse
/// beim Aufbereiten, statt jedes Foto auf 2560x1600 zu bringen und der WebView
/// pro Bild sieben Megabyte aufzuladen, die auf dem Schirm nicht ankommen.
#[tauri::command]
pub fn set_display_size(state: State<'_, AppState>, width: u32, height: u32) {
    state.set_display_size(width, height);
    log::info!("Displaygroesse gemeldet: {width}x{height}");
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
    /// Wartet auf Freigabe (F4, E-31).
    Quarantine,
    /// Noch nie in der Diashow gewesen (Wartung F4).
    ///
    /// Zaehlt nur, was ueberhaupt laufen kann: ein ausgeblendetes Bild wurde
    /// nie gezeigt, aber daran ist nichts zu erklaeren.
    NeverShown,
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
        .filter(|e| matches_image_filter(e, filter))
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

/// Welches Passwort der Verbindungstest verwendet.
///
/// Leer eingetippt heisst „das gespeicherte behalten" (so steht es am Feld),
/// nicht „ohne Passwort anmelden". Als eigene Funktion, weil `test_source`
/// eine laufende App braucht und sich nicht im Unit-Test aufrufen laesst —
/// die Entscheidung selbst soll trotzdem geprueft sein.
fn effective_password(typed: &str, stored: &str) -> String {
    if typed.is_empty() {
        stored.to_string()
    } else {
        typed.to_string()
    }
}

/// Prüft die Erreichbarkeit einer Quelle, bevor sie gespeichert wird.
///
/// Rückgabe: bei einem Postfach die Zahl der ungelesenen Nachrichten, sonst
/// `None`. Die Zahl ist beim Einrichten die eigentlich nützliche Angabe — sie
/// belegt, dass nicht nur die Anmeldung stimmt, sondern auch der Ordner
/// (siehe `mail::imap::test_connection`). Sie wanderte vorher nur ins
/// Protokoll, wo sie niemand sieht; die Oberfläche meldete bloß „erfolgreich".
#[tauri::command]
pub async fn test_source(app: AppHandle, source: Source, password: String) -> Res<Option<u32>> {
    // Beim Bearbeiten bleibt das Passwortfeld leer -- der Hinweis darunter
    // sagt ausdruecklich, dass das gespeicherte dann erhalten bleibt. Der
    // Test reichte diesen leeren Wert wortwoertlich an den Server weiter und
    // scheiterte an der Anmeldung, obwohl die Zugangsdaten stimmten. Damit
    // pruefte er genau das Gegenteil dessen, was der Kommentar unten
    // behauptet.
    let password = if password.is_empty() {
        let state = app.state::<AppState>();
        let reference = password_ref(&source.kind).unwrap_or(&source.id).to_string();
        let stored = state
            .secrets
            .lock()
            .map_err(|_| "Zugangsdaten gesperrt")?
            .get(&reference)
            .unwrap_or_default()
            .to_string();
        effective_password(&password, &stored)
    } else {
        password
    };

    // Postfaecher gehen nicht ueber den DAV-Client. Der Test nimmt denselben
    // Weg wie der Abruf -- ein Test, der etwas anderes prueft als den
    // Ernstfall, ist keiner (Wartung F5).
    if matches!(source.kind, SourceKind::Mail { .. }) {
        let Some(mailbox) = crate::mail::sync::mailbox_config(&source, &password) else {
            return Err("Postfach unvollständig konfiguriert".into());
        };
        let unseen = crate::mail::imap::test_connection(&mailbox)
            .await
            .map_err(|e| e.to_string())?;
        log::info!("Postfach erreichbar, {unseen} ungelesene Nachricht(en)");
        return Ok(Some(unseen));
    }

    let client = RemoteClient::from_source(&app, &source, &password)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Für diese Quelle gibt es keinen Verbindungstest".to_string())?;
    client.test().await.map_err(|e| e.to_string())?;
    Ok(None)
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

/// Führt die Synchronisierung aus (FA-28, E-43).
///
/// `only_due` beschränkt auf Quellen, deren Intervall abgelaufen ist — so
/// arbeitet der Hintergrund-Zeitgeber.
///
/// Quellen werden eingereiht, nicht abgewiesen: höchstens
/// [`MAX_PARALLEL_SOURCES`] laufen gleichzeitig, der Rest wartet der Reihe
/// nach. Vorher gab es eine Sperre über alle Quellen, und ein zweiter Aufruf
/// bekam einen Fehler — ein „Jetzt abgleichen" während des Hintergrundlaufs
/// war damit verloren, und jeder Takt, der einen laufenden Sync antraf,
/// übersprang sich ganz.
///
/// Zurück kommen die Berichte der Quellen, die **dieser** Aufruf eingereiht
/// hat, in der Reihenfolge des Eingangs. Eine leere Liste heißt „läuft schon",
/// nicht „nichts passiert".
pub async fn run_sync(
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
        (sources, state.effective_cache_config())
    };

    // Entdoppeln statt abweisen (E-43): was schon laeuft oder wartet, wird
    // nicht ein zweites Mal angestossen. Eine leere Liste ist deshalb kein
    // Fehler mehr, sondern heisst "ist bereits unterwegs".
    let sources: Vec<Source> = {
        let state = app.state::<AppState>();
        let ids: Vec<String> = sources.iter().map(|s| s.id.clone()).collect();
        let claimed = state.claim_sources(&ids);
        sources
            .into_iter()
            .filter(|s| claimed.contains(&s.id))
            .collect()
    };

    if sources.is_empty() {
        log::debug!("Sync: nichts einzureihen, die Quellen sind bereits unterwegs");
        return Ok(Vec::new());
    }

    log::info!(
        "Sync angestossen: {} Quelle(n) eingereiht (only_due={only_due})",
        sources.len()
    );

    // Alle Quellen als Futures nebeneinander, der Semaphor laesst hoechstens
    // `MAX_PARALLEL_SOURCES` gleichzeitig hindurch — das ist die ganze
    // Warteschlange.
    //
    // Bewusst **ohne** `spawn`: die Arbeit bleibt damit in der Aufgabe des
    // Aufrufers und wird mit ihr abgebrochen. Abgesetzte Aufgaben liefen beim
    // Beenden der App weiter und griffen auf einen Zustand zu, den Tauri schon
    // abgeraeumt hat — genau der Absturz, gegen den `SHUTTING_DOWN` in `lib.rs`
    // steht.
    let slots = app.state::<AppState>().sync_slots();
    let laeufe = sources.into_iter().map(|source| {
        let slots = std::sync::Arc::clone(&slots);
        let cfg = &cfg;
        async move {
            let _permit = slots.acquire().await.ok();
            // Der Waechter gibt die Anmeldung auch dann frei, wenn der Lauf
            // unterwegs abbricht.
            let _guard = SourceGuard(app.clone(), source.id.clone());
            sync_one_source(app, source, cfg, only_due, now).await
        }
    });

    // `join_all` behaelt die Reihenfolge des Eingangs bei, nicht die des Endes:
    // sonst haenge die Zuordnung Bericht -> Quelle daran, welche Quelle
    // zufaellig schneller war.
    let reports = futures_util::future::join_all(laeufe)
        .await
        .into_iter()
        .flatten()
        .collect();

    Ok(reports)
}

/// Gibt die Anmeldung einer Quelle frei, sobald sie den Gueltigkeitsbereich
/// verlaesst (E-43).
///
/// Wie die frueherere Sync-Sperre ueber `Drop` und nicht am Ende der Funktion:
/// ein Panic im Sync-Pfad liesse die Quelle sonst dauerhaft angemeldet, und sie
/// wuerde bis zum Neustart der App nie wieder abgeglichen — auf einem
/// unbeaufsichtigten Geraet faellt das niemandem auf.
struct SourceGuard(AppHandle, String);

impl Drop for SourceGuard {
    fn drop(&mut self) {
        self.0.state::<AppState>().release_source(&self.1);
    }
}

/// Gleicht eine einzelne Quelle ab.
///
/// `None` heisst "uebersprungen, kein Bericht" — etwa wenn zu der Quellenart
/// kein Client gehoert. Ein Fehler *beim* Abgleich kommt dagegen als Bericht
/// mit gesetztem `error` zurueck: er gehoert in die Anzeige und ins Protokoll.
async fn sync_one_source(
    app: &AppHandle,
    source: Source,
    cfg: &crate::model::CacheConfig,
    only_due: bool,
    now: i64,
) -> Option<SyncReport> {
    {
        let password = {
            let state = app.state::<AppState>();
            let reference = password_ref(&source.kind).unwrap_or(&source.id).to_string();
            let Ok(guard) = state.secrets.lock() else {
                let mut r = SyncReport::for_source(&source.id);
                r.error = Some("Zugangsdaten gesperrt".into());
                return Some(r);
            };
            guard.get(&reference).unwrap_or_default().to_string()
        };

        // Postfaecher gehen einen eigenen Weg: eine Mail wird als Ganzes
        // geholt und ihre Anhaenge sofort abgelegt, statt erst zu listen und
        // dann einzeln zu holen (E-30).
        if matches!(source.kind, SourceKind::Mail { .. }) {
            let state = app.state::<AppState>();
            // Das Gedaechtnis liegt im Zustand, `sync` soll davon nichts
            // wissen -- deshalb als Rueckrufe (E-36).
            let mail_report = crate::mail::sync::sync_mailbox(
                &source,
                &password,
                &state.cache,
                cfg,
                now,
                &crate::mail::sync::MailMemory {
                    seen: &|h: &str| {
                        state
                            .seen_mails
                            .lock()
                            .map(|s| s.contains(h))
                            .unwrap_or(false)
                    },
                    remember: &|h: String| state.remember_mail(h),
                },
            )
            .await;

            // Wartung F6: jeder Lauf kommt ins Protokoll, auch der
            // erfolglose. Gerade der erfolglose -- „seit Tagen kommt nichts"
            // laesst sich sonst nicht von „es wurde nichts geschickt"
            // unterscheiden.
            state.record_fetch(crate::mail::log::FetchLogEntry {
                at: now,
                source_id: source.id.clone(),
                trigger: if only_due {
                    crate::mail::log::Trigger::Interval
                } else {
                    crate::mail::log::Trigger::Manual
                },
                seen_in_folder: mail_report.seen_in_folder,
                already_known: mail_report.already_known,
                checked: mail_report.checked,
                added: mail_report.added,
                quarantined: mail_report.quarantined,
                skipped: mail_report.skipped,
                failed: mail_report.failed,
                error: mail_report.error.clone(),
            });

            let mut r = SyncReport::for_source(&source.id);
            r.added = mail_report.added;
            r.skipped = mail_report.skipped;
            r.failed = mail_report.failed;
            r.truncated = mail_report.rate_limited;
            r.error = mail_report.error;

            if mail_report.added > 0 {
                state.rebuild_playlist();
            }
            // Zeitstempel nur bei Erfolg, wie bei den uebrigen Quellen: sonst
            // verschoebe ein Ausfall den naechsten Versuch um ein volles
            // Intervall.
            if r.error.is_none() {
                let _ = state.update_config(|c| {
                    if let Some(s) = c.sources.iter_mut().find(|s| s.id == source.id) {
                        s.last_sync = Some(now);
                    }
                });
            }
            let _ = app.emit(events::SYNC, &r);
            return Some(r);
        }

        let client = match RemoteClient::from_source(app, &source, &password) {
            Ok(Some(c)) => c,
            Ok(None) => {
                log::warn!("'{}': kein Client fuer diese Quellenart", source.name);
                return None;
            }
            Err(e) => {
                let mut r = SyncReport::for_source(&source.id);
                r.error = Some(e.to_string());
                return Some(r);
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
                cfg,
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
        Some(report)
    }
}

// ── Hilfen ───────────────────────────────────────────────────────────────────

/// Schlüssel, unter dem das Passwort einer Quelle liegt.
fn password_ref(kind: &SourceKind) -> Option<&str> {
    match kind {
        SourceKind::Local { .. } => None,
        SourceKind::WebDav { password_ref, .. }
        | SourceKind::Nextcloud { password_ref, .. }
        | SourceKind::Mail { password_ref, .. } => Some(password_ref),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bild mit den Eigenschaften, auf die die Ansichten schauen.
    fn bild(excluded: bool, quarantaene: bool, shows: u32) -> CacheEntry {
        CacheEntry {
            id: "i".into(),
            source_id: "s".into(),
            rel_path: "a.jpg".into(),
            file_name: "a.jpg".into(),
            etag: None,
            remote_size: None,
            remote_mtime: None,
            taken_at: None,
            width: 10,
            height: 10,
            bytes: 1,
            added_at: 0,
            last_shown: if shows > 0 { Some(1) } else { None },
            show_count: shows,
            excluded,
            mail: if quarantaene {
                Some(crate::cache::index::MailMeta {
                    sender: "wer@example.org".into(),
                    subject: "x".into(),
                    message_id: "m".into(),
                    quarantined: true,
                })
            } else {
                None
            },
            thumb_bytes: None,
        }
    }

    #[test]
    fn nie_gezeigt_meint_nur_bilder_die_drankommen_koennen() {
        // Ein ausgeblendetes Bild wurde nie gezeigt -- daran ist aber nichts
        // zu erklaeren, und es gehoert nicht in die Liste derer, die noch
        // drankommen sollen. Dasselbe gilt fuer wartende (Wartung F4).
        let f = ImageFilter::NeverShown;
        assert!(
            matches_image_filter(&bild(false, false, 0), f),
            "frisch importiert"
        );
        assert!(
            !matches_image_filter(&bild(false, false, 1), f),
            "schon gezeigt"
        );
        assert!(
            !matches_image_filter(&bild(true, false, 0), f),
            "ausgeblendet"
        );
        assert!(
            !matches_image_filter(&bild(false, true, 0), f),
            "in Quarantaene"
        );
    }

    #[test]
    fn die_ansichten_ueberschneiden_sich_wie_erwartet() {
        // Gegenprobe zu den uebrigen Filtern -- ein vertauschtes `!` faende
        // sonst niemand, ausser am Geraet beim Durchklicken.
        let frisch = bild(false, false, 0);
        assert!(matches_image_filter(&frisch, ImageFilter::All));
        assert!(matches_image_filter(&frisch, ImageFilter::Included));
        assert!(!matches_image_filter(&frisch, ImageFilter::Excluded));
        assert!(!matches_image_filter(&frisch, ImageFilter::Quarantine));

        let wartend = bild(false, true, 0);
        assert!(matches_image_filter(&wartend, ImageFilter::Quarantine));
        assert!(
            !matches_image_filter(&wartend, ImageFilter::Included),
            "wartende laufen noch nicht in der Diashow"
        );

        let versteckt = bild(true, false, 3);
        assert!(matches_image_filter(&versteckt, ImageFilter::Excluded));
        assert!(!matches_image_filter(&versteckt, ImageFilter::Included));
    }

    #[test]
    fn alle_zeigt_wirklich_alles() {
        for e in [
            bild(false, false, 0),
            bild(true, false, 0),
            bild(false, true, 0),
            bild(true, true, 9),
        ] {
            assert!(matches_image_filter(&e, ImageFilter::All));
        }
    }

    /// Am Geraet gemeldet: „Verbindung testen" schlug beim Bearbeiten einer
    /// vorhandenen Quelle fehl. Das Passwortfeld ist dort leer -- der Hinweis
    /// darunter sagt ausdruecklich, dass das gespeicherte erhalten bleibt --
    /// und genau dieser leere Wert ging an den Server.
    #[test]
    fn leeres_feld_bedeutet_gespeichertes_passwort() {
        assert_eq!(effective_password("", "geheim"), "geheim");
    }

    #[test]
    fn eingetipptes_passwort_hat_vorrang() {
        // Sonst liesse sich ein falsch gespeichertes Passwort nie ersetzen:
        // man tippt das neue ein, und der Test prueft weiter das alte.
        assert_eq!(effective_password("neu", "alt"), "neu");
    }

    #[test]
    fn ohne_beides_bleibt_es_leer() {
        // Eine neue Quelle ohne Eingabe. Der Server lehnt dann ab -- richtig
        // so, und die Meldung nennt seit E-33 seinen Wortlaut.
        assert_eq!(effective_password("", ""), "");
    }

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

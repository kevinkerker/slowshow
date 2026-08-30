//! Slowshow — digitaler Bilderrahmen für Android-Tablets.
//!
//! Aufbau des Backends (siehe auch CLAUDE.md, Abschnitt „Architekturregel"):
//!
//! | Modul       | Aufgabe                                        | Anforderung        |
//! |-------------|------------------------------------------------|--------------------|
//! | `model`     | Konfiguration und Quellen                      | FA-42              |
//! | `config`    | Persistenz der Konfiguration                   | FA-42, FA-45       |
//! | `secrets`   | verschlüsselte Zugangsdaten                    | NF-05              |
//! | `decode`    | Dekodierung, EXIF, Skalierung                  | FA-04, NF-12, NF-13|
//! | `cache`     | permanenter Ringpuffer                         | FA-26, FA-27       |
//! | `sources`   | WebDAV und Nextcloud                           | FA-21, FA-23       |
//! | `sync`      | Delta-Synchronisierung                         | FA-28, NF-14       |
//! | `playlist`  | Reihenfolge und Fortschritt                    | FA-01, FA-03, FA-08|
//! | `schedule`  | Zeitplan, Helligkeit, Nachtmodus               | FA-52–54           |
//! | `remote`    | Heimnetz-Steuerung per REST                    | FA-55              |
//! | `commands`  | Schnittstelle zum Frontend                     | —                  |

pub mod android_bridge;
pub mod battery;
pub mod brightness;
pub mod cache;
pub mod commands;
pub mod config;
pub mod control;
pub mod decode;
pub mod model;
pub mod mqtt;
pub mod orientation;
pub mod playlist;
pub mod remote;
pub mod schedule;
pub mod scheduler;
pub mod secrets;
pub mod sources;
pub mod state;
pub mod sync;

use mqtt::MqttService;
use remote::RemoteServer;
use state::{events, AppState};
use std::time::Duration;
use tauri::{Emitter, Manager};

/// Wie oft der Zeitgeber prüft, ob eine Quelle synchronisiert werden muss (FA-28).
const SYNC_TICK: Duration = Duration::from_secs(60);
/// Wie oft Zeitplan und Helligkeit ausgewertet werden (FA-52–54).
const DISPLAY_TICK: Duration = Duration::from_secs(20);
/// Wie oft der Cache-Index auf die Platte geschrieben wird.
///
/// Bewusst selten: im Dauerbetrieb wäre häufiges Schreiben unnötige Last auf
/// dem Flash-Speicher (R-08).
const FLUSH_TICK: Duration = Duration::from_secs(120);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // NF-02: ein Panic in einer Hintergrundaufgabe soll im Log auftauchen,
    // statt die App still zu beenden.
    std::panic::set_hook(Box::new(|info| {
        log::error!("Panic: {info}");
        eprintln!("Panic: {info}");
    }));

    let builder = tauri::Builder::default()
        // Ohne Logger verschwinden alle `log::`-Ausgaben spurlos — auf Android
        // erst recht. Das betrifft nicht nur die Fehlersuche: FA-09 verlangt,
        // dass uebersprungene HEIC-Dateien "im Log vermerkt" werden, und der
        // 7-Tage-Dauertest aus Abschnitt 5.2 waere sonst nicht auswertbar.
        //
        // Ziel ist Stdout (auf Android = logcat) und eine Datei im
        // App-Verzeichnis, die sich nach einem Dauerlauf per adb holen laesst.
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        // Bilder gehen ausschließlich über dieses Protokoll ans Frontend —
        // nie über IPC (NF-13, R-03).
        .register_uri_scheme_protocol("slowshow", |ctx, request| {
            serve_image(ctx.app_handle(), request)
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::export_config,
            commands::import_config,
            commands::get_display_state,
            commands::current_slide,
            commands::next_slide,
            commands::prev_slide,
            commands::set_playing,
            commands::is_playing,
            commands::prefetch_window,
            commands::image_info,
            commands::exclude_image,
            commands::include_image,
            commands::image_page,
            commands::set_frame_orientation,
            commands::apply_orientation,
            commands::cache_stats,
            commands::source_counts,
            commands::add_source,
            commands::update_source,
            commands::remove_source,
            commands::set_mqtt_password,
            commands::has_mqtt_password,
            commands::mqtt_status,
            commands::mqtt_reconnect,
            commands::test_source,
            commands::list_nextcloud_albums,
            commands::sync_now,
        ])
        .setup(|app| {
            #[cfg(target_os = "android")]
            app.handle().plugin(tauri_plugin_android_fs::init())?;

            // Desktop-Nebenprodukt (Lastenheft 1.3): zweiter Start holt das
            // vorhandene Fenster nach vorn statt eine zweite Instanz zu öffnen.
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
                    if let Some(win) = app.get_webview_window("main") {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }))?;

            let data_dir = app
                .path()
                .app_local_data_dir()
                .map_err(|e| format!("Datenverzeichnis nicht ermittelbar: {e}"))?;
            log::info!("Datenverzeichnis: {}", data_dir.display());

            let state = AppState::new(&data_dir)?;
            app.manage(state);

            // Die Ausrichtung (E-26) wird hier bewusst *nicht* gesetzt.
            //
            // `_start_app` läuft auf einem eigenen Thread, `onCreate` kehrt
            // sofort zurück, und `nativeRegisterActivity` rennt damit gegen
            // dieses `setup`. Ein Aufruf an dieser Stelle trifft die JNI-Brücke
            // mal und mal nicht — am Gerät gemessen: er lief ins Leere. Das
            // Frontend ruft `apply_orientation`, sobald die WebView steht; dann
            // ist die Brücke sicher da.
            app.manage(RemoteServer::default());
            app.manage(MqttService::default());

            // FA-55: Heimnetz-Steuerung starten, falls konfiguriert.
            let handle = app.handle().clone();
            handle.state::<RemoteServer>().apply_config(&handle);
            handle.state::<MqttService>().apply_config(&handle);
            watch_state_for_mqtt(&handle);

            spawn_background_tasks(app.handle().clone());
            Ok(())
        });

    builder
        .build(tauri::generate_context!())
        .expect("Slowshow konnte nicht gestartet werden")
        .run(|app, event| {
            // Index sichern, damit nach einem Stromausfall (R-08) nichts
            // verloren geht. Das ist billig und darf ruhig oft passieren.
            if matches!(
                event,
                tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }
            ) {
                app.state::<AppState>().flush();
            }

            // Den Heimnetz-Server nur beim echten Beenden abschalten.
            // `ExitRequested` feuert auf Android auch, wenn die Activity nur
            // neu aufgebaut wird — dann liefe die Steuerung aus FA-55 nach
            // einer Drehung oder einem Konfigurationswechsel nicht mehr.
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<RemoteServer>().stop();
                app.state::<MqttService>().stop();
            }
        });
}

/// Liefert ein Bild aus dem Cache aus.
///
/// Die Id wird vom Cache gegen den Index geprüft; eine manipulierte URL kann
/// deshalb keine beliebige Datei vom Gerät ausliefern.
/// Präfix, an dem das Asset-Protokoll ein Vorschaubild erkennt (E-25).
///
/// Muss mit `THUMB_PREFIX` in `src/lib/api.ts` übereinstimmen; die Grenze
/// prüft kein Compiler.
pub const THUMB_PREFIX: &str = "t_";

fn serve_image(
    app: &tauri::AppHandle,
    request: tauri::http::Request<Vec<u8>>,
) -> tauri::http::Response<Vec<u8>> {
    let not_found = || {
        tauri::http::Response::builder()
            .status(404)
            .body(Vec::new())
            .expect("statische Antwort")
    };

    let path = request.uri().path();
    let Some(name) = path.rsplit('/').next().filter(|s| !s.is_empty()) else {
        return not_found();
    };

    // Vorschaubilder tragen ein Präfix statt eines eigenen Pfadsegments (E-25).
    //
    // `convertFileSrc` steckt die Id per `encodeURIComponent` in *ein* Segment;
    // aus `thumb/<id>` würde `thumb%2F<id>`, und ein Aufteilen am Schrägstrich
    // ginge ins Leere. Ein Präfix aus Buchstabe und Unterstrich übersteht die
    // Kodierung unverändert und kann mit keiner Id kollidieren, weil Ids
    // hexadezimal sind.
    let (want_thumb, id) = match name.strip_prefix(THUMB_PREFIX) {
        Some(rest) => (true, rest),
        None => (false, name),
    };
    if id.is_empty() {
        return not_found();
    }

    let state = app.state::<AppState>();
    let Ok(mut cache) = state.cache.lock() else {
        return not_found();
    };
    let bytes = if want_thumb {
        // Braucht `&mut`, weil die Größe des erzeugten Vorschaubilds in den
        // Index nachgetragen wird.
        cache.read_or_make_thumb(id)
    } else {
        cache.read_image(id)
    };
    let Some(bytes) = bytes else {
        log::debug!("Nicht im Cache: {name}");
        return not_found();
    };

    tauri::http::Response::builder()
        .status(200)
        .header("Content-Type", "image/jpeg")
        .header("Content-Length", bytes.len().to_string())
        // Der Inhalt einer Id ändert sich nur, wenn die Quelldatei sich ändert;
        // dann bekommt sie beim Sync ohnehin neue Bytes. Aggressives Caching
        // in der WebView spart beim Zurückblättern das erneute Dekodieren.
        .header("Cache-Control", "public, max-age=86400")
        .body(bytes)
        .unwrap_or_else(|_| not_found())
}

/// Meldet Zustandsänderungen an den MQTT-Dienst.
///
/// Die App verschickt ihre Änderungen ohnehin schon als Tauri-Ereignisse an das
/// Frontend. MQTT hängt sich hier als zweiter Abnehmer an, statt dass jede
/// Stelle im Code zusätzlich MQTT benachrichtigen müsste — das wäre eine
/// Aufrufstelle, die man beim nächsten Feature vergisst.
fn watch_state_for_mqtt(app: &tauri::AppHandle) {
    use tauri::Listener;

    for event in [
        events::SLIDE,
        events::SYNC,
        events::SYNC_PROGRESS,
        events::DISPLAY,
        events::CONFIG,
    ] {
        let handle = app.clone();
        app.listen(event, move |_| {
            handle.state::<MqttService>().notify_changed();
        });
    }
}

/// Startet die drei Dauerläufer der App.
fn spawn_background_tasks(app: tauri::AppHandle) {
    // 1. Fällige Synchronisierungen (FA-28).
    let sync_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(SYNC_TICK);
        // Der erste Tick feuert sofort — beim Start soll einmal geprüft werden.
        loop {
            ticker.tick().await;
            match commands::run_sync(&sync_app, None, true).await {
                Ok(reports) if !reports.is_empty() => {
                    log::info!("Hintergrund-Sync: {} Quelle(n)", reports.len());
                }
                // „läuft bereits" ist der Normalfall, wenn der Nutzer von Hand
                // synchronisiert — kein Grund für eine Warnung.
                Err(e) if e.contains("bereits") => log::debug!("{e}"),
                Err(e) => log::warn!("Hintergrund-Sync: {e}"),
                _ => {}
            }
        }
    });

    // 2. Zeitplan und Helligkeit (FA-52–54).
    let display_app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(DISPLAY_TICK);
        let mut last: Option<schedule::DisplayState> = None;
        loop {
            ticker.tick().await;
            let current = display_app.state::<AppState>().display_state();
            if last != Some(current) {
                // Frontend und Displaybeleuchtung gemeinsam nachziehen (FA-53).
                brightness::apply(current.brightness);
                let _ = display_app.emit(events::DISPLAY, current);
                last = Some(current);
            }
        }
    });

    // 3. Cache-Index sichern.
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(FLUSH_TICK);
        loop {
            ticker.tick().await;
            app.state::<AppState>().flush();
        }
    });
}

//! Der Rahmen als DLNA-Medienrenderer (E-47).
//!
//! Home Assistant findet UPnP-Renderer von selbst per SSDP und zeigt „Neues
//! Geraet gefunden" — ohne dass am Rahmen eine Adresse, ein Broker oder ein
//! Passwort eingetragen wuerde. Das war der Wunsch: Rahmen ins WLAN, fertig.
//!
//! Dafuer tritt Slowshow als `MediaRenderer:1` auf. Was die Aktionen eines
//! Renderers hier bedeuten, steht in [`renderer`]; wie sie ueber das Netz
//! kommen, in [`ssdp`], [`http`] und [`gena`]. Dieses Modul haelt den Dienst
//! zusammen: Start und Stopp nach Konfiguration, die Anbindung an den
//! Anwendungszustand, und das Weiterreichen von Aenderungen an Abonnenten.
//!
//! Der Dienst laeuft neben der REST-Steuerung (FA-55), nicht statt ihrer: REST
//! liefert, was DLNA nicht kennt — Sensoren, Konfiguration, Sync.

pub mod description;
pub mod gena;
pub mod http;
pub mod mjpeg;
pub mod renderer;
pub mod soap;
pub mod ssdp;

use crate::control::{self, Origin};
use crate::model::UpnpConfig;
use crate::state::AppState;
use renderer::{Backend, Snapshot};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use tokio::sync::watch;

/// `SERVER`-Kopfzeile in SSDP und HTTP — UPnP verlangt die Form
/// `OS/Version UPnP/1.0 Produkt/Version`.
pub const SERVER_HEADER: &str = concat!("Android/1.0 UPnP/1.0 Slowshow/", env!("CARGO_PKG_VERSION"));

/// Eine zufaellige UUID (Version 4) als Text — fuer UDN und SIDs.
///
/// Von Hand aus 16 Zufallsbytes statt ueber eine weitere Abhaengigkeit: die
/// Bits fuer Version und Variante sind zwei Zeilen.
pub fn new_uuid() -> String {
    let mut b = [0u8; 16];
    if getrandom::getrandom(&mut b).is_err() {
        // Ohne Zufallsquelle lieber ein erkennbar konstanter Wert als ein
        // Panic beim Start; kommt auf Android nicht vor.
        b = [0x42; 16];
    }
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

/// Der laufende Dienst. Wie bei der REST-Steuerung liegt der Griff zum
/// Beenden im Zustand, damit ein Umschalten in den Einstellungen sofort wirkt.
#[derive(Default)]
pub struct UpnpService {
    stop: Mutex<Option<watch::Sender<bool>>>,
    ctx: Mutex<Option<Arc<http::Ctx>>>,
}

impl UpnpService {
    pub fn is_running(&self) -> bool {
        self.stop.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    /// Startet den Dienst neu passend zur aktuellen Konfiguration.
    pub fn apply_config(&self, app: &AppHandle) {
        let config = app.state::<AppState>().config_snapshot().upnp;
        self.stop();
        if config.enabled {
            self.start(app.clone(), config);
        }
    }

    pub fn stop(&self) {
        let sender = self.stop.lock().ok().and_then(|mut g| g.take());
        if let Some(tx) = sender {
            let _ = tx.send(true);
            if let Ok(mut c) = self.ctx.lock() {
                *c = None;
            }
            hold_multicast(false);
            log::info!("UPnP: Renderer beendet");
        }
    }

    fn start(&self, app: AppHandle, config: UpnpConfig) {
        let (tx, rx) = watch::channel(false);
        if let Ok(mut g) = self.stop.lock() {
            *g = Some(tx);
        }

        let ctx = Arc::new(http::Ctx {
            backend: Arc::new(AppBackend { app: app.clone() }),
            udn: config.udn.clone(),
            subs: Arc::new(gena::Subscriptions::default()),
            client: reqwest::Client::new(),
        });
        if let Ok(mut c) = self.ctx.lock() {
            *c = Some(ctx.clone());
        }

        // Ohne den Multicast-Lock verwirft Androids WLAN die M-SEARCH-Pakete,
        // und niemand faende uns.
        hold_multicast(true);

        // HTTP: Beschreibungen, Steuerung, Ereignisse, Cover.
        let port = config.port;
        let mut http_stop = rx.clone();
        tauri::async_runtime::spawn(async move {
            let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    log::error!("UPnP: Port {port} nicht zu belegen: {e}");
                    return;
                }
            };
            log::info!("UPnP: Renderer lauscht auf {addr}");
            let served = axum::serve(listener, http::router(ctx))
                .with_graceful_shutdown(async move {
                    let _ = http_stop.changed().await;
                })
                .await;
            if let Err(e) = served {
                log::error!("UPnP: HTTP beendet sich mit Fehler: {e}");
            }
        });

        // SSDP: gefunden werden.
        tauri::async_runtime::spawn(ssdp::run(config.udn, port, SERVER_HEADER.to_string(), rx));
    }

    /// Der Rahmen hat sich geaendert — Abonnenten erfahren es.
    pub fn notify_changed(&self) {
        let ctx = self.ctx.lock().ok().and_then(|c| c.clone());
        if let Some(ctx) = ctx {
            tauri::async_runtime::spawn(http::publish(ctx));
        }
    }
}

/// Der echte Rahmen hinter der Renderer-Schnittstelle.
struct AppBackend {
    app: AppHandle,
}

impl Backend for AppBackend {
    fn snapshot(&self) -> Snapshot {
        let state = self.app.state::<AppState>();
        let config = state.config_snapshot();
        let display = state.display_state();
        let external = state.external_info();
        let current_id = state
            .current_slide()
            .map(|slide| slide.ids()[0].to_string());
        let current_title = match &external {
            // Das Fremdbild (E-52) steht in keinem Index; sein Titel kommt
            // von Home Assistant.
            Some((_, title, _)) => title.clone(),
            None => current_id
                .as_deref()
                .and_then(|id| {
                    state
                        .cache
                        .lock()
                        .ok()
                        .and_then(|c| c.index().get(id).map(|e| e.file_name.clone()))
                })
                .unwrap_or_default(),
        };
        Snapshot {
            playing: state.is_playing(),
            screen_active: display.slideshow_active,
            brightness: config.brightness.level.clamp(1, 100),
            current_id,
            current_title,
            friendly_name: config.upnp.friendly_name,
            external: external.is_some(),
            external_uri: external.map(|(_, _, uri)| uri),
        }
    }

    fn play(&self) {
        // Ein vorgemerktes Fremdbild (E-52) geht vor: Home Assistant schickt
        // Play unmittelbar nach SetAVTransportURI und meint genau das.
        if control::show_external(&self.app) {
            return;
        }
        // Play heisst: sehen wollen. Ein dunkler Schirm wird dabei geweckt.
        if !self.app.state::<AppState>().display_state().slideshow_active {
            control::set_screen(&self.app, true, Origin::Upnp);
        }
        control::set_slideshow(&self.app, true, Origin::Upnp);
    }

    fn stop(&self) {
        // Haengt ein Fremdbild, heisst Stop zuerst „weg damit" (E-52); erst
        // ohne Fremdbild ist es der Nachtmodus.
        if control::end_external(&self.app) {
            return;
        }
        control::set_screen(&self.app, false, Origin::Upnp);
    }

    fn next(&self) {
        control::next_slide(&self.app, Origin::Upnp);
    }

    fn previous(&self) {
        control::prev_slide(&self.app, Origin::Upnp);
    }

    fn set_volume(&self, level: u8) {
        // 0 waere „Geraet regelt selbst" (E-22) — als Lautstaerke gemeint ist
        // es aber „ganz dunkel", und das ist die 1.
        let patch = control::ConfigPatch {
            brightness: Some(level.clamp(1, 100)),
            ..Default::default()
        };
        if let Err(e) = control::patch_config(&self.app, patch, Origin::Upnp) {
            log::warn!("UPnP: Helligkeit nicht uebernommen: {e}");
        }
    }

    fn set_mute(&self, mute: bool) {
        control::set_screen(&self.app, !mute, Origin::Upnp);
    }

    fn stage_external(&self, image: Vec<u8>, title: String, uri: String) -> Result<(), String> {
        self.app
            .state::<AppState>()
            .stage_external(&image, &title, &uri)
    }

    fn current_image(&self) -> Option<Vec<u8>> {
        let state = self.app.state::<AppState>();
        let slide = state.current_slide()?;
        let id = slide.ids()[0].to_string();
        // Das Fremdbild (E-52) liegt nicht im Cache.
        if crate::state::is_external_id(&id) {
            return state.read_external(&id);
        }
        // Guard an einen Namen binden: als Temporaer im Rueckgabeausdruck
        // ueberlebte er `state` und der Borrow-Checker sagt zu Recht nein.
        let cache = state.cache.lock().ok()?;
        let bytes = cache.read_image(&id);
        drop(cache);
        bytes
    }
}

/// Multicast-Lock der Android-WLAN-Schicht halten oder freigeben.
///
/// Ohne ihn filtert das WLAN Multicast heraus, und SSDP-Suchen kaemen nie an.
/// Auf dem Schreibtisch gibt es nichts zu halten.
fn hold_multicast(on: bool) {
    #[cfg(target_os = "android")]
    {
        let method = if on {
            "acquireMulticastLock"
        } else {
            "releaseMulticastLock"
        };
        crate::android_bridge::with_activity("Multicast", |env, activity| {
            env.call_method(activity, method, "()V", &[]).map(|_| ())
        });
    }
    #[cfg(not(target_os = "android"))]
    let _ = on;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_hat_form_und_versionsbits() {
        let u = new_uuid();
        assert_eq!(u.len(), 36);
        let teile: Vec<&str> = u.split('-').collect();
        assert_eq!(teile.iter().map(|t| t.len()).collect::<Vec<_>>(), vec![8, 4, 4, 4, 12]);
        assert!(teile[2].starts_with('4'), "Version 4: {u}");
        assert!(matches!(teile[3].chars().next(), Some('8' | '9' | 'a' | 'b')), "Variante: {u}");
        assert_ne!(new_uuid(), u, "zwei Aufrufe, zwei Werte");
    }

    #[test]
    fn server_kopfzeile_nennt_upnp_und_version() {
        assert!(SERVER_HEADER.contains("UPnP/1.0"));
        assert!(SERVER_HEADER.contains(env!("CARGO_PKG_VERSION")));
    }
}

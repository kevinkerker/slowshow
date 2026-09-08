//! Der Medienrenderer ohne Tauri-App — zum Pruefen mit einem echten
//! UPnP-Steuerpunkt auf dem Schreibtisch (E-47).
//!
//! ```text
//! cargo run --example upnp_renderer -- 8128
//! ```
//!
//! Dahinter steckt ein Rahmen aus Pappe: er merkt sich Play/Pause, Schirm,
//! Helligkeit und zaehlt bei Next/Previous eine Bildnummer. Was ankommt,
//! landet auf der Konsole.

use slowshow_lib::upnp::renderer::{Backend, Snapshot};
use slowshow_lib::upnp::{gena, http, ssdp, SERVER_HEADER};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

struct Pappe {
    state: Mutex<Snapshot>,
}

impl Pappe {
    fn update(&self, f: impl FnOnce(&mut Snapshot)) {
        if let Ok(mut s) = self.state.lock() {
            f(&mut s);
            println!(
                "[pappe] playing={} screen={} brightness={} bild={}",
                s.playing, s.screen_active, s.brightness, s.current_title
            );
        }
    }
}

impl Backend for Pappe {
    fn snapshot(&self) -> Snapshot {
        self.state.lock().map(|s| s.clone()).expect("Pappe")
    }
    fn play(&self) {
        self.update(|s| {
            s.playing = true;
            s.screen_active = true;
        })
    }
    fn stop(&self) {
        self.update(|s| {
            // Wie der echte Rahmen: erst das Fremdbild weg, sonst dunkel (E-52).
            if s.external {
                s.external = false;
                s.external_uri = None;
            } else {
                s.screen_active = false;
            }
        })
    }
    fn next(&self) {
        self.update(|s| {
            let n: u32 = s.current_title[4..8].parse::<u32>().unwrap_or(0) + 1;
            s.current_title = format!("IMG_{n:04}.jpg");
            s.current_id = Some(format!("id{n}"));
        })
    }
    fn previous(&self) {
        self.update(|s| {
            let n: u32 = s.current_title[4..8].parse::<u32>().unwrap_or(1).saturating_sub(1);
            s.current_title = format!("IMG_{n:04}.jpg");
            s.current_id = Some(format!("id{n}"));
        })
    }
    fn set_volume(&self, level: u8) {
        self.update(|s| s.brightness = level.clamp(1, 100))
    }
    fn set_mute(&self, mute: bool) {
        self.update(|s| s.screen_active = !mute)
    }
    fn stage_external(&self, image: Vec<u8>, title: String, uri: String) -> Result<(), String> {
        println!("[pappe] fremdbild {title:?} ({} bytes) von {uri}", image.len());
        self.update(|s| {
            s.current_title = title;
            s.current_id = Some("ext".into());
            s.external = true;
            s.external_uri = Some(uri);
            s.screen_active = true;
        });
        Ok(())
    }
    fn current_image(&self) -> Option<Vec<u8>> {
        // Kleinstes gueltiges JPEG: nur die Marker, reicht fuer den Content-Type.
        Some(vec![0xFF, 0xD8, 0xFF, 0xD9])
    }
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args().nth(1).and_then(|p| p.parse().ok()).unwrap_or(8128);
    let udn = format!("uuid:{}", slowshow_lib::upnp::new_uuid());
    let backend = Arc::new(Pappe {
        state: Mutex::new(Snapshot {
            playing: true,
            screen_active: true,
            brightness: 70,
            current_id: Some("id1".into()),
            current_title: "IMG_0001.jpg".into(),
            friendly_name: "Slowshow Pappe".into(),
            external: false,
            external_uri: None,
        }),
    });
    let ctx = Arc::new(http::Ctx {
        backend,
        udn: udn.clone(),
        subs: Arc::new(gena::Subscriptions::default()),
        client: reqwest::Client::new(),
    });

    let (tx, rx) = watch::channel(false);
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.expect("Port frei?");
    println!("[pappe] HTTP auf {addr}, UDN {udn}");
    tokio::spawn(ssdp::run(udn, port, SERVER_HEADER.to_string(), rx));

    // Alle 20 s ein Bildwechsel, damit Abonnenten Ereignisse sehen.
    let ticker_ctx = ctx.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(20));
        tick.tick().await;
        loop {
            tick.tick().await;
            ticker_ctx.backend.next();
            http::publish(ticker_ctx.clone()).await;
        }
    });

    // Laeuft, bis der Prozess beendet wird; SSDP verabschiedet sich dann nicht
    // mehr per byebye — fuer den Schreibtisch belanglos.
    if let Err(e) = axum::serve(listener, http::router(ctx)).await {
        eprintln!("HTTP: {e}");
    }
    let _ = tx.send(true);
}

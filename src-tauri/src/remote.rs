//! Steuerung über das Heimnetz per REST (FA-55, E-09).
//!
//! Ersetzt die gestrichene Kamera-Präsenzerkennung (E-05): ein vorhandener
//! Smart-Home-Bewegungsmelder ruft `/api/screen` auf und weckt den Rahmen.
//! Zusätzlich decken die Endpunkte die Grundeinstellungen ab, sodass keine
//! eigene Weboberfläche nötig ist (E-09, FA-44 entfällt).
//!
//! Beispiel für Home Assistant:
//!
//! ```yaml
//! rest_command:
//!   slowshow_wake:
//!     url: "http://tablet.local:8127/api/screen"
//!     method: POST
//!     headers: { Authorization: "Bearer DEIN_TOKEN" }
//!     payload: '{"on": true}'
//!     content_type: application/json
//! ```

use crate::commands;
use crate::control;
use crate::state::AppState;
use axum::extract::State as AxumState;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

#[derive(Clone)]
struct Ctx {
    app: AppHandle,
    /// Leeres Token = keine Prüfung. Im Heimnetz hinter dem Router vertretbar,
    /// deshalb ist das Token optional und nicht erzwungen.
    token: String,
}

impl Ctx {
    fn authorized(&self, headers: &HeaderMap) -> bool {
        if self.token.is_empty() {
            return true;
        }
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.trim_start_matches("Bearer ").trim() == self.token)
            .unwrap_or(false)
    }
}

/// Läuft der Server? Der Griff zum Beenden liegt im Zustand, damit ein
/// Umschalten in den Einstellungen sofort wirkt.
#[derive(Default)]
pub struct RemoteServer {
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
}

impl RemoteServer {
    pub fn is_running(&self) -> bool {
        self.shutdown.lock().map(|g| g.is_some()).unwrap_or(false)
    }

    /// Startet den Server neu passend zur aktuellen Konfiguration.
    pub fn apply_config(&self, app: &AppHandle) {
        let config = app.state::<AppState>().config_snapshot().remote;
        self.stop();
        if config.enabled {
            self.start(app.clone(), config.port, config.token);
        }
    }

    pub fn stop(&self) {
        if let Ok(mut guard) = self.shutdown.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(());
                log::info!("Heimnetz-Steuerung beendet");
            }
        }
    }

    fn start(&self, app: AppHandle, port: u16, token: String) {
        let (tx, rx) = oneshot::channel();
        if let Ok(mut guard) = self.shutdown.lock() {
            *guard = Some(tx);
        }

        let ctx = Ctx { app, token };
        tauri::async_runtime::spawn(async move {
            let router = build_router(ctx);
            let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    log::error!("Heimnetz-Steuerung konnte Port {port} nicht belegen: {e}");
                    return;
                }
            };
            log::info!("Heimnetz-Steuerung lauscht auf {addr}");

            let served = axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = rx.await;
                })
                .await;
            if let Err(e) = served {
                log::error!("Heimnetz-Steuerung beendet sich mit Fehler: {e}");
            }
        });
    }
}

fn build_router(ctx: Ctx) -> Router {
    Router::new()
        .route("/api/status", get(status))
        .route("/api/slideshow", post(slideshow))
        .route("/api/screen", post(screen))
        .route("/api/next", post(next))
        .route("/api/prev", post(prev))
        .route("/api/sync", post(sync))
        .route("/api/config", get(get_config).post(patch_config))
        .with_state(ctx)
}

#[derive(Deserialize)]
struct OnOff {
    on: bool,
}

fn guard(ctx: &Ctx, headers: &HeaderMap) -> Result<(), StatusCode> {
    if ctx.authorized(headers) {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

// Alle Handler sind bewusst duenn: die eigentliche Wirkung steht in
// `control`, damit MQTT dieselben Aktionen ausloest und beide Wege nicht
// auseinanderlaufen koennen.

async fn status(
    AxumState(ctx): AxumState<Ctx>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    guard(&ctx, &headers)?;
    Ok(Json(control::status(&ctx.app)))
}

async fn slideshow(
    AxumState(ctx): AxumState<Ctx>,
    headers: HeaderMap,
    Json(body): Json<OnOff>,
) -> Result<Json<Value>, StatusCode> {
    guard(&ctx, &headers)?;
    control::set_slideshow(&ctx.app, body.on);
    Ok(Json(json!({ "playing": body.on })))
}

async fn screen(
    AxumState(ctx): AxumState<Ctx>,
    headers: HeaderMap,
    Json(body): Json<OnOff>,
) -> Result<Json<Value>, StatusCode> {
    guard(&ctx, &headers)?;
    control::set_screen(&ctx.app, body.on);
    Ok(Json(json!({ "screen": body.on })))
}

async fn next(
    AxumState(ctx): AxumState<Ctx>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    guard(&ctx, &headers)?;
    control::next_slide(&ctx.app);
    Ok(Json(
        json!({ "slide": ctx.app.state::<AppState>().current_slide() }),
    ))
}

async fn prev(
    AxumState(ctx): AxumState<Ctx>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    guard(&ctx, &headers)?;
    control::prev_slide(&ctx.app);
    Ok(Json(
        json!({ "slide": ctx.app.state::<AppState>().current_slide() }),
    ))
}

async fn sync(
    AxumState(ctx): AxumState<Ctx>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    guard(&ctx, &headers)?;
    match commands::run_sync(&ctx.app, None, false).await {
        Ok(reports) => Ok(Json(json!({ "reports": reports }))),
        Err(e) => Ok(Json(json!({ "error": e }))),
    }
}

async fn get_config(
    AxumState(ctx): AxumState<Ctx>,
    headers: HeaderMap,
) -> Result<Json<Value>, StatusCode> {
    guard(&ctx, &headers)?;
    Ok(Json(control::config_summary(&ctx.app)))
}

async fn patch_config(
    AxumState(ctx): AxumState<Ctx>,
    headers: HeaderMap,
    Json(patch): Json<control::ConfigPatch>,
) -> Result<Json<Value>, StatusCode> {
    guard(&ctx, &headers)?;
    control::patch_config(&ctx.app, patch)
        .map(Json)
        .map_err(|e| {
            log::warn!("REST: Einstellung nicht uebernommen: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers_with(auth: Option<&str>) -> HeaderMap {
        let mut h = HeaderMap::new();
        if let Some(a) = auth {
            h.insert("authorization", a.parse().unwrap());
        }
        h
    }

    /// Prüft nur die Tokenlogik — dafür wird kein AppHandle gebraucht.
    fn check(token: &str, header: Option<&str>) -> bool {
        let h = headers_with(header);
        if token.is_empty() {
            return true;
        }
        h.get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(|v| v.trim_start_matches("Bearer ").trim() == token)
            .unwrap_or(false)
    }

    #[test]
    fn ohne_token_ist_alles_erlaubt() {
        // Im Heimnetz hinter dem Router ist das die bequeme Voreinstellung.
        assert!(check("", None));
        assert!(check("", Some("Bearer irgendwas")));
    }

    #[test]
    fn mit_token_wird_geprueft() {
        assert!(check("geheim", Some("Bearer geheim")));
        assert!(!check("geheim", Some("Bearer falsch")));
        assert!(!check("geheim", None));
    }

    #[test]
    fn token_ohne_bearer_praefix_wird_akzeptiert() {
        // Manche Automatisierungen schicken den nackten Wert.
        assert!(check("geheim", Some("geheim")));
    }

    #[test]
    fn onoff_verlangt_das_feld() {
        assert!(serde_json::from_str::<OnOff>(r#"{"on": true}"#).is_ok());
        assert!(serde_json::from_str::<OnOff>("{}").is_err());
    }
}

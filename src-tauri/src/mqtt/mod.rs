//! MQTT-Anbindung an Home Assistant (FA-55).
//!
//! ## Warum zusätzlich zu REST
//!
//! Bei REST verbindet sich Home Assistant zum Tablet — es muss dessen Adresse
//! kennen, und es erfährt Änderungen erst beim nächsten Abruf. Bei MQTT dreht
//! sich die Richtung um: der Rahmen meldet sich beim Broker. Damit braucht das
//! Tablet keine feste Adresse mehr, Zustandsänderungen kommen ohne Verzögerung
//! an, und über das „letzte Wort" (Last Will) sieht Home Assistant einen
//! Ausfall sofort statt erst beim nächsten fehlgeschlagenen Abruf.
//!
//! Beide Wege laufen über dieselben Aktionen aus [`crate::control`] — sie
//! können also nicht auseinanderlaufen.

pub mod topics;

pub use topics::{Topics, PAYLOAD_OFFLINE, PAYLOAD_ONLINE};

use crate::control;
use crate::state::{events, AppState};
use rumqttc::{AsyncClient, Event, LastWill, MqttOptions, Packet, QoS};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, oneshot};

/// Schlüssel, unter dem das Broker-Passwort verschlüsselt liegt (NF-05).
pub const PASSWORD_REF: &str = "__mqtt__";

/// Wie lange nach einer Zustandsänderung gewartet wird, bevor veröffentlicht
/// wird. Ein Sync löst viele Änderungen kurz hintereinander aus; ohne diese
/// Sammelfrist ginge pro Bild eine Nachricht an den Broker.
const DEBOUNCE: Duration = Duration::from_millis(400);

/// Auch ohne Änderung regelmäßig senden — hält die Werte in Home Assistant
/// frisch, falls eine Nachricht verloren ging.
const HEARTBEAT: Duration = Duration::from_secs(60);

/// Was die Oberfläche über die Verbindung wissen muss.
///
/// `connected` meint tatsächlich verbunden — nicht „die Aufgabe läuft".
/// Der Unterschied ist der ganze Zweck dieser Struktur: bei falscher
/// Broker-Adresse läuft die Aufgabe munter weiter und wiederholt im
/// Fünf-Sekunden-Takt, ohne je eine Verbindung zu bekommen. Stünde in der
/// Oberfläche dann „Verbunden", suchte man den Fehler an der falschen Stelle.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MqttStatus {
    /// Ist der Dienst überhaupt gestartet?
    pub running: bool,
    /// Steht die Verbindung zum Broker?
    pub connected: bool,
    /// Letzter Fehler, solange keine Verbindung steht.
    pub last_error: Option<String>,
}

/// Laufender MQTT-Dienst.
#[derive(Default)]
pub struct MqttService {
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
    /// Anstoß, den Zustand zu veröffentlichen.
    dirty: Mutex<Option<mpsc::Sender<()>>>,
    status: Mutex<MqttStatus>,
}

impl MqttService {
    pub fn status(&self) -> MqttStatus {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn set_status(&self, app: &AppHandle, status: MqttStatus) {
        if let Ok(mut guard) = self.status.lock() {
            *guard = status.clone();
        }
        let _ = app.emit(events::MQTT, status);
    }

    /// Startet den Dienst passend zur Konfiguration neu.
    pub fn apply_config(&self, app: &AppHandle) {
        let state = app.state::<AppState>();
        let config = state.config_snapshot().mqtt;
        self.stop();

        if !config.enabled || config.host.trim().is_empty() {
            self.set_status(app, MqttStatus::default());
            return;
        }

        self.set_status(
            app,
            MqttStatus {
                running: true,
                connected: false,
                last_error: None,
            },
        );

        let password = state
            .secrets
            .lock()
            .ok()
            .and_then(|s| s.get(PASSWORD_REF).map(|p| p.to_string()))
            .unwrap_or_default();

        self.start(app.clone(), config, password);
    }

    pub fn stop(&self) {
        if let Ok(mut guard) = self.shutdown.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(());
                log::info!("MQTT: Verbindung wird beendet");
            }
        }
        if let Ok(mut guard) = self.dirty.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.status.lock() {
            *guard = MqttStatus::default();
        }
    }

    /// Meldet, dass sich der Zustand geändert hat.
    ///
    /// Wird aus den Tauri-Ereignissen heraus gerufen. `try_send` statt `send`:
    /// der Kanal fasst genau einen ausstehenden Anstoß, mehr braucht es nicht —
    /// und ein voller Kanal darf den Aufrufer nicht blockieren.
    pub fn notify_changed(&self) {
        if let Ok(guard) = self.dirty.lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.try_send(());
            }
        }
    }

    fn start(&self, app: AppHandle, config: crate::model::MqttConfig, password: String) {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let (dirty_tx, mut dirty_rx) = mpsc::channel::<()>(1);

        if let Ok(mut guard) = self.shutdown.lock() {
            *guard = Some(shutdown_tx);
        }
        if let Ok(mut guard) = self.dirty.lock() {
            *guard = Some(dirty_tx);
        }

        let topics = Topics::new(&config.base_topic);

        tauri::async_runtime::spawn(async move {
            let mut options = MqttOptions::new(
                format!("slowshow-{}", topics.base),
                config.host.trim(),
                config.port,
            );
            options.set_keep_alive(Duration::from_secs(30));
            options.set_clean_session(false);

            if !config.username.trim().is_empty() {
                options.set_credentials(config.username.trim(), password);
            }

            // Das „letzte Wort": bricht die Verbindung ab, setzt der Broker
            // die Verfügbarkeit selbst auf offline. Retained, damit Home
            // Assistant den Zustand auch nach eigenem Neustart kennt.
            options.set_last_will(LastWill::new(
                topics.availability.clone(),
                PAYLOAD_OFFLINE,
                QoS::AtLeastOnce,
                true,
            ));

            let (client, mut eventloop) = AsyncClient::new(options, 32);
            let mut heartbeat = tokio::time::interval(HEARTBEAT);
            log::info!("MQTT: verbinde zu {}:{}", config.host, config.port);

            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        // Sauber abmelden, damit Home Assistant nicht auf das
                        // Zeitfenster des Last Will warten muss.
                        let _ = client
                            .publish(&topics.availability, QoS::AtLeastOnce, true, PAYLOAD_OFFLINE)
                            .await;
                        let _ = client.disconnect().await;
                        log::info!("MQTT: beendet");
                        return;
                    }

                    _ = dirty_rx.recv() => {
                        // Sammelfrist: waehrend eines Syncs aendert sich der
                        // Zustand im Sekundentakt.
                        tokio::time::sleep(DEBOUNCE).await;
                        while dirty_rx.try_recv().is_ok() {}
                        publish_state(&client, &topics, &app).await;
                    }

                    _ = heartbeat.tick() => {
                        publish_state(&client, &topics, &app).await;
                    }

                    event = eventloop.poll() => {
                        match event {
                            Ok(Event::Incoming(Packet::ConnAck(_))) => {
                                app.state::<MqttService>().set_status(
                                    &app,
                                    MqttStatus {
                                        running: true,
                                        connected: true,
                                        last_error: None,
                                    },
                                );
                                on_connected(&client, &topics, &config, &app).await;
                            }
                            Ok(Event::Incoming(Packet::Publish(p))) => {
                                let payload = String::from_utf8_lossy(&p.payload).to_string();
                                handle_command(&app, &topics, &p.topic, &payload);
                            }
                            Ok(_) => {}
                            Err(e) => {
                                // rumqttc verbindet beim naechsten poll selbst
                                // neu; ohne die Pause liefe das in einer engen
                                // Schleife und wuerde den Akku leeren (NF-06).
                                //
                                // Nur beim ersten Auftreten protokollieren: der
                                // Fehler wiederholt sich sonst alle fuenf
                                // Sekunden und ueberschwemmt das Log, das im
                                // Dauerbetrieb noch lesbar bleiben soll.
                                let text = e.to_string();
                                let before = app.state::<MqttService>().status();
                                if before.connected || before.last_error.as_deref() != Some(&text) {
                                    log::warn!("MQTT: {text}");
                                }
                                app.state::<MqttService>().set_status(
                                    &app,
                                    MqttStatus {
                                        running: true,
                                        connected: false,
                                        last_error: Some(text),
                                    },
                                );
                                tokio::time::sleep(Duration::from_secs(5)).await;
                            }
                        }
                    }
                }
            }
        });
    }
}

/// Nach dem Verbindungsaufbau: anmelden, Entitäten bekanntmachen, Zustand senden.
async fn on_connected(
    client: &AsyncClient,
    topics: &Topics,
    config: &crate::model::MqttConfig,
    app: &AppHandle,
) {
    log::info!("MQTT: verbunden, Basistopic '{}'", topics.base);

    if let Err(e) = client
        .publish(&topics.availability, QoS::AtLeastOnce, true, PAYLOAD_ONLINE)
        .await
    {
        log::warn!("MQTT: Verfügbarkeit nicht sendbar: {e}");
    }

    if let Err(e) = client
        .subscribe(&topics.command_filter, QoS::AtLeastOnce)
        .await
    {
        log::warn!("MQTT: Kommandos nicht abonnierbar: {e}");
    }

    if config.discovery {
        let entries = topics::discovery(topics, &config.discovery_prefix);
        let count = entries.len();
        for entry in entries {
            let payload = serde_json::to_vec(&entry.payload).unwrap_or_default();
            // Retained: Home Assistant findet die Entitäten auch dann, wenn es
            // später startet als der Rahmen.
            if let Err(e) = client
                .publish(&entry.topic, QoS::AtLeastOnce, true, payload)
                .await
            {
                log::warn!("MQTT: Discovery für {} fehlgeschlagen: {e}", entry.topic);
            }
        }
        log::info!("MQTT: {count} Entitäten angemeldet");
    }

    publish_state(client, topics, app).await;
}

async fn publish_state(client: &AsyncClient, topics: &Topics, app: &AppHandle) {
    let state = control::status(app);
    let payload = match serde_json::to_vec(&state) {
        Ok(p) => p,
        Err(e) => {
            log::warn!("MQTT: Zustand nicht serialisierbar: {e}");
            return;
        }
    };
    if let Err(e) = client
        .publish(&topics.state, QoS::AtMostOnce, true, payload)
        .await
    {
        log::debug!("MQTT: Zustand nicht sendbar: {e}");
    }
}

/// Führt ein eingehendes Kommando aus.
///
/// Als eigene Funktion gehalten, damit die Zuordnung Topic → Aktion ohne Broker
/// testbar bleibt.
pub fn handle_command(app: &AppHandle, topics: &Topics, topic: &str, payload: &str) {
    let Some(name) = topics.command_name(topic) else {
        return;
    };

    match name {
        "slideshow" => {
            if let Some(on) = control::parse_switch(payload) {
                control::set_slideshow(app, on);
            }
        }
        "screen" => {
            if let Some(on) = control::parse_switch(payload) {
                control::set_screen(app, on);
            }
        }
        "next" => control::next_slide(app),
        "prev" => control::prev_slide(app),
        "sync" => {
            // Nicht abwarten: ein Sync über tausende Bilder darf die
            // MQTT-Schleife nicht anhalten.
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::commands::run_sync(&app, None, false).await {
                    log::info!("MQTT: Sync nicht gestartet: {e}");
                }
            });
        }
        "interval" => match payload.trim().parse::<u32>() {
            Ok(v) => apply_patch(
                app,
                control::ConfigPatch {
                    interval_seconds: Some(v),
                    ..Default::default()
                },
            ),
            Err(_) => log::warn!("MQTT: 'interval' erwartet eine Zahl, war '{payload}'"),
        },
        "device_brightness" => {
            if let Some(on) = control::parse_switch(payload) {
                apply_patch(
                    app,
                    control::ConfigPatch {
                        device_brightness: Some(on),
                        ..Default::default()
                    },
                );
            }
        }
        "brightness" => match payload.trim().parse::<u8>() {
            Ok(v) => apply_patch(
                app,
                control::ConfigPatch {
                    brightness: Some(v),
                    ..Default::default()
                },
            ),
            Err(_) => log::warn!("MQTT: 'brightness' erwartet eine Zahl, war '{payload}'"),
        },
        "config" => match serde_json::from_str::<control::ConfigPatch>(payload) {
            Ok(patch) => apply_patch(app, patch),
            Err(e) => log::warn!("MQTT: 'config' ist kein gültiges JSON: {e}"),
        },
        other => log::debug!("MQTT: unbekanntes Kommando '{other}'"),
    }
}

fn apply_patch(app: &AppHandle, patch: control::ConfigPatch) {
    if let Err(e) = control::patch_config(app, patch) {
        log::warn!("MQTT: Einstellung nicht übernommen: {e}");
    }
}

//! Steueraktionen, die REST und MQTT gemeinsam nutzen (FA-55).
//!
//! Ohne diese Schicht müsste jede Aktion zweimal geschrieben werden — einmal
//! als axum-Handler, einmal als MQTT-Kommando. Zwei Umsetzungen desselben
//! Befehls laufen erfahrungsgemäß auseinander, und der Zustandsschnappschuss
//! ([`status`]) ist zusätzlich das, was Home Assistant über beide Wege liest:
//! Weicht er ab, zeigt die eine Anbindung etwas anderes als die andere.

use crate::schedule::DisplayState;
use crate::state::{events, AppState};
use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

/// Teilaktualisierung der Grundeinstellungen (E-09).
/// Alle Felder optional — Automatisierungen schicken meist nur eines.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPatch {
    pub interval_seconds: Option<u32>,
    pub schedule_enabled: Option<bool>,
    pub active_from: Option<String>,
    pub active_to: Option<String>,
    pub brightness: Option<u8>,
}

/// Vollständiger Zustand — die eine Wahrheit für REST und MQTT.
pub fn status(app: &AppHandle) -> Value {
    let state = app.state::<AppState>();
    let config = state.config_snapshot();
    let display = state.display_state();
    let stats = {
        let max = config.cache.max_bytes;
        state.cache.lock().map(|c| c.stats(max)).ok()
    };

    json!({
        "playing": state.is_playing(),
        "syncing": state.is_syncing(),
        "intervalSeconds": config.interval_seconds,
        "display": display,
        "currentSlide": state.current_slide(),
        "cache": stats,
        "sources": config.sources.iter().map(|s| json!({
            "id": s.id,
            "name": s.name,
            "enabled": s.enabled,
            "lastSync": s.last_sync,
        })).collect::<Vec<_>>(),
    })
}

/// Grundeinstellungen für `GET /api/config`.
pub fn config_summary(app: &AppHandle) -> Value {
    let c = app.state::<AppState>().config_snapshot();
    json!({
        "intervalSeconds": c.interval_seconds,
        "scheduleEnabled": c.schedule.enabled,
        "activeFrom": c.schedule.active_from,
        "activeTo": c.schedule.active_to,
        "brightness": c.brightness.level,
    })
}

/// Diashow starten oder pausieren.
pub fn set_slideshow(app: &AppHandle, on: bool) {
    let state = app.state::<AppState>();
    state.set_playing(on);
    let _ = app.emit(events::SLIDE, state.current_slide());
}

/// Bildschirm wecken oder schlafen legen (FA-55).
///
/// Setzt den Zeitplan nicht außer Kraft, sondern schickt einen Anzeigezustand:
/// die Oberfläche entscheidet über schwarzes Overlay bzw. Nachtuhr, der native
/// Teil über die Displayhelligkeit.
pub fn set_screen(app: &AppHandle, on: bool) {
    let config = app.state::<AppState>().config_snapshot();

    let display = if on {
        DisplayState {
            slideshow_active: true,
            show_night_clock: false,
            brightness: config.brightness.level,
        }
    } else {
        DisplayState {
            slideshow_active: false,
            show_night_clock: config.schedule.night_clock,
            brightness: 1,
        }
    };

    crate::brightness::apply(display.brightness);
    let _ = app.emit(events::DISPLAY, display);
}

pub fn next_slide(app: &AppHandle) {
    let slide = app.state::<AppState>().advance();
    let _ = app.emit(events::SLIDE, &slide);
}

pub fn prev_slide(app: &AppHandle) {
    let slide = app.state::<AppState>().back();
    let _ = app.emit(events::SLIDE, &slide);
}

/// Übernimmt eine Teilaktualisierung.
///
/// `AppState::update_config` klemmt alle Werte auf die im Lastenheft
/// festgelegten Bereiche — eine fehlerhafte Automatisierung kann den Rahmen
/// also nicht in einen unsinnigen Zustand bringen (FA-02).
pub fn patch_config(app: &AppHandle, patch: ConfigPatch) -> Result<Value, String> {
    let state = app.state::<AppState>();

    let updated = state.update_config(|c| {
        if let Some(v) = patch.interval_seconds {
            c.interval_seconds = v;
        }
        if let Some(v) = patch.schedule_enabled {
            c.schedule.enabled = v;
        }
        if let Some(v) = patch.active_from {
            c.schedule.active_from = v;
        }
        if let Some(v) = patch.active_to {
            c.schedule.active_to = v;
        }
        if let Some(v) = patch.brightness {
            c.brightness.level = v;
        }
    })?;

    let display = state.display_state();
    crate::brightness::apply(display.brightness);
    let _ = app.emit(events::CONFIG, &updated);
    let _ = app.emit(events::DISPLAY, display);

    Ok(config_summary(app))
}

/// Wandelt „ON"/„OFF" und Verwandtes in einen Schaltzustand.
///
/// Home Assistant schickt je nach Entität `ON`/`OFF`, manche Automatisierungen
/// `true`/`1`. Alle Schreibweisen zu akzeptieren erspart Fehlersuche an einer
/// Stelle, an der ein Tippfehler sonst einfach nichts täte.
pub fn parse_switch(payload: &str) -> Option<bool> {
    match payload.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "1" | "an" | "ein" => Some(true),
        "off" | "false" | "0" | "aus" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_switch_versteht_die_gaengigen_schreibweisen() {
        for s in ["ON", "on", "true", "1", "an", " Ein "] {
            assert_eq!(parse_switch(s), Some(true), "war: {s}");
        }
        for s in ["OFF", "off", "false", "0", "aus"] {
            assert_eq!(parse_switch(s), Some(false), "war: {s}");
        }
    }

    #[test]
    fn parse_switch_lehnt_unsinn_ab() {
        assert_eq!(parse_switch(""), None);
        assert_eq!(parse_switch("vielleicht"), None);
        assert_eq!(parse_switch("2"), None);
    }

    #[test]
    fn config_patch_akzeptiert_einzelne_felder() {
        let p: ConfigPatch = serde_json::from_str(r#"{"intervalSeconds": 60}"#).unwrap();
        assert_eq!(p.interval_seconds, Some(60));
        assert_eq!(p.schedule_enabled, None);
    }

    #[test]
    fn config_patch_akzeptiert_leeres_objekt() {
        let p: ConfigPatch = serde_json::from_str("{}").unwrap();
        assert!(p.interval_seconds.is_none());
        assert!(p.brightness.is_none());
    }
}

//! Steueraktionen, die REST, MQTT und UPnP gemeinsam nutzen (FA-55, E-47).
//!
//! Ohne diese Schicht müsste jede Aktion dreimal geschrieben werden — als
//! axum-Handler, als MQTT-Kommando und als UPnP-Aktion. Drei Umsetzungen
//! desselben Befehls laufen erfahrungsgemäß auseinander, und der
//! Zustandsschnappschuss ([`status`]) ist zusätzlich das, was Home Assistant
//! über REST und MQTT liest: Weicht er ab, zeigt die eine Anbindung etwas
//! anderes als die andere.
//!
//! Jeder Befehl schreibt eine INFO-Zeile mit seinem Kanal (E-53): Die Diagnose
//! zu E-48 hatte für einen Helligkeitsbefehl über DLNA keine einzige Logzeile,
//! ob er ankam, ließ sich nur über `GetVolume` zeigen.

use crate::model::AppConfig;
use crate::state::{events, AppState};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fmt;
use tauri::{AppHandle, Emitter, Manager};

/// Woher ein Befehl kommt — nur für das Log (E-53).
///
/// Die Oberfläche fehlt bewusst: sie ruft die Kommandos in `commands.rs`
/// direkt, und wer vor dem Rahmen steht, braucht keine Logzeile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Rest,
    Mqtt,
    Upnp,
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Origin::Rest => "REST",
            Origin::Mqtt => "MQTT",
            Origin::Upnp => "UPnP",
        })
    }
}

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
    /// Regelung an das Gerät abgeben oder zurückholen (E-22).
    pub device_brightness: Option<bool>,
}

/// Übernimmt einen Patch in die Konfiguration.
///
/// Reine Funktion, damit die Regel prüfbar ist: Liefert `true`, wenn dabei die
/// Gerätesteuerung (E-22) abgeschaltet wurde. Ein Helligkeitsbefehl, der sie
/// nicht ausdrücklich mitsetzt, holt die Regelung zurück (E-48). Vorher
/// speicherte er nur einen Wert, den niemand sah — der Regler in Home
/// Assistant stand auf 100, der Schirm auf 8 Prozent Automatik.
pub fn apply_patch(c: &mut AppConfig, patch: &ConfigPatch) -> bool {
    if let Some(v) = patch.interval_seconds {
        c.interval_seconds = v;
    }
    if let Some(v) = patch.schedule_enabled {
        c.schedule.enabled = v;
    }
    if let Some(v) = &patch.active_from {
        c.schedule.active_from = v.clone();
    }
    if let Some(v) = &patch.active_to {
        c.schedule.active_to = v.clone();
    }
    if let Some(v) = patch.device_brightness {
        c.brightness.device_controlled = v;
    }
    let mut took_over = false;
    if let Some(v) = patch.brightness {
        c.brightness.level = v;
        if patch.device_brightness.is_none() && c.brightness.device_controlled {
            c.brightness.device_controlled = false;
            took_over = true;
        }
    }
    took_over
}

/// Kurzfassung eines Patches für das Log (E-53).
pub fn describe_patch(patch: &ConfigPatch, took_over: bool) -> String {
    let an_aus = |v: bool| if v { "an" } else { "aus" };
    let mut parts: Vec<String> = Vec::new();
    if let Some(v) = patch.interval_seconds {
        parts.push(format!("Anzeigedauer {v} s"));
    }
    if let Some(v) = patch.schedule_enabled {
        parts.push(format!("Zeitplan {}", an_aus(v)));
    }
    if let Some(v) = &patch.active_from {
        parts.push(format!("aktiv ab {v}"));
    }
    if let Some(v) = &patch.active_to {
        parts.push(format!("aktiv bis {v}"));
    }
    if let Some(v) = patch.brightness {
        parts.push(format!("Helligkeit {v}"));
    }
    if let Some(v) = patch.device_brightness {
        parts.push(format!("Gerätesteuerung {}", an_aus(v)));
    }
    if took_over {
        parts.push("Gerätesteuerung aus".into());
    }
    if parts.is_empty() {
        "keine Änderung".into()
    } else {
        parts.join(", ")
    }
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
        // Die *eingestellte* Grundhelligkeit, nicht die gerade wirksame aus
        // `display`: Letztere ist nachts 1 und bei Gerätesteuerung 0 — beides
        // liegt außerhalb des Bereichs, den der Regler in Home Assistant
        // annimmt, und ein Regler soll ohnehin den Wert zeigen, den er stellt.
        "brightness": config.brightness.level,
        "deviceBrightness": config.brightness.device_controlled,
        // `null`, solange die JNI-Brücke nicht steht oder das Gerät nichts
        // meldet — Home Assistant zeigt die Entität dann als nicht verfügbar,
        // statt einen erfundenen Wert anzuzeigen (E-23).
        "battery": crate::battery::read(),
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
        "deviceBrightness": c.brightness.device_controlled,
    })
}

/// Diashow starten oder pausieren.
pub fn set_slideshow(app: &AppHandle, on: bool, origin: Origin) {
    log::info!("{origin}: Diashow {}", if on { "an" } else { "aus" });
    let state = app.state::<AppState>();
    state.set_playing(on);
    let _ = app.emit(events::SLIDE, state.current_slide());
}

/// Bildschirm wecken oder schlafen legen (FA-55).
///
/// Setzt den Zeitplan nicht außer Kraft, sondern überlagert ihn bis zu dessen
/// nächstem Umschalten (E-46). Der Zustand liegt im `AppState`, damit die
/// Anzeigeschleife, ein Neuladen der Oberfläche und der Status über REST und
/// MQTT dasselbe sehen — vorher kannte nur dieses eine Ereignis den Befehl,
/// und die Schleife sendete danach nie wieder. Die Oberfläche entscheidet über
/// schwarzes Overlay bzw. Nachtuhr, der native Teil über die Displayhelligkeit.
pub fn set_screen(app: &AppHandle, on: bool, origin: Origin) {
    log::info!("{origin}: Bildschirm {}", if on { "an" } else { "aus" });
    let state = app.state::<AppState>();
    // Ein Fremdbild (E-52) geht mit dem Schirm: Wer ihn dunkel will, will
    // auch die Türklingel nicht mehr sehen.
    let ended = !on && state.end_external();
    let display = state.set_display_override(on);
    crate::brightness::apply(display.brightness);
    let _ = app.emit(events::DISPLAY, display);
    if ended {
        let _ = app.emit(events::SLIDE, state.current_slide());
    }
}

pub fn next_slide(app: &AppHandle, origin: Origin) {
    log::info!("{origin}: nächstes Bild");
    finish_external(app);
    let slide = app.state::<AppState>().advance();
    let _ = app.emit(events::SLIDE, &slide);
}

pub fn prev_slide(app: &AppHandle, origin: Origin) {
    log::info!("{origin}: vorheriges Bild");
    finish_external(app);
    let slide = app.state::<AppState>().back();
    let _ = app.emit(events::SLIDE, &slide);
}

/// Zeigt ein vorgemerktes Fremdbild (E-52) und weckt dafür den Schirm.
///
/// Über den Anzeigebefehl aus E-46, nicht am Zeitplan vorbei: so sehen
/// Anzeigeschleife, Oberfläche und Status dasselbe, und nach dem Ende gilt
/// wieder der Plan. `false`, wenn nichts vorgemerkt ist.
pub fn show_external(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let Some(slide) = state.show_external() else {
        return false;
    };
    let title = state
        .external_info()
        .map(|(_, title, _)| title)
        .unwrap_or_default();
    log::info!("UPnP: Fremdbild '{title}' angezeigt");
    let display = state.set_display_override(true);
    crate::brightness::apply(display.brightness);
    let _ = app.emit(events::DISPLAY, display);
    let _ = app.emit(events::SLIDE, Some(slide));
    true
}

/// Räumt ein Fremdbild ab und gibt die Anzeige an den Zeitplan zurück (E-52).
///
/// Sendet bewusst **kein** SLIDE: Der Aufrufer weiß, was danach an der Wand
/// hängt, und ein zweites Ereignis dazwischen ließe die Oberfläche kurz das
/// falsche Bild zeigen. Der vorherige Weck- oder Schlafbefehl kommt nicht
/// zurück — Home Assistant schickt vor jedem `play_media` ein Stop, das ihn
/// ohnehin überschrieben hätte.
pub fn finish_external(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    if !state.end_external() {
        return false;
    }
    log::info!("Fremdbild beendet, es gilt wieder der Zeitplan");
    state.clear_display_override();
    let display = state.display_state();
    crate::brightness::apply(display.brightness);
    let _ = app.emit(events::DISPLAY, display);
    true
}

/// Wie [`finish_external`], meldet aber auch das dann sichtbare Bild.
pub fn end_external(app: &AppHandle) -> bool {
    let ended = finish_external(app);
    if ended {
        let _ = app.emit(events::SLIDE, app.state::<AppState>().current_slide());
    }
    ended
}

/// Übernimmt eine Teilaktualisierung.
///
/// `AppState::update_config` klemmt alle Werte auf die im Lastenheft
/// festgelegten Bereiche — eine fehlerhafte Automatisierung kann den Rahmen
/// also nicht in einen unsinnigen Zustand bringen (FA-02).
pub fn patch_config(app: &AppHandle, patch: ConfigPatch, origin: Origin) -> Result<Value, String> {
    let state = app.state::<AppState>();

    let mut took_over = false;
    let updated = state.update_config(|c| took_over = apply_patch(c, &patch))?;
    log::info!("{origin}: {}", describe_patch(&patch, took_over));

    // Ein aktiver Weck- oder Schlafbefehl trägt sonst die Helligkeit von
    // vorhin weiter — der Regler bewegte sich, der Schirm nicht (E-50).
    state.refresh_display_override();
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
        assert!(p.device_brightness.is_none());
    }

    #[test]
    fn config_patch_nimmt_die_geraetesteuerung_entgegen_e_22() {
        // Der Feldname ist der Vertrag mit REST (`POST /api/config`) und mit
        // dem MQTT-Kommando `config`. Schreibt Rust `device_brightness` statt
        // `deviceBrightness`, wird das Feld stumm ignoriert — serde meldet
        // unbekannte Felder nicht.
        let p: ConfigPatch = serde_json::from_str(r#"{"deviceBrightness": true}"#).unwrap();
        assert_eq!(p.device_brightness, Some(true));

        let p: ConfigPatch = serde_json::from_str(r#"{"deviceBrightness": false}"#).unwrap();
        assert_eq!(p.device_brightness, Some(false));
    }

    fn geraet_regelt() -> AppConfig {
        let mut c = AppConfig::default();
        c.brightness.device_controlled = true;
        c.brightness.level = 80;
        c
    }

    #[test]
    fn helligkeitsbefehl_holt_die_regelung_zurueck_e_48() {
        // Der Befund am Referenzgeraet: Regler auf 100, Schirm auf 8 Prozent
        // Automatik, weil der Wert nur gespeichert wurde. Ein Regler, der
        // sichtbar nichts tut, ist die schlechteste Variante.
        let mut c = geraet_regelt();
        let patch = ConfigPatch {
            brightness: Some(50),
            ..Default::default()
        };
        assert!(apply_patch(&mut c, &patch), "Uebernahme muss gemeldet werden");
        assert_eq!(c.brightness.level, 50);
        assert!(!c.brightness.device_controlled);
    }

    #[test]
    fn ausdrueckliche_geraetesteuerung_schlaegt_die_uebernahme_e_48() {
        // MQTT `config` kann beides zugleich schicken; wer die Automatik
        // ausdruecklich anlaesst, meint das auch so.
        let mut c = geraet_regelt();
        let patch = ConfigPatch {
            brightness: Some(50),
            device_brightness: Some(true),
            ..Default::default()
        };
        assert!(!apply_patch(&mut c, &patch));
        assert_eq!(c.brightness.level, 50);
        assert!(c.brightness.device_controlled);
    }

    #[test]
    fn ohne_helligkeit_bleibt_die_geraetesteuerung_e_48() {
        let mut c = geraet_regelt();
        let patch = ConfigPatch {
            interval_seconds: Some(45),
            ..Default::default()
        };
        assert!(!apply_patch(&mut c, &patch));
        assert_eq!(c.interval_seconds, 45);
        assert!(c.brightness.device_controlled);
    }

    #[test]
    fn uebernahme_nur_wenn_das_geraet_vorher_regelte_e_48() {
        // Sonst meldete jede Helligkeitsaenderung eine Uebernahme ins Log.
        let mut c = AppConfig::default();
        assert!(!c.brightness.device_controlled);
        let patch = ConfigPatch {
            brightness: Some(50),
            ..Default::default()
        };
        assert!(!apply_patch(&mut c, &patch));
        assert_eq!(c.brightness.level, 50);
    }

    #[test]
    fn origin_nennt_den_kanal_e_53() {
        assert_eq!(Origin::Rest.to_string(), "REST");
        assert_eq!(Origin::Mqtt.to_string(), "MQTT");
        assert_eq!(Origin::Upnp.to_string(), "UPnP");
    }

    #[test]
    fn describe_patch_nennt_werte_und_uebernahme_e_53() {
        let patch = ConfigPatch {
            brightness: Some(50),
            interval_seconds: Some(20),
            ..Default::default()
        };
        assert_eq!(
            describe_patch(&patch, true),
            "Anzeigedauer 20 s, Helligkeit 50, Gerätesteuerung aus"
        );
        assert_eq!(describe_patch(&ConfigPatch::default(), false), "keine Änderung");
    }
}

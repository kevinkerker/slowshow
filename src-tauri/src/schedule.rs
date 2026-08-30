//! Zeitsteuerung für den Dauerbetrieb (FA-52, FA-53, FA-54).
//!
//! Reine Rechenlogik ohne Systemzugriff: die aktuelle Uhrzeit kommt als
//! Parameter herein. Dadurch sind auch die unbequemen Fälle testbar —
//! Zeiträume über Mitternacht und die Grenzminuten.
//!
//! Das tatsächliche Abdunkeln teilt sich auf: das schwarze Overlay setzt das
//! Frontend (funktioniert überall), die echte Displayhelligkeit setzt der
//! native Android-Code (siehe `android-src/MainActivity.kt`).

use crate::model::{AppConfig, BrightnessConfig, ScheduleConfig};

/// Was Anzeige und Helligkeit gerade tun sollen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayState {
    /// Läuft die Diashow? Außerhalb der Aktivzeit `false` (FA-52).
    pub slideshow_active: bool,
    /// Statt Schwarz eine gedimmte Uhr zeigen (FA-54).
    pub show_night_clock: bool,
    /// Zielhelligkeit in Prozent (FA-53), oder [`DEVICE_CONTROLLED`].
    pub brightness: u8,
}

/// „Die App regelt die Helligkeit nicht" (E-22).
///
/// Als Wert im selben Feld statt als zweites Feld: die Helligkeit reist durch
/// das Ereignis `slowshow://display`, über die REST-Schnittstelle (FA-55) und
/// nach MQTT. Ein zusätzliches Feld müsste überall mitgeführt werden und wäre
/// an jeder Stelle, die es übersieht, stumm wirkungslos. Null liegt außerhalb
/// des gültigen Bereichs 1..=100 und kann deshalb nichts anderes bedeuten.
pub const DEVICE_CONTROLLED: u8 = 0;

/// Helligkeit außerhalb der Aktivzeit (FA-52).
///
/// 1 statt 0, weil 0 auf manchen Geräten die Hintergrundbeleuchtung ganz
/// abschaltet und das Aufwecken per FA-55 dann nicht mehr sichtbar wäre — und
/// weil 0 seit E-22 „Gerät regelt selbst" bedeutet.
pub const NIGHT_BRIGHTNESS: u8 = 1;

/// Filtert jede Helligkeit, die die App setzen möchte (E-22).
///
/// Überlässt der Nutzer die Regelung dem Gerät, gibt die App sie in **jedem**
/// Zustand ab — auch nachts und auch auf einen Schlafbefehl aus dem Heimnetz
/// hin. Eine Ausnahme „nur nachts doch" wäre nicht zu erklären: der Rahmen
/// verhielte sich dann abends anders als morgens, ohne dass jemand etwas
/// umgestellt hätte.
///
/// FA-52 bleibt trotzdem erfüllt, ohne die Beleuchtung anzufassen: außerhalb
/// der Aktivzeit legt die Oberfläche den Schirm auf Schwarz (`dimOpacity` in
/// `src/lib/dim.ts`). Geschwärzt wird also der Inhalt — nur eben nicht
/// zusätzlich die Hintergrundbeleuchtung.
pub fn app_brightness(b: &BrightnessConfig, wanted: u8) -> u8 {
    if b.device_controlled {
        DEVICE_CONTROLLED
    } else {
        wanted
    }
}

/// Zerlegt "HH:MM" in Minuten seit Mitternacht.
///
/// Gibt `None` bei allem, was nicht exakt dem Format entspricht — die
/// Konfiguration kann von Hand oder per REST (FA-55) gesetzt worden sein.
pub fn parse_hhmm(s: &str) -> Option<u32> {
    let (h, m) = s.trim().split_once(':')?;
    let h: u32 = h.parse().ok()?;
    let m: u32 = m.parse().ok()?;
    if h > 23 || m > 59 {
        return None;
    }
    Some(h * 60 + m)
}

/// Liegt `now` im Zeitraum `from`..`to`?
///
/// `to <= from` bedeutet einen über Mitternacht laufenden Zeitraum
/// (z. B. 22:00–07:00). `from == to` heißt „ganzer Tag" — sonst wäre ein
/// versehentlich gleich gesetztes Paar ein dauerhaft schwarzer Bildschirm.
pub fn is_within(now: u32, from: u32, to: u32) -> bool {
    if from == to {
        true
    } else if from < to {
        now >= from && now < to
    } else {
        now >= from || now < to
    }
}

/// Ist die Diashow laut Zeitplan aktiv? (FA-52)
pub fn is_active(schedule: &ScheduleConfig, now_minutes: u32) -> bool {
    if !schedule.enabled {
        return true;
    }
    // Unlesbare Zeiten dürfen den Bilderrahmen nicht dauerhaft schwärzen.
    let (Some(from), Some(to)) = (
        parse_hhmm(&schedule.active_from),
        parse_hhmm(&schedule.active_to),
    ) else {
        log::warn!(
            "Zeitplan unlesbar ({} - {}), Diashow bleibt aktiv",
            schedule.active_from,
            schedule.active_to
        );
        return true;
    };
    is_within(now_minutes, from, to)
}

/// Wann die abendliche Absenkung endet, wenn kein Zeitplan aktiv ist.
const DEFAULT_DIM_UNTIL: &str = "06:00";

/// Zielhelligkeit innerhalb der Aktivzeit (FA-53).
///
/// Die Absenkung läuft von `dim_from` bis zum Beginn der Aktivzeit — also über
/// Mitternacht hinweg. Eine reine `now >= dim_from`-Prüfung wäre um 00:00 wieder
/// aufgehellt und ließe den Rahmen den Rest der Nacht auf voller Helligkeit
/// laufen; genau das soll FA-53 verhindern.
///
/// Ohne Zeitplan gibt es keine Aufwachzeit, dann gilt [`DEFAULT_DIM_UNTIL`].
pub fn active_brightness(b: &BrightnessConfig, schedule: &ScheduleConfig, now_minutes: u32) -> u8 {
    // Überlässt der Nutzer die Regelung dem Gerät, hat die App hier nichts zu
    // bestimmen — auch nicht die abendliche Absenkung, die sonst gegen die
    // Automatik des Systems arbeiten würde (E-22). Dieselbe Regel wie in
    // `app_brightness`, nur früher: der Rest der Funktion wäre gegenstandslos.
    if b.device_controlled {
        return DEVICE_CONTROLLED;
    }
    if !b.auto_dim {
        return b.level;
    }
    let Some(dim_from) = parse_hhmm(&b.dim_from) else {
        return b.level;
    };

    let wake = if schedule.enabled {
        parse_hhmm(&schedule.active_from)
    } else {
        parse_hhmm(DEFAULT_DIM_UNTIL)
    }
    .unwrap_or(360);

    // Gleiche Zeiten hieße bei `is_within` „ganztägig" — als Absenkung wäre das
    // ein dauerhaft dunkler Rahmen. Dann lieber gar nicht absenken.
    if dim_from == wake {
        return b.level;
    }

    if is_within(now_minutes, dim_from, wake) {
        b.dim_level.min(b.level)
    } else {
        b.level
    }
}

/// Helligkeit beim Wecken über die Heimnetz-Steuerung (FA-55).
///
/// Bewusst ohne Zeitplan und ohne Abendabsenkung: „Bildschirm an" heißt, dass
/// jemand den Rahmen jetzt sehen will.
pub fn wake_brightness(b: &BrightnessConfig) -> u8 {
    app_brightness(b, b.level)
}

/// Gesamtzustand aus Zeitplan und Helligkeit.
pub fn evaluate(config: &AppConfig, now_minutes: u32) -> DisplayState {
    if is_active(&config.schedule, now_minutes) {
        DisplayState {
            slideshow_active: true,
            show_night_clock: false,
            brightness: active_brightness(&config.brightness, &config.schedule, now_minutes),
        }
    } else {
        DisplayState {
            slideshow_active: false,
            show_night_clock: config.schedule.night_clock,
            // FA-52: außerhalb der Aktivzeit „Bildschirm geschwärzt bzw.
            // Helligkeit maximal reduziert". Den Inhalt schwärzt in jedem Fall
            // die Oberfläche; die Beleuchtung senkt die App nur, solange sie
            // sie überhaupt regeln darf (E-22).
            brightness: app_brightness(&config.brightness, NIGHT_BRIGHTNESS),
        }
    }
}

/// Minuten seit Mitternacht in lokaler Zeit — der einzige Systemzugriff des Moduls.
pub fn now_local_minutes() -> u32 {
    use chrono::Timelike;
    let now = chrono::Local::now();
    now.hour() * 60 + now.minute()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hm(h: u32, m: u32) -> u32 {
        h * 60 + m
    }

    #[test]
    fn parse_hhmm_liest_gueltige_zeiten() {
        assert_eq!(parse_hhmm("07:00"), Some(420));
        assert_eq!(parse_hhmm("00:00"), Some(0));
        assert_eq!(parse_hhmm("23:59"), Some(1439));
        assert_eq!(
            parse_hhmm(" 7:05 "),
            Some(425),
            "Leerraum und einstellige Stunde"
        );
    }

    #[test]
    fn parse_hhmm_weist_unsinn_zurueck() {
        assert_eq!(parse_hhmm("24:00"), None);
        assert_eq!(parse_hhmm("12:60"), None);
        assert_eq!(parse_hhmm("abc"), None);
        assert_eq!(parse_hhmm("12"), None);
        assert_eq!(parse_hhmm(""), None);
    }

    #[test]
    fn is_within_normaler_zeitraum() {
        let (from, to) = (hm(7, 0), hm(22, 0));
        assert!(!is_within(hm(6, 59), from, to));
        assert!(is_within(hm(7, 0), from, to), "Startminute gehört dazu");
        assert!(is_within(hm(14, 30), from, to));
        assert!(
            !is_within(hm(22, 0), from, to),
            "Endminute gehört nicht mehr dazu"
        );
        assert!(!is_within(hm(23, 0), from, to));
    }

    #[test]
    fn is_within_ueber_mitternacht() {
        // Nachtschicht-Bilderrahmen: 22:00 bis 07:00.
        let (from, to) = (hm(22, 0), hm(7, 0));
        assert!(is_within(hm(23, 0), from, to));
        assert!(is_within(hm(0, 0), from, to));
        assert!(is_within(hm(6, 59), from, to));
        assert!(!is_within(hm(7, 0), from, to));
        assert!(!is_within(hm(12, 0), from, to));
    }

    #[test]
    fn is_within_gleiche_zeiten_heisst_ganztaegig() {
        // Sonst wäre ein Tippfehler ein dauerhaft schwarzer Bildschirm.
        assert!(is_within(hm(3, 0), hm(9, 0), hm(9, 0)));
        assert!(is_within(hm(15, 0), hm(9, 0), hm(9, 0)));
    }

    #[test]
    fn is_active_ohne_zeitplan_immer_an() {
        let s = ScheduleConfig {
            enabled: false,
            ..Default::default()
        };
        assert!(is_active(&s, hm(3, 0)));
    }

    #[test]
    fn is_active_mit_zeitplan_fa_52() {
        let s = ScheduleConfig {
            enabled: true,
            active_from: "07:00".into(),
            active_to: "22:00".into(),
            night_clock: true,
            ..Default::default()
        };
        assert!(is_active(&s, hm(12, 0)));
        assert!(!is_active(&s, hm(23, 0)));
        assert!(!is_active(&s, hm(3, 0)));
    }

    #[test]
    fn is_active_bleibt_an_wenn_der_zeitplan_kaputt_ist() {
        // Ein defekter Konfigurationswert darf den Bilderrahmen nicht stilllegen.
        let s = ScheduleConfig {
            enabled: true,
            active_from: "kaputt".into(),
            active_to: "22:00".into(),
            night_clock: true,
            ..Default::default()
        };
        assert!(is_active(&s, hm(3, 0)));
    }

    /// Kein Zeitplan — dann endet die Absenkung um DEFAULT_DIM_UNTIL.
    fn kein_zeitplan() -> ScheduleConfig {
        ScheduleConfig {
            enabled: false,
            ..Default::default()
        }
    }

    #[test]
    fn brightness_ohne_automatik_bleibt_konstant() {
        let b = BrightnessConfig {
            level: 80,
            auto_dim: false,
            ..Default::default()
        };
        assert_eq!(active_brightness(&b, &kein_zeitplan(), hm(23, 0)), 80);
    }

    #[test]
    fn brightness_senkt_abends_ab_fa_53() {
        let b = BrightnessConfig {
            level: 100,
            auto_dim: true,
            dim_from: "20:00".into(),
            dim_level: 30,
            ..Default::default()
        };
        let s = kein_zeitplan();
        assert_eq!(active_brightness(&b, &s, hm(19, 59)), 100);
        assert_eq!(active_brightness(&b, &s, hm(20, 0)), 30);
        assert_eq!(active_brightness(&b, &s, hm(23, 30)), 30);
        assert_eq!(
            active_brightness(&b, &s, hm(8, 0)),
            100,
            "morgens wieder hell"
        );
    }

    #[test]
    fn brightness_bleibt_ueber_mitternacht_abgesenkt() {
        // Der eigentliche Fehlerfall: um 00:00 darf der Rahmen nicht wieder
        // auf volle Helligkeit springen und so die Nacht durchleuchten.
        let b = BrightnessConfig {
            level: 100,
            auto_dim: true,
            dim_from: "20:00".into(),
            dim_level: 30,
            ..Default::default()
        };
        let s = kein_zeitplan();
        assert_eq!(active_brightness(&b, &s, hm(0, 0)), 30);
        assert_eq!(active_brightness(&b, &s, hm(3, 0)), 30);
        assert_eq!(active_brightness(&b, &s, hm(5, 59)), 30);
        assert_eq!(
            active_brightness(&b, &s, hm(6, 0)),
            100,
            "ab 06:00 wieder hell"
        );
    }

    #[test]
    fn brightness_endet_mit_dem_beginn_der_aktivzeit() {
        let b = BrightnessConfig {
            level: 100,
            auto_dim: true,
            dim_from: "20:00".into(),
            dim_level: 30,
            ..Default::default()
        };
        let s = ScheduleConfig {
            enabled: true,
            active_from: "09:00".into(),
            active_to: "23:00".into(),
            night_clock: true,
            ..Default::default()
        };
        assert_eq!(active_brightness(&b, &s, hm(2, 0)), 30);
        assert_eq!(active_brightness(&b, &s, hm(8, 59)), 30);
        assert_eq!(active_brightness(&b, &s, hm(9, 0)), 100);
    }

    #[test]
    fn brightness_senkt_nicht_ab_wenn_start_und_ende_gleich_sind() {
        // Sonst wäre eine unglückliche Eingabe ein dauerhaft dunkler Rahmen.
        let b = BrightnessConfig {
            level: 100,
            auto_dim: true,
            dim_from: "09:00".into(),
            dim_level: 20,
            ..Default::default()
        };
        let s = ScheduleConfig {
            enabled: true,
            active_from: "09:00".into(),
            active_to: "23:00".into(),
            night_clock: true,
            ..Default::default()
        };
        assert_eq!(active_brightness(&b, &s, hm(12, 0)), 100);
        assert_eq!(active_brightness(&b, &s, hm(22, 0)), 100);
    }

    #[test]
    fn brightness_dimmt_nie_ueber_die_grundhelligkeit_hinaus() {
        let b = BrightnessConfig {
            level: 20,
            auto_dim: true,
            dim_from: "20:00".into(),
            dim_level: 60,
            ..Default::default()
        };
        assert_eq!(
            active_brightness(&b, &kein_zeitplan(), hm(21, 0)),
            20,
            "Absenkung darf nicht aufhellen"
        );
    }

    #[test]
    fn evaluate_liefert_nachtmodus_ausserhalb_der_aktivzeit_fa_54() {
        let c = AppConfig {
            schedule: ScheduleConfig {
                enabled: true,
                active_from: "07:00".into(),
                active_to: "22:00".into(),
                night_clock: true,
                ..Default::default()
            },
            ..Default::default()
        };

        let tag = evaluate(&c, hm(12, 0));
        assert!(tag.slideshow_active);
        assert!(!tag.show_night_clock);
        assert_eq!(tag.brightness, 100);

        let nacht = evaluate(&c, hm(23, 0));
        assert!(!nacht.slideshow_active);
        assert!(nacht.show_night_clock);
        assert_eq!(
            nacht.brightness, 1,
            "maximal reduziert, aber nicht ganz aus"
        );
    }

    #[test]
    fn geraetesteuerung_gibt_die_helligkeit_frei_e_22() {
        let b = BrightnessConfig {
            level: 40,
            device_controlled: true,
            ..Default::default()
        };
        assert_eq!(
            active_brightness(&b, &kein_zeitplan(), hm(12, 0)),
            DEVICE_CONTROLLED,
            "die eingestellte Grundhelligkeit gilt nicht mehr"
        );
    }

    #[test]
    fn geraetesteuerung_schaltet_die_abendabsenkung_ab_e_22() {
        // Sonst arbeitete die Absenkung gegen die Systemautomatik: die App
        // setzte abends 30 %, das Gerät regelte dagegen wieder hoch.
        let b = BrightnessConfig {
            level: 100,
            auto_dim: true,
            dim_from: "20:00".into(),
            dim_level: 30,
            device_controlled: true,
        };
        let s = ScheduleConfig {
            enabled: true,
            active_from: "07:00".into(),
            active_to: "22:00".into(),
            ..Default::default()
        };
        assert_eq!(active_brightness(&b, &s, hm(21, 0)), DEVICE_CONTROLLED);
    }

    #[test]
    fn wecken_laesst_dem_geraet_die_regelung_e_22() {
        let b = BrightnessConfig {
            level: 80,
            device_controlled: true,
            ..Default::default()
        };
        assert_eq!(wake_brightness(&b), DEVICE_CONTROLLED);

        let b = BrightnessConfig {
            level: 80,
            device_controlled: false,
            ..Default::default()
        };
        assert_eq!(wake_brightness(&b), 80, "sonst gilt die Grundhelligkeit");
    }

    #[test]
    fn geraetesteuerung_gilt_auch_nachts_e_22() {
        // Die App fasst die Beleuchtung in keinem Zustand mehr an. FA-52 wird
        // dann nicht ueber die Helligkeit erfuellt, sondern ueber das schwarze
        // Overlay der Oberflaeche — siehe `dimOpacity` in `src/lib/dim.ts`.
        let c = AppConfig {
            schedule: ScheduleConfig {
                enabled: true,
                active_from: "07:00".into(),
                active_to: "22:00".into(),
                ..Default::default()
            },
            brightness: BrightnessConfig {
                device_controlled: true,
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(evaluate(&c, hm(12, 0)).brightness, DEVICE_CONTROLLED);
        assert_eq!(evaluate(&c, hm(23, 0)).brightness, DEVICE_CONTROLLED);
        assert!(
            !evaluate(&c, hm(23, 0)).slideshow_active,
            "der Zeitplan gilt weiterhin, nur die Helligkeit nicht"
        );
    }

    #[test]
    fn ohne_geraetesteuerung_senkt_der_zeitplan_die_helligkeit_fa_52() {
        let c = AppConfig {
            schedule: ScheduleConfig {
                enabled: true,
                active_from: "07:00".into(),
                active_to: "22:00".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(evaluate(&c, hm(23, 0)).brightness, NIGHT_BRIGHTNESS);
    }

    #[test]
    fn app_brightness_reicht_ohne_geraetesteuerung_durch() {
        let b = BrightnessConfig::default();
        assert_eq!(app_brightness(&b, 42), 42);

        let b = BrightnessConfig {
            device_controlled: true,
            ..Default::default()
        };
        assert_eq!(app_brightness(&b, 42), DEVICE_CONTROLLED);
    }

    #[test]
    fn evaluate_ohne_nachtuhr_bleibt_schwarz() {
        let c = AppConfig {
            schedule: ScheduleConfig {
                enabled: true,
                active_from: "07:00".into(),
                active_to: "22:00".into(),
                night_clock: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let nacht = evaluate(&c, hm(2, 0));
        assert!(!nacht.slideshow_active);
        assert!(!nacht.show_night_clock);
    }

    #[test]
    fn now_local_minutes_liegt_im_tagesbereich() {
        assert!(now_local_minutes() < 1440);
    }
}

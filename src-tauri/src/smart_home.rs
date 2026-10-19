//! Smart-Home-Ansicht (FA-46, E-72 bis E-79).
//!
//! Auf dem Wandtablet ist Slowshow die Hauptanwendung. Ist in den
//! Einstellungen die Adresse einer Smart-Home-Oberfläche eingetragen, öffnet
//! ein kurzer Tipp in die Bildmitte diese Seite als Vollbild; nach einer
//! einstellbaren Zeit ohne Berührung oder über einen kleinen Knopf geht es
//! zurück zur Diashow.
//!
//! ## Aufteilung
//!
//! Die Seite selbst zeigt eine **native** Android-WebView über der Tauri-WebView
//! (`android-src/SmartHomeOverlay.kt`), kein iFrame (E-72). Nur dort sieht
//! jemand die Berührungen, also zählt dort auch der Timeout. Hier steht, was
//! sich ohne Gerät entscheiden und prüfen lässt:
//!
//! - welche Adressen überhaupt zulässig sind ([`sanitize_url`]),
//! - wann der Tipp die Ansicht öffnet und mit welchen Werten ([`plan_open`]),
//! - wann die WebView ihre Einstellungen neu bekommt ([`needs_reconfigure`]),
//! - ob die Ansicht gerade offen ist ([`SmartHomeView`]).
//!
//! Der JNI-Unterbau liegt wie bei Helligkeit und Ausrichtung in
//! [`crate::android_bridge`]. Auf allen anderen Plattformen ist jeder Aufruf
//! ein No-op mit `false` — der Desktop-Build darf am Feature weder abstürzen
//! noch einen Fehler werfen (Lastenheft 1.3).

use crate::model::{ButtonCorner, SmartHomeConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// Kürzeste Zeit bis zur Rückkehr zur Diashow (E-76, vorläufig).
///
/// Darunter schlösse sich die Ansicht, während jemand noch liest, welcher
/// Schalter welcher ist.
pub const TIMEOUT_MIN_SECS: u32 = 30;
/// Längste Zeit bis zur Rückkehr (E-76, vorläufig) — dieselbe Obergrenze wie
/// die Anzeigedauer eines Bildes (FA-02).
pub const TIMEOUT_MAX_SECS: u32 = 1800;
/// Voreinstellung (E-76, vorläufig): zwei Minuten, wie das Zeitlimit des
/// Kamera-Livebilds (E-67).
pub const TIMEOUT_DEFAULT_SECS: u32 = 120;

/// Prüft und bereinigt eine Adresse (FA-46).
///
/// Zulässig sind nur `http` und `https` mit Rechnernamen. Alles andere wird
/// zur leeren Zeichenkette, also „aus“. Die Adresse bleibt sonst wörtlich
/// stehen, wie sie eingetippt wurde: `Url::to_string` würde normalisieren
/// (etwa einen Schrägstrich anhängen), und die Einstellungen zeigten dann
/// etwas anderes als das, was der Nutzer eingegeben hat.
pub fn sanitize_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    match reqwest::Url::parse(trimmed) {
        Ok(url)
            if matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some_and(|h| !h.is_empty()) =>
        {
            trimmed.to_string()
        }
        _ => String::new(),
    }
}

/// Zahlenwert der Ecke für `SmartHomeOverlay` (E-74).
///
/// Eigene Funktion, damit die Zuordnung ohne Gerät prüfbar ist: eine
/// vertauschte Zahl setzte den Knopf in eine andere Ecke, und das fiele erst
/// am Gerät auf.
pub fn corner_jni_value(corner: ButtonCorner) -> i32 {
    match corner {
        ButtonCorner::TopLeft => 0,
        ButtonCorner::TopRight => 1,
        ButtonCorner::BottomLeft => 2,
        ButtonCorner::BottomRight => 3,
    }
}

/// Was die native Ansicht zum Öffnen braucht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRequest {
    pub url: String,
    pub timeout_seconds: u32,
    pub show_button: bool,
    pub corner: i32,
}

impl OpenRequest {
    fn from_config(cfg: &SmartHomeConfig) -> Self {
        Self {
            url: cfg.url.clone(),
            timeout_seconds: cfg.timeout_seconds,
            show_button: cfg.show_back_button,
            corner: corner_jni_value(cfg.back_button_corner),
        }
    }
}

/// Entscheidet, ob ein Tipp in die Mitte die Ansicht öffnet (E-73, E-79).
///
/// `None` heißt: es bleibt beim bisherigen Verhalten, der Tipp schaltet die
/// Pause um. Das gilt
///
/// - ohne eingetragene Adresse — ohne URL bleibt alles wie bisher (E-73);
/// - außerhalb der Aktivzeit bzw. im Ruhemodus (E-79, vorläufig): dort steht
///   die Fensterhelligkeit auf 1 % (`schedule::NIGHT_BRIGHTNESS`), die Seite
///   wäre kaum zu lesen, und der Schirm soll nachts dunkel bleiben (FA-54).
///
/// Die Werte werden hier noch einmal geklemmt, obwohl `AppConfig::clamp` das
/// schon tut — die WebView soll nie eine Adresse bekommen, die nicht durch
/// [`sanitize_url`] gegangen ist, egal auf welchem Weg sie in die Konfiguration
/// kam.
pub fn plan_open(cfg: &SmartHomeConfig, slideshow_active: bool) -> Option<OpenRequest> {
    if !slideshow_active {
        return None;
    }
    let mut cfg = cfg.clone();
    cfg.clamp();
    cfg.is_enabled().then(|| OpenRequest::from_config(&cfg))
}

/// Muss die native Ansicht ihre Einstellungen neu bekommen?
///
/// Eine geänderte Adresse lädt die WebView neu, eine geleerte zerstört sie
/// (E-75); Timeout, Knopf und Ecke gelten ab sofort. Alles andere an der
/// Konfiguration geht die Ansicht nichts an — ein Aufruf über die JNI-Brücke
/// bei jedem Schalter in den Einstellungen wäre Rauschen im Log.
pub fn needs_reconfigure(before: &SmartHomeConfig, after: &SmartHomeConfig) -> bool {
    before != after
}

/// Ob die Ansicht gerade offen ist.
///
/// Eigener verwalteter Zustand statt eines Felds im `AppState`: die native
/// Seite meldet das Schließen über JNI, und dort gibt es nur den
/// [`tauri::AppHandle`] aus [`register_app`].
#[derive(Debug, Default)]
pub struct SmartHomeView {
    open: AtomicBool,
}

impl SmartHomeView {
    pub fn is_open(&self) -> bool {
        self.open.load(Ordering::Relaxed)
    }

    /// Merkt die Ansicht als offen. Liefert, ob sie es vorher schon war.
    pub fn mark_open(&self) -> bool {
        self.open.swap(true, Ordering::Relaxed)
    }

    /// Merkt die Ansicht als geschlossen. Liefert, ob sie vorher offen war.
    pub fn mark_closed(&self) -> bool {
        self.open.swap(false, Ordering::Relaxed)
    }
}

/// Die laufende App — für die Meldung „Ansicht geschlossen“ aus Kotlin.
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();

/// Aus `setup()` gerufen, sobald der Zustand angemeldet ist.
pub fn register_app(app: &tauri::AppHandle) {
    let _ = APP.set(app.clone());
}

/// Öffnet die Ansicht (FA-46).
///
/// Liefert `true`, wenn die native Seite den Auftrag angenommen hat. Auf dem
/// Desktop, ohne Adresse, nachts oder ohne JNI-Brücke `false` — die
/// Oberfläche bleibt dann beim bisherigen Verhalten und schaltet die Pause um.
///
/// `label` ist die Aufschrift des Zurück-Knopfs in der Sprache der
/// Oberfläche; die Texte liegen in `de.json` und `en.json`, nicht hier.
pub fn open(view: &SmartHomeView, cfg: &SmartHomeConfig, slideshow_active: bool, label: &str) -> bool {
    let Some(request) = plan_open(cfg, slideshow_active) else {
        return false;
    };
    // Vor dem Aufruf setzen: die native Seite könnte das Schließen melden,
    // bevor dieser Aufruf zurückkehrt, und ein danach gesetztes `true` stünde
    // dann für immer.
    let was_open = view.mark_open();
    if native::show(&request, label) {
        if !was_open {
            log::info!("Smart-Home-Ansicht geöffnet");
        }
        true
    } else {
        if !was_open {
            view.mark_closed();
        }
        false
    }
}

/// Reicht geänderte Einstellungen an die native Ansicht weiter (E-75).
///
/// Gerufen aus `set_config` und `import_config`. Gibt es die WebView noch
/// nicht, verwirft die native Seite den Aufruf; sie bekommt beim ersten
/// Öffnen ohnehin alles mit.
pub fn apply_config(before: &SmartHomeConfig, after: &SmartHomeConfig) {
    if !needs_reconfigure(before, after) {
        return;
    }
    let mut after = after.clone();
    after.clamp();
    native::configure(&OpenRequest::from_config(&after));
}

/// Die native Seite meldet: Ansicht geschlossen (Timeout, Knopf, Zurück-Taste,
/// geleerte Adresse oder abgestürzter Renderer).
///
/// Das Ereignis geht aus einer eigenen Aufgabe hinaus und nicht direkt aus dem
/// JNI-Aufruf: der kommt auf dem UI-Thread an, und den soll das Senden an die
/// WebView nicht aufhalten.
pub fn closed_from_native() {
    let Some(app) = APP.get() else {
        log::warn!("Smart-Home-Ansicht geschlossen, aber die App ist noch nicht angemeldet");
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        use tauri::{Emitter, Manager};
        if let Some(view) = app.try_state::<SmartHomeView>() {
            if view.mark_closed() {
                log::info!("Smart-Home-Ansicht geschlossen, Diashow läuft weiter");
            }
        }
        let _ = app.emit(crate::state::events::SMART_HOME_CLOSED, ());
    });
}

#[cfg(target_os = "android")]
mod native {
    use super::OpenRequest;
    use jni::objects::{JObject, JValue};
    use jni::JNIEnv;

    pub fn show(req: &OpenRequest, label: &str) -> bool {
        crate::android_bridge::with_activity("Smart-Home-Ansicht öffnen", |env, activity| {
            let url = env.new_string(&req.url)?;
            let label = env.new_string(label)?;
            env.call_method(
                activity,
                "showSmartHome",
                "(Ljava/lang/String;IZILjava/lang/String;)V",
                &[
                    JValue::from(&url),
                    JValue::Int(req.timeout_seconds as i32),
                    JValue::Bool(u8::from(req.show_button)),
                    JValue::Int(req.corner),
                    JValue::from(&label),
                ],
            )
            .map(|_| ())
        })
        .is_some()
    }

    pub fn configure(req: &OpenRequest) {
        crate::android_bridge::with_activity("Smart-Home-Ansicht einstellen", |env, activity| {
            let url = env.new_string(&req.url)?;
            env.call_method(
                activity,
                "configureSmartHome",
                "(Ljava/lang/String;IZI)V",
                &[
                    JValue::from(&url),
                    JValue::Int(req.timeout_seconds as i32),
                    JValue::Bool(u8::from(req.show_button)),
                    JValue::Int(req.corner),
                ],
            )
            .map(|_| ())
        });
    }

    /// Wird von `MainActivity.nativeSmartHomeClosed` aufgerufen.
    ///
    /// Der Name folgt der JNI-Konvention `Java_<paket>_<klasse>_<methode>`;
    /// ändert sich der Paketname, muss er hier mitgeändert werden — wie bei
    /// `nativeRegisterActivity` in `android_bridge.rs`.
    #[no_mangle]
    pub extern "system" fn Java_dev_kerker_slowshow_MainActivity_nativeSmartHomeClosed(
        _env: JNIEnv,
        _activity: JObject,
    ) {
        super::closed_from_native();
    }
}

/// Ohne Android gibt es keine native Ansicht (Lastenheft 1.3).
#[cfg(not(target_os = "android"))]
mod native {
    use super::OpenRequest;

    pub fn show(_req: &OpenRequest, _label: &str) -> bool {
        false
    }

    pub fn configure(_req: &OpenRequest) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eingerichtet() -> SmartHomeConfig {
        SmartHomeConfig {
            url: "http://homeassistant.local:8123/wandtablet/jetzt?kiosk".into(),
            ..SmartHomeConfig::default()
        }
    }

    #[test]
    fn http_und_https_bleiben_woertlich_stehen() {
        for ok in [
            "http://homeassistant.local:8123/wandtablet/jetzt?kiosk",
            "https://ha.example.org/lovelace/0",
            "http://192.168.1.20:8123",
            "HTTP://homeassistant.local:8123/",
        ] {
            assert_eq!(sanitize_url(ok), ok, "{ok} ist zulaessig");
        }
    }

    #[test]
    fn leerraum_wird_abgeschnitten() {
        assert_eq!(
            sanitize_url("  http://ha.local:8123/x \n"),
            "http://ha.local:8123/x"
        );
    }

    #[test]
    fn andere_schemata_werden_geleert_fa_46() {
        for bad in [
            "javascript:alert(document.cookie)",
            "file:///sdcard/Download/x.html",
            "content://com.android.externalstorage.documents/tree/primary",
            "intent://scan/#Intent;scheme=zxing;end",
            "data:text/html,<h1>x</h1>",
            "ws://homeassistant.local:8123/api/websocket",
            "ftp://nas.local/",
            // Ohne Schema ist es keine Adresse, sondern ein Rechnername — die
            // WebView wuerde es als relativen Pfad deuten.
            "homeassistant.local:8123",
            "http://",
            "kein url",
        ] {
            assert_eq!(sanitize_url(bad), "", "{bad} haette abgewiesen werden muessen");
        }
    }

    #[test]
    fn leere_adresse_bleibt_leer() {
        assert_eq!(sanitize_url(""), "");
        assert_eq!(sanitize_url("   "), "");
    }

    #[test]
    fn ecken_entsprechen_der_kotlin_seite_e_74() {
        // Die Zuordnung steht doppelt: hier und als CORNER_* in
        // `SmartHomeOverlay.kt`. Laeuft sie auseinander, steht der Knopf in
        // der falschen Ecke -- ohne Fehlermeldung.
        assert_eq!(corner_jni_value(ButtonCorner::TopLeft), 0);
        assert_eq!(corner_jni_value(ButtonCorner::TopRight), 1);
        assert_eq!(corner_jni_value(ButtonCorner::BottomLeft), 2);
        assert_eq!(corner_jni_value(ButtonCorner::BottomRight), 3);
    }

    #[test]
    fn ohne_adresse_bleibt_der_tipp_eine_pause_e_73() {
        assert_eq!(plan_open(&SmartHomeConfig::default(), true), None);
    }

    #[test]
    fn mit_adresse_oeffnet_der_tipp_die_ansicht_e_73() {
        let req = plan_open(&eingerichtet(), true).expect("muss oeffnen");
        assert_eq!(req.url, "http://homeassistant.local:8123/wandtablet/jetzt?kiosk");
        assert_eq!(req.timeout_seconds, TIMEOUT_DEFAULT_SECS);
        assert!(req.show_button);
        assert_eq!(req.corner, 1, "oben rechts");
    }

    #[test]
    fn nachts_oeffnet_der_tipp_nichts_e_79() {
        // Ausserhalb der Aktivzeit steht die Helligkeit auf 1 %; die Seite
        // waere kaum zu lesen, und der Schirm soll dunkel bleiben (FA-54).
        assert_eq!(plan_open(&eingerichtet(), false), None);
    }

    #[test]
    fn plan_open_klemmt_selbst_nach() {
        // Eine Adresse, die an `clamp` vorbei in die Konfiguration kam, darf
        // die WebView trotzdem nie erreichen.
        let cfg = SmartHomeConfig {
            url: "javascript:alert(1)".into(),
            ..SmartHomeConfig::default()
        };
        assert_eq!(plan_open(&cfg, true), None);

        let cfg = SmartHomeConfig {
            timeout_seconds: 0,
            ..eingerichtet()
        };
        assert_eq!(plan_open(&cfg, true).unwrap().timeout_seconds, TIMEOUT_MIN_SECS);
    }

    #[test]
    fn knopf_und_ecke_reisen_mit_e_74() {
        let cfg = SmartHomeConfig {
            show_back_button: false,
            back_button_corner: ButtonCorner::BottomLeft,
            ..eingerichtet()
        };
        let req = plan_open(&cfg, true).unwrap();
        assert!(!req.show_button);
        assert_eq!(req.corner, 2);
    }

    #[test]
    fn neu_eingestellt_wird_nur_bei_aenderung_e_75() {
        let a = eingerichtet();
        assert!(!needs_reconfigure(&a, &a.clone()));

        let andere_adresse = SmartHomeConfig {
            url: "http://homeassistant.local:8123/wandtablet/kamera?kiosk".into(),
            ..a.clone()
        };
        assert!(needs_reconfigure(&a, &andere_adresse), "neue Adresse wird geladen");

        let geleert = SmartHomeConfig {
            url: String::new(),
            ..a.clone()
        };
        assert!(needs_reconfigure(&a, &geleert), "leere Adresse zerstoert die WebView");

        let kuerzer = SmartHomeConfig {
            timeout_seconds: 60,
            ..a.clone()
        };
        assert!(needs_reconfigure(&a, &kuerzer));
    }

    #[test]
    fn offen_und_geschlossen_werden_gemerkt() {
        let view = SmartHomeView::default();
        assert!(!view.is_open());
        assert!(!view.mark_open(), "war vorher zu");
        assert!(view.is_open());
        assert!(view.mark_open(), "zweites Oeffnen erkennt die offene Ansicht");
        assert!(view.mark_closed(), "war offen");
        assert!(!view.mark_closed(), "doppeltes Schliessen meldet nichts mehr");
        assert!(!view.is_open());
    }

    #[test]
    fn auf_dem_desktop_oeffnet_nichts_und_nichts_bleibt_haengen() {
        // Lastenheft 1.3: das Feature hat dort keine Wirkung, darf aber weder
        // abstuerzen noch die Ansicht als offen vermerken -- sonst stuende die
        // Diashow fuer immer.
        let view = SmartHomeView::default();
        assert!(!open(&view, &eingerichtet(), true, "Diashow"));
        assert!(!view.is_open());
        // Und das Weiterreichen von Einstellungen ist ein No-op.
        apply_config(&SmartHomeConfig::default(), &eingerichtet());
    }

    #[test]
    fn ohne_adresse_wird_auch_nichts_vermerkt() {
        let view = SmartHomeView::default();
        assert!(!open(&view, &SmartHomeConfig::default(), true, "Diashow"));
        assert!(!view.is_open());
    }
}

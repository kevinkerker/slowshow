//! Brücke zur Displayhelligkeit (FA-53).
//!
//! Die Frontend-Abdunkelung (schwarzes Overlay in `App.vue`) wirkt überall,
//! senkt aber nur die wahrgenommene Helligkeit — die Hintergrundbeleuchtung
//! läuft weiter auf voller Leistung. Für den 24/7-Betrieb zählt der
//! Unterschied: Wärmeentwicklung und Stromverbrauch (NF-06) hängen an der
//! Beleuchtung, nicht am angezeigten Bild.
//!
//! ## Warum die Activity sich selbst anmeldet
//!
//! Der naheliegende Weg über `ndk_context::android_context()` funktioniert
//! hier **nicht**: Tauri initialisiert diesen Kontext nicht, und die Funktion
//! paniert dann. In einem Release-Build mit `panic = "abort"` würde das die App
//! beenden — aus einer Hintergrundaufgabe heraus, die nur die Helligkeit
//! nachziehen wollte.
//!
//! Stattdessen ruft `MainActivity.onCreate` einmal
//! [`Java_dev_kerker_slowshow_MainActivity_nativeRegisterActivity`] auf und
//! übergibt sich selbst. Von da an ist der Aufruf ein gewöhnlicher
//! JNI-Methodenaufruf auf eine bekannte Referenz.
//!
//! Jeder Fehlerfall — Registrierung nicht erfolgt, Thread nicht anbindbar,
//! Java-Ausnahme — endet in einem No-op mit Log-Eintrag. Ein Bilderrahmen darf
//! an der Helligkeitssteuerung nicht scheitern (NF-01); die Abdunkelung im
//! Frontend bleibt in jedem Fall wirksam.

#[cfg(target_os = "android")]
mod android {
    use jni::objects::{GlobalRef, JObject, JValue};
    use jni::{JNIEnv, JavaVM};
    use std::sync::OnceLock;

    struct Registration {
        vm: JavaVM,
        activity: GlobalRef,
    }

    static ACTIVITY: OnceLock<Registration> = OnceLock::new();

    /// Wird von `MainActivity.onCreate` aufgerufen.
    ///
    /// Der Name folgt der JNI-Konvention `Java_<paket>_<klasse>_<methode>`;
    /// ändert sich der Paketname, muss er hier mitgeändert werden.
    ///
    /// # Safety
    ///
    /// Wird ausschließlich von der JVM aufgerufen, die gültige Parameter
    /// garantiert.
    #[no_mangle]
    pub extern "system" fn Java_dev_kerker_slowshow_MainActivity_nativeRegisterActivity(
        env: JNIEnv,
        activity: JObject,
    ) {
        let vm = match env.get_java_vm() {
            Ok(vm) => vm,
            Err(e) => {
                log::warn!("Helligkeit: JavaVM nicht ermittelbar: {e}");
                return;
            }
        };
        // Globale Referenz: die lokale gilt nur für diesen JNI-Aufruf.
        let activity = match env.new_global_ref(&activity) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("Helligkeit: Activity nicht referenzierbar: {e}");
                return;
            }
        };

        if ACTIVITY.set(Registration { vm, activity }).is_err() {
            // Kann bei einem Neustart der Activity vorkommen; die erste
            // Referenz bleibt gültig, weil es dieselbe Activity-Instanz ist.
            log::debug!("Helligkeit: bereits registriert");
        } else {
            log::info!("Helligkeit: Brücke zur MainActivity steht");
        }
    }

    pub fn apply(level: u8) {
        let Some(reg) = ACTIVITY.get() else {
            // Vor `onCreate` oder wenn das Laden der Bibliothek fehlschlug.
            return;
        };

        let mut env = match reg.vm.attach_current_thread() {
            Ok(env) => env,
            Err(e) => {
                log::warn!("Helligkeit: Thread nicht an die VM gebunden: {e}");
                return;
            }
        };

        // 0 wird durchgereicht statt auf 1 geklemmt: es ist der Sentinel aus
        // `schedule::DEVICE_CONTROLLED` und weist die Activity an, den
        // Helligkeits-Override wieder abzugeben (E-22).
        let value = if level == crate::schedule::DEVICE_CONTROLLED {
            0
        } else {
            level.clamp(1, 100) as i32
        };

        if let Err(e) = env.call_method(
            &reg.activity,
            "setScreenBrightness",
            "(I)V",
            &[JValue::Int(value)],
        ) {
            log::warn!("Helligkeit: Aufruf fehlgeschlagen: {e}");
            // Eine geworfene Java-Ausnahme muss gelöscht werden, sonst schlägt
            // der nächste JNI-Aufruf auf demselben Thread ebenfalls fehl.
            let _ = env.exception_clear();
        }
    }
}

/// Setzt die Displayhelligkeit auf `level` Prozent (1..=100).
///
/// [`schedule::DEVICE_CONTROLLED`](crate::schedule::DEVICE_CONTROLLED) gibt die
/// Regelung an das Gerät zurück, statt eine Helligkeit zu erzwingen (E-22).
///
/// Auf allen Plattformen außer Android ein No-op — der Desktop-Build ist
/// ohnehin kein Abnahmegegenstand (Lastenheft 1.3).
pub fn apply(level: u8) {
    #[cfg(target_os = "android")]
    android::apply(level);

    #[cfg(not(target_os = "android"))]
    let _ = level;
}

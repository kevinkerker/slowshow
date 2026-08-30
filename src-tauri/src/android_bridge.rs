//! Gemeinsame JNI-Brücke zur `MainActivity`.
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
//! übergibt sich selbst. Von da an ist jeder Aufruf ein gewöhnlicher
//! JNI-Methodenaufruf auf eine bekannte Referenz.
//!
//! ## Warum das ein eigenes Modul ist
//!
//! Die Registrierung lag zuerst in `brightness.rs`, weil die Helligkeit der
//! einzige Grund für nativen Code war. Mit der Akku-Telemetrie gibt es einen
//! zweiten Aufrufer, und eine zweite Registrierung wäre nicht nur doppelt,
//! sondern falsch: `OnceLock::set` schlägt beim zweiten Mal fehl, und je
//! nachdem, welches Modul zuerst dran ist, bliebe das andere ohne Brücke.
//!
//! Jeder Fehlerfall — Registrierung nicht erfolgt, Thread nicht anbindbar,
//! Java-Ausnahme — endet in einem `None` mit Log-Eintrag. Ein Bilderrahmen darf
//! an nativem Beiwerk nicht scheitern (NF-01).

#[cfg(target_os = "android")]
mod android {
    use jni::objects::{GlobalRef, JObject};
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
                log::warn!("JNI: JavaVM nicht ermittelbar: {e}");
                return;
            }
        };
        // Globale Referenz: die lokale gilt nur für diesen JNI-Aufruf.
        let activity = match env.new_global_ref(&activity) {
            Ok(r) => r,
            Err(e) => {
                log::warn!("JNI: Activity nicht referenzierbar: {e}");
                return;
            }
        };

        if ACTIVITY.set(Registration { vm, activity }).is_err() {
            // Kann bei einem Neustart der Activity vorkommen; die erste
            // Referenz bleibt gültig, weil es dieselbe Activity-Instanz ist.
            log::debug!("JNI: bereits registriert");
        } else {
            log::info!("JNI: Brücke zur MainActivity steht");
        }
    }

    /// Führt `f` mit angebundenem Thread und der registrierten Activity aus.
    ///
    /// Gibt `None` zurück, solange die Brücke nicht steht oder der Aufruf
    /// fehlschlägt — der Aufrufer entscheidet, was das bedeutet.
    pub fn with_activity<T>(
        what: &str,
        f: impl FnOnce(&mut JNIEnv, &GlobalRef) -> Result<T, jni::errors::Error>,
    ) -> Option<T> {
        let Some(reg) = ACTIVITY.get() else {
            // Sichtbar machen statt still nichts tun: genau so ist die
            // Ausrichtung beim Start ins Leere gelaufen, ohne eine Spur im Log
            // zu hinterlassen.
            log::warn!("JNI ({what}): Brücke steht noch nicht, Aufruf verworfen");
            return None;
        };

        let mut env = match reg.vm.attach_current_thread() {
            Ok(env) => env,
            Err(e) => {
                log::warn!("JNI ({what}): Thread nicht an die VM gebunden: {e}");
                return None;
            }
        };

        match f(&mut env, &reg.activity) {
            Ok(v) => Some(v),
            Err(e) => {
                log::warn!("JNI ({what}): Aufruf fehlgeschlagen: {e}");
                // Eine geworfene Java-Ausnahme muss gelöscht werden, sonst
                // schlägt der nächste JNI-Aufruf auf demselben Thread ebenfalls
                // fehl — mit einer Meldung, die nichts mit der Ursache zu tun hat.
                let _ = env.exception_clear();
                None
            }
        }
    }
}

#[cfg(target_os = "android")]
pub use android::with_activity;

/// Auf allen Plattformen außer Android gibt es keine Activity.
///
/// Der Desktop-Build ist kein Abnahmegegenstand (Lastenheft 1.3), muss aber
/// übersetzen und laufen, damit `cargo test` ohne Emulator nutzbar bleibt.
#[cfg(not(target_os = "android"))]
pub fn with_activity<T, F>(_what: &str, _f: F) -> Option<T> {
    None
}

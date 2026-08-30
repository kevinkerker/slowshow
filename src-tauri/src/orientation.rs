//! Ausrichtung des Rahmens (E-26).
//!
//! Quer oder hochkant wird als Einstellung gewählt und nicht dem Lagesensor
//! überlassen: ein fest an die Wand geschraubter Rahmen wird einmal
//! ausgerichtet und soll danach nie wieder drehen — auch nicht, wenn jemand
//! ihn beim Putzen anstößt. Wer es doch beweglich will, wählt
//! [`Orientation::Auto`].
//!
//! Der JNI-Unterbau liegt in [`crate::android_bridge`].

use crate::model::Orientation;

/// Umrechnung auf den Zahlenwert, den `MainActivity.setOrientation` erwartet.
///
/// Als eigene Funktion, damit die Zuordnung ohne Gerät prüfbar ist: eine
/// vertauschte Zahl würde den Rahmen quer stellen, obwohl hochkant eingestellt
/// ist, und wäre nur am Gerät zu bemerken.
pub fn jni_value(orientation: Orientation) -> i32 {
    match orientation {
        Orientation::Landscape => 0,
        Orientation::Portrait => 1,
        Orientation::Auto => 2,
    }
}

/// Setzt die Ausrichtung des Fensters.
///
/// Auf allen Plattformen außer Android ein No-op — der Desktop-Build ist
/// ohnehin kein Abnahmegegenstand (Lastenheft 1.3).
pub fn apply(orientation: Orientation) {
    let value = jni_value(orientation);

    #[cfg(target_os = "android")]
    crate::android_bridge::with_activity("Ausrichtung", |env, activity| {
        env.call_method(
            activity,
            "setOrientation",
            "(I)V",
            &[jni::objects::JValue::Int(value)],
        )
        .map(|_| ())
    });

    #[cfg(not(target_os = "android"))]
    let _ = value;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jni_werte_entsprechen_der_kotlin_seite_e_26() {
        // Die Zuordnung steht doppelt: hier und in `MainActivity.setOrientation`.
        // Laeuft sie auseinander, stellt sich der Rahmen falsch -- ohne
        // Fehlermeldung, weil beide Seiten fuer sich genommen gueltig sind.
        assert_eq!(jni_value(Orientation::Landscape), 0);
        assert_eq!(jni_value(Orientation::Portrait), 1);
        assert_eq!(jni_value(Orientation::Auto), 2);
    }

    #[test]
    fn voreinstellung_ist_querformat() {
        // Der Design-Canvas (E-13) kennt nur Querformat; ein Update darf einen
        // haengenden Rahmen nicht von selbst drehen.
        assert_eq!(jni_value(Orientation::default()), 0);
    }
}

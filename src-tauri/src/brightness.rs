//! Brücke zur Displayhelligkeit (FA-53).
//!
//! Die Frontend-Abdunkelung (schwarzes Overlay, `lib/dim.ts`) wirkt überall,
//! senkt aber nur die wahrgenommene Helligkeit — die Hintergrundbeleuchtung
//! läuft weiter auf voller Leistung. Für den 24/7-Betrieb zählt der
//! Unterschied: Wärmeentwicklung und Stromverbrauch (NF-06) hängen an der
//! Beleuchtung, nicht am angezeigten Bild.
//!
//! Der JNI-Unterbau liegt in [`crate::android_bridge`]; hier steht nur noch,
//! welche Java-Methode mit welchem Wert gerufen wird.

/// Umrechnung vom Anzeigezustand auf den Wert, den die Activity erwartet.
///
/// Als eigene Funktion, damit die Sonderbehandlung der Null ohne Gerät prüfbar
/// ist: 0 wird durchgereicht statt auf 1 geklemmt: es ist der Sentinel aus
/// [`schedule::DEVICE_CONTROLLED`](crate::schedule::DEVICE_CONTROLLED) und
/// weist die Activity an, den Helligkeits-Override abzugeben (E-22).
pub fn jni_value(level: u8) -> i32 {
    if level == crate::schedule::DEVICE_CONTROLLED {
        0
    } else {
        level.clamp(1, 100) as i32
    }
}

/// Setzt die Displayhelligkeit auf `level` Prozent (1..=100).
///
/// Auf allen Plattformen außer Android ein No-op — der Desktop-Build ist
/// ohnehin kein Abnahmegegenstand (Lastenheft 1.3).
pub fn apply(level: u8) {
    let value = jni_value(level);

    // `jni` ist eine Android-Abhängigkeit; ohne das `cfg` bräche der
    // Desktop-Build, auf dem `cargo test` ohne Emulator läuft.
    #[cfg(target_os = "android")]
    crate::android_bridge::with_activity("Helligkeit", |env, activity| {
        env.call_method(
            activity,
            "setScreenBrightness",
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
    use crate::schedule::DEVICE_CONTROLLED;

    #[test]
    fn geraetesteuerung_reicht_die_null_durch_e_22() {
        // Auf 1 geklemmt hiesse: die App erzwingt 1 % statt die Regelung
        // abzugeben. Der Rahmen waere dann dauerhaft fast dunkel.
        assert_eq!(jni_value(DEVICE_CONTROLLED), 0);
    }

    #[test]
    fn helligkeit_bleibt_im_gueltigen_bereich() {
        assert_eq!(jni_value(1), 1);
        assert_eq!(jni_value(50), 50);
        assert_eq!(jni_value(100), 100);
        assert_eq!(jni_value(200), 100, "oberhalb von 100 wird geklemmt");
    }
}

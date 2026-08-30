//! Akku-Telemetrie für den Dauerbetrieb.
//!
//! ## Warum ein Bilderrahmen das überhaupt braucht
//!
//! Ein Tablet, das monatelang am Netz hängt, steht dauerhaft bei 100 %
//! Ladestand und erwärmt sich dabei. Beides altert Lithium-Zellen deutlich
//! schneller als der normale Gebrauch; im schlechtesten Fall bläht sich der
//! Akku auf und drückt das Display aus dem Rahmen. Genau dieser Betriebsfall
//! ist in R-08 (Dauerbetrieb) angelegt, war aber bislang nicht beobachtbar.
//!
//! ## Warum die App nur misst und nicht regelt
//!
//! Die naheliegende Gegenmaßnahme — den Ladevorgang zwischen etwa 40 und 80 %
//! takten — braucht einen schaltbaren Zwischenstecker. Den gibt es im
//! Smart Home, und dort gehört die Automatik auch hin: Slowshow veröffentlicht
//! Ladestand, Temperatur und Ladezustand über die vorhandene Anbindung
//! (FA-55), Home Assistant entscheidet. Dieselbe Trennung wie bei der
//! Präsenzerkennung, die E-05 aus der App heraus in das Smart Home verlegt hat.
//!
//! Der native Teil liefert eine Zeichenkette statt drei Werte, weil ein
//! einzelner JNI-Aufruf mit `String`-Rückgabe deutlich weniger Zeremonie
//! braucht als drei Aufrufe oder ein `int[]`. Das Zerlegen passiert in
//! [`parse`] und ist damit ohne Gerät prüfbar.

use serde::Serialize;

/// Momentaufnahme des Akkus.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatteryState {
    /// Ladestand in Prozent (0..=100).
    pub level: u8,
    /// Temperatur in Grad Celsius.
    ///
    /// `f64` und nicht `f32`: aus 31,2 wird als `f32` im JSON
    /// `31.200000762939453`, weil serde die kürzeste Darstellung sucht, die den
    /// *f32*-Wert trifft, und ihn dafür in `f64` aufbläst. In Home Assistant
    /// stünde diese Zahl so im Verlauf. Mit `f64` schreibt serde `31.2`.
    pub temperature: f64,
    /// Hängt das Gerät gerade am Strom?
    pub charging: bool,
}

/// Zerlegt die Antwort von `MainActivity.batteryState`.
///
/// Erwartet `"<Prozent>;<Zehntelgrad>;<0|1>"`, also genau das, was die
/// Android-API liefert: `BatteryManager` gibt die Temperatur in Zehntelgrad
/// zurück, nicht in Grad. Die Umrechnung steht hier und nicht in Kotlin, damit
/// sie einen Test hat.
///
/// Gibt `None` bei allem, was nicht passt. Ein Bilderrahmen darf an einer
/// unlesbaren Akkuangabe nicht scheitern (NF-01) — dann gibt es eben keine.
pub fn parse(raw: &str) -> Option<BatteryState> {
    let mut parts = raw.trim().split(';');
    let level: i32 = parts.next()?.trim().parse().ok()?;
    let deci_celsius: i32 = parts.next()?.trim().parse().ok()?;
    let charging = parts.next()?.trim() == "1";
    if parts.next().is_some() {
        return None;
    }

    // Android meldet -1, wenn ein Wert nicht verfügbar ist. Als Ladestand wäre
    // das nach `as u8` die 255 und in Home Assistant ein Akku mit 255 Prozent.
    if !(0..=100).contains(&level) {
        return None;
    }

    Some(BatteryState {
        level: level as u8,
        temperature: deci_celsius as f64 / 10.0,
        charging,
    })
}

/// Liest den aktuellen Akkuzustand vom Gerät.
///
/// `None` auf allen Plattformen außer Android und immer dann, wenn die
/// JNI-Brücke nicht steht.
pub fn read() -> Option<BatteryState> {
    #[cfg(target_os = "android")]
    {
        let raw: String = crate::android_bridge::with_activity("Akku", |env, activity| {
            let value = env
                .call_method(activity, "batteryState", "()Ljava/lang/String;", &[])?
                .l()?;
            let s: String = env.get_string(&jni::objects::JString::from(value))?.into();
            Ok(s)
        })?;
        parse(&raw)
    }

    #[cfg(not(target_os = "android"))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_liest_die_drei_werte() {
        let b = parse("85;312;1").expect("gueltige Angabe");
        assert_eq!(b.level, 85);
        assert_eq!(b.temperature, 31.2, "Zehntelgrad werden umgerechnet");
        assert!(b.charging);
    }

    #[test]
    fn parse_erkennt_den_entladenen_zustand() {
        let b = parse("100;280;0").expect("gueltige Angabe");
        assert_eq!(b.level, 100);
        assert!(!b.charging, "0 heisst nicht am Strom");
    }

    #[test]
    fn parse_vertraegt_leerzeichen() {
        // Die Zeichenkette kommt aus Kotlin; ein Leerzeichen mehr oder weniger
        // darf die Telemetrie nicht abschalten.
        assert!(parse(" 50 ; 300 ; 1 ").is_some());
    }

    #[test]
    fn parse_lehnt_unsinn_ab() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("85"), None, "zu wenige Felder");
        assert_eq!(parse("85;312;1;7"), None, "zu viele Felder");
        assert_eq!(parse("viel;312;1"), None);
    }

    #[test]
    fn parse_lehnt_androids_minus_eins_ab() {
        // BatteryManager meldet -1 fuer "unbekannt". Ungeprueft wuerde daraus
        // nach `as u8` ein Ladestand von 255 Prozent.
        assert_eq!(parse("-1;312;1"), None);
        assert_eq!(parse("101;312;1"), None);
    }

    #[test]
    fn temperatur_bleibt_im_json_lesbar() {
        // Mit `f32` schrieb serde `31.200000762939453` -- am Geraet gemessen.
        let b = parse("80;312;1").unwrap();
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.contains(r#""temperature":31.2"#), "{json}");
    }

    #[test]
    fn parse_vertraegt_temperaturen_unter_null() {
        // Ein Rahmen im unbeheizten Wintergarten. Nur der Ladestand ist
        // begrenzt, die Temperatur nicht.
        let b = parse("60;-45;0").expect("gueltige Angabe");
        assert_eq!(b.temperature, -4.5);
    }
}

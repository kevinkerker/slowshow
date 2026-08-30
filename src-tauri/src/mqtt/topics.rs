//! Topics und Home-Assistant-Discovery.
//!
//! Bewusst frei von Netzwerkcode: Topic-Aufbau und Discovery-Nutzlast sind
//! reine Zeichenkettenarbeit und damit ohne Broker prüfbar. Genau dort passieren
//! die Fehler, die man sonst erst im Home-Assistant-Protokoll sieht.

use serde_json::{json, Value};

/// Erlaubt in Topics und Entitäts-Ids: Buchstaben, Ziffern, Unterstrich.
///
/// Ein Basistopic mit `/`, `+` oder `#` würde die Topic-Struktur zerlegen, und
/// Home Assistant baut aus der Knoten-Id Entitätsnamen — Leerzeichen und
/// Umlaute führen dort zu unbrauchbaren Ids.
pub fn sanitize(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "slowshow".to_string()
    } else {
        trimmed
    }
}

/// Alle Topics einer Instanz.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topics {
    pub base: String,
    pub availability: String,
    pub state: String,
    /// Abonnement-Muster für alle Kommandos.
    pub command_filter: String,
}

impl Topics {
    pub fn new(base_topic: &str) -> Self {
        let base = sanitize(base_topic);
        Self {
            availability: format!("{base}/availability"),
            state: format!("{base}/state"),
            command_filter: format!("{base}/cmd/+"),
            base,
        }
    }

    pub fn command(&self, name: &str) -> String {
        format!("{}/cmd/{name}", self.base)
    }

    /// Der Teil hinter `cmd/` — der eigentliche Befehl.
    pub fn command_name<'a>(&self, topic: &'a str) -> Option<&'a str> {
        topic.strip_prefix(&format!("{}/cmd/", self.base))
    }
}

pub const PAYLOAD_ONLINE: &str = "online";
pub const PAYLOAD_OFFLINE: &str = "offline";

/// Gerätebeschreibung — hängt alle Entitäten unter ein Gerät in Home Assistant.
fn device(node: &str) -> Value {
    json!({
        "identifiers": [format!("slowshow_{node}")],
        "name": "Slowshow",
        "manufacturer": "Kevin Kerker",
        "model": "Digitaler Bilderrahmen",
        "sw_version": env!("CARGO_PKG_VERSION"),
    })
}

/// Eine Discovery-Nachricht: wohin und was.
#[derive(Debug, Clone)]
pub struct Discovery {
    pub topic: String,
    pub payload: Value,
}

/// Baut alle Discovery-Nachrichten.
///
/// Home Assistant legt daraus von allein Schalter, Knöpfe und Sensoren an —
/// ohne eine Zeile YAML. Die Nachrichten werden retained veröffentlicht, damit
/// die Entitäten auch nach einem Neustart von Home Assistant wieder auftauchen.
pub fn discovery(topics: &Topics, prefix: &str) -> Vec<Discovery> {
    let node = &topics.base;
    let prefix = sanitize_prefix(prefix);
    let dev = device(node);

    // Jede Entität bekommt dieselbe Verfügbarkeitsquelle: bricht die
    // Verbindung ab, zeigt Home Assistant sie sofort als nicht verfügbar
    // (Last Will) statt veraltete Werte weiter anzuzeigen.
    let common = |extra: Value| -> Value {
        let mut v = json!({
            "availability_topic": topics.availability,
            "payload_available": PAYLOAD_ONLINE,
            "payload_not_available": PAYLOAD_OFFLINE,
            "device": dev,
        });
        merge(&mut v, extra);
        v
    };

    let mut out = Vec::new();
    let mut add = |component: &str, object: &str, payload: Value| {
        out.push(Discovery {
            topic: format!("{prefix}/{component}/{node}/{object}/config"),
            payload,
        });
    };

    // ── Schalter ────────────────────────────────────────────────────────────
    add(
        "switch",
        "slideshow",
        common(json!({
            "name": "Diashow",
            "unique_id": format!("slowshow_{node}_slideshow"),
            "state_topic": topics.state,
            "value_template": "{{ 'ON' if value_json.playing else 'OFF' }}",
            "command_topic": topics.command("slideshow"),
            "icon": "mdi:play-circle",
        })),
    );
    add(
        "switch",
        "screen",
        common(json!({
            "name": "Bildschirm",
            "unique_id": format!("slowshow_{node}_screen"),
            "state_topic": topics.state,
            "value_template": "{{ 'ON' if value_json.display.slideshowActive else 'OFF' }}",
            "command_topic": topics.command("screen"),
            "icon": "mdi:monitor",
        })),
    );

    add(
        "switch",
        "device_brightness",
        common(json!({
            "name": "Helligkeit vom Gerät",
            "unique_id": format!("slowshow_{node}_device_brightness"),
            "state_topic": topics.state,
            "value_template": "{{ 'ON' if value_json.deviceBrightness else 'OFF' }}",
            "command_topic": topics.command("device_brightness"),
            "icon": "mdi:brightness-auto",
        })),
    );

    // ── Knöpfe ──────────────────────────────────────────────────────────────
    for (object, name, icon) in [
        ("next", "Nächstes Bild", "mdi:skip-next"),
        ("prev", "Vorheriges Bild", "mdi:skip-previous"),
        ("sync", "Synchronisieren", "mdi:cloud-sync"),
    ] {
        add(
            "button",
            object,
            common(json!({
                "name": name,
                "unique_id": format!("slowshow_{node}_{object}"),
                "command_topic": topics.command(object),
                "payload_press": "PRESS",
                "icon": icon,
            })),
        );
    }

    // ── Zahlenwerte ─────────────────────────────────────────────────────────
    add(
        "number",
        "interval",
        common(json!({
            "name": "Anzeigedauer",
            "unique_id": format!("slowshow_{node}_interval"),
            "state_topic": topics.state,
            "value_template": "{{ value_json.intervalSeconds }}",
            "command_topic": topics.command("interval"),
            // Grenzen aus FA-02: 5 Sekunden bis 30 Minuten.
            "min": 5,
            "max": 1800,
            "step": 5,
            "unit_of_measurement": "s",
            "mode": "box",
            "icon": "mdi:timer-outline",
        })),
    );
    add(
        "number",
        "brightness",
        common(json!({
            "name": "Helligkeit",
            "unique_id": format!("slowshow_{node}_brightness"),
            "state_topic": topics.state,
            // Die eingestellte Grundhelligkeit, nicht die wirksame aus
            // `display`: die ist nachts 1 und bei Gerätesteuerung 0 und fiele
            // damit aus dem Bereich des Reglers (E-22).
            "value_template": "{{ value_json.brightness }}",
            "command_topic": topics.command("brightness"),
            "min": 1,
            "max": 100,
            "step": 1,
            "unit_of_measurement": "%",
            "icon": "mdi:brightness-6",
        })),
    );

    // ── Sensoren ────────────────────────────────────────────────────────────
    add(
        "sensor",
        "images",
        common(json!({
            "name": "Bilder im Cache",
            "unique_id": format!("slowshow_{node}_images"),
            "state_topic": topics.state,
            "value_template": "{{ value_json.cache.images }}",
            "state_class": "measurement",
            "icon": "mdi:image-multiple",
        })),
    );
    add(
        "sensor",
        "cache_size",
        common(json!({
            "name": "Cachegröße",
            "unique_id": format!("slowshow_{node}_cache_size"),
            "state_topic": topics.state,
            "value_template": "{{ (value_json.cache.bytes / 1048576) | round(1) }}",
            "unit_of_measurement": "MB",
            "state_class": "measurement",
            "icon": "mdi:database",
        })),
    );

    // ── Binärsensoren ───────────────────────────────────────────────────────
    add(
        "binary_sensor",
        "syncing",
        common(json!({
            "name": "Synchronisiert",
            "unique_id": format!("slowshow_{node}_syncing"),
            "state_topic": topics.state,
            "value_template": "{{ 'ON' if value_json.syncing else 'OFF' }}",
            "device_class": "running",
        })),
    );
    add(
        "binary_sensor",
        "active",
        common(json!({
            "name": "Aktivzeit",
            "unique_id": format!("slowshow_{node}_active"),
            "state_topic": topics.state,
            "value_template": "{{ 'ON' if value_json.display.slideshowActive else 'OFF' }}",
            "icon": "mdi:clock-outline",
        })),
    );

    out
}

fn sanitize_prefix(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('/');
    if trimmed.is_empty() {
        "homeassistant".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Flaches Zusammenführen zweier Objekte.
fn merge(target: &mut Value, extra: Value) {
    if let (Some(t), Some(e)) = (target.as_object_mut(), extra.as_object()) {
        for (k, v) in e {
            t.insert(k.clone(), v.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_entfernt_topic_sonderzeichen() {
        // `/`, `+` und `#` wuerden die Topic-Struktur zerlegen.
        assert_eq!(sanitize("wohnzimmer/rahmen"), "wohnzimmer_rahmen");
        assert_eq!(sanitize("a+b#c"), "a_b_c");
        assert_eq!(sanitize("Flur Rahmen"), "flur_rahmen");
    }

    #[test]
    fn sanitize_faellt_auf_einen_namen_zurueck() {
        assert_eq!(sanitize(""), "slowshow");
        assert_eq!(sanitize("///"), "slowshow");
        assert_eq!(sanitize("___"), "slowshow");
    }

    #[test]
    fn topics_bauen_sich_aus_dem_basistopic() {
        let t = Topics::new("slowshow");
        assert_eq!(t.availability, "slowshow/availability");
        assert_eq!(t.state, "slowshow/state");
        assert_eq!(t.command_filter, "slowshow/cmd/+");
        assert_eq!(t.command("next"), "slowshow/cmd/next");
    }

    #[test]
    fn command_name_liest_den_befehl_aus_dem_topic() {
        let t = Topics::new("flur");
        assert_eq!(t.command_name("flur/cmd/next"), Some("next"));
        assert_eq!(t.command_name("flur/cmd/brightness"), Some("brightness"));
        // Fremde Topics duerfen nicht als Befehl durchgehen.
        assert_eq!(t.command_name("andere/cmd/next"), None);
        assert_eq!(t.command_name("flur/state"), None);
    }

    #[test]
    fn discovery_deckt_alle_entitaeten_ab() {
        let t = Topics::new("slowshow");
        let d = discovery(&t, "homeassistant");
        assert_eq!(
            d.len(),
            12,
            "3 Schalter, 3 Knoepfe, 2 Zahlen, 2 Sensoren, 2 Binaersensoren"
        );
    }

    #[test]
    fn discovery_topics_folgen_der_ha_konvention() {
        let t = Topics::new("slowshow");
        let d = discovery(&t, "homeassistant");
        let topics: Vec<&str> = d.iter().map(|x| x.topic.as_str()).collect();
        assert!(topics.contains(&"homeassistant/switch/slowshow/slideshow/config"));
        assert!(topics.contains(&"homeassistant/button/slowshow/sync/config"));
        assert!(topics.contains(&"homeassistant/number/slowshow/interval/config"));
        assert!(topics.contains(&"homeassistant/binary_sensor/slowshow/syncing/config"));
        assert!(topics.contains(&"homeassistant/switch/slowshow/device_brightness/config"));
    }

    #[test]
    fn helligkeitsregler_liest_die_eingestellte_grundhelligkeit_e_22() {
        // `display.brightness` ist nachts 1 und bei Geraetesteuerung 0 — beides
        // faellt aus dem Bereich min=1..max=100, den der Regler ankuendigt.
        // Home Assistant zeigte dann einen Wert, den er selbst nicht annimmt.
        let t = Topics::new("slowshow");
        let d = discovery(&t, "homeassistant");
        let regler = d
            .iter()
            .find(|x| x.topic.ends_with("/number/slowshow/brightness/config"))
            .expect("Helligkeitsregler fehlt");
        assert_eq!(
            regler.payload["value_template"], "{{ value_json.brightness }}",
            "nicht value_json.display.brightness"
        );
    }

    #[test]
    fn geraetesteuerung_haengt_am_richtigen_kommando_e_22() {
        let t = Topics::new("slowshow");
        let d = discovery(&t, "homeassistant");
        let schalter = d
            .iter()
            .find(|x| x.topic.ends_with("/switch/slowshow/device_brightness/config"))
            .expect("Schalter fehlt");
        assert_eq!(
            schalter.payload["command_topic"], "slowshow/cmd/device_brightness"
        );
        assert_eq!(
            schalter.payload["value_template"],
            "{{ 'ON' if value_json.deviceBrightness else 'OFF' }}"
        );
    }

    #[test]
    fn jede_entitaet_haengt_an_der_verfuegbarkeit() {
        // Ohne das zeigte Home Assistant nach einem Verbindungsabbruch
        // veraltete Werte an, statt die Entitaeten auszugrauen.
        let t = Topics::new("slowshow");
        for d in discovery(&t, "homeassistant") {
            assert_eq!(
                d.payload["availability_topic"], "slowshow/availability",
                "fehlt bei {}",
                d.topic
            );
            assert_eq!(d.payload["payload_not_available"], "offline");
        }
    }

    #[test]
    fn jede_entitaet_hat_eine_eindeutige_id_und_ein_geraet() {
        let t = Topics::new("flur");
        let d = discovery(&t, "homeassistant");
        let mut ids: Vec<String> = d
            .iter()
            .map(|x| x.payload["unique_id"].as_str().unwrap_or("").to_string())
            .collect();
        ids.sort();
        let anzahl = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), anzahl, "unique_id muss eindeutig sein");
        assert!(ids.iter().all(|i| i.starts_with("slowshow_flur_")));

        for x in &d {
            assert_eq!(x.payload["device"]["identifiers"][0], "slowshow_flur");
        }
    }

    #[test]
    fn zwei_geraete_kollidieren_nicht() {
        // Zwei Rahmen im selben Haus muessen unterscheidbar bleiben.
        let a = discovery(&Topics::new("wohnzimmer"), "homeassistant");
        let b = discovery(&Topics::new("flur"), "homeassistant");
        for (x, y) in a.iter().zip(b.iter()) {
            assert_ne!(x.topic, y.topic);
            assert_ne!(x.payload["unique_id"], y.payload["unique_id"]);
        }
    }

    #[test]
    fn zahlenwerte_tragen_die_grenzen_aus_dem_lastenheft() {
        let t = Topics::new("slowshow");
        let d = discovery(&t, "homeassistant");
        let interval = d
            .iter()
            .find(|x| x.topic.contains("/interval/"))
            .expect("Anzeigedauer erwartet");
        // FA-02: 5 Sekunden bis 30 Minuten.
        assert_eq!(interval.payload["min"], 5);
        assert_eq!(interval.payload["max"], 1800);
    }

    #[test]
    fn discovery_prefix_wird_normalisiert() {
        let t = Topics::new("slowshow");
        assert!(discovery(&t, "")[0].topic.starts_with("homeassistant/"));
        assert!(discovery(&t, "/ha/")[0].topic.starts_with("ha/"));
    }
}

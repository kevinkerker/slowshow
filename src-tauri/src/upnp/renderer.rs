//! Bedeutung des Medienrenderers: was der Rahmen ist, wenn Home Assistant ihn
//! als DLNA-Geraet sieht (E-47).
//!
//! UPnP kennt keinen Bilderrahmen. Es kennt einen *Renderer*, der spielt,
//! stoppt, weiterschaltet und eine Lautstaerke hat. Die Zuordnung ist hier
//! an einer Stelle festgelegt, damit HTTP-Antworten und Ereignisse dieselbe
//! Sprache sprechen:
//!
//! | UPnP                | Slowshow                                              |
//! |---------------------|-------------------------------------------------------|
//! | Play                | Schirm an, Diashow laeuft — oder das vorgemerkte      |
//! |                     | Fremdbild (E-52)                                      |
//! | Stop                | Fremdbild weg; sonst Nachtmodus (FA-52, FA-55)        |
//! | Mute                | Nachtmodus                                            |
//! | Next / Previous     | Bildwechsel (FA-41), beendet ein Fremdbild            |
//! | Volume 0–100        | Helligkeit 1–100 % (FA-53), holt die Regelung zurueck |
//! |                     | (E-48)                                                |
//! | SetAVTransportURI   | Fremdbild laden (E-52)                                |
//! | Titel / Albumcover  | Dateiname und das laufende Foto                       |
//!
//! `Pause` gibt es seit E-49 nicht mehr: Home Assistant zeigt bei `playing`
//! genau einen Knopf und nimmt Pause, sobald der Renderer Pause meldet — der
//! Nachtmodus war in der Oberflaeche unerreichbar. Ohne Pause ist Stop der
//! Hauptknopf. Der Zustand `PAUSED_PLAYBACK` wird weiter gemeldet, wenn jemand
//! am Rahmen tippt; Play setzt dann fort.
//!
//! Der Rahmen selbst steckt hinter [`Backend`]: HTTP und Ereignisse kennen nur
//! diese Schnittstelle, und die Tests ein Doppel davon.

use std::sync::Arc;

/// Was der Rahmen gerade tut — die Momentaufnahme, aus der alle Antworten und
/// Ereignisse gebaut werden.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// Laeuft die Diashow? (Pause ueber Tippen oder Heimnetz, FA-41/FA-55)
    pub playing: bool,
    /// Ist der Schirm an? `false` im Nachtmodus oder nach einem Schlafbefehl.
    pub screen_active: bool,
    /// Eingestellte Grundhelligkeit in Prozent, 1..=100 (FA-53).
    pub brightness: u8,
    /// Id des sichtbaren Bildes, falls eines haengt.
    pub current_id: Option<String>,
    /// Anzeigetitel des sichtbaren Bildes — der Dateiname, beim Fremdbild
    /// der Titel aus Home Assistant.
    pub current_title: String,
    /// Wie der Rahmen sich in Home Assistant nennt.
    pub friendly_name: String,
    /// Haengt gerade ein Fremdbild (E-52)?
    pub external: bool,
    /// Die Adresse, von der das Fremdbild kam — das, was Home Assistant als
    /// `AVTransportURI` zurueckerwartet.
    pub external_uri: Option<String>,
}

/// Was der Renderer vom Rahmen braucht.
///
/// Als Schnittstelle und nicht als `AppHandle`, damit HTTP-Router und
/// Ereignisse ohne laufende Tauri-App pruefbar sind — mit einem Doppel, das
/// sich merkt, was gerufen wurde.
pub trait Backend: Send + Sync {
    fn snapshot(&self) -> Snapshot;
    /// Schirm an und Diashow los — oder das vorgemerkte Fremdbild zeigen.
    fn play(&self);
    /// Fremdbild beenden; ohne Fremdbild Nachtmodus.
    fn stop(&self);
    fn next(&self);
    fn previous(&self);
    /// Helligkeit 0..=100; 0 wird zu 1, weil 0 „Geraet regelt selbst" heisst (E-22).
    fn set_volume(&self, level: u8);
    fn set_mute(&self, mute: bool);
    /// Ein Bild von aussen vormerken (E-52): rohe Bytes, wie geladen. Das
    /// Dekodieren und Skalieren ist Sache des Rahmens (NF-12, NF-13).
    fn stage_external(&self, image: Vec<u8>, title: String, uri: String) -> Result<(), String>;
    /// Das sichtbare Bild als JPEG — fuer das Albumcover in Home Assistant.
    fn current_image(&self) -> Option<Vec<u8>>;
}

pub type SharedBackend = Arc<dyn Backend>;

/// UPnP-Transportzustand aus der Momentaufnahme.
///
/// `STOPPED` ist der dunkle Schirm: fuer Home Assistant ist der Rahmen dann
/// „aus", auch wenn die Diashow im Hintergrund weiterliefe. Genau das erwartet
/// jemand, der „Stop" gedrueckt hat. Ein Fremdbild ist immer `PLAYING`, auch
/// wenn die Diashow dahinter pausiert — es haengt ja sichtbar an der Wand.
pub fn transport_state(s: &Snapshot) -> &'static str {
    if !s.screen_active {
        "STOPPED"
    } else if s.playing || s.external {
        "PLAYING"
    } else {
        "PAUSED_PLAYBACK"
    }
}

/// Was gerade sinnvoll ist — Home Assistant blendet den Rest aus.
pub fn transport_actions(s: &Snapshot) -> &'static str {
    match transport_state(s) {
        "PLAYING" => "Stop,Next,Previous",
        "PAUSED_PLAYBACK" => "Play,Stop,Next,Previous",
        _ => "Play,Next,Previous",
    }
}

/// Stumm heisst dunkel: Mute ist der zweite Weg in den Nachtmodus, weil
/// Home Assistant fuer Renderer ohne Netzschalter genau diesen Knopf zeigt.
pub fn is_muted(s: &Snapshot) -> bool {
    !s.screen_active
}

/// Adresse des laufenden Fotos unter der Basis-URL des Servers.
///
/// Mit der Bild-Id als Anfrageparameter, damit Home Assistant bei jedem
/// Bildwechsel eine *andere* URL sieht und das Cover neu laedt — dieselbe URL
/// wuerde es aus seinem Zwischenspeicher zeigen.
pub fn track_uri(base: &str, s: &Snapshot) -> String {
    match &s.current_id {
        Some(id) => format!("{base}/upnp/current.jpg?id={id}"),
        None => String::new(),
    }
}

/// Was als `AVTransportURI` gemeldet wird: beim Fremdbild die Adresse, die
/// Home Assistant geschickt hat — daran erkennt es sein eigenes Medium wieder —,
/// sonst das Cover des laufenden Fotos.
pub fn media_uri(base: &str, s: &Snapshot) -> String {
    match &s.external_uri {
        Some(uri) if s.external => uri.clone(),
        _ => track_uri(base, s),
    }
}

/// DIDL-Lite-Metadaten des laufenden Fotos (Titel und Cover).
///
/// `async_upnp_client` liest den Titel aus `dc:title` und das Cover aus
/// `upnp:albumArtURI` bzw. einer `res` mit `image/`-ProtocolInfo — beides
/// steht drin, damit es gleich ist, welchen Weg die Version nimmt.
pub fn didl(base: &str, s: &Snapshot) -> String {
    let Some(id) = &s.current_id else {
        return String::new();
    };
    let uri = track_uri(base, s);
    let title = crate::upnp::soap::escape(&s.current_title);
    format!(
        concat!(
            "<DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" ",
            "xmlns:dc=\"http://purl.org/dc/elements/1.1/\" ",
            "xmlns:upnp=\"urn:schemas-upnp-org:metadata-1-0/upnp/\">",
            "<item id=\"{id}\" parentID=\"0\" restricted=\"1\">",
            "<dc:title>{title}</dc:title>",
            "<upnp:class>object.item.imageItem.photo</upnp:class>",
            "<upnp:albumArtURI>{uri}</upnp:albumArtURI>",
            "<res protocolInfo=\"http-get:*:image/jpeg:*\">{uri}</res>",
            "</item></DIDL-Lite>"
        ),
        id = crate::upnp::soap::escape(id),
        title = title,
        uri = crate::upnp::soap::escape(&uri),
    )
}

/// Der Titel aus einem DIDL-Lite-Dokument, wie Home Assistant es mit
/// `SetAVTransportURI` schickt (E-52).
///
/// Nur das erste `title`-Element, ganz gleich mit welchem Praefix: der
/// Namensraum ist mal `dc:`, mal fehlt die Deklaration ganz. Ohne Titel `None`,
/// der Aufrufer nimmt dann den Dateinamen aus der Adresse.
pub fn didl_title(didl: &str) -> Option<String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(didl);
    let mut in_title = false;
    let mut title = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = e.name();
                let local = name
                    .as_ref()
                    .rsplit(|b| *b == b':')
                    .next()
                    .unwrap_or(name.as_ref());
                in_title = local == b"title";
            }
            Ok(Event::Text(t)) if in_title => {
                if let Ok(s) = t.unescape() {
                    title.push_str(&s);
                }
            }
            Ok(Event::End(_)) => {
                if in_title && !title.trim().is_empty() {
                    break;
                }
                in_title = false;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let trimmed = title.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// `LastChange` des AVTransport — das, was ein Abonnent bei jeder Aenderung
/// bekommt. Die Werte stehen als Attribute, so will es der Standard, und
/// `async_upnp_client` liest genau diese Form.
pub fn last_change_avt(base: &str, s: &Snapshot) -> String {
    let esc = crate::upnp::soap::escape;
    let meta = esc(&didl(base, s));
    let uri = esc(&track_uri(base, s));
    let media = esc(&media_uri(base, s));
    format!(
        concat!(
            "<Event xmlns=\"urn:schemas-upnp-org:metadata-1-0/AVT/\">",
            "<InstanceID val=\"0\">",
            "<TransportState val=\"{state}\"/>",
            "<TransportStatus val=\"OK\"/>",
            "<CurrentPlayMode val=\"NORMAL\"/>",
            "<CurrentTransportActions val=\"{actions}\"/>",
            "<NumberOfTracks val=\"{tracks}\"/>",
            "<CurrentTrack val=\"{tracks}\"/>",
            "<CurrentTrackURI val=\"{uri}\"/>",
            "<CurrentTrackMetaData val=\"{meta}\"/>",
            "<AVTransportURI val=\"{media}\"/>",
            "<AVTransportURIMetaData val=\"{meta}\"/>",
            "</InstanceID></Event>"
        ),
        state = transport_state(s),
        actions = transport_actions(s),
        tracks = if s.current_id.is_some() { 1 } else { 0 },
        uri = uri,
        meta = meta,
        media = media,
    )
}

/// `LastChange` der RenderingControl: Lautstaerke und Stummschaltung, Kanal
/// `Master` — den einzigen, den Home Assistant auswertet.
pub fn last_change_rcs(s: &Snapshot) -> String {
    format!(
        concat!(
            "<Event xmlns=\"urn:schemas-upnp-org:metadata-1-0/RCS/\">",
            "<InstanceID val=\"0\">",
            "<Volume channel=\"Master\" val=\"{volume}\"/>",
            "<Mute channel=\"Master\" val=\"{mute}\"/>",
            "<PresetNameList val=\"FactoryDefaults\"/>",
            "</InstanceID></Event>"
        ),
        volume = s.brightness,
        mute = if is_muted(s) { 1 } else { 0 },
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Doppel des Rahmens: merkt sich jeden Aufruf.
    #[derive(Default)]
    pub struct FakeBackend {
        pub snapshot: Mutex<Snapshot>,
        pub calls: Mutex<Vec<String>>,
    }

    impl Default for Snapshot {
        fn default() -> Self {
            Self {
                playing: true,
                screen_active: true,
                brightness: 80,
                current_id: Some("abc123".into()),
                current_title: "IMG_0001.jpg".into(),
                friendly_name: "Slowshow".into(),
                external: false,
                external_uri: None,
            }
        }
    }

    impl FakeBackend {
        pub fn shared() -> Arc<FakeBackend> {
            Arc::new(FakeBackend::default())
        }
        pub fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
        fn note(&self, what: &str) {
            self.calls.lock().unwrap().push(what.to_string());
        }
    }

    impl Backend for FakeBackend {
        fn snapshot(&self) -> Snapshot {
            self.snapshot.lock().unwrap().clone()
        }
        fn play(&self) {
            self.note("play");
            let mut s = self.snapshot.lock().unwrap();
            s.playing = true;
            s.screen_active = true;
        }
        fn stop(&self) {
            self.note("stop");
            self.snapshot.lock().unwrap().screen_active = false;
        }
        fn next(&self) {
            self.note("next");
        }
        fn previous(&self) {
            self.note("previous");
        }
        fn set_volume(&self, level: u8) {
            self.note(&format!("volume:{level}"));
            self.snapshot.lock().unwrap().brightness = level.max(1);
        }
        fn set_mute(&self, mute: bool) {
            self.note(&format!("mute:{mute}"));
            self.snapshot.lock().unwrap().screen_active = !mute;
        }
        fn stage_external(&self, image: Vec<u8>, title: String, uri: String) -> Result<(), String> {
            self.note(&format!("stage:{title}:{}", image.len()));
            self.snapshot.lock().unwrap().external_uri = Some(uri);
            Ok(())
        }
        fn current_image(&self) -> Option<Vec<u8>> {
            Some(vec![0xFF, 0xD8, 0xFF, 0xD9])
        }
    }

    #[test]
    fn transportzustand_folgt_schirm_und_diashow() {
        let mut s = Snapshot::default();
        assert_eq!(transport_state(&s), "PLAYING");
        s.playing = false;
        assert_eq!(transport_state(&s), "PAUSED_PLAYBACK");
        // Dunkler Schirm schlaegt alles: fuer Home Assistant ist der Rahmen aus.
        s.screen_active = false;
        assert_eq!(transport_state(&s), "STOPPED");
        assert!(is_muted(&s));
    }

    #[test]
    fn fremdbild_ist_immer_playing_e_52() {
        // Auch wenn die Diashow dahinter pausiert: das Bild haengt sichtbar,
        // und Home Assistant soll den Stop-Knopf zeigen, der es beendet.
        let s = Snapshot {
            playing: false,
            external: true,
            ..Snapshot::default()
        };
        assert_eq!(transport_state(&s), "PLAYING");
    }

    #[test]
    fn aktionen_ohne_pause_e_49() {
        // Home Assistant zeigt bei `playing` einen Knopf: Pause, wenn es
        // Pause gibt, sonst Stop. Nur ohne Pause ist der Nachtmodus in der
        // Oberflaeche erreichbar.
        let mut s = Snapshot::default();
        assert_eq!(transport_actions(&s), "Stop,Next,Previous");
        s.playing = false;
        assert_eq!(transport_actions(&s), "Play,Stop,Next,Previous");
        s.screen_active = false;
        assert_eq!(transport_actions(&s), "Play,Next,Previous");
        for zustand in [true, false] {
            s.playing = zustand;
            assert!(!transport_actions(&s).contains("Pause"));
        }
    }

    #[test]
    fn didl_traegt_titel_und_cover_mit_bild_id() {
        let s = Snapshot::default();
        let xml = didl("http://192.168.1.5:8128", &s);
        assert!(xml.contains("<dc:title>IMG_0001.jpg</dc:title>"));
        assert!(xml.contains("object.item.imageItem.photo"));
        // Die Id in der URL: sonst zeigt Home Assistant das alte Cover aus
        // seinem Zwischenspeicher, wenn das Bild wechselt.
        assert!(xml.contains("upnp:albumArtURI>http://192.168.1.5:8128/upnp/current.jpg?id=abc123<"));
        assert!(xml.contains("protocolInfo=\"http-get:*:image/jpeg:*\""));
    }

    #[test]
    fn ohne_bild_gibt_es_keine_metadaten() {
        let s = Snapshot {
            current_id: None,
            ..Snapshot::default()
        };
        assert_eq!(didl("http://h", &s), "");
        assert_eq!(track_uri("http://h", &s), "");
        assert!(last_change_avt("http://h", &s).contains("<NumberOfTracks val=\"0\"/>"));
    }

    #[test]
    fn titel_mit_sonderzeichen_bleibt_gueltiges_xml() {
        let s = Snapshot {
            current_title: "Tom & Jerry <2024>.jpg".into(),
            ..Snapshot::default()
        };
        let xml = didl("http://h", &s);
        assert!(xml.contains("Tom &amp; Jerry &lt;2024&gt;.jpg"));
        assert!(!xml.contains("Tom & Jerry"));
    }

    #[test]
    fn last_change_ist_doppelt_maskiert() {
        // Das DIDL steckt als Attributwert im Ereignis: Anfuehrungszeichen und
        // Winkel muessen dort maskiert sein, sonst zerbricht der Parser des
        // Abonnenten am ersten Bildwechsel.
        let s = Snapshot::default();
        let ev = last_change_avt("http://h", &s);
        assert!(ev.contains("<TransportState val=\"PLAYING\"/>"));
        assert!(ev.contains("CurrentTrackMetaData val=\"&lt;DIDL-Lite"));
        assert!(!ev.contains("val=\"<DIDL-Lite"));
    }

    #[test]
    fn fremdbild_meldet_seine_quelle_als_avtransporturi_e_52() {
        // Home Assistant vergleicht die gemeldete AVTransportURI mit dem, was
        // es geschickt hat; das Cover kommt trotzdem vom Rahmen.
        let s = Snapshot {
            current_id: Some("x_1".into()),
            current_title: "Haustuer".into(),
            external: true,
            external_uri: Some("http://ha:8123/api/camera_proxy/camera.tuer?token=abc&x=1".into()),
            ..Snapshot::default()
        };
        assert_eq!(
            media_uri("http://h", &s),
            "http://ha:8123/api/camera_proxy/camera.tuer?token=abc&x=1"
        );
        assert_eq!(track_uri("http://h", &s), "http://h/upnp/current.jpg?id=x_1");
        let ev = last_change_avt("http://h", &s);
        assert!(ev.contains(
            "<AVTransportURI val=\"http://ha:8123/api/camera_proxy/camera.tuer?token=abc&amp;x=1\"/>"
        ));
        assert!(ev.contains("<CurrentTrackURI val=\"http://h/upnp/current.jpg?id=x_1\"/>"));

        // Ohne Fremdbild bleibt eine alte Adresse ohne Wirkung.
        let s = Snapshot {
            external: false,
            ..s
        };
        assert_eq!(media_uri("http://h", &s), "http://h/upnp/current.jpg?id=x_1");
    }

    #[test]
    fn didl_title_liest_den_titel_mit_und_ohne_praefix_e_52() {
        let mit = "<DIDL-Lite xmlns=\"urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><item id=\"0\"><dc:title>Haust&#252;r &amp; Hof</dc:title><upnp:class>object.item.imageItem</upnp:class></item></DIDL-Lite>";
        assert_eq!(didl_title(mit).as_deref(), Some("Haustür & Hof"));
        // Home Assistant deklariert `dc:` nicht immer — quick-xml ohne
        // Namensraeume stoert das nicht.
        let ohne = "<DIDL-Lite><item><dc:title>Kamera</dc:title></item></DIDL-Lite>";
        assert_eq!(didl_title(ohne).as_deref(), Some("Kamera"));
        assert_eq!(didl_title("<DIDL-Lite><item><title>  </title></item></DIDL-Lite>"), None);
        assert_eq!(didl_title(""), None);
        assert_eq!(didl_title("kein xml <"), None);
    }

    #[test]
    fn rcs_meldet_helligkeit_als_lautstaerke() {
        let s = Snapshot {
            brightness: 42,
            screen_active: false,
            ..Snapshot::default()
        };
        let ev = last_change_rcs(&s);
        assert!(ev.contains("<Volume channel=\"Master\" val=\"42\"/>"));
        assert!(ev.contains("<Mute channel=\"Master\" val=\"1\"/>"));
    }
}

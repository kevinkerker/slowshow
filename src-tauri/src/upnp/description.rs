//! Geraete- und Dienstbeschreibungen des Medienrenderers (E-47).
//!
//! Drei Dienste muss ein DLNA-Renderer anbieten, sonst legt `async_upnp_client`
//! — und damit Home Assistant — ihn nicht an: AVTransport, RenderingControl,
//! ConnectionManager. Welche *Aktionen* darin stehen, entscheidet, welche
//! Knoepfe Home Assistant zeigt: `Play` vorhanden heisst Play-Knopf, `SetVolume`
//! plus Zustandsvariable `Volume` heisst Lautstaerkeregler. Was hier fehlt,
//! fehlt dort — bewusst fehlt `Seek` (ein Foto hat keine Position),
//! `SetPlayMode` (die Reihenfolge stellt man am Rahmen ein) und seit E-49
//! `Pause`: Home Assistant zeigt bei `playing` genau einen Knopf und nimmt
//! Pause, sobald es Pause gibt — erst ohne Pause ist Stop, und damit der
//! Nachtmodus, in der Oberflaeche erreichbar.

use super::soap::escape;

pub const DEVICE_TYPE: &str = "urn:schemas-upnp-org:device:MediaRenderer:1";

/// Die drei Dienste des Renderers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Service {
    /// Abspielen, Pause, Weiter — die Diashow selbst.
    Avt,
    /// Lautstaerke und Stumm — Helligkeit und Nachtmodus.
    Rcs,
    /// Was der Renderer annehmen kann. Pflicht, inhaltlich fast leer.
    Cm,
}

impl Service {
    pub const ALL: [Service; 3] = [Service::Avt, Service::Rcs, Service::Cm];

    pub fn service_type(self) -> &'static str {
        match self {
            Service::Avt => "urn:schemas-upnp-org:service:AVTransport:1",
            Service::Rcs => "urn:schemas-upnp-org:service:RenderingControl:1",
            Service::Cm => "urn:schemas-upnp-org:service:ConnectionManager:1",
        }
    }

    pub fn service_id(self) -> &'static str {
        match self {
            Service::Avt => "urn:upnp-org:serviceId:AVTransport",
            Service::Rcs => "urn:upnp-org:serviceId:RenderingControl",
            Service::Cm => "urn:upnp-org:serviceId:ConnectionManager",
        }
    }

    /// Kurzname in den Pfaden: `/upnp/control/avt`.
    pub fn slug(self) -> &'static str {
        match self {
            Service::Avt => "avt",
            Service::Rcs => "rcs",
            Service::Cm => "cm",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Service> {
        Service::ALL.into_iter().find(|s| s.slug() == slug)
    }

    pub fn scpd_path(self) -> String {
        format!("/upnp/scpd/{}", self.slug())
    }

    pub fn control_path(self) -> String {
        format!("/upnp/control/{}", self.slug())
    }

    pub fn event_path(self) -> String {
        format!("/upnp/event/{}", self.slug())
    }

    pub fn scpd(self) -> String {
        match self {
            Service::Avt => avt_scpd(),
            Service::Rcs => rcs_scpd(),
            Service::Cm => cm_scpd(),
        }
    }
}

/// Die Geraetebeschreibung — das, was hinter `LOCATION` steht.
///
/// `X_DLNADOC` ist nicht Pflicht, hilft aber manchen Steuerpunkten, den
/// Renderer als DLNA-Geraet einzuordnen.
pub fn device_xml(udn: &str, friendly_name: &str) -> String {
    let services: String = Service::ALL
        .iter()
        .map(|s| {
            format!(
                concat!(
                    "<service>",
                    "<serviceType>{st}</serviceType>",
                    "<serviceId>{sid}</serviceId>",
                    "<SCPDURL>{scpd}</SCPDURL>",
                    "<controlURL>{ctrl}</controlURL>",
                    "<eventSubURL>{evt}</eventSubURL>",
                    "</service>"
                ),
                st = s.service_type(),
                sid = s.service_id(),
                scpd = s.scpd_path(),
                ctrl = s.control_path(),
                evt = s.event_path(),
            )
        })
        .collect();

    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>",
            "<root xmlns=\"urn:schemas-upnp-org:device-1-0\" xmlns:dlna=\"urn:schemas-dlna-org:device-1-0\">",
            "<specVersion><major>1</major><minor>0</minor></specVersion>",
            "<device>",
            "<deviceType>{device_type}</deviceType>",
            "<friendlyName>{name}</friendlyName>",
            "<manufacturer>Kevin Kerker</manufacturer>",
            "<manufacturerURL>https://github.com/kevinkerker/slowshow</manufacturerURL>",
            "<modelDescription>Digitaler Bilderrahmen</modelDescription>",
            "<modelName>Slowshow</modelName>",
            "<modelNumber>{version}</modelNumber>",
            "<UDN>{udn}</UDN>",
            "<dlna:X_DLNADOC>DMR-1.50</dlna:X_DLNADOC>",
            "<serviceList>{services}</serviceList>",
            "</device></root>"
        ),
        device_type = DEVICE_TYPE,
        name = escape(friendly_name),
        version = env!("CARGO_PKG_VERSION"),
        udn = escape(udn),
        services = services,
    )
}

// ── SCPD-Bausteine ───────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum Dir {
    In,
    Out,
}

/// Eine Zustandsvariable der Dienstbeschreibung.
struct Var {
    name: &'static str,
    kind: &'static str,
    evented: bool,
    allowed: &'static [&'static str],
    /// (min, max, step) fuer Zahlen.
    range: Option<(i32, i32, i32)>,
}

const fn var(name: &'static str, kind: &'static str) -> Var {
    Var {
        name,
        kind,
        evented: false,
        allowed: &[],
        range: None,
    }
}

const fn evented(name: &'static str, kind: &'static str) -> Var {
    Var {
        name,
        kind,
        evented: true,
        allowed: &[],
        range: None,
    }
}

const fn choice(name: &'static str, allowed: &'static [&'static str]) -> Var {
    Var {
        name,
        kind: "string",
        evented: false,
        allowed,
        range: None,
    }
}

type Args = &'static [(&'static str, Dir, &'static str)];

fn scpd(actions: &[(&str, Args)], vars: &[Var]) -> String {
    let mut out = String::from(concat!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>",
        "<scpd xmlns=\"urn:schemas-upnp-org:service-1-0\">",
        "<specVersion><major>1</major><minor>0</minor></specVersion>",
        "<actionList>"
    ));
    for (name, args) in actions {
        out.push_str(&format!("<action><name>{name}</name>"));
        if !args.is_empty() {
            out.push_str("<argumentList>");
            for (arg, dir, related) in args.iter() {
                out.push_str(&format!(
                    "<argument><name>{arg}</name><direction>{}</direction><relatedStateVariable>{related}</relatedStateVariable></argument>",
                    match dir {
                        Dir::In => "in",
                        Dir::Out => "out",
                    }
                ));
            }
            out.push_str("</argumentList>");
        }
        out.push_str("</action>");
    }
    out.push_str("</actionList><serviceStateTable>");
    for v in vars {
        out.push_str(&format!(
            "<stateVariable sendEvents=\"{}\"><name>{}</name><dataType>{}</dataType>",
            if v.evented { "yes" } else { "no" },
            v.name,
            v.kind
        ));
        if !v.allowed.is_empty() {
            out.push_str("<allowedValueList>");
            for a in v.allowed {
                out.push_str(&format!("<allowedValue>{a}</allowedValue>"));
            }
            out.push_str("</allowedValueList>");
        }
        if let Some((min, max, step)) = v.range {
            out.push_str(&format!(
                "<allowedValueRange><minimum>{min}</minimum><maximum>{max}</maximum><step>{step}</step></allowedValueRange>"
            ));
        }
        out.push_str("</stateVariable>");
    }
    out.push_str("</serviceStateTable></scpd>");
    out
}

use Dir::{In, Out};

/// Aktionen, die Home Assistant beim ersten Abruf pollt und beim Bedienen
/// ruft. Alle hier aufgefuehrten muessen in `http.rs` beantwortet werden.
pub const AVT_ACTIONS: &[&str] = &[
    "SetAVTransportURI",
    "GetMediaInfo",
    "GetTransportInfo",
    "GetPositionInfo",
    "GetDeviceCapabilities",
    "GetTransportSettings",
    "Stop",
    "Play",
    "Next",
    "Previous",
    "GetCurrentTransportActions",
];

fn avt_scpd() -> String {
    const ID: (&str, Dir, &str) = ("InstanceID", In, "A_ARG_TYPE_InstanceID");
    let actions: &[(&str, Args)] = &[
        (
            "SetAVTransportURI",
            &[
                ID,
                ("CurrentURI", In, "AVTransportURI"),
                ("CurrentURIMetaData", In, "AVTransportURIMetaData"),
            ],
        ),
        (
            "GetMediaInfo",
            &[
                ID,
                ("NrTracks", Out, "NumberOfTracks"),
                ("MediaDuration", Out, "CurrentMediaDuration"),
                ("CurrentURI", Out, "AVTransportURI"),
                ("CurrentURIMetaData", Out, "AVTransportURIMetaData"),
                ("NextURI", Out, "NextAVTransportURI"),
                ("NextURIMetaData", Out, "NextAVTransportURIMetaData"),
                ("PlayMedium", Out, "PlaybackStorageMedium"),
                ("RecordMedium", Out, "RecordStorageMedium"),
                ("WriteStatus", Out, "RecordMediumWriteStatus"),
            ],
        ),
        (
            "GetTransportInfo",
            &[
                ID,
                ("CurrentTransportState", Out, "TransportState"),
                ("CurrentTransportStatus", Out, "TransportStatus"),
                ("CurrentSpeed", Out, "TransportPlaySpeed"),
            ],
        ),
        (
            "GetPositionInfo",
            &[
                ID,
                ("Track", Out, "CurrentTrack"),
                ("TrackDuration", Out, "CurrentTrackDuration"),
                ("TrackMetaData", Out, "CurrentTrackMetaData"),
                ("TrackURI", Out, "CurrentTrackURI"),
                ("RelTime", Out, "RelativeTimePosition"),
                ("AbsTime", Out, "AbsoluteTimePosition"),
                ("RelCount", Out, "RelativeCounterPosition"),
                ("AbsCount", Out, "AbsoluteCounterPosition"),
            ],
        ),
        (
            "GetDeviceCapabilities",
            &[
                ID,
                ("PlayMedia", Out, "PossiblePlaybackStorageMedia"),
                ("RecMedia", Out, "PossibleRecordStorageMedia"),
                ("RecQualityModes", Out, "PossibleRecordQualityModes"),
            ],
        ),
        (
            "GetTransportSettings",
            &[
                ID,
                ("PlayMode", Out, "CurrentPlayMode"),
                ("RecQualityMode", Out, "CurrentRecordQualityMode"),
            ],
        ),
        ("Stop", &[ID]),
        ("Play", &[ID, ("Speed", In, "TransportPlaySpeed")]),
        ("Next", &[ID]),
        ("Previous", &[ID]),
        (
            "GetCurrentTransportActions",
            &[ID, ("Actions", Out, "CurrentTransportActions")],
        ),
    ];
    let vars = [
        choice(
            "TransportState",
            &[
                "STOPPED",
                "PLAYING",
                "PAUSED_PLAYBACK",
                "TRANSITIONING",
                "NO_MEDIA_PRESENT",
            ],
        ),
        choice("TransportStatus", &["OK", "ERROR_OCCURRED"]),
        choice("PlaybackStorageMedium", &["NETWORK", "NONE", "UNKNOWN"]),
        choice("RecordStorageMedium", &["NOT_IMPLEMENTED"]),
        var("PossiblePlaybackStorageMedia", "string"),
        var("PossibleRecordStorageMedia", "string"),
        choice("CurrentPlayMode", &["NORMAL", "SHUFFLE"]),
        choice("TransportPlaySpeed", &["1"]),
        choice("RecordMediumWriteStatus", &["NOT_IMPLEMENTED"]),
        choice("CurrentRecordQualityMode", &["NOT_IMPLEMENTED"]),
        var("PossibleRecordQualityModes", "string"),
        var("NumberOfTracks", "ui4"),
        var("CurrentTrack", "ui4"),
        var("CurrentTrackDuration", "string"),
        var("CurrentMediaDuration", "string"),
        var("CurrentTrackMetaData", "string"),
        var("CurrentTrackURI", "string"),
        var("AVTransportURI", "string"),
        var("AVTransportURIMetaData", "string"),
        var("NextAVTransportURI", "string"),
        var("NextAVTransportURIMetaData", "string"),
        var("RelativeTimePosition", "string"),
        var("AbsoluteTimePosition", "string"),
        var("RelativeCounterPosition", "i4"),
        var("AbsoluteCounterPosition", "i4"),
        var("CurrentTransportActions", "string"),
        evented("LastChange", "string"),
        var("A_ARG_TYPE_InstanceID", "ui4"),
    ];
    scpd(actions, &vars)
}

pub const RCS_ACTIONS: &[&str] = &[
    "ListPresets",
    "SelectPreset",
    "GetMute",
    "SetMute",
    "GetVolume",
    "SetVolume",
];

fn rcs_scpd() -> String {
    const ID: (&str, Dir, &str) = ("InstanceID", In, "A_ARG_TYPE_InstanceID");
    const CHANNEL: (&str, Dir, &str) = ("Channel", In, "A_ARG_TYPE_Channel");
    let actions: &[(&str, Args)] = &[
        (
            "ListPresets",
            &[ID, ("CurrentPresetNameList", Out, "PresetNameList")],
        ),
        (
            "SelectPreset",
            &[ID, ("PresetName", In, "A_ARG_TYPE_PresetName")],
        ),
        ("GetMute", &[ID, CHANNEL, ("CurrentMute", Out, "Mute")]),
        ("SetMute", &[ID, CHANNEL, ("DesiredMute", In, "Mute")]),
        ("GetVolume", &[ID, CHANNEL, ("CurrentVolume", Out, "Volume")]),
        ("SetVolume", &[ID, CHANNEL, ("DesiredVolume", In, "Volume")]),
    ];
    let vars = [
        var("PresetNameList", "string"),
        evented("LastChange", "string"),
        var("Mute", "boolean"),
        Var {
            name: "Volume",
            kind: "ui2",
            evented: false,
            allowed: &[],
            range: Some((0, 100, 1)),
        },
        var("A_ARG_TYPE_InstanceID", "ui4"),
        choice("A_ARG_TYPE_Channel", &["Master"]),
        choice("A_ARG_TYPE_PresetName", &["FactoryDefaults"]),
    ];
    scpd(actions, &vars)
}

pub const CM_ACTIONS: &[&str] = &[
    "GetProtocolInfo",
    "GetCurrentConnectionIDs",
    "GetCurrentConnectionInfo",
];

fn cm_scpd() -> String {
    let actions: &[(&str, Args)] = &[
        (
            "GetProtocolInfo",
            &[
                ("Source", Out, "SourceProtocolInfo"),
                ("Sink", Out, "SinkProtocolInfo"),
            ],
        ),
        (
            "GetCurrentConnectionIDs",
            &[("ConnectionIDs", Out, "CurrentConnectionIDs")],
        ),
        (
            "GetCurrentConnectionInfo",
            &[
                ("ConnectionID", In, "A_ARG_TYPE_ConnectionID"),
                ("RcsID", Out, "A_ARG_TYPE_RcsID"),
                ("AVTransportID", Out, "A_ARG_TYPE_AVTransportID"),
                ("ProtocolInfo", Out, "A_ARG_TYPE_ProtocolInfo"),
                ("PeerConnectionManager", Out, "A_ARG_TYPE_ConnectionManager"),
                ("PeerConnectionID", Out, "A_ARG_TYPE_ConnectionID"),
                ("Direction", Out, "A_ARG_TYPE_Direction"),
                ("Status", Out, "A_ARG_TYPE_ConnectionStatus"),
            ],
        ),
    ];
    let vars = [
        evented("SourceProtocolInfo", "string"),
        evented("SinkProtocolInfo", "string"),
        evented("CurrentConnectionIDs", "string"),
        choice(
            "A_ARG_TYPE_ConnectionStatus",
            &["OK", "ContentFormatMismatch", "InsufficientBandwidth", "UnreliableChannel", "Unknown"],
        ),
        var("A_ARG_TYPE_ConnectionManager", "string"),
        choice("A_ARG_TYPE_Direction", &["Input", "Output"]),
        var("A_ARG_TYPE_ProtocolInfo", "string"),
        var("A_ARG_TYPE_ConnectionID", "i4"),
        var("A_ARG_TYPE_AVTransportID", "i4"),
        var("A_ARG_TYPE_RcsID", "i4"),
    ];
    scpd(actions, &vars)
}

/// Was der Renderer als Senke annimmt — Fotos.
pub const SINK_PROTOCOL_INFO: &str = "http-get:*:image/jpeg:*,http-get:*:image/png:*";

#[cfg(test)]
mod tests {
    use super::*;
    use quick_xml::events::Event;
    use quick_xml::Reader;

    /// Zaehlt Elemente mit diesem lokalen Namen — und prueft dabei, dass das
    /// XML ueberhaupt bis zum Ende lesbar ist.
    fn count(xml: &str, local_name: &str) -> usize {
        let mut r = Reader::from_str(xml);
        let mut n = 0;
        loop {
            match r.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name.rsplit(':').next() == Some(local_name) {
                        n += 1;
                    }
                }
                Ok(Event::Eof) => return n,
                Err(e) => panic!("kein gueltiges XML: {e}"),
                _ => {}
            }
        }
    }

    fn text_after(xml: &str, tag: &str) -> Vec<String> {
        let mut r = Reader::from_str(xml);
        let mut out = Vec::new();
        let mut inside = false;
        loop {
            match r.read_event() {
                Ok(Event::Start(e)) => inside = e.name().as_ref() == tag.as_bytes(),
                Ok(Event::Text(t)) if inside => out.push(t.unescape().unwrap().to_string()),
                Ok(Event::End(_)) => inside = false,
                Ok(Event::Eof) => return out,
                Err(e) => panic!("kein gueltiges XML: {e}"),
                _ => {}
            }
        }
    }

    #[test]
    fn geraetebeschreibung_nennt_typ_udn_und_drei_dienste() {
        let xml = device_xml("uuid:1234", "Wohnzimmer & Küche");
        assert_eq!(count(&xml, "service"), 3);
        assert_eq!(text_after(&xml, "deviceType"), vec![DEVICE_TYPE]);
        assert_eq!(text_after(&xml, "UDN"), vec!["uuid:1234"]);
        // Sonderzeichen im Namen bleiben gueltiges XML.
        assert_eq!(text_after(&xml, "friendlyName"), vec!["Wohnzimmer & Küche"]);
        for s in Service::ALL {
            assert!(xml.contains(s.service_type()), "{}", s.service_type());
            assert!(xml.contains(&s.control_path()));
            assert!(xml.contains(&s.event_path()));
        }
    }

    #[test]
    fn jede_scpd_ist_gueltig_und_nennt_ihre_aktionen() {
        for (svc, actions) in [
            (Service::Avt, AVT_ACTIONS),
            (Service::Rcs, RCS_ACTIONS),
            (Service::Cm, CM_ACTIONS),
        ] {
            let xml = svc.scpd();
            let names = text_after(&xml, "name");
            for a in actions {
                assert!(names.iter().any(|n| n == a), "{:?} fehlt {a}", svc);
            }
            assert_eq!(count(&xml, "action"), actions.len(), "{svc:?}");
        }
    }

    #[test]
    fn jedes_argument_verweist_auf_eine_vorhandene_zustandsvariable() {
        // `async_upnp_client` schlaegt Argumente ueber relatedStateVariable
        // nach; ein Tippfehler dort ist ein Dienst ohne Aktionen.
        for svc in Service::ALL {
            let xml = svc.scpd();
            let vars: Vec<String> = text_after(&xml, "name")
                .into_iter()
                .collect();
            for related in text_after(&xml, "relatedStateVariable") {
                assert!(
                    vars.contains(&related),
                    "{svc:?}: {related} ist keine Zustandsvariable"
                );
            }
        }
    }

    #[test]
    fn home_assistant_findet_die_faehigkeiten() {
        // has_play/has_pause/... schauen auf genau diese Aktionen, der
        // Lautstaerkeregler auf SetVolume plus Variable Volume.
        let avt = Service::Avt.scpd();
        for a in ["Play", "Stop", "Next", "Previous", "SetAVTransportURI"] {
            assert!(text_after(&avt, "name").contains(&a.to_string()), "{a}");
        }
        // Kein Pause (E-49): sonst zeigt Home Assistant Pause statt Stop, und
        // der Nachtmodus ist in der Oberflaeche nicht erreichbar.
        assert!(!text_after(&avt, "name").contains(&"Pause".to_string()));
        let rcs = Service::Rcs.scpd();
        let names = text_after(&rcs, "name");
        assert!(names.contains(&"SetVolume".to_string()));
        assert!(names.contains(&"Volume".to_string()));
        assert!(names.contains(&"SetMute".to_string()));
        assert!(names.contains(&"Mute".to_string()));
        assert!(rcs.contains("<maximum>100</maximum>"));
        // Bewusst nicht dabei: Seek und SetPlayMode.
        assert!(!text_after(&avt, "name").contains(&"Seek".to_string()));
        assert!(!text_after(&avt, "name").contains(&"SetPlayMode".to_string()));
    }

    #[test]
    fn last_change_ist_in_avt_und_rcs_evented() {
        for svc in [Service::Avt, Service::Rcs] {
            assert!(
                svc.scpd()
                    .contains("<stateVariable sendEvents=\"yes\"><name>LastChange</name>"),
                "{svc:?}"
            );
        }
    }

    #[test]
    fn pfade_und_slugs_gehoeren_zusammen() {
        for svc in Service::ALL {
            assert_eq!(Service::from_slug(svc.slug()), Some(svc));
            assert!(svc.control_path().ends_with(svc.slug()));
        }
        assert_eq!(Service::from_slug("nix"), None);
    }
}

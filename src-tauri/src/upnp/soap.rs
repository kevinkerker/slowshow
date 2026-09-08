//! SOAP fuer UPnP-Steueraufrufe (E-47): Anfrage lesen, Antwort und Fehler bauen.
//!
//! UPnP benutzt SOAP 1.1 in einer sehr engen Form: ein `Envelope`, ein `Body`,
//! darin genau ein Element mit dem Aktionsnamen, darin die Argumente als
//! Kindelemente mit Text. Mehr wird hier nicht verstanden — und mehr schickt
//! auch niemand.

use quick_xml::events::Event;
use quick_xml::Reader;

pub const ENVELOPE_NS: &str = "http://schemas.xmlsoap.org/soap/envelope/";
pub const ENCODING: &str = "http://schemas.xmlsoap.org/soap/encoding/";

/// Eine gelesene Steueranfrage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub name: String,
    pub args: Vec<(String, String)>,
}

impl Action {
    pub fn arg(&self, name: &str) -> Option<&str> {
        self.args
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Maskiert Text fuer XML — auch fuer Attributwerte, deshalb auch die
/// Anfuehrungszeichen.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Der lokale Teil eines qualifizierten Namens: `u:Play` -> `Play`.
fn local(name: &[u8]) -> String {
    let s = String::from_utf8_lossy(name);
    s.rsplit(':').next().unwrap_or(&s).to_string()
}

/// Liest Aktionsname und Argumente aus einem SOAP-Rumpf.
///
/// Tiefe 1 ist der Umschlag, 2 der Body, 3 die Aktion, 4 ihre Argumente.
/// Ein `Header` auf Tiefe 2 wird ueberlesen. Leere Argumente
/// (`<InstanceID/>`) zaehlen als leerer Text.
pub fn parse_action(body: &str) -> Option<Action> {
    let mut reader = Reader::from_str(body);
    let mut depth = 0usize;
    let mut in_body = false;
    let mut action: Option<String> = None;
    let mut args: Vec<(String, String)> = Vec::new();
    let mut current: Option<String> = None;
    let mut text = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                depth += 1;
                let name = local(e.name().as_ref());
                match depth {
                    2 => in_body = name == "Body",
                    3 if in_body && action.is_none() => action = Some(name),
                    4 if in_body && action.is_some() => {
                        current = Some(name);
                        text.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                // Selbstschliessend: ein Argument ohne Wert.
                if depth == 3 && in_body && action.is_some() {
                    args.push((local(e.name().as_ref()), String::new()));
                } else if depth == 2 && in_body && action.is_none() {
                    // Aktion ganz ohne Argumente, z. B. <u:GetProtocolInfo/>.
                    action = Some(local(e.name().as_ref()));
                }
            }
            Ok(Event::Text(t)) => {
                if current.is_some() {
                    if let Ok(s) = t.unescape() {
                        text.push_str(&s);
                    }
                }
            }
            Ok(Event::CData(c)) => {
                if current.is_some() {
                    text.push_str(&String::from_utf8_lossy(&c));
                }
            }
            Ok(Event::End(_)) => {
                if depth == 4 {
                    if let Some(name) = current.take() {
                        args.push((name, text.trim().to_string()));
                    }
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
    }

    action.map(|name| Action { name, args })
}

/// Antwort auf eine Aktion: `<u:{Action}Response xmlns:u="{service}">…`.
pub fn response(service_type: &str, action: &str, out: &[(&str, String)]) -> String {
    let mut body = String::new();
    for (k, v) in out {
        body.push_str(&format!("<{k}>{}</{k}>", escape(v)));
    }
    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>",
            "<s:Envelope xmlns:s=\"{env}\" s:encodingStyle=\"{enc}\">",
            "<s:Body><u:{action}Response xmlns:u=\"{svc}\">{body}</u:{action}Response></s:Body>",
            "</s:Envelope>"
        ),
        env = ENVELOPE_NS,
        enc = ENCODING,
        action = action,
        svc = service_type,
        body = body,
    )
}

/// UPnP-Fehler im SOAP-Fault-Umschlag. 401 = unbekannte Aktion, 402 =
/// ungueltige Argumente, 501 = Aktion fehlgeschlagen — die drei, die der
/// Standard fuer alle Dienste kennt.
pub fn fault(code: u16, description: &str) -> String {
    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>",
            "<s:Envelope xmlns:s=\"{env}\" s:encodingStyle=\"{enc}\">",
            "<s:Body><s:Fault>",
            "<faultcode>s:Client</faultcode><faultstring>UPnPError</faultstring>",
            "<detail><UPnPError xmlns=\"urn:schemas-upnp-org:control-1-0\">",
            "<errorCode>{code}</errorCode><errorDescription>{desc}</errorDescription>",
            "</UPnPError></detail>",
            "</s:Fault></s:Body></s:Envelope>"
        ),
        env = ENVELOPE_NS,
        enc = ENCODING,
        code = code,
        desc = escape(description),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAY: &str = r#"<?xml version="1.0"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:Play xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
      <Speed>1</Speed>
    </u:Play>
  </s:Body>
</s:Envelope>"#;

    #[test]
    fn liest_aktion_und_argumente() {
        let a = parse_action(PLAY).unwrap();
        assert_eq!(a.name, "Play");
        assert_eq!(a.arg("InstanceID"), Some("0"));
        assert_eq!(a.arg("Speed"), Some("1"));
        assert_eq!(a.arg("Gibtsnicht"), None);
    }

    #[test]
    fn liest_aktion_ohne_argumente_und_mit_leeren() {
        let leer = r#"<s:Envelope xmlns:s="x"><s:Body><u:GetProtocolInfo xmlns:u="y"/></s:Body></s:Envelope>"#;
        assert_eq!(parse_action(leer).unwrap().name, "GetProtocolInfo");

        let leeres_arg = r#"<s:Envelope xmlns:s="x"><s:Body><u:SetAVTransportURI xmlns:u="y"><InstanceID>0</InstanceID><CurrentURIMetaData/></u:SetAVTransportURI></s:Body></s:Envelope>"#;
        let a = parse_action(leeres_arg).unwrap();
        assert_eq!(a.arg("CurrentURIMetaData"), Some(""));
    }

    #[test]
    fn ueberliest_einen_header() {
        let mit_header = r#"<s:Envelope xmlns:s="x"><s:Header><foo>1</foo></s:Header><s:Body><u:Pause xmlns:u="y"><InstanceID>0</InstanceID></u:Pause></s:Body></s:Envelope>"#;
        let a = parse_action(mit_header).unwrap();
        assert_eq!(a.name, "Pause");
        assert_eq!(a.args.len(), 1);
    }

    #[test]
    fn entmaskiert_argumentwerte() {
        // Metadaten kommen als maskiertes XML im Text an.
        let xml = r#"<s:Envelope xmlns:s="x"><s:Body><u:SetAVTransportURI xmlns:u="y"><CurrentURIMetaData>&lt;DIDL-Lite&gt;&amp;&lt;/DIDL-Lite&gt;</CurrentURIMetaData></u:SetAVTransportURI></s:Body></s:Envelope>"#;
        let a = parse_action(xml).unwrap();
        assert_eq!(a.arg("CurrentURIMetaData"), Some("<DIDL-Lite>&</DIDL-Lite>"));
    }

    #[test]
    fn kein_soap_ist_keine_aktion() {
        assert!(parse_action("hallo").is_none());
        assert!(parse_action("<a><b></a>").is_none());
    }

    #[test]
    fn antwort_traegt_namensraum_und_maskierte_werte() {
        let r = response(
            "urn:schemas-upnp-org:service:RenderingControl:1",
            "GetVolume",
            &[("CurrentVolume", "42".into()), ("Note", "a<b".into())],
        );
        assert!(r.contains("<u:GetVolumeResponse xmlns:u=\"urn:schemas-upnp-org:service:RenderingControl:1\">"));
        assert!(r.contains("<CurrentVolume>42</CurrentVolume>"));
        assert!(r.contains("<Note>a&lt;b</Note>"));
        assert!(r.contains("</u:GetVolumeResponse>"));
    }

    #[test]
    fn fehler_folgt_dem_upnp_schema() {
        let f = fault(401, "Invalid Action");
        assert!(f.contains("<errorCode>401</errorCode>"));
        assert!(f.contains("<errorDescription>Invalid Action</errorDescription>"));
        assert!(f.contains("urn:schemas-upnp-org:control-1-0"));
    }

    #[test]
    fn escape_maskiert_alle_fuenf() {
        assert_eq!(escape("a&b<c>d\"e'f"), "a&amp;b&lt;c&gt;d&quot;e&apos;f");
    }
}

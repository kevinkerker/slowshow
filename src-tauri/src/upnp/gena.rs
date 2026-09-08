//! GENA: Ereignis-Abonnements, damit Home Assistant Aenderungen sofort sieht
//! (E-47).
//!
//! Ohne Abonnement fragt Home Assistant den Zustand nur alle paar Sekunden ab;
//! mit Abonnement schicken wir ihn bei jeder Aenderung — Bildwechsel, Pause,
//! Nachtmodus, Helligkeit. Der Ablauf ist der des Standards: `SUBSCRIBE` mit
//! Rueckruf-URL ergibt eine `SID`, die wir uns merken; jede Aenderung geht als
//! `NOTIFY` mit laufender `SEQ` an den Rueckruf; die erste Nachricht kommt
//! sofort nach dem Abonnieren mit dem vollen Zustand.
//!
//! Abonnements laufen ab (`TIMEOUT`), Home Assistant verlaengert sie
//! rechtzeitig. Was abgelaufen ist, faellt beim naechsten Durchgang heraus —
//! ein Steuerpunkt, der ohne Abmeldung verschwindet, hinterlaesst so keine
//! Rueckrufe ins Leere.

use super::description::Service;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Laengste Frist, die wir gewaehren — der Standardvorschlag.
pub const MAX_TIMEOUT_SECS: u32 = 1800;
/// Kuerzeste Frist; darunter erneuerte ein Steuerpunkt im Dauerlauf.
pub const MIN_TIMEOUT_SECS: u32 = 30;

#[derive(Debug, Clone)]
pub struct Subscription {
    pub sid: String,
    pub service: Service,
    pub callbacks: Vec<String>,
    /// `Host`-Kopfzeile des Abonnenten-Aufrufs: unter dieser Adresse hat er uns
    /// erreicht, unter ihr bauen wir die Bild-URLs in seinen Ereignissen.
    pub host: String,
    pub expires: Instant,
    pub seq: u32,
}

/// Eine faellige Benachrichtigung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub sid: String,
    pub callback: String,
    pub host: String,
    pub seq: u32,
}

#[derive(Default)]
pub struct Subscriptions {
    inner: Mutex<Vec<Subscription>>,
}

impl Subscriptions {
    /// Legt ein Abonnement an; zurueck kommt die SID und die gewaehrte Frist.
    pub fn subscribe(
        &self,
        service: Service,
        callbacks: Vec<String>,
        host: String,
        wanted_secs: u32,
    ) -> (String, u32) {
        let secs = clamp_timeout(wanted_secs);
        let sid = format!("uuid:{}", super::new_uuid());
        if let Ok(mut list) = self.inner.lock() {
            list.push(Subscription {
                sid: sid.clone(),
                service,
                callbacks,
                host,
                expires: Instant::now() + Duration::from_secs(secs as u64),
                seq: 0,
            });
        }
        (sid, secs)
    }

    /// Verlaengert ein bestehendes Abonnement. `None`, wenn es die SID nicht
    /// (mehr) gibt — dann antwortet HTTP mit 412, und der Steuerpunkt
    /// abonniert neu.
    pub fn renew(&self, sid: &str, wanted_secs: u32) -> Option<u32> {
        let secs = clamp_timeout(wanted_secs);
        let mut list = self.inner.lock().ok()?;
        let sub = list.iter_mut().find(|s| s.sid == sid)?;
        sub.expires = Instant::now() + Duration::from_secs(secs as u64);
        Some(secs)
    }

    pub fn unsubscribe(&self, sid: &str) -> bool {
        let Ok(mut list) = self.inner.lock() else {
            return false;
        };
        let before = list.len();
        list.retain(|s| s.sid != sid);
        list.len() != before
    }

    /// Die erste Benachrichtigung eines frischen Abonnements (SEQ 0).
    pub fn initial(&self, sid: &str) -> Vec<Notification> {
        let Ok(mut list) = self.inner.lock() else {
            return Vec::new();
        };
        let Some(sub) = list.iter_mut().find(|s| s.sid == sid) else {
            return Vec::new();
        };
        let out = fan_out(sub);
        sub.seq = sub.seq.wrapping_add(1);
        out
    }

    /// Alle faelligen Benachrichtigungen eines Dienstes; abgelaufene
    /// Abonnements werden dabei entsorgt.
    pub fn due(&self, service: Service) -> Vec<Notification> {
        let Ok(mut list) = self.inner.lock() else {
            return Vec::new();
        };
        let now = Instant::now();
        list.retain(|s| s.expires > now);
        let mut out = Vec::new();
        for sub in list.iter_mut().filter(|s| s.service == service) {
            out.extend(fan_out(sub));
            // SEQ 0 ist der ersten Nachricht vorbehalten; danach zaehlt sie
            // durch und springt beim Ueberlauf auf 1, nie auf 0.
            sub.seq = match sub.seq.wrapping_add(1) {
                0 => 1,
                n => n,
            };
        }
        out
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map(|l| l.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn fan_out(sub: &Subscription) -> Vec<Notification> {
    sub.callbacks
        .iter()
        .map(|cb| Notification {
            sid: sub.sid.clone(),
            callback: cb.clone(),
            host: sub.host.clone(),
            seq: sub.seq,
        })
        .collect()
}

fn clamp_timeout(secs: u32) -> u32 {
    secs.clamp(MIN_TIMEOUT_SECS, MAX_TIMEOUT_SECS)
}

/// `CALLBACK: <http://a/x><http://b/y>` -> beide URLs.
pub fn parse_callbacks(header: &str) -> Vec<String> {
    header
        .split('<')
        .filter_map(|part| part.split('>').next())
        .map(str::trim)
        .filter(|s| s.starts_with("http://") || s.starts_with("https://"))
        .map(String::from)
        .collect()
}

/// `TIMEOUT: Second-1800` -> 1800; `Second-infinite` und Unsinn -> Hoechstwert.
pub fn parse_timeout(header: Option<&str>) -> u32 {
    header
        .and_then(|h| h.trim().strip_prefix("Second-"))
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(MAX_TIMEOUT_SECS)
}

/// Der Rumpf einer `NOTIFY`: ein `propertyset` mit dem `LastChange` als Text.
pub fn property_set(last_change: &str) -> String {
    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?>",
            "<e:propertyset xmlns:e=\"urn:schemas-upnp-org:event-1-0\">",
            "<e:property><LastChange>{}</LastChange></e:property>",
            "</e:propertyset>"
        ),
        super::soap::escape(last_change)
    )
}

/// Schickt eine Benachrichtigung. `NOTIFY` ist keine Standard-HTTP-Methode;
/// reqwest traegt sie trotzdem, wenn man sie ihm buchstabiert.
pub async fn send(
    client: &reqwest::Client,
    n: &Notification,
    body: String,
) -> Result<(), String> {
    let method = reqwest::Method::from_bytes(b"NOTIFY").map_err(|e| e.to_string())?;
    let resp = client
        .request(method, &n.callback)
        .header("CONTENT-TYPE", "text/xml; charset=\"utf-8\"")
        .header("NT", "upnp:event")
        .header("NTS", "upnp:propchange")
        .header("SID", &n.sid)
        .header("SEQ", n.seq.to_string())
        .timeout(Duration::from_secs(5))
        .body(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("Abonnent antwortete mit {}", resp.status()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abonnieren_vergibt_sid_und_klemmt_die_frist() {
        let subs = Subscriptions::default();
        let (sid, secs) = subs.subscribe(Service::Avt, vec!["http://ha/cb".into()], "h".into(), 999_999);
        assert!(sid.starts_with("uuid:"));
        assert_eq!(secs, MAX_TIMEOUT_SECS);
        let (_, kurz) = subs.subscribe(Service::Avt, vec![], "h".into(), 1);
        assert_eq!(kurz, MIN_TIMEOUT_SECS);
        assert_eq!(subs.len(), 2);
    }

    #[test]
    fn erste_nachricht_hat_seq_null_danach_zaehlt_es() {
        let subs = Subscriptions::default();
        let (sid, _) = subs.subscribe(Service::Rcs, vec!["http://ha/cb".into()], "h".into(), 300);

        let erste = subs.initial(&sid);
        assert_eq!(erste.len(), 1);
        assert_eq!(erste[0].seq, 0);
        assert_eq!(erste[0].callback, "http://ha/cb");

        let zweite = subs.due(Service::Rcs);
        assert_eq!(zweite[0].seq, 1);
        assert_eq!(subs.due(Service::Rcs)[0].seq, 2);
        // Ein anderer Dienst bekommt nichts davon.
        assert!(subs.due(Service::Avt).is_empty());
    }

    #[test]
    fn verlaengern_und_abmelden() {
        let subs = Subscriptions::default();
        let (sid, _) = subs.subscribe(Service::Avt, vec![], "h".into(), 300);
        assert_eq!(subs.renew(&sid, 600), Some(600));
        assert_eq!(subs.renew("uuid:fremd", 600), None);
        assert!(subs.unsubscribe(&sid));
        assert!(!subs.unsubscribe(&sid));
        assert!(subs.is_empty());
    }

    #[test]
    fn abgelaufene_abonnements_fallen_heraus() {
        let subs = Subscriptions::default();
        let (sid, _) = subs.subscribe(Service::Avt, vec!["http://weg".into()], "h".into(), 300);
        if let Ok(mut l) = subs.inner.lock() {
            l.iter_mut().find(|s| s.sid == sid).unwrap().expires = Instant::now() - Duration::from_secs(1);
        }
        assert!(subs.due(Service::Avt).is_empty());
        assert!(subs.is_empty(), "der Rueckruf ins Leere ist weg");
    }

    #[test]
    fn mehrere_rueckrufe_werden_alle_bedient() {
        let subs = Subscriptions::default();
        let (sid, _) = subs.subscribe(
            Service::Avt,
            vec!["http://a/1".into(), "http://b/2".into()],
            "h".into(),
            300,
        );
        let n = subs.initial(&sid);
        assert_eq!(n.len(), 2);
        assert!(n.iter().all(|x| x.seq == 0));
    }

    #[test]
    fn liest_callback_und_timeout_kopfzeilen() {
        assert_eq!(
            parse_callbacks("<http://10.0.0.2:8123/api/upnp/x><http://10.0.0.2:8124/y>"),
            vec!["http://10.0.0.2:8123/api/upnp/x", "http://10.0.0.2:8124/y"]
        );
        assert!(parse_callbacks("<ftp://nein>").is_empty());
        assert_eq!(parse_timeout(Some("Second-300")), 300);
        assert_eq!(parse_timeout(Some("Second-infinite")), MAX_TIMEOUT_SECS);
        assert_eq!(parse_timeout(None), MAX_TIMEOUT_SECS);
    }

    #[test]
    fn propertyset_traegt_das_maskierte_lastchange() {
        let body = property_set("<Event><InstanceID val=\"0\"/></Event>");
        assert!(body.contains("urn:schemas-upnp-org:event-1-0"));
        assert!(body.contains("<LastChange>&lt;Event&gt;&lt;InstanceID val=&quot;0&quot;/&gt;&lt;/Event&gt;</LastChange>"));
    }
}

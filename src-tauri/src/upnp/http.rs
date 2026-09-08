//! HTTP-Seite des Medienrenderers (E-47): Beschreibungen, SOAP-Steuerung,
//! Ereignis-Abonnements und das laufende Foto.
//!
//! Alles haengt unter `/upnp/`, damit es sich mit nichts anderem in die Quere
//! kommt. Der Router kennt den Rahmen nur als [`Backend`] — im Test ein Doppel.

use super::description::{Service, SINK_PROTOCOL_INFO};
use super::gena::{self, Subscriptions};
use super::renderer::{self, SharedBackend, Snapshot};
use super::soap::{self, Action};
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::Response;
use axum::routing::{any, get, post};
use axum::Router;
use std::sync::Arc;

pub struct Ctx {
    pub backend: SharedBackend,
    pub udn: String,
    pub subs: Arc<Subscriptions>,
    pub client: reqwest::Client,
}

pub fn router(ctx: Arc<Ctx>) -> Router {
    Router::new()
        .route("/upnp/device.xml", get(device_xml))
        .route("/upnp/scpd/{svc}", get(scpd))
        .route("/upnp/control/{svc}", post(control))
        .route("/upnp/event/{svc}", any(event))
        .route("/upnp/current.jpg", get(current_image))
        .with_state(ctx)
}

fn xml(status: StatusCode, body: String) -> Response {
    Response::builder()
        .status(status)
        .header("CONTENT-TYPE", "text/xml; charset=\"utf-8\"")
        .header("SERVER", super::SERVER_HEADER)
        .header("EXT", "")
        .body(Body::from(body))
        .expect("statische Antwort")
}

fn empty(status: StatusCode) -> Response {
    Response::builder()
        .status(status)
        .header("SERVER", super::SERVER_HEADER)
        .body(Body::empty())
        .expect("statische Antwort")
}

/// `http://host:port`, wie der Aufrufer uns erreicht hat. Unter dieser Adresse
/// muss auch das Cover liegen — eine andere Schnittstelle koennte er nicht
/// erreichen.
fn base_url(headers: &HeaderMap) -> String {
    let host = headers
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");
    format!("http://{host}")
}

async fn device_xml(State(ctx): State<Arc<Ctx>>) -> Response {
    let name = ctx.backend.snapshot().friendly_name;
    xml(StatusCode::OK, super::description::device_xml(&ctx.udn, &name))
}

async fn scpd(Path(svc): Path<String>) -> Response {
    match Service::from_slug(&svc) {
        Some(s) => xml(StatusCode::OK, s.scpd()),
        None => empty(StatusCode::NOT_FOUND),
    }
}

async fn current_image(State(ctx): State<Arc<Ctx>>) -> Response {
    match ctx.backend.current_image() {
        Some(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header("CONTENT-TYPE", "image/jpeg")
            .header("CACHE-CONTROL", "no-store")
            .body(Body::from(bytes))
            .expect("statische Antwort"),
        None => empty(StatusCode::NOT_FOUND),
    }
}

// ── Steuerung ────────────────────────────────────────────────────────────────

async fn control(
    State(ctx): State<Arc<Ctx>>,
    Path(svc): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let Some(service) = Service::from_slug(&svc) else {
        return empty(StatusCode::NOT_FOUND);
    };
    let Some(action) = soap::parse_action(&body) else {
        return xml(StatusCode::BAD_REQUEST, soap::fault(402, "Invalid Args"));
    };
    let base = base_url(&headers);

    // Das Fremdbild (E-52) ist die einzige Aktion, die ins Netz muss — und
    // deshalb die einzige, die hier asynchron behandelt wird.
    if service == Service::Avt && action.name == "SetAVTransportURI" {
        return set_transport_uri(&ctx, &action).await;
    }

    match dispatch(&ctx, service, &action, &base) {
        Ok(Outcome { out, changed }) => {
            if changed {
                publish(ctx.clone()).await;
            }
            xml(StatusCode::OK, soap::response(service.service_type(), &action.name, &out))
        }
        Err((code, text)) => xml(StatusCode::INTERNAL_SERVER_ERROR, soap::fault(code, text)),
    }
}

/// Groesstes Fremdbild, das geladen wird (E-52). Ein Kamerabild hat wenige MB;
/// darueber ist es kein Bild fuer einen Rahmen, sondern ein Versehen.
const MAX_EXTERNAL_BYTES: usize = 25 * 1024 * 1024;
/// Wie lange auf die Quelle gewartet wird. Home Assistant wartet seinerseits
/// auf unsere Antwort; laenger als das haelt es nicht durch.
const EXTERNAL_FETCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// `SetAVTransportURI` (E-52): Bild laden und dem Rahmen zum Vormerken geben.
///
/// Ob es sofort haengt, entscheidet der Zustand davor (E-56): Ist der Renderer
/// PLAYING, zeigt er das Bild direkt — so will es der AVTransport-Standard,
/// und Home Assistant verlaesst sich darauf und schickt in diesem Fall kein
/// `Play` mehr. Vorher wartete das Bild dann bis zur 60-s-Frist unsichtbar
/// im Speicher, und HA zeigte nicht einmal einen Play-Knopf. Ist der Rahmen
/// dunkel oder pausiert, wartet das Bild wie bisher auf das `Play`, das HA
/// dann auch schickt. Eine leere URI wird angenommen und ignoriert — manche
/// Steuerpunkte schicken sie beim Anlegen, und ein Fehler liesse den Renderer
/// als kaputt erscheinen. UPnP-Fehlercodes: 714 fuer „kein Bild", 716 fuer
/// „nicht erreichbar".
async fn set_transport_uri(ctx: &Arc<Ctx>, action: &Action) -> Response {
    let service_type = Service::Avt.service_type();
    let ok = || xml(StatusCode::OK, soap::response(service_type, &action.name, &[]));
    let fault = |code: u16, text: &str| {
        xml(StatusCode::INTERNAL_SERVER_ERROR, soap::fault(code, text))
    };

    let uri = action.arg("CurrentURI").map(str::trim).unwrap_or("");
    if uri.is_empty() {
        return ok();
    }
    if !(uri.starts_with("http://") || uri.starts_with("https://")) {
        log::info!("UPnP: Fremdbild abgewiesen, keine http-Adresse: {uri}");
        return fault(714, "Illegal MIME-Type");
    }
    let title = action
        .arg("CurrentURIMetaData")
        .and_then(renderer::didl_title)
        .unwrap_or_else(|| file_name_of(uri));

    // Mit Zeit im Log, weil das Ende allein nichts ueber die Dauer sagt — die
    // Frage „warum dauert das" war ohne diese Zeile nicht zu beantworten.
    log::info!("UPnP: Fremdbild angefordert von {uri}");
    let bytes = match fetch_image(&ctx.client, uri).await {
        Ok(bytes) => bytes,
        Err(Fetch::Unreachable(e)) => {
            log::info!("UPnP: Fremdbild nicht ladbar von {uri}: {e}");
            return fault(716, "Resource not found");
        }
        Err(Fetch::NotImage(e)) => {
            log::info!("UPnP: Fremdbild von {uri} ist kein Bild: {e}");
            return fault(714, "Illegal MIME-Type");
        }
    };

    // Zustand *vor* dem Vormerken festhalten (E-56): das Vormerken ersetzt ein
    // haengendes Fremdbild durch ein wartendes, und danach saehe ein Rahmen
    // mit pausierter Diashow nicht mehr nach PLAYING aus — Home Assistant hat
    // ihn aber genau so in Erinnerung und schickt kein Play.
    let was_playing = renderer::transport_state(&ctx.backend.snapshot()) == "PLAYING";

    // Dekodieren ist Rechenarbeit; sie gehoert nicht auf den Netz-Thread.
    let backend = ctx.backend.clone();
    let (t, u) = (title.clone(), uri.to_string());
    let staged = tokio::task::spawn_blocking(move || backend.stage_external(bytes, t, u)).await;
    match staged {
        Ok(Ok(())) => {
            log::info!("UPnP: Fremdbild {title:?} vorgemerkt von {uri}");
            if was_playing {
                // Laeuft der Renderer, laeuft die neue Adresse sofort — wie bei
                // jedem Medienrenderer. `play` zeigt das eben vorgemerkte Bild.
                ctx.backend.play();
            }
            publish(ctx.clone()).await;
            ok()
        }
        Ok(Err(e)) => {
            log::info!("UPnP: Fremdbild von {uri} nicht aufbereitet: {e}");
            fault(714, "Illegal MIME-Type")
        }
        Err(e) => {
            log::warn!("UPnP: Aufbereitung des Fremdbilds abgebrochen: {e}");
            fault(501, "Action Failed")
        }
    }
}

enum Fetch {
    Unreachable(String),
    NotImage(String),
}

async fn fetch_image(client: &reqwest::Client, uri: &str) -> Result<Vec<u8>, Fetch> {
    let resp = client
        .get(uri)
        .timeout(EXTERNAL_FETCH_TIMEOUT)
        .send()
        .await
        .map_err(|e| Fetch::Unreachable(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(Fetch::Unreachable(format!("HTTP {}", resp.status())));
    }
    if let Some(len) = resp.content_length() {
        if len > MAX_EXTERNAL_BYTES as u64 {
            return Err(Fetch::NotImage(format!("{len} Bytes")));
        }
    }
    if let Some(ct) = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
    {
        let ct = ct.to_ascii_lowercase();
        // Ein Kamerastrom (E-55): das erste Bild daraus, dann Schluss.
        if super::mjpeg::is_multipart_stream(&ct) {
            return first_frame_of_stream(resp, uri, super::mjpeg::boundary_of(&ct)).await;
        }
        if !(ct.starts_with("image/") || ct.starts_with("application/octet-stream")) {
            return Err(Fetch::NotImage(ct));
        }
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| Fetch::Unreachable(e.to_string()))?;
    if bytes.len() > MAX_EXTERNAL_BYTES {
        return Err(Fetch::NotImage(format!("{} Bytes", bytes.len())));
    }
    Ok(bytes.to_vec())
}

/// Liest einen `multipart/x-mixed-replace`-Strom nur so weit, bis das erste
/// Teilbild vollstaendig ist, und gibt dieses zurueck (E-55).
///
/// Der Strom endet von sich aus nie; wer ihn ganz lesen wollte, liefe in den
/// Zeitablauf und die Groessengrenze. Deshalb Stueck fuer Stueck, und nach
/// jedem Stueck die Frage an den Parser. Der Zeitablauf der Anfrage gilt
/// weiter — bleibt die Kamera stumm, kommt 716 wie bei jeder anderen Quelle.
async fn first_frame_of_stream(
    mut resp: reqwest::Response,
    uri: &str,
    boundary: Option<String>,
) -> Result<Vec<u8>, Fetch> {
    use super::mjpeg::{first_part, Part};

    let mut buf: Vec<u8> = Vec::new();
    loop {
        let chunk = resp
            .chunk()
            .await
            .map_err(|e| Fetch::Unreachable(e.to_string()))?;
        let Some(chunk) = chunk else {
            // Strom zu Ende, ohne dass ein Teil fertig wurde.
            return Err(Fetch::NotImage(
                "Strom endete ohne vollstaendiges Bild".to_string(),
            ));
        };
        buf.extend_from_slice(&chunk);
        if buf.len() > MAX_EXTERNAL_BYTES {
            return Err(Fetch::NotImage(format!(
                "Strom ohne Bild nach {} Bytes",
                buf.len()
            )));
        }
        match first_part(&buf, boundary.as_deref()) {
            Part::Complete { body, .. } => {
                log::info!(
                    "UPnP: Kamerastrom von {uri}: erstes Bild uebernommen ({} Bytes)",
                    body.len()
                );
                return Ok(body);
            }
            Part::Invalid(why) => return Err(Fetch::NotImage(why)),
            Part::Incomplete => continue,
        }
    }
}

/// Letztes Pfadsegment ohne Anfrageparameter — der Titel, wenn keiner mitkommt.
fn file_name_of(uri: &str) -> String {
    let path = uri.split(['?', '#']).next().unwrap_or(uri);
    let name = path.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    if name.is_empty() || name.contains(':') {
        "Bild".to_string()
    } else {
        name.to_string()
    }
}

struct Outcome {
    out: Vec<(&'static str, String)>,
    /// Hat die Aktion den Zustand geaendert? Dann gehen Ereignisse raus.
    changed: bool,
}

fn reply(out: Vec<(&'static str, String)>) -> Result<Outcome, (u16, &'static str)> {
    Ok(Outcome { out, changed: false })
}

fn did(out: Vec<(&'static str, String)>) -> Result<Outcome, (u16, &'static str)> {
    Ok(Outcome { out, changed: true })
}

/// Fuehrt eine Aktion aus. Die Rueckgabe sind die Ausgabeargumente in der
/// Reihenfolge der Dienstbeschreibung.
fn dispatch(
    ctx: &Ctx,
    service: Service,
    action: &Action,
    base: &str,
) -> Result<Outcome, (u16, &'static str)> {
    let b = &ctx.backend;
    let s = b.snapshot();
    match (service, action.name.as_str()) {
        // ── AVTransport ─────────────────────────────────────────────────
        (Service::Avt, "Play") => {
            b.play();
            did(vec![])
        }
        // Kein Pause (E-49): Home Assistant zeigt sonst Pause statt Stop,
        // und der Nachtmodus ist in der Oberflaeche unerreichbar.
        (Service::Avt, "Stop") => {
            b.stop();
            did(vec![])
        }
        (Service::Avt, "Next") => {
            b.next();
            did(vec![])
        }
        (Service::Avt, "Previous") => {
            b.previous();
            did(vec![])
        }
        // SetAVTransportURI laeuft nicht hier durch: es holt ein Bild aus dem
        // Netz und ist deshalb asynchron (`set_transport_uri`, E-52).
        (Service::Avt, "GetTransportInfo") => reply(vec![
            ("CurrentTransportState", renderer::transport_state(&s).into()),
            ("CurrentTransportStatus", "OK".into()),
            ("CurrentSpeed", "1".into()),
        ]),
        (Service::Avt, "GetPositionInfo") => reply(vec![
            ("Track", if s.current_id.is_some() { "1" } else { "0" }.into()),
            ("TrackDuration", "00:00:00".into()),
            ("TrackMetaData", renderer::didl(base, &s)),
            ("TrackURI", renderer::track_uri(base, &s)),
            ("RelTime", "00:00:00".into()),
            ("AbsTime", "00:00:00".into()),
            ("RelCount", "0".into()),
            ("AbsCount", "0".into()),
        ]),
        (Service::Avt, "GetMediaInfo") => reply(vec![
            ("NrTracks", if s.current_id.is_some() { "1" } else { "0" }.into()),
            ("MediaDuration", "00:00:00".into()),
            ("CurrentURI", renderer::media_uri(base, &s)),
            ("CurrentURIMetaData", renderer::didl(base, &s)),
            ("NextURI", String::new()),
            ("NextURIMetaData", String::new()),
            ("PlayMedium", "NETWORK".into()),
            ("RecordMedium", "NOT_IMPLEMENTED".into()),
            ("WriteStatus", "NOT_IMPLEMENTED".into()),
        ]),
        (Service::Avt, "GetDeviceCapabilities") => reply(vec![
            ("PlayMedia", "NETWORK".into()),
            ("RecMedia", "NOT_IMPLEMENTED".into()),
            ("RecQualityModes", "NOT_IMPLEMENTED".into()),
        ]),
        (Service::Avt, "GetTransportSettings") => reply(vec![
            ("PlayMode", "NORMAL".into()),
            ("RecQualityMode", "NOT_IMPLEMENTED".into()),
        ]),
        (Service::Avt, "GetCurrentTransportActions") => {
            reply(vec![("Actions", renderer::transport_actions(&s).into())])
        }

        // ── RenderingControl ────────────────────────────────────────────
        (Service::Rcs, "GetVolume") => reply(vec![("CurrentVolume", s.brightness.to_string())]),
        (Service::Rcs, "SetVolume") => {
            let level: u8 = action
                .arg("DesiredVolume")
                .and_then(|v| v.trim().parse::<u32>().ok())
                .filter(|v| *v <= 100)
                .ok_or((402, "Invalid Args"))? as u8;
            b.set_volume(level);
            did(vec![])
        }
        (Service::Rcs, "GetMute") => reply(vec![(
            "CurrentMute",
            if renderer::is_muted(&s) { "1" } else { "0" }.into(),
        )]),
        (Service::Rcs, "SetMute") => {
            let mute = match action.arg("DesiredMute").map(str::trim) {
                Some("1") | Some("true") | Some("True") => true,
                Some("0") | Some("false") | Some("False") => false,
                _ => return Err((402, "Invalid Args")),
            };
            b.set_mute(mute);
            did(vec![])
        }
        (Service::Rcs, "ListPresets") => {
            reply(vec![("CurrentPresetNameList", "FactoryDefaults".into())])
        }
        (Service::Rcs, "SelectPreset") => reply(vec![]),

        // ── ConnectionManager ───────────────────────────────────────────
        (Service::Cm, "GetProtocolInfo") => reply(vec![
            ("Source", String::new()),
            ("Sink", SINK_PROTOCOL_INFO.into()),
        ]),
        (Service::Cm, "GetCurrentConnectionIDs") => reply(vec![("ConnectionIDs", "0".into())]),
        (Service::Cm, "GetCurrentConnectionInfo") => reply(vec![
            ("RcsID", "0".into()),
            ("AVTransportID", "0".into()),
            ("ProtocolInfo", String::new()),
            ("PeerConnectionManager", String::new()),
            ("PeerConnectionID", "-1".into()),
            ("Direction", "Input".into()),
            ("Status", "OK".into()),
        ]),

        _ => Err((401, "Invalid Action")),
    }
}

// ── Ereignisse ───────────────────────────────────────────────────────────────

/// `SUBSCRIBE` und `UNSUBSCRIBE` sind keine Standardmethoden; axum kennt sie
/// nicht als Route, also nimmt eine Route alles an und schaut selbst nach.
async fn event(
    State(ctx): State<Arc<Ctx>>,
    Path(svc): Path<String>,
    method: Method,
    headers: HeaderMap,
) -> Response {
    let Some(service) = Service::from_slug(&svc) else {
        return empty(StatusCode::NOT_FOUND);
    };
    let header = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(str::trim);

    match method.as_str() {
        "SUBSCRIBE" => {
            let timeout = gena::parse_timeout(header("TIMEOUT"));
            if let Some(sid) = header("SID") {
                // Verlaengerung. Ein unbekanntes SID ist der Normalfall nach
                // einem Neustart der App: die Abonnements leben im Speicher,
                // Home Assistant kennt sein altes noch. 412 sagt ihm, dass es
                // neu abonnieren muss — im Log, weil sonst nicht zu sehen ist,
                // warum HA eine Weile nur alle 30 s nachfragt (E-53).
                return match ctx.subs.renew(sid, timeout) {
                    Some(secs) => {
                        log::info!("UPnP: Abonnement {} fuer {secs} s verlaengert", short_sid(sid));
                        subscribed(sid, secs)
                    }
                    None => {
                        log::info!(
                            "UPnP: Verlaengerung fuer unbekanntes Abonnement {} abgewiesen (412)",
                            short_sid(sid)
                        );
                        empty(StatusCode::PRECONDITION_FAILED)
                    }
                };
            }
            if header("NT") != Some("upnp:event") {
                return empty(StatusCode::PRECONDITION_FAILED);
            }
            let callbacks = header("CALLBACK").map(gena::parse_callbacks).unwrap_or_default();
            if callbacks.is_empty() {
                return empty(StatusCode::PRECONDITION_FAILED);
            }
            let host = header("host").unwrap_or("localhost").to_string();
            let (sid, secs) = ctx.subs.subscribe(service, callbacks.clone(), host, timeout);
            log::info!(
                "UPnP: {} abonniert {:?} fuer {secs} s, Rueckruf {}",
                short_sid(&sid),
                service,
                callbacks.join(" ")
            );

            // Die erste Nachricht traegt den vollen Zustand — der Standard
            // verlangt sie, und Home Assistant wartet darauf.
            let initial = ctx.subs.initial(&sid);
            let snapshot = ctx.backend.snapshot();
            let ctx2 = ctx.clone();
            tokio::spawn(async move {
                for n in initial {
                    let body = gena::property_set(&last_change(service, &n.host, &snapshot));
                    if let Err(e) = gena::send(&ctx2.client, &n, body).await {
                        log::warn!("UPnP: erste Benachrichtigung an {} scheiterte: {e}", n.callback);
                    }
                }
            });
            subscribed(&sid, secs)
        }
        "UNSUBSCRIBE" => match header("SID") {
            Some(sid) if ctx.subs.unsubscribe(sid) => {
                log::info!("UPnP: Abonnement {} beendet", short_sid(sid));
                empty(StatusCode::OK)
            }
            _ => empty(StatusCode::PRECONDITION_FAILED),
        },
        _ => empty(StatusCode::METHOD_NOT_ALLOWED),
    }
}

fn subscribed(sid: &str, secs: u32) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header("SERVER", super::SERVER_HEADER)
        .header("SID", sid)
        .header("TIMEOUT", format!("Second-{secs}"))
        .body(Body::empty())
        .expect("statische Antwort")
}

fn last_change(service: Service, host: &str, s: &Snapshot) -> String {
    match service {
        Service::Avt => renderer::last_change_avt(&format!("http://{host}"), s),
        Service::Rcs => renderer::last_change_rcs(s),
        Service::Cm => String::new(),
    }
}

/// Schickt den aktuellen Zustand an alle Abonnenten von AVTransport und
/// RenderingControl. Gerufen nach jeder Aktion und bei jedem Ereignis des
/// Rahmens (Bildwechsel, Anzeige, Einstellungen).
pub async fn publish(ctx: Arc<Ctx>) {
    let snapshot = ctx.backend.snapshot();
    for service in [Service::Avt, Service::Rcs] {
        for n in ctx.subs.due(service) {
            let body = gena::property_set(&last_change(service, &n.host, &snapshot));
            let client = ctx.client.clone();
            tokio::spawn(async move {
                if let Err(e) = gena::send(&client, &n, body).await {
                    // Als Warnung, nicht Debug: „Home Assistant zeigt nichts"
                    // war ohne diese Zeile am Geraet nicht zu beantworten.
                    log::warn!("UPnP: Benachrichtigung an {} scheiterte: {e}", n.callback);
                }
            });
        }
    }
}

/// Kurzform eines SID fuers Log: `uuid:` und die ersten acht Zeichen reichen,
/// um zwei Abonnements auseinanderzuhalten.
fn short_sid(sid: &str) -> String {
    let id = sid.strip_prefix("uuid:").unwrap_or(sid);
    format!("uuid:{}", id.chars().take(8).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::super::renderer::tests::FakeBackend;
    use super::super::renderer::Backend;
    use super::*;
    use axum::http::Request;
    use tower::ServiceExt;

    fn setup() -> (Arc<FakeBackend>, Router) {
        let fake = FakeBackend::shared();
        let ctx = Arc::new(Ctx {
            backend: fake.clone(),
            udn: "uuid:test-udn".into(),
            subs: Arc::new(Subscriptions::default()),
            client: reqwest::Client::new(),
        });
        (fake, router(ctx))
    }

    async fn body_of(resp: Response) -> String {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    fn soap_req(svc: &str, service_type: &str, action: &str, args: &str) -> Request<Body> {
        let body = format!(
            "<s:Envelope xmlns:s=\"{}\"><s:Body><u:{action} xmlns:u=\"{service_type}\">{args}</u:{action}></s:Body></s:Envelope>",
            soap::ENVELOPE_NS
        );
        Request::builder()
            .method("POST")
            .uri(format!("/upnp/control/{svc}"))
            .header("host", "10.0.0.7:8128")
            .header("SOAPACTION", format!("\"{service_type}#{action}\""))
            .body(Body::from(body))
            .unwrap()
    }

    #[tokio::test]
    async fn geraetebeschreibung_und_scpds_sind_erreichbar() {
        let (_, app) = setup();
        let resp = app
            .clone()
            .oneshot(Request::get("/upnp/device.xml").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(resp.headers()["content-type"].to_str().unwrap().starts_with("text/xml"));
        let xml = body_of(resp).await;
        assert!(xml.contains("<UDN>uuid:test-udn</UDN>"));
        assert!(xml.contains("<friendlyName>Slowshow</friendlyName>"));

        for svc in Service::ALL {
            let resp = app
                .clone()
                .oneshot(Request::get(svc.scpd_path()).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK, "{svc:?}");
        }
        let resp = app
            .oneshot(Request::get("/upnp/scpd/nix").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn play_next_stop_landen_im_rahmen_und_pause_gibt_es_nicht_e_49() {
        let (fake, app) = setup();
        let avt = Service::Avt.service_type();
        for action in ["Play", "Next", "Previous", "Stop"] {
            let resp = app
                .clone()
                .oneshot(soap_req("avt", avt, action, "<InstanceID>0</InstanceID>"))
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::OK, "{action}");
            let xml = body_of(resp).await;
            assert!(xml.contains(&format!("<u:{action}Response")), "{action}: {xml}");
        }
        // Pause ist keine Aktion mehr (E-49): Home Assistant zeigt sonst Pause
        // statt Stop, und der Nachtmodus waere in der Oberflaeche unerreichbar.
        let resp = app
            .oneshot(soap_req("avt", avt, "Pause", "<InstanceID>0</InstanceID>"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(body_of(resp).await.contains("<errorCode>401</errorCode>"));
        assert_eq!(fake.calls(), vec!["play", "next", "previous", "stop"]);
    }

    #[tokio::test]
    async fn transportinfo_spiegelt_den_zustand() {
        let (fake, app) = setup();
        let avt = Service::Avt.service_type();
        let resp = app
            .clone()
            .oneshot(soap_req("avt", avt, "GetTransportInfo", "<InstanceID>0</InstanceID>"))
            .await
            .unwrap();
        assert!(body_of(resp).await.contains("<CurrentTransportState>PLAYING</CurrentTransportState>"));

        fake.stop();
        let resp = app
            .oneshot(soap_req("avt", avt, "GetTransportInfo", "<InstanceID>0</InstanceID>"))
            .await
            .unwrap();
        assert!(body_of(resp).await.contains("<CurrentTransportState>STOPPED</CurrentTransportState>"));
    }

    #[tokio::test]
    async fn positionsinfo_traegt_cover_unter_der_aufrufadresse() {
        // Home Assistant erreicht uns unter der Adresse aus LOCATION; das
        // Cover muss unter derselben liegen — deshalb der Host der Anfrage.
        let (_, app) = setup();
        let resp = app
            .oneshot(soap_req(
                "avt",
                Service::Avt.service_type(),
                "GetPositionInfo",
                "<InstanceID>0</InstanceID>",
            ))
            .await
            .unwrap();
        let xml = body_of(resp).await;
        assert!(xml.contains("<TrackURI>http://10.0.0.7:8128/upnp/current.jpg?id=abc123</TrackURI>"));
        assert!(xml.contains("&lt;dc:title&gt;IMG_0001.jpg&lt;/dc:title&gt;"), "{xml}");
    }

    #[tokio::test]
    async fn lautstaerke_ist_helligkeit_und_stumm_ist_nachtmodus() {
        let (fake, app) = setup();
        let rcs = Service::Rcs.service_type();
        let resp = app
            .clone()
            .oneshot(soap_req(
                "rcs",
                rcs,
                "SetVolume",
                "<InstanceID>0</InstanceID><Channel>Master</Channel><DesiredVolume>35</DesiredVolume>",
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = app
            .clone()
            .oneshot(soap_req(
                "rcs",
                rcs,
                "GetVolume",
                "<InstanceID>0</InstanceID><Channel>Master</Channel>",
            ))
            .await
            .unwrap();
        assert!(body_of(resp).await.contains("<CurrentVolume>35</CurrentVolume>"));

        let resp = app
            .clone()
            .oneshot(soap_req(
                "rcs",
                rcs,
                "SetMute",
                "<InstanceID>0</InstanceID><Channel>Master</Channel><DesiredMute>1</DesiredMute>",
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(fake.calls(), vec!["volume:35", "mute:true"]);

        let resp = app
            .oneshot(soap_req(
                "rcs",
                rcs,
                "GetMute",
                "<InstanceID>0</InstanceID><Channel>Master</Channel>",
            ))
            .await
            .unwrap();
        assert!(body_of(resp).await.contains("<CurrentMute>1</CurrentMute>"));
    }

    #[tokio::test]
    async fn ungueltige_lautstaerke_ist_ein_argumentfehler() {
        let (fake, app) = setup();
        let resp = app
            .oneshot(soap_req(
                "rcs",
                Service::Rcs.service_type(),
                "SetVolume",
                "<InstanceID>0</InstanceID><Channel>Master</Channel><DesiredVolume>250</DesiredVolume>",
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(body_of(resp).await.contains("<errorCode>402</errorCode>"));
        assert!(fake.calls().is_empty(), "nichts am Rahmen veraendert");
    }

    #[tokio::test]
    async fn unbekannte_aktion_ist_401() {
        let (_, app) = setup();
        let resp = app
            .oneshot(soap_req("avt", Service::Avt.service_type(), "Seek", "<InstanceID>0</InstanceID>"))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(body_of(resp).await.contains("<errorCode>401</errorCode>"));
    }

    /// Ein kleiner Bildserver fuer das Fremdbild (E-52): liefert ein „JPEG"
    /// und eine Textdatei, mehr braucht der Router nicht zu wissen.
    async fn bildserver() -> String {
        use axum::routing::get;
        let app = Router::new()
            .route(
                "/klingel.jpg",
                get(|| async { ([("content-type", "image/jpeg")], vec![0xFFu8, 0xD8, 0xFF, 0xD9]) }),
            )
            .route(
                "/text.txt",
                get(|| async { ([("content-type", "text/plain")], "kein Bild") }),
            )
            // Ein Kamerastrom, wie Home Assistant ihn liefert (E-55): Trenner
            // im Kopf mit zwei Strichen, im Rumpf ebenso, und er endet nie.
            .route("/strom", get(|| async { endless_stream("image/jpeg") }))
            .route("/textstrom", get(|| async { endless_stream("text/plain") }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    /// Endloser `multipart/x-mixed-replace`-Strom in der Schreibweise von
    /// Home Assistant: alle 50 ms ein Teil, der Rahmen darf nicht auf das Ende
    /// warten.
    fn endless_stream(part_type: &'static str) -> Response {
        let frames = futures_util::stream::unfold(0u32, move |n| async move {
            if n > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            let mut part = format!(
                "--frameboundary\r\nContent-Type: {part_type}\r\nContent-Length: 4\r\n\r\n"
            )
            .into_bytes();
            part.extend_from_slice(&[0xFF, 0xD8, 0xFF, 0xD9]);
            part.extend_from_slice(b"\r\n");
            Some((Ok::<_, std::io::Error>(axum::body::Bytes::from(part)), n + 1))
        });
        Response::builder()
            .header(
                "content-type",
                "multipart/x-mixed-replace; boundary=--frameboundary",
            )
            .body(Body::from_stream(frames))
            .unwrap()
    }

    fn set_uri(uri: &str, meta: &str) -> Request<Body> {
        soap_req(
            "avt",
            Service::Avt.service_type(),
            "SetAVTransportURI",
            &format!(
                "<InstanceID>0</InstanceID><CurrentURI>{}</CurrentURI><CurrentURIMetaData>{}</CurrentURIMetaData>",
                soap::escape(uri),
                soap::escape(meta)
            ),
        )
    }

    #[tokio::test]
    async fn fremdbild_wird_geladen_und_vorgemerkt_e_52() {
        let (fake, app) = setup();
        let base = bildserver().await;
        let didl = "<DIDL-Lite xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><item><dc:title>Haustuer</dc:title></item></DIDL-Lite>";
        let resp = app
            .clone()
            .oneshot(set_uri(&format!("{base}/klingel.jpg"), didl))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(body_of(resp).await.contains("<u:SetAVTransportURIResponse"));
        // Rohe Bytes und der Titel aus dem DIDL; dekodiert wird im Rahmen. Das
        // Double laeuft (PLAYING), also folgt sofort das Zeigen (E-56).
        assert_eq!(fake.calls(), vec!["stage:Haustuer:4", "play"]);

        // Ohne Metadaten heisst das Bild wie seine Datei.
        let resp = app
            .oneshot(set_uri(&format!("{base}/klingel.jpg?token=abc"), ""))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let calls = fake.calls();
        assert_eq!(
            &calls[calls.len() - 2..],
            ["stage:klingel.jpg:4", "play"],
            "war: {calls:?}"
        );
    }

    #[tokio::test]
    async fn kamerastrom_liefert_sein_erstes_bild_e_55() {
        // Der Befund vom Geraet: „Auf Media-Player abspielen" im Kamera-Dialog
        // schickt `camera_proxy_stream`, einen Strom ohne Ende. Der Rahmen
        // nimmt das erste Bild und antwortet, lange bevor der Zeitablauf greift.
        let (fake, app) = setup();
        let base = bildserver().await;
        let started = std::time::Instant::now();
        let resp = app
            .clone()
            .oneshot(set_uri(
                &format!("{base}/strom?authSig=abc"),
                "",
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "{}", body_of(resp).await);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "hat auf das Ende des Stroms gewartet: {:?}",
            started.elapsed()
        );
        // Genau das erste JPEG, ohne Kopfzeilen und Trenner, benannt nach dem
        // Pfad — und sofort gezeigt, weil das Double laeuft (E-56).
        assert_eq!(fake.calls(), vec!["stage:strom:4", "play"]);

        // Ein Strom aus Text ist kein Bild: 714, wie bei einer Textdatei.
        let resp = app
            .oneshot(set_uri(&format!("{base}/textstrom"), ""))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(body_of(resp).await.contains("<errorCode>714</errorCode>"));
        assert_eq!(fake.calls().len(), 2, "nichts weiteres vorgemerkt oder gezeigt");
    }

    #[tokio::test]
    async fn dunkler_oder_pausierter_rahmen_wartet_auf_play_e_56() {
        // Der Befund: Home Assistant laesst Play weg, wenn es den Renderer als
        // PLAYING kennt — dann muss das Bild von selbst erscheinen (siehe die
        // beiden Tests oben). Kennt es ihn als dunkel oder pausiert, schickt
        // es Play; dann darf das Vormerken nichts zeigen und vor allem keinen
        // dunklen Schirm wecken, den erst das Play wecken soll.
        let base = bildserver().await;

        // Dunkel (STOPPED): nur vormerken.
        let (fake, app) = setup();
        fake.snapshot.lock().unwrap().screen_active = false;
        let resp = app
            .oneshot(set_uri(&format!("{base}/klingel.jpg"), ""))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(fake.calls(), vec!["stage:klingel.jpg:4"]);

        // Pausiert (PAUSED_PLAYBACK): ebenfalls nur vormerken.
        let (fake, app) = setup();
        fake.snapshot.lock().unwrap().playing = false;
        let resp = app
            .oneshot(set_uri(&format!("{base}/klingel.jpg"), ""))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(fake.calls(), vec!["stage:klingel.jpg:4"]);

        // Pausiert, aber ein Fremdbild haengt schon: fuer HA ist das PLAYING,
        // also wird das neue Bild sofort gezeigt — sonst bliebe das alte.
        let (fake, app) = setup();
        {
            let mut s = fake.snapshot.lock().unwrap();
            s.playing = false;
            s.external = true;
        }
        let resp = app
            .oneshot(set_uri(&format!("{base}/klingel.jpg"), ""))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(fake.calls(), vec!["stage:klingel.jpg:4", "play"]);
    }

    #[tokio::test]
    async fn fremdbild_fehler_haben_upnp_codes_e_52() {
        let (fake, app) = setup();
        let base = bildserver().await;

        // Kein Bild: 714.
        let resp = app
            .clone()
            .oneshot(set_uri(&format!("{base}/text.txt"), ""))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(body_of(resp).await.contains("<errorCode>714</errorCode>"));

        // Nicht erreichbar: 716.
        let resp = app
            .clone()
            .oneshot(set_uri(&format!("{base}/gibt-es-nicht.jpg"), ""))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(body_of(resp).await.contains("<errorCode>716</errorCode>"));

        // Kein http: 714, ohne dass das Netz angefasst wird.
        let resp = app
            .clone()
            .oneshot(set_uri("ftp://irgendwo/bild.jpg", ""))
            .await
            .unwrap();
        assert!(body_of(resp).await.contains("<errorCode>714</errorCode>"));

        // Leer: angenommen und ignoriert — sonst gaelte der Renderer als kaputt.
        let resp = app.oneshot(set_uri("", "")).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(fake.calls().is_empty(), "nichts vorgemerkt: {:?}", fake.calls());
    }

    #[test]
    fn dateiname_aus_der_adresse_e_52() {
        assert_eq!(file_name_of("http://ha:8123/local/klingel.jpg?x=1#f"), "klingel.jpg");
        assert_eq!(
            file_name_of("http://ha:8123/api/camera_proxy/camera.tuer?token=abc"),
            "camera.tuer"
        );
        assert_eq!(file_name_of("http://ha:8123/"), "Bild");
        assert_eq!(file_name_of("http://ha:8123"), "Bild");
    }

    #[tokio::test]
    async fn connectionmanager_antwortet_home_assistant() {
        let (_, app) = setup();
        let cm = Service::Cm.service_type();
        let resp = app
            .oneshot(soap_req("cm", cm, "GetProtocolInfo", ""))
            .await
            .unwrap();
        let xml = body_of(resp).await;
        assert!(xml.contains("<Sink>http-get:*:image/jpeg:*"));
    }

    #[tokio::test]
    async fn abonnieren_gibt_sid_und_frist_verlaengern_und_abmelden_gehen() {
        let (_, app) = setup();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("SUBSCRIBE")
                    .uri("/upnp/event/avt")
                    .header("host", "10.0.0.7:8128")
                    .header("NT", "upnp:event")
                    .header("CALLBACK", "<http://127.0.0.1:1/cb>")
                    .header("TIMEOUT", "Second-300")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let sid = resp.headers()["sid"].to_str().unwrap().to_string();
        assert!(sid.starts_with("uuid:"));
        assert_eq!(resp.headers()["timeout"], "Second-300");

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("SUBSCRIBE")
                    .uri("/upnp/event/avt")
                    .header("SID", &sid)
                    .header("TIMEOUT", "Second-600")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers()["timeout"], "Second-600");

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("UNSUBSCRIBE")
                    .uri("/upnp/event/avt")
                    .header("SID", &sid)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Noch einmal: die SID gibt es nicht mehr.
        let resp = app
            .oneshot(
                Request::builder()
                    .method("UNSUBSCRIBE")
                    .uri("/upnp/event/avt")
                    .header("SID", &sid)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::PRECONDITION_FAILED);
    }

    #[tokio::test]
    async fn abonnieren_ohne_rueckruf_wird_abgewiesen() {
        let (_, app) = setup();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("SUBSCRIBE")
                    .uri("/upnp/event/rcs")
                    .header("NT", "upnp:event")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::PRECONDITION_FAILED);
    }

    #[tokio::test]
    async fn laufendes_foto_wird_ausgeliefert() {
        let (_, app) = setup();
        let resp = app
            .oneshot(Request::get("/upnp/current.jpg?id=abc123").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers()["content-type"], "image/jpeg");
    }
}

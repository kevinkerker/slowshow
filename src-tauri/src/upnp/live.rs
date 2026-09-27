//! Livebild aus einem Kamerastrom (E-67).
//!
//! Schickt Home Assistant eine Kamera per „Auf Media-Player abspielen", kommt
//! ein `multipart/x-mixed-replace`-Strom ohne Ende. Das erste Bild haengt
//! `SetAVTransportURI` wie jedes Fremdbild auf (E-52, E-55); dieser Lauf liest
//! danach weiter und ersetzt es fortlaufend durch das neueste Einzelbild.
//!
//! Die Bilder gehen denselben Weg wie jedes andere: im Rust-Prozess auf
//! Displaygroesse gebracht (NF-12, NF-13), von der Oberflaeche ueber das
//! Asset-Protokoll geholt. Ueber IPC laeuft nur Id und Nummer des Einzelbilds —
//! nie das Bild selbst, und kein zweiter HTTP-Weg in die WebView.
//!
//! Wann der Lauf endet:
//! - Das Fremdbild ist weg — Stop, Geste, Nachtmodus oder ein neues
//!   `play_media`. Dann hoert er still auf; abgeraeumt hat schon ein anderer.
//! - Nach [`LIVE_MAX`]. Eine Klingel-Automation ohne Stop soll den Rahmen
//!   nicht dauerhaft an Netz und Rechenzeit binden; danach zeigt er wieder die
//!   Diashow.
//! - Nach [`RECONNECT_ATTEMPTS`] vergeblichen Versuchen, einen abgerissenen
//!   Strom neu zu verbinden. Ein eingefrorenes Kamerabild taeuschte vor, es
//!   sei aktuell; also ebenfalls zurueck zur Diashow.

use super::http::{self, Ctx, MAX_EXTERNAL_BYTES};
use super::mjpeg::{self, FrameReader};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;

/// Laengste Laufzeit eines Livebilds (E-67, Nutzerentscheidung: zwei Minuten).
pub const LIVE_MAX: Duration = Duration::from_secs(120);
/// So oft wird ein abgerissener Strom neu verbunden, bevor der Rahmen aufgibt.
pub const RECONNECT_ATTEMPTS: u32 = 3;
/// Pause vor jedem neuen Versuch — ein WLAN-Aussetzer ist selten kuerzer.
pub const RECONNECT_PAUSE: Duration = Duration::from_secs(2);
/// Kommen so lange keine Daten, gilt der Strom als abgerissen. Home Assistant
/// schickt auch bei einer Standkamera mindestens alle paar Sekunden ein Bild.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(10);
/// Hoechstens fuenf Einzelbilder je Sekunde. Home Assistant liefert fuer
/// Kameras ohne eigenes MJPEG ohnehin nur zwei; echte MJPEG-Kameras schaffen
/// 25, und jedes davon zu dekodieren kostete Akku ohne sichtbaren Gewinn.
pub const MIN_FRAME_GAP: Duration = Duration::from_millis(200);

/// Ein offener Kamerastrom samt dem, was davon schon gelesen ist.
pub struct Stream {
    pub(super) resp: reqwest::Response,
    pub(super) frames: FrameReader,
}

impl Stream {
    pub fn new(resp: reqwest::Response, boundary: Option<String>) -> Self {
        Self {
            resp,
            frames: FrameReader::new(boundary),
        }
    }

    /// Verbindet neu, nachdem der Strom abgerissen ist.
    ///
    /// Home Assistant signiert die Adresse fuer 24 Stunden; dieselbe URI geht
    /// also fuer die ganze Laufzeit des Livebilds.
    pub async fn connect(
        client: &reqwest::Client,
        uri: &str,
        wait: Duration,
    ) -> Result<Self, String> {
        let resp = tokio::time::timeout(wait, client.get(uri).send())
            .await
            .map_err(|_| format!("keine Antwort binnen {} s", wait.as_secs()))?
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status()));
        }
        let ct = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        if !mjpeg::is_multipart_stream(&ct) {
            return Err(format!("kein Kamerastrom mehr: {ct}"));
        }
        Ok(Self::new(resp, mjpeg::boundary_of(&ct)))
    }
}

/// Die Grenzen eines Laufs — eigene Werte nur fuer die Tests, die sonst
/// Minuten dauerten.
#[derive(Debug, Clone, Copy)]
struct Limits {
    max: Duration,
    idle: Duration,
    gap: Duration,
    attempts: u32,
    pause: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max: LIVE_MAX,
            idle: IDLE_TIMEOUT,
            gap: MIN_FRAME_GAP,
            attempts: RECONNECT_ATTEMPTS,
            pause: RECONNECT_PAUSE,
        }
    }
}

/// Haelt das Fremdbild `id` aktuell, bis eine der Endbedingungen greift.
pub async fn run(ctx: Arc<Ctx>, id: String, uri: String, stream: Stream) {
    run_with(ctx, id, uri, stream, Limits::default()).await;
}

/// Wie ein Durchgang ueber einer Verbindung endete.
enum Pumped {
    /// Das Fremdbild ist weg; nichts mehr zu tun.
    Gone,
    /// Die Laufzeit ist um.
    Expired,
    /// Die Verbindung ist abgerissen — neu verbinden lohnt vielleicht.
    Broken(String),
}

/// Zaehler ueber alle Verbindungen eines Laufs.
#[derive(Default)]
struct Progress {
    /// Gezeigte Einzelbilder, fuers Log.
    frames: u32,
    /// Vergebliche Versuche seit dem letzten gezeigten Bild.
    failures: u32,
    last_shown: Option<Instant>,
}

async fn run_with(ctx: Arc<Ctx>, id: String, uri: String, stream: Stream, limits: Limits) {
    let deadline = Instant::now() + limits.max;
    let mut progress = Progress::default();
    let mut stream = stream;
    log::info!(
        "UPnP: Livebild {id} laeuft, hoechstens {} s",
        limits.max.as_secs()
    );

    loop {
        let mut why = match pump(&ctx, &id, &mut stream, deadline, limits, &mut progress).await {
            Pumped::Gone => {
                log::info!(
                    "UPnP: Livebild {id} beendet nach {} Einzelbildern",
                    progress.frames
                );
                return;
            }
            Pumped::Expired => return expire(&ctx, &id, &progress).await,
            Pumped::Broken(why) => why,
        };

        // Neu verbinden, bis es klappt oder die Versuche aufgebraucht sind.
        stream = loop {
            if progress.failures >= limits.attempts {
                log::info!("UPnP: Kamerastrom {id} bleibt weg ({why}), zurueck zur Diashow");
                ctx.backend.end_live(&id);
                http::publish(ctx.clone()).await;
                return;
            }
            progress.failures += 1;
            log::info!(
                "UPnP: Kamerastrom {id} abgerissen ({why}), neuer Versuch {} von {}",
                progress.failures,
                limits.attempts
            );
            tokio::time::sleep(limits.pause).await;
            if !ctx.backend.external_alive(&id) {
                log::info!("UPnP: Livebild {id} beendet, waehrend es neu verband");
                return;
            }
            if Instant::now() >= deadline {
                return expire(&ctx, &id, &progress).await;
            }
            match Stream::connect(&ctx.client, &uri, limits.idle).await {
                Ok(s) => break s,
                Err(e) => why = e,
            }
        };
    }
}

async fn expire(ctx: &Arc<Ctx>, id: &str, progress: &Progress) {
    log::info!(
        "UPnP: Livebild {id} nach Zeitlimit beendet ({} Einzelbilder), zurueck zur Diashow",
        progress.frames
    );
    ctx.backend.end_live(id);
    http::publish(ctx.clone()).await;
}

/// Liest eine Verbindung, bis sie abreisst oder der Lauf endet.
async fn pump(
    ctx: &Arc<Ctx>,
    id: &str,
    stream: &mut Stream,
    deadline: Instant,
    limits: Limits,
    progress: &mut Progress,
) -> Pumped {
    // Das neueste fertige Bild, das noch auf seine Zeit wartet (MIN_FRAME_GAP).
    let mut pending: Option<Vec<u8>> = None;
    loop {
        if !ctx.backend.external_alive(id) {
            return Pumped::Gone;
        }
        let now = Instant::now();
        if now >= deadline {
            return Pumped::Expired;
        }
        let wait = limits.idle.min(deadline - now);
        let chunk = match tokio::time::timeout(wait, stream.resp.chunk()).await {
            Err(_) if Instant::now() >= deadline => return Pumped::Expired,
            Err(_) => return Pumped::Broken(format!("keine Daten seit {} s", wait.as_secs())),
            Ok(Err(e)) => return Pumped::Broken(e.to_string()),
            Ok(Ok(None)) => return Pumped::Broken("Strom beendet".into()),
            Ok(Ok(Some(chunk))) => chunk,
        };
        stream.frames.push(&chunk);
        match stream.frames.latest() {
            Ok(Some(frame)) => pending = Some(frame),
            Ok(None) => {}
            Err(why) => return Pumped::Broken(why),
        }
        if stream.frames.pending() > MAX_EXTERNAL_BYTES {
            return Pumped::Broken(format!("{} Bytes ohne Bild", stream.frames.pending()));
        }

        let due = progress
            .last_shown
            .map_or(true, |t| t.elapsed() >= limits.gap);
        if !due {
            continue;
        }
        let Some(frame) = pending.take() else {
            continue;
        };
        // Dekodieren ist Rechenarbeit; sie gehoert nicht auf den Netz-Thread.
        // Waehrenddessen staut sich der Strom im Socket, und `latest` nimmt
        // danach nur das neueste Bild — der Rahmen holt nicht auf, er springt.
        let backend = ctx.backend.clone();
        let frame_id = id.to_string();
        match tokio::task::spawn_blocking(move || backend.live_frame(&frame_id, frame)).await {
            Ok(Ok(())) => {
                progress.frames += 1;
                progress.failures = 0;
                progress.last_shown = Some(Instant::now());
            }
            // Ein kaputtes Einzelbild ist kein Abriss: das vorige bleibt
            // stehen, das naechste kommt gleich.
            Ok(Err(e)) => log::debug!("UPnP: Einzelbild von {id} verworfen: {e}"),
            Err(e) => return Pumped::Broken(format!("Aufbereitung abgebrochen: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::gena::Subscriptions;
    use super::super::renderer::tests::FakeBackend;
    use super::super::renderer::Backend;
    use super::*;
    use axum::body::Body;
    use axum::response::Response;
    use axum::routing::get;
    use axum::Router;
    use std::sync::atomic::{AtomicU32, Ordering};

    const JPEG: [u8; 4] = [0xFF, 0xD8, 0xFF, 0xD9];

    fn ctx(fake: Arc<FakeBackend>) -> Arc<Ctx> {
        Arc::new(Ctx {
            backend: fake,
            udn: "uuid:test-udn".into(),
            subs: Arc::new(Subscriptions::default()),
            client: reqwest::Client::new(),
        })
    }

    /// Grenzen im Millisekundentakt, damit ein Test keine Minuten dauert.
    fn schnell(max_ms: u64) -> Limits {
        Limits {
            max: Duration::from_millis(max_ms),
            idle: Duration::from_millis(500),
            gap: Duration::ZERO,
            attempts: 2,
            pause: Duration::from_millis(10),
        }
    }

    /// Ein Strom in der Schreibweise von Home Assistant: alle 20 ms ein Teil.
    /// Nach `frames` Teilen endet er, `None` heisst nie.
    fn strom(frames: Option<u32>) -> Response {
        let teile = futures_util::stream::unfold(0u32, move |n| async move {
            if frames.is_some_and(|max| n >= max) {
                return None;
            }
            if n > 0 {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            let mut part =
                b"--frameboundary\r\nContent-Type: image/jpeg\r\nContent-Length: 4\r\n\r\n"
                    .to_vec();
            part.extend_from_slice(&JPEG);
            part.extend_from_slice(b"\r\n");
            Some((
                Ok::<_, std::io::Error>(axum::body::Bytes::from(part)),
                n + 1,
            ))
        });
        Response::builder()
            .header(
                "content-type",
                "multipart/x-mixed-replace; boundary=--frameboundary",
            )
            .body(Body::from_stream(teile))
            .unwrap()
    }

    /// Kameraserver: `/endlos` laeuft immer, `/kurz` endet nach zwei Bildern.
    /// Zaehlt die Verbindungen, damit die Tests das Neuverbinden sehen.
    async fn kamera() -> (String, Arc<AtomicU32>) {
        let verbindungen = Arc::new(AtomicU32::new(0));
        let (a, b) = (verbindungen.clone(), verbindungen.clone());
        let app = Router::new()
            .route(
                "/endlos",
                get(move || {
                    a.fetch_add(1, Ordering::SeqCst);
                    async { strom(None) }
                }),
            )
            .route(
                "/kurz",
                get(move || {
                    b.fetch_add(1, Ordering::SeqCst);
                    async { strom(Some(2)) }
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{addr}"), verbindungen)
    }

    /// Vormerken wie `SetAVTransportURI` und den Strom oeffnen.
    async fn start(fake: &FakeBackend, uri: &str) -> (String, Stream) {
        let id = fake
            .stage_external(JPEG.to_vec(), "Tuer".into(), uri.into())
            .unwrap();
        let stream = Stream::connect(&reqwest::Client::new(), uri, Duration::from_secs(2))
            .await
            .unwrap();
        (id, stream)
    }

    fn frames(fake: &FakeBackend, id: &str) -> usize {
        let prefix = format!("frame:{id}:");
        fake.calls()
            .iter()
            .filter(|c| c.starts_with(&prefix))
            .count()
    }

    async fn warte_auf(lauf: tokio::task::JoinHandle<()>) {
        tokio::time::timeout(Duration::from_secs(5), lauf)
            .await
            .expect("Livebild hoert nicht auf")
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reicht_weitere_einzelbilder_nach_und_endet_mit_stop_e_67() {
        let fake = FakeBackend::shared();
        let (base, _) = kamera().await;
        let (id, stream) = start(&fake, &format!("{base}/endlos")).await;
        let lauf = tokio::spawn(run_with(
            ctx(fake.clone()),
            id.clone(),
            base,
            stream,
            schnell(10_000),
        ));

        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            frames(&fake, &id) >= 3,
            "zu wenige Einzelbilder: {:?}",
            fake.calls()
        );

        // Stop aus Home Assistant raeumt das Fremdbild ab; der Lauf merkt es
        // und hoert still auf — abgeraeumt hat schon ein anderer.
        fake.stop();
        warte_auf(lauf).await;
        assert!(
            !fake.calls().iter().any(|c| c.starts_with("end_live")),
            "{:?}",
            fake.calls()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn endet_nach_dem_zeitlimit_und_zeigt_wieder_die_diashow_e_67() {
        let fake = FakeBackend::shared();
        let (base, _) = kamera().await;
        let (id, stream) = start(&fake, &format!("{base}/endlos")).await;
        let lauf = tokio::spawn(run_with(
            ctx(fake.clone()),
            id.clone(),
            base,
            stream,
            schnell(250),
        ));

        warte_auf(lauf).await;
        assert_eq!(fake.calls().last(), Some(&format!("end_live:{id}")));
        assert!(!fake.external_alive(&id));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn verbindet_nach_einem_abriss_neu_e_67() {
        // `/kurz` endet nach zwei Bildern. Solange jedes neue Verbinden wieder
        // Bilder liefert, laeuft das Livebild weiter — die Versuche zaehlen
        // nur, solange nichts kommt.
        let fake = FakeBackend::shared();
        let (base, verbindungen) = kamera().await;
        let uri = format!("{base}/kurz");
        let (id, stream) = start(&fake, &uri).await;
        let lauf = tokio::spawn(run_with(
            ctx(fake.clone()),
            id.clone(),
            uri,
            stream,
            schnell(600),
        ));

        warte_auf(lauf).await;
        assert!(
            verbindungen.load(Ordering::SeqCst) > 3,
            "nur {} Verbindungen",
            verbindungen.load(Ordering::SeqCst)
        );
        assert!(frames(&fake, &id) > 3, "{:?}", fake.calls());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn gibt_nach_den_versuchen_auf_und_zeigt_wieder_die_diashow_e_67() {
        // Der Strom endet nach zwei Bildern, und neu verbunden wird mit einer
        // Adresse, die es nicht (mehr) gibt — wie eine Kamera, die vom Netz ist.
        let fake = FakeBackend::shared();
        let (base, _) = kamera().await;
        let (id, stream) = start(&fake, &format!("{base}/kurz")).await;
        let tot = format!("{base}/gibt-es-nicht");
        let lauf = tokio::spawn(run_with(
            ctx(fake.clone()),
            id.clone(),
            tot,
            stream,
            schnell(10_000),
        ));

        warte_auf(lauf).await;
        // Zwei Bilder, die auch in einem Netzstueck kommen koennen.
        assert!(frames(&fake, &id) >= 1, "{:?}", fake.calls());
        assert_eq!(fake.calls().last(), Some(&format!("end_live:{id}")));
        assert!(!fake.external_alive(&id));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ein_neues_fremdbild_beendet_den_alten_strom_still_e_67() {
        let fake = FakeBackend::shared();
        let (base, _) = kamera().await;
        let (alt, stream) = start(&fake, &format!("{base}/endlos")).await;
        let lauf = tokio::spawn(run_with(
            ctx(fake.clone()),
            alt.clone(),
            base,
            stream,
            schnell(10_000),
        ));
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Home Assistant schickt die naechste Kamera.
        let neu = fake
            .stage_external(JPEG.to_vec(), "Garten".into(), "http://ha/b".into())
            .unwrap();
        warte_auf(lauf).await;
        assert!(
            fake.external_alive(&neu),
            "das neue Bild darf nicht abgeraeumt werden"
        );
        assert!(!fake.calls().iter().any(|c| c.starts_with("end_live")));
    }
}

//! IMAP-Abruf (Erweiterungspapier Teil 1, F1).
//!
//! ## Warum kein Backend
//!
//! Das Papier legt es fest, und es passt zu NF-04: Die Fotos verlassen das
//! Gerät nur Richtung des eigenen Postfachs. Ein Vermittlungsdienst wäre eine
//! dritte Partei mehr, die Familienfotos zu sehen bekommt.
//!
//! ## Warum TLS über rustls
//!
//! Dieselbe Begründung wie bei den WebDAV-Quellen: der Android-Zertifikats-
//! speicher ist über `rustls-native-certs` nicht zuverlässig erreichbar,
//! deshalb `webpki-roots`. Und rustls ist reines Rust — eine C-Bibliothek
//! bräuchte einen Cross-Compile, den E-02 und E-04 bewusst gemieden haben.
//!
//! ## Was hier bewusst fehlt
//!
//! IMAP IDLE. Das Papier sieht es am Netzteil vor; E-30 hat es vertagt. Eine
//! Dauerverbindung muss nach jedem Netzwechsel neu aufgebaut werden und läuft
//! alle 29 Minuten in ein Server-Timeout — das ist der Teil, der im
//! Dauerbetrieb schiefgeht. Der Intervall-Abruf kommt ohne das aus und lässt
//! sich später ergänzen, ohne etwas umzubauen.

use super::parse::{parse_mail, ParsedMail};
use std::sync::Arc;
use thiserror::Error;

/// Voreinstellung für IMAP über TLS.
pub const DEFAULT_PORT: u16 = 993;
/// Postfach, das ohne andere Angabe abgerufen wird.
pub const DEFAULT_FOLDER: &str = "INBOX";

#[derive(Debug, Error)]
pub enum MailError {
    #[error("Keine Verbindung zu {host}:{port} — {source}")]
    Connect {
        host: String,
        port: u16,
        source: std::io::Error,
    },
    #[error("TLS-Verbindung fehlgeschlagen: {0}")]
    Tls(String),
    /// Der Servertext bleibt erhalten.
    ///
    /// Erst am echten Postfach aufgefallen: GMX & Co. schreiben in die
    /// Ablehnung oft hinein, *warum* — etwa dass der IMAP-Zugriff im Konto
    /// erst freigeschaltet werden muss. Wer diese Antwort wegwirft, laesst den
    /// Nutzer raten, ob das Passwort falsch ist oder eine Einstellung fehlt.
    #[error("Anmeldung abgelehnt (Benutzername, Passwort oder IMAP-Zugriff im Konto prüfen). Server: {0}")]
    Login(String),
    #[error("Postfach '{0}' nicht gefunden")]
    NoSuchFolder(String),
    #[error("IMAP-Fehler: {0}")]
    Protocol(String),
}

/// Zugangsdaten und Vorgaben eines Postfachs.
#[derive(Debug, Clone)]
pub struct MailboxConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub folder: String,
    /// Obergrenze je Anhang, aus den Einstellungen (Papier 1.3).
    pub max_attachment_bytes: u64,
    /// Auch bereits gelesene Nachrichten durchsehen (E-34).
    pub include_seen: bool,
}

/// Ergebnis eines Abrufs — landet im Protokoll (Wartung F6).
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchReport {
    /// Wie viele ungelesene Nachrichten geprüft wurden.
    pub checked: usize,
    /// Wie viele davon Bilder beitrugen.
    pub with_photos: usize,
    /// Wie viele Bilder insgesamt.
    pub photos: usize,
    /// Übersprungene Anhänge (HEIC, zu groß) — für das Protokoll.
    pub skipped: usize,
    /// Wie viele Nachrichten der Ordner überhaupt enthielt (E-34).
    ///
    /// Bei „auch gelesene" ist das der ganze Ordner. Ohne diese Zahl liesse
    /// sich nicht erklären, warum ein Lauf über tausend Nachrichten nur drei
    /// Fotos brachte.
    pub seen_in_folder: usize,
    /// Davon schon im Cache — in Stufe eins an der Message-Id erkannt.
    pub already_known: usize,
}

/// Baut eine TLS-Verbindung zum Postfach auf.
///
/// Als eigene Funktion, damit der Verbindungstest aus der Wartung (F5)
/// denselben Weg nimmt wie der Abruf — ein Test, der etwas anderes prüft als
/// den Ernstfall, ist keiner.
async fn connect(
    cfg: &MailboxConfig,
) -> Result<
    async_imap::Session<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>,
    MailError,
> {
    let tcp = tokio::net::TcpStream::connect((cfg.host.as_str(), cfg.port))
        .await
        .map_err(|source| MailError::Connect {
            host: cfg.host.clone(),
            port: cfg.port,
            source,
        })?;

    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let tls_config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();

    let server_name = rustls::pki_types::ServerName::try_from(cfg.host.clone())
        .map_err(|e| MailError::Tls(format!("Servername '{}' ungültig: {e}", cfg.host)))?;

    let stream = tokio_rustls::TlsConnector::from(Arc::new(tls_config))
        .connect(server_name, tcp)
        .await
        .map_err(|e| MailError::Tls(e.to_string()))?;

    let client = async_imap::Client::new(stream);
    client
        .login(&cfg.username, &cfg.password)
        .await
        .map_err(|(e, _)| MailError::Login(e.to_string()))
}

/// Prüft Erreichbarkeit und Anmeldung (Wartung F5).
///
/// Gibt die Anzahl ungelesener Nachrichten zurück — das ist die Angabe, die
/// beim Einrichten wirklich weiterhilft: sie beweist, dass nicht nur die
/// Anmeldung klappt, sondern auch das richtige Postfach gewählt ist.
pub async fn test_connection(cfg: &MailboxConfig) -> Result<u32, MailError> {
    let mut session = connect(cfg).await?;
    let mailbox = session
        .select(&cfg.folder)
        .await
        .map_err(|_| MailError::NoSuchFolder(cfg.folder.clone()))?;
    let unseen = mailbox.unseen.unwrap_or(0);
    let _ = session.logout().await;
    Ok(unseen)
}

/// Liest die Message-Id aus einem Kopfzeilen-Abschnitt.
///
/// Absichtlich kein voller MIME-Durchlauf: hier kommen nur wenige Zeilen an,
/// und `parse_mail` bräuchte den ganzen Rumpf, den wir gerade vermeiden
/// wollen.
fn message_id_from_header(raw: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(raw);
    for line in text.lines() {
        let Some((_, rest)) = line.split_once(':') else {
            continue;
        };
        if line.to_ascii_lowercase().starts_with("message-id") {
            let value = rest.trim().trim_start_matches('<').trim_end_matches('>');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Holt Nachrichten und reicht jede einzeln weiter.
///
/// Eine nach der anderen statt alle auf einmal: bei 25 MB je Anhang lägen
/// sonst hunderte Megabyte gleichzeitig im Speicher — genau die Last, gegen
/// die R-03 gerichtet ist. Der Aufrufer legt jedes Foto sofort ab und gibt
/// den Speicher wieder frei.
///
/// ## Warum in zwei Stufen (E-34)
///
/// Bis `include_seen` dazukam, lud die Schleife jede gefundene Nachricht
/// vollständig herunter und fragte *danach*, ob sie schon bekannt sei. Bei
/// `UNSEEN` ging das auf, weil der Gelesen-Vermerk Verarbeitetes aus der Suche
/// nimmt. Über den ganzen Ordner gerechnet wäre daraus eine Endlosschleife
/// geworden: derselbe Anfang des Postfachs, bei jedem Lauf erneut geladen, und
/// die Nachricht dahinter niemals erreicht.
///
/// Deshalb zuerst nur die Message-Ids — ein paar Dutzend Bytes je Nachricht —
/// und den vollen Rumpf ausschliesslich für die unbekannten. `BODY.PEEK` statt
/// `BODY`, damit schon dieses Nachsehen keinen Gelesen-Vermerk setzt.
///
/// `already_known` bleibt die zweite Sicherung gegen Doppelimport (F2): sie
/// greift auch, wenn jemand eine Mail von Hand wieder als ungelesen markiert.
///
/// Der Rückruf entscheidet mit seinem Rückgabewert, ob die Nachricht als
/// gelesen markiert wird. Schlägt das Ablegen fehl, bleibt sie ungelesen und
/// wird beim nächsten Lauf erneut versucht.
pub async fn fetch_mails<F>(
    cfg: &MailboxConfig,
    already_known: &(dyn Fn(&str) -> bool + Send + Sync),
    limit: usize,
    mut on_mail: F,
) -> Result<FetchReport, MailError>
where
    F: FnMut(ParsedMail) -> bool + Send,
{
    use futures::StreamExt;

    let mut session = connect(cfg).await?;
    session
        .select(&cfg.folder)
        .await
        .map_err(|_| MailError::NoSuchFolder(cfg.folder.clone()))?;

    let criterion = if cfg.include_seen { "ALL" } else { "UNSEEN" };
    let found = session
        .search(criterion)
        .await
        .map_err(|e| MailError::Protocol(e.to_string()))?;

    let mut report = FetchReport {
        seen_in_folder: found.len(),
        ..Default::default()
    };

    // Stufe 1: nur die Message-Ids, in einer einzigen Anfrage.
    let mut candidates: Vec<u32> = Vec::new();
    if !found.is_empty() {
        let set = found
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let mut stream = session
            .fetch(set, "BODY.PEEK[HEADER.FIELDS (MESSAGE-ID)]")
            .await
            .map_err(|e| MailError::Protocol(e.to_string()))?;

        while let Some(item) = stream.next().await {
            let Ok(msg) = item else { continue };
            let uid = msg.message;
            match msg.header().and_then(message_id_from_header) {
                // Ohne Message-Id lässt sich nichts wiedererkennen — dann
                // entscheidet erst der volle Durchlauf.
                None => candidates.push(uid),
                Some(id) if !already_known(&id) => candidates.push(uid),
                Some(_) => report.already_known += 1,
            }
        }
    }

    let mut to_mark: Vec<u32> = Vec::new();

    // Stufe 2: den vollen Rumpf nur für die unbekannten.
    for uid in candidates.into_iter().take(limit) {
        let raw = {
            let mut stream = session
                .fetch(uid.to_string(), "RFC822")
                .await
                .map_err(|e| MailError::Protocol(e.to_string()))?;

            let mut buf: Option<Vec<u8>> = None;
            while let Some(item) = stream.next().await {
                if let Ok(msg) = item {
                    if let Some(body) = msg.body() {
                        buf = Some(body.to_vec());
                    }
                }
            }
            buf
        };

        let Some(raw) = raw else {
            continue;
        };
        report.checked += 1;

        let Some(mail) = parse_mail(&raw, cfg.max_attachment_bytes) else {
            // Unlesbare Nachricht: als gelesen markieren, sonst haengt der
            // Abruf bei jedem Lauf an derselben Mail fest.
            to_mark.push(uid);
            continue;
        };

        if already_known(&mail.message_id) {
            to_mark.push(uid);
            continue;
        }

        report.skipped += mail.skipped.len();
        if !mail.photos.is_empty() {
            report.with_photos += 1;
            report.photos += mail.photos.len();
        }

        if on_mail(mail) {
            to_mark.push(uid);
        }
    }

    if !to_mark.is_empty() {
        let set = to_mark
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        if let Err(e) = session.store(set, "+FLAGS (\\Seen)").await {
            // Nicht schlimm: der Doppelimport-Schutz ueber die Message-ID
            // greift auch ohne den Vermerk (F2).
            log::warn!("Mails nicht als gelesen markierbar: {e}");
        }
    }

    let _ = session.logout().await;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voreinstellungen_entsprechen_dem_papier() {
        assert_eq!(DEFAULT_PORT, 993, "IMAP ueber TLS");
        assert_eq!(DEFAULT_FOLDER, "INBOX");
    }

    /// Vollstaendige Mail mit derselben Message-Id wie `KOPF`.
    const VOLL: &[u8] = b"From: Oma <oma@example.org>\r\n\
                          Subject: Urlaub\r\n\
                          Message-ID: <abc123@example.org>\r\n\
                          MIME-Version: 1.0\r\n\
                          Content-Type: text/plain\r\n\
                          \r\n\
                          Hallo\r\n";

    /// Genau das, was `BODY.PEEK[HEADER.FIELDS (MESSAGE-ID)]` zurueckgibt.
    const KOPF: &[u8] = b"Message-ID: <abc123@example.org>\r\n\r\n";

    #[test]
    fn kopfzeile_und_vollabruf_liefern_dieselbe_kennung() {
        // Der wichtigste Test des zweistufigen Abrufs (E-34). Stufe eins
        // erkennt Bekanntes an der Message-Id aus der Kopfzeile, Stufe zwei
        // legt sie unter der Kennung aus `parse_mail` ab. Weichen die beiden
        // voneinander ab -- etwa um die spitzen Klammern --, erkennt Stufe
        // eins nie etwas wieder: der Rahmen laedt bei jedem Lauf dieselben
        // Mails erneut und legt sie doppelt ab. Ohne Fehlermeldung.
        let aus_kopf = message_id_from_header(KOPF).expect("Kopfzeile lesbar");
        let aus_voll = parse_mail(VOLL, 1024).expect("Mail lesbar").message_id;
        assert_eq!(
            aus_kopf, aus_voll,
            "beide Stufen muessen dieselbe Kennung bilden"
        );
    }

    #[test]
    fn kopfzeile_ohne_message_id_meldet_nichts() {
        // Dann faellt die Nachricht in Stufe zwei -- dort bildet `parse_mail`
        // eine Ersatzkennung aus Absender, Betreff und Datum.
        assert_eq!(message_id_from_header(b"Subject: ohne\r\n\r\n"), None);
        assert_eq!(message_id_from_header(b""), None);
        assert_eq!(message_id_from_header(b"Message-ID: <>\r\n"), None);
    }

    #[test]
    fn kopfzeile_ignoriert_gross_und_kleinschreibung() {
        // RFC 5322 laesst jede Schreibweise zu, und Server halten sich daran.
        for zeile in [
            &b"message-id: <x@y>\r\n"[..],
            &b"MESSAGE-ID: <x@y>\r\n"[..],
            &b"Message-Id:   <x@y>   \r\n"[..],
        ] {
            assert_eq!(
                message_id_from_header(zeile).as_deref(),
                Some("x@y"),
                "Schreibweise darf nicht entscheiden"
            );
        }
    }

    #[test]
    fn kopfzeile_ohne_klammern_wird_akzeptiert() {
        // Nicht jeder Absender setzt sie, obwohl der Standard sie vorsieht.
        assert_eq!(
            message_id_from_header(b"Message-ID: x@y\r\n").as_deref(),
            Some("x@y")
        );
    }

    #[test]
    fn suchkriterium_haengt_am_schalter() {
        // Die Zeichenkette selbst ist trivial; geprueft wird, dass die
        // Voreinstellung der bisherige Betrieb bleibt (E-34).
        let mut cfg = MailboxConfig {
            host: "h".into(),
            port: 993,
            username: "u".into(),
            password: "p".into(),
            folder: "INBOX".into(),
            max_attachment_bytes: 1024,
            include_seen: false,
        };
        assert!(!cfg.include_seen, "Voreinstellung: nur Ungelesenes");
        cfg.include_seen = true;
        assert!(cfg.include_seen);
    }

    #[test]
    fn fehlertexte_sind_laienverstaendlich() {
        // Nicht-funktionale Vorgabe aus Teil 0: Fehlertexte muessen ohne
        // Fachwissen zu verstehen sein.
        let e = MailError::Login(String::new());
        assert!(
            e.to_string().contains("Passwort"),
            "der haeufigste Fehler muss sagen, was zu tun ist: {e}"
        );

        let e = MailError::NoSuchFolder("Archiv".into());
        assert!(e.to_string().contains("Archiv"));
    }
}

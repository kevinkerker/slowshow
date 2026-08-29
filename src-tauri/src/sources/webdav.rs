//! WebDAV-Client für NAS-Quellen (FA-21).
//!
//! SMB wurde per E-02 gestrichen, WebDAV ist damit der einzige Weg zum NAS.
//! Der XML-Parser ist von der Netzwerkschicht getrennt, damit sich das
//! Auswerten echter Serverantworten ohne NAS testen lässt — genau dort sitzen
//! die Unterschiede zwischen Nextcloud, Synology und ownCloud.

use percent_encoding::percent_decode_str;
use quick_xml::events::Event;
use quick_xml::Reader;

#[derive(Debug, thiserror::Error)]
pub enum DavError {
    #[error("Netzwerkfehler: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Server antwortete mit {status}")]
    Status { status: u16 },
    #[error("Antwort des Servers war kein gültiges WebDAV-XML: {0}")]
    Xml(String),
    #[error("Ungültige Adresse: {0}")]
    Url(String),
}

/// Ein Eintrag aus einer PROPFIND-Antwort.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DavEntry {
    /// Serverseitiger Pfad, bereits prozent-dekodiert.
    pub href: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub etag: Option<String>,
    /// Änderungsdatum als Unix-Zeitstempel.
    pub mtime: Option<i64>,
    /// Nextcloud-spezifisch: `oc:fileid` für die Preview-API (FA-23).
    pub file_id: Option<String>,
}

impl DavEntry {
    /// Letztes Pfadsegment.
    pub fn name(&self) -> &str {
        self.href
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("")
    }
}

/// PROPFIND-Rumpf. Fragt gezielt die Eigenschaften ab, die der Delta-Abgleich
/// braucht (NF-14) — `allprop` würde bei großen Alben unnötig viel liefern.
const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
  <d:prop>
    <d:getcontentlength/>
    <d:getlastmodified/>
    <d:getetag/>
    <d:resourcetype/>
    <oc:fileid/>
  </d:prop>
</d:propfind>"#;

/// Entfernt den Namensraum-Präfix (`d:getetag` -> `getetag`).
fn local_name(raw: &[u8]) -> String {
    let s = String::from_utf8_lossy(raw);
    s.rsplit(':').next().unwrap_or(&s).to_ascii_lowercase()
}

/// Wandelt ein HTTP-Datum in einen Unix-Zeitstempel.
///
/// WebDAV liefert `getlastmodified` als RFC-1123-Datum. Manche NAS-Systeme
/// weichen ab, deshalb ein zweiter Versuch mit ISO-8601.
pub fn parse_http_date(s: &str) -> Option<i64> {
    let s = s.trim();
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(s) {
        return Some(dt.timestamp());
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp());
    }
    // "Sun, 15 Jun 2025 18:22:31 GMT" ohne RFC-2822-konforme Zone.
    chrono::NaiveDateTime::parse_from_str(s, "%a, %d %b %Y %H:%M:%S GMT")
        .ok()
        .map(|n| n.and_utc().timestamp())
}

/// Zerlegt eine `multistatus`-Antwort.
///
/// Bewusst tolerant: unbekannte Elemente werden übersprungen, fehlende
/// Eigenschaften bleiben `None`. Ein Server, der `getetag` nicht liefert, muss
/// weiterhin funktionieren — der Delta-Abgleich fällt dann auf Größe und
/// Datum zurück (NF-14).
pub fn parse_multistatus(xml: &str) -> Result<Vec<DavEntry>, DavError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut entries = Vec::new();
    let mut current: Option<DavEntry> = None;
    let mut field: Option<String> = None;
    // <d:collection/> innerhalb von <resourcetype> markiert einen Ordner.
    let mut in_resourcetype = false;
    // Offene Elemente. Am Dateiende muss die Bilanz null sein — sonst war die
    // Antwort abgeschnitten.
    //
    // Das ist kein Schönheitsfehler: eine halb übertragene Antwort ergäbe eine
    // zu kurze Dateiliste, und `Cache::remove_missing` würde daraufhin alle
    // nicht genannten Bilder löschen. Eine abgebrochene Verbindung darf den
    // Cache nicht leeren (FA-26).
    let mut depth: i32 = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                depth += 1;
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "response" => current = Some(DavEntry::default()),
                    "resourcetype" => in_resourcetype = true,
                    "href" | "getcontentlength" | "getlastmodified" | "getetag" | "fileid" => {
                        field = Some(name)
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let name = local_name(e.name().as_ref());
                if name == "collection" && in_resourcetype {
                    if let Some(c) = current.as_mut() {
                        c.is_dir = true;
                    }
                }
            }
            Ok(Event::Text(t)) => {
                let Some(f) = field.as_deref() else { continue };
                let Some(c) = current.as_mut() else { continue };
                let value = t.unescape().unwrap_or_default().trim().to_string();
                match f {
                    "href" => {
                        c.href = percent_decode_str(&value).decode_utf8_lossy().to_string();
                        c.is_dir = c.is_dir || c.href.ends_with('/');
                    }
                    "getcontentlength" => c.size = value.parse().ok(),
                    "getlastmodified" => c.mtime = parse_http_date(&value),
                    // Anführungszeichen und der W/-Präfix schwacher ETags stören
                    // den Vergleich, deshalb hier normalisieren.
                    "getetag" => {
                        let cleaned = value.trim_start_matches("W/").trim_matches('"').to_string();
                        if !cleaned.is_empty() {
                            c.etag = Some(cleaned);
                        }
                    }
                    "fileid" if !value.is_empty() => c.file_id = Some(value),
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                depth -= 1;
                let name = local_name(e.name().as_ref());
                match name.as_str() {
                    "response" => {
                        if let Some(c) = current.take() {
                            if !c.href.is_empty() {
                                entries.push(c);
                            }
                        }
                    }
                    "resourcetype" => in_resourcetype = false,
                    _ => field = None,
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DavError::Xml(e.to_string())),
            _ => {}
        }
    }

    if depth != 0 {
        return Err(DavError::Xml(format!(
            "Antwort war unvollständig ({depth} offene Elemente)"
        )));
    }
    Ok(entries)
}

/// Pfadanteil einer URL, immer mit abschließendem `/`.
///
/// Grundlage für das Kürzen der zurückgelieferten hrefs auf quellrelative Pfade.
pub fn base_path(url: &str) -> Result<String, DavError> {
    let without_scheme = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .ok_or_else(|| DavError::Url(format!("Schema fehlt: {url}")))?;
    let path = match without_scheme.find('/') {
        Some(i) => &without_scheme[i..],
        None => "/",
    };
    let path = percent_decode_str(path).decode_utf8_lossy().to_string();
    Ok(if path.ends_with('/') {
        path
    } else {
        format!("{path}/")
    })
}

/// Kürzt einen href auf den Pfad relativ zur Quellenwurzel.
///
/// Gibt `None`, wenn der href außerhalb der Wurzel liegt — das schützt davor,
/// dass ein Server durch manipulierte hrefs Einträge außerhalb des
/// konfigurierten Ordners einschleust.
pub fn relative_path(href: &str, base: &str) -> Option<String> {
    let rel = href.strip_prefix(base)?;
    let rel = rel.trim_end_matches('/');
    if rel.is_empty() || rel.contains("..") {
        None
    } else {
        Some(rel.to_string())
    }
}

// ── Netzwerkschicht ──────────────────────────────────────────────────────────

pub struct WebDavClient {
    base_url: String,
    username: String,
    password: String,
    http: reqwest::Client,
}

impl WebDavClient {
    pub fn new(
        base_url: &str,
        username: &str,
        password: &str,
        allow_insecure_tls: bool,
    ) -> Result<Self, DavError> {
        let http = reqwest::Client::builder()
            // Ein NAS im Heimnetz kann langsam sein; 60 s sind großzügig genug,
            // ohne dass ein hängender Server den Sync ewig blockiert.
            .timeout(std::time::Duration::from_secs(60))
            .danger_accept_invalid_certs(allow_insecure_tls)
            .build()?;

        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            username: username.to_string(),
            password: password.to_string(),
            http,
        })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// PROPFIND mit Tiefe 1 auf eine Adresse.
    pub async fn propfind(&self, url: &str, depth: u8) -> Result<Vec<DavEntry>, DavError> {
        let method = reqwest::Method::from_bytes(b"PROPFIND")
            .map_err(|e| DavError::Url(format!("PROPFIND: {e}")))?;

        let resp = self
            .http
            .request(method, url)
            .basic_auth(&self.username, Some(&self.password))
            .header("Depth", depth.to_string())
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(PROPFIND_BODY)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            return Err(DavError::Status {
                status: status.as_u16(),
            });
        }
        parse_multistatus(&resp.text().await?)
    }

    /// Prüft die Verbindung — für den „Verbindung testen"-Knopf.
    pub async fn test(&self) -> Result<(), DavError> {
        self.propfind(&self.base_url, 0).await.map(|_| ())
    }

    /// Lädt eine Datei vollständig.
    ///
    /// Bewusst ohne Streaming: die Datei wird direkt danach dekodiert und
    /// verkleinert (NF-12), der Puffer lebt also nur für einen Sync-Schritt.
    pub async fn get(&self, url: &str) -> Result<Vec<u8>, DavError> {
        let resp = self
            .http
            .get(url)
            .basic_auth(&self.username, Some(&self.password))
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            return Err(DavError::Status {
                status: status.as_u16(),
            });
        }
        Ok(resp.bytes().await?.to_vec())
    }

    /// Baut die absolute URL zu einem quellrelativen Pfad.
    pub fn url_for(&self, rel_path: &str) -> String {
        let encoded = rel_path
            .split('/')
            .map(|seg| {
                percent_encoding::utf8_percent_encode(seg, percent_encoding::NON_ALPHANUMERIC)
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("/");
        format!("{}/{}", self.base_url, encoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYNOLOGY_ANTWORT: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/photo/Fotos/</D:href>
    <D:propstat>
      <D:prop>
        <D:resourcetype><D:collection/></D:resourcetype>
        <D:getlastmodified>Sun, 15 Jun 2025 18:22:31 GMT</D:getlastmodified>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
  <D:response>
    <D:href>/photo/Fotos/Urlaub%202025/</D:href>
    <D:propstat>
      <D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
  <D:response>
    <D:href>/photo/Fotos/strand.jpg</D:href>
    <D:propstat>
      <D:prop>
        <D:resourcetype/>
        <D:getcontentlength>4823910</D:getcontentlength>
        <D:getetag>"a1b2c3"</D:getetag>
        <D:getlastmodified>Sun, 15 Jun 2025 18:22:31 GMT</D:getlastmodified>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
</D:multistatus>"#;

    const NEXTCLOUD_ANTWORT: &str = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
  <d:response>
    <d:href>/remote.php/dav/photos/kevin/albums/Sommer/</d:href>
    <d:propstat>
      <d:prop>
        <d:resourcetype><d:collection/></d:resourcetype>
        <oc:fileid>12</oc:fileid>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/photos/kevin/albums/Sommer/IMG_0042.HEIC</d:href>
    <d:propstat>
      <d:prop>
        <d:resourcetype/>
        <d:getcontentlength>3210000</d:getcontentlength>
        <d:getetag>W/"deadbeef"</d:getetag>
        <oc:fileid>4711</oc:fileid>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

    #[test]
    fn parse_multistatus_trennt_ordner_und_dateien() {
        let e = parse_multistatus(SYNOLOGY_ANTWORT).unwrap();
        assert_eq!(e.len(), 3);
        assert!(e[0].is_dir);
        assert!(e[1].is_dir);
        assert!(!e[2].is_dir, "eine Datei darf nicht als Ordner gelten");
    }

    #[test]
    fn parse_multistatus_liest_groesse_etag_und_datum() {
        let e = parse_multistatus(SYNOLOGY_ANTWORT).unwrap();
        let datei = &e[2];
        assert_eq!(datei.size, Some(4_823_910));
        assert_eq!(
            datei.etag.as_deref(),
            Some("a1b2c3"),
            "Anfuehrungszeichen entfernt"
        );
        assert!(datei.mtime.is_some());
        assert_eq!(datei.name(), "strand.jpg");
    }

    #[test]
    fn parse_multistatus_dekodiert_prozentkodierte_pfade() {
        let e = parse_multistatus(SYNOLOGY_ANTWORT).unwrap();
        assert_eq!(e[1].href, "/photo/Fotos/Urlaub 2025/");
        assert_eq!(e[1].name(), "Urlaub 2025");
    }

    #[test]
    fn parse_multistatus_normalisiert_schwache_etags() {
        let e = parse_multistatus(NEXTCLOUD_ANTWORT).unwrap();
        assert_eq!(
            e[1].etag.as_deref(),
            Some("deadbeef"),
            "W/ und Quotes entfernt"
        );
    }

    #[test]
    fn parse_multistatus_liest_nextcloud_fileid_fuer_previews() {
        let e = parse_multistatus(NEXTCLOUD_ANTWORT).unwrap();
        assert_eq!(e[1].file_id.as_deref(), Some("4711"));
    }

    #[test]
    fn parse_multistatus_vertraegt_kleinschreibung_und_fehlende_props() {
        let xml = r#"<multistatus xmlns="DAV:">
          <response><href>/a/b.jpg</href>
            <propstat><prop><resourcetype/></prop></propstat>
          </response>
        </multistatus>"#;
        let e = parse_multistatus(xml).unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].size, None, "fehlende Groesse ist kein Fehler");
        assert_eq!(e[0].etag, None);
    }

    #[test]
    fn parse_multistatus_meldet_kaputtes_xml() {
        assert!(parse_multistatus("<multistatus><response>").is_err());
    }

    #[test]
    fn parse_multistatus_lehnt_abgeschnittene_antwort_ab() {
        // Entscheidend fuer FA-26: eine abgebrochene Verbindung darf nicht als
        // "kuerzere Dateiliste" durchgehen, sonst raeumt remove_missing den
        // Cache leer. Lieber ein Fehler und die Diashow laeuft unveraendert
        // aus dem vorhandenen Cache weiter.
        let abgeschnitten = &SYNOLOGY_ANTWORT[..SYNOLOGY_ANTWORT.len() / 2];
        assert!(
            parse_multistatus(abgeschnitten).is_err(),
            "halbe Antwort muss als Fehler gelten, nicht als leere Liste"
        );
    }

    #[test]
    fn parse_multistatus_auf_leerer_antwort() {
        let e = parse_multistatus(r#"<multistatus xmlns="DAV:"></multistatus>"#).unwrap();
        assert!(e.is_empty());
    }

    #[test]
    fn parse_http_date_versteht_die_gaengigen_formate() {
        assert!(parse_http_date("Sun, 15 Jun 2025 18:22:31 GMT").is_some());
        assert!(parse_http_date("Sun, 15 Jun 2025 18:22:31 +0000").is_some());
        assert!(parse_http_date("2025-06-15T18:22:31Z").is_some());
        assert_eq!(parse_http_date("gestern"), None);
        assert_eq!(parse_http_date(""), None);
    }

    #[test]
    fn base_path_liefert_pfad_mit_schraegstrich() {
        assert_eq!(
            base_path("https://nas.local/photo/Fotos").unwrap(),
            "/photo/Fotos/"
        );
        assert_eq!(
            base_path("https://nas.local/photo/Fotos/").unwrap(),
            "/photo/Fotos/"
        );
        assert_eq!(base_path("http://nas.local").unwrap(), "/");
        assert_eq!(
            base_path("https://nas.local/Urlaub%202025").unwrap(),
            "/Urlaub 2025/"
        );
    }

    #[test]
    fn base_path_lehnt_adresse_ohne_schema_ab() {
        assert!(base_path("nas.local/photo").is_err());
    }

    #[test]
    fn relative_path_kuerzt_auf_die_quellenwurzel() {
        let base = "/photo/Fotos/";
        assert_eq!(
            relative_path("/photo/Fotos/strand.jpg", base),
            Some("strand.jpg".into())
        );
        assert_eq!(
            relative_path("/photo/Fotos/Urlaub/1.jpg", base),
            Some("Urlaub/1.jpg".into())
        );
    }

    #[test]
    fn relative_path_verwirft_die_wurzel_selbst() {
        assert_eq!(relative_path("/photo/Fotos/", "/photo/Fotos/"), None);
    }

    #[test]
    fn relative_path_verwirft_ausbrueche() {
        // Ein Server darf uns keine Pfade ausserhalb der Quelle unterschieben.
        let base = "/photo/Fotos/";
        assert_eq!(relative_path("/etc/passwd", base), None);
        assert_eq!(relative_path("/photo/Fotos/../../etc/passwd", base), None);
    }

    #[test]
    fn url_for_kodiert_sonderzeichen() {
        let c = WebDavClient::new("https://nas.local/photo/Fotos", "u", "p", false).unwrap();
        let url = c.url_for("Urlaub 2025/strand & meer.jpg");
        assert!(url.starts_with("https://nas.local/photo/Fotos/"));
        assert!(
            !url.contains(' '),
            "Leerzeichen muessen kodiert sein: {url}"
        );
        assert!(url.contains("%20"));
        assert!(
            url.contains("%26"),
            "das Und-Zeichen muss kodiert sein: {url}"
        );
        // Der Trenner zwischen Segmenten bleibt ein echter Schraegstrich:
        // 2x in "https://", dann /photo /Fotos /Urlaub... /strand...
        assert_eq!(url.matches('/').count(), 6, "war: {url}");
    }

    #[test]
    fn client_kuerzt_abschliessenden_schraegstrich() {
        let c = WebDavClient::new("https://nas.local/photo/", "u", "p", false).unwrap();
        assert_eq!(c.base_url(), "https://nas.local/photo");
    }
}

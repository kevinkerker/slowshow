//! Nextcloud-Anbindung über Photos-Alben (FA-23, E-03).
//!
//! Zwei Besonderheiten gegenüber einer normalen WebDAV-Quelle:
//!
//! 1. **Photos-Endpunkt** statt Dateibaum: Alben liegen unter
//!    `remote.php/dav/photos/{user}/albums/` und spiegeln die Alben der
//!    Photos-App wider, nicht die Ordnerstruktur.
//! 2. **Preview-API** statt Originaldownload: `index.php/core/preview` liefert
//!    JPEG in gewünschter Größe. Das entlastet NF-12 (der Server skaliert
//!    bereits) und löst HEIC serverseitig, ohne libheif auf dem Tablet
//!    (FA-09, E-04).
//!
//! Google Photos fehlt bewusst (E-08, R-07); Handy-Fotos gelangen über den
//! Nextcloud-Auto-Upload hierher.

use super::webdav::{DavEntry, DavError, WebDavClient};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

/// Kodiert ein einzelnes Pfadsegment.
fn seg(s: &str) -> String {
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

/// Adresse der Albenliste eines Nutzers.
pub fn albums_url(base: &str, user: &str) -> String {
    format!(
        "{}/remote.php/dav/photos/{}/albums",
        base.trim_end_matches('/'),
        seg(user)
    )
}

/// Adresse eines einzelnen Albums.
pub fn album_url(base: &str, user: &str, album: &str) -> String {
    format!("{}/{}", albums_url(base, user), seg(album))
}

/// Adresse der Preview-API für eine Datei-Id.
///
/// `a=1` erhält das Seitenverhältnis — ohne das liefert Nextcloud beschnittene
/// Quadrate, was FA-05 widerspräche.
pub fn preview_url(base: &str, file_id: &str, width: u32, height: u32) -> String {
    format!(
        "{}/index.php/core/preview?fileId={}&x={}&y={}&a=1&forceIcon=0&mode=fill",
        base.trim_end_matches('/'),
        seg(file_id),
        width,
        height
    )
}

/// Ein Album, wie es die Einstellungsoberfläche zur Auswahl anbietet.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub name: String,
}

pub struct NextcloudClient {
    base: String,
    user: String,
    album: String,
    use_preview_api: bool,
    dav: WebDavClient,
}

impl NextcloudClient {
    pub fn new(
        base: &str,
        user: &str,
        password: &str,
        album: &str,
        use_preview_api: bool,
        allow_insecure_tls: bool,
    ) -> Result<Self, DavError> {
        let base = base.trim_end_matches('/').to_string();
        // Der WebDAV-Client zeigt direkt auf das Album; damit sind die
        // zurückgelieferten hrefs bereits albumrelativ kürzbar.
        let dav = WebDavClient::new(
            &album_url(&base, user, album),
            user,
            password,
            allow_insecure_tls,
        )?;
        Ok(Self {
            base,
            user: user.to_string(),
            album: album.to_string(),
            use_preview_api,
            dav,
        })
    }

    pub fn dav(&self) -> &WebDavClient {
        &self.dav
    }

    pub fn album(&self) -> &str {
        &self.album
    }

    /// Listet die verfügbaren Alben — für die Auswahl in den Einstellungen.
    pub async fn list_albums(&self) -> Result<Vec<Album>, DavError> {
        let url = albums_url(&self.base, &self.user);
        let entries = self.dav.propfind(&url, 1).await?;
        Ok(entries
            .into_iter()
            // Der erste Eintrag ist die Sammlung selbst.
            .filter(|e| e.is_dir && !e.href.trim_end_matches('/').ends_with("/albums"))
            .map(|e| Album {
                name: e.name().to_string(),
            })
            .collect())
    }

    /// Holt ein Bild — bevorzugt als Preview in Zielgröße (E-03).
    ///
    /// Fällt auf den Originaldownload zurück, wenn die Preview-API abgeschaltet
    /// ist oder keine `fileid` geliefert wurde. Für HEIC-Originale ist der
    /// Rückfall wirkungslos — die Datei wird dann beim Dekodieren verworfen
    /// und protokolliert (FA-09).
    pub async fn fetch(
        &self,
        entry: &DavEntry,
        rel_path: &str,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, DavError> {
        if self.use_preview_api {
            if let Some(id) = entry.file_id.as_deref() {
                let url = preview_url(&self.base, id, width, height);
                match self.dav.get(&url).await {
                    Ok(bytes) if !bytes.is_empty() => return Ok(bytes),
                    Ok(_) => log::warn!("Preview für {rel_path} war leer, lade Original"),
                    Err(e) => {
                        log::warn!("Preview für {rel_path} fehlgeschlagen ({e}), lade Original")
                    }
                }
            }
        }
        self.dav.get(&self.dav.url_for(rel_path)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn albums_url_trifft_den_photos_endpunkt_aus_e_03() {
        assert_eq!(
            albums_url("https://cloud.example.org", "kevin"),
            "https://cloud.example.org/remote.php/dav/photos/kevin/albums"
        );
    }

    #[test]
    fn albums_url_vertraegt_abschliessenden_schraegstrich() {
        assert_eq!(
            albums_url("https://cloud.example.org/", "kevin"),
            "https://cloud.example.org/remote.php/dav/photos/kevin/albums"
        );
    }

    #[test]
    fn album_url_kodiert_namen_mit_sonderzeichen() {
        let url = album_url("https://cloud.example.org", "kevin", "Sommer 2026");
        assert!(url.ends_with("/albums/Sommer%202026"), "war: {url}");
        assert!(!url.contains(' '));
    }

    #[test]
    fn album_url_kodiert_umlaute() {
        let url = album_url("https://cloud.example.org", "kevin", "Grüße");
        assert!(!url.contains('ü'), "war: {url}");
        assert!(url.contains('%'));
    }

    #[test]
    fn preview_url_haelt_das_seitenverhaeltnis() {
        let url = preview_url("https://cloud.example.org", "4711", 2560, 1600);
        assert!(url.contains("fileId=4711"));
        assert!(url.contains("x=2560"));
        assert!(url.contains("y=1600"));
        assert!(
            url.contains("a=1"),
            "ohne a=1 wuerde Nextcloud quadratisch beschneiden"
        );
    }

    #[test]
    fn client_zeigt_direkt_auf_das_album() {
        let c = NextcloudClient::new(
            "https://cloud.example.org",
            "kevin",
            "app-passwort",
            "Sommer",
            true,
            false,
        )
        .unwrap();
        assert_eq!(
            c.dav().base_url(),
            "https://cloud.example.org/remote.php/dav/photos/kevin/albums/Sommer"
        );
        assert_eq!(c.album(), "Sommer");
    }
}

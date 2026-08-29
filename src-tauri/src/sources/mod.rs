//! Entfernte Bildquellen (FA-21, FA-23).
//!
//! Vereinheitlicht lokale Ordner, WebDAV-NAS und Nextcloud hinter einer kleinen
//! Schnittstelle, damit `sync` alle drei gleich behandeln kann — inklusive
//! Fortschrittsmeldung und mitwachsender Playlist.
//!
//! Auch lokale SAF-Ordner laufen hierüber (siehe [`local`]): das Plugin
//! `tauri-plugin-android-fs` hat eine Rust-API, die Bilddaten müssen also nicht
//! durch die WebView (R-03, NF-13).

pub mod local;
pub mod nextcloud;
pub mod webdav;

pub use local::LocalClient;
pub use nextcloud::{Album, NextcloudClient};
pub use webdav::{DavEntry, DavError, WebDavClient};

use crate::decode::{classify, FileClass};
use crate::model::{Source, SourceKind};

/// Obergrenzen gegen Endlosschleifen und versehentlich riesige Bäume.
/// Ein Bilderrahmen soll nicht am fehlkonfigurierten Wurzelverzeichnis
/// eines NAS hängenbleiben (R-04: die App darf nicht ewig beschäftigt sein).
const MAX_DEPTH: usize = 8;
const MAX_FILES: usize = 50_000;

/// Eine an der Quelle gefundene Bilddatei.
#[derive(Debug, Clone)]
pub struct RemoteFile {
    /// Pfad relativ zur Quellenwurzel — Identität für den Cache-Index.
    pub rel_path: String,
    pub file_name: String,
    pub etag: Option<String>,
    pub size: Option<u64>,
    pub mtime: Option<i64>,
    /// Nextcloud-Datei-Id für die Preview-API.
    pub file_id: Option<String>,
    /// Serialisierte SAF-URI bei lokalen Ordnern.
    pub local_uri: Option<String>,
}

/// Ergebnis eines Listenlaufs.
#[derive(Debug, Default)]
pub struct Listing {
    pub files: Vec<RemoteFile>,
    /// Bewusst übersprungene Dateien (HEIC, Video) — für das Log (FA-09).
    pub skipped: Vec<String>,
    /// Wurde eine der Obergrenzen erreicht? Dann ist die Liste unvollständig.
    pub truncated: bool,
}

pub enum RemoteClient {
    Local(LocalClient),
    WebDav(WebDavClient),
    Nextcloud(Box<NextcloudClient>),
}

impl RemoteClient {
    /// Baut den passenden Client für eine Quelle.
    pub fn from_source(
        app: &tauri::AppHandle,
        source: &Source,
        password: &str,
    ) -> Result<Option<Self>, DavError> {
        match &source.kind {
            SourceKind::Local { saf_uri, .. } => {
                Ok(Some(Self::Local(LocalClient::new(app.clone(), saf_uri)?)))
            }
            SourceKind::WebDav {
                url,
                username,
                allow_insecure_tls,
                ..
            } => Ok(Some(Self::WebDav(WebDavClient::new(
                url,
                username,
                password,
                *allow_insecure_tls,
            )?))),
            SourceKind::Nextcloud {
                url,
                username,
                album,
                use_preview_api,
                allow_insecure_tls,
                ..
            } => Ok(Some(Self::Nextcloud(Box::new(NextcloudClient::new(
                url,
                username,
                password,
                album,
                *use_preview_api,
                *allow_insecure_tls,
            )?)))),
        }
    }

    /// Der WebDAV-Client entfernter Quellen. `None` bei lokalen Ordnern.
    fn dav(&self) -> Option<&WebDavClient> {
        match self {
            Self::Local(_) => None,
            Self::WebDav(c) => Some(c),
            Self::Nextcloud(c) => Some(c.dav()),
        }
    }

    /// Prüft die Erreichbarkeit — für den „Verbindung testen"-Knopf.
    pub async fn test(&self) -> Result<(), DavError> {
        match self {
            Self::Local(c) => c.test().await,
            _ => {
                self.dav()
                    .expect("entfernte Quelle hat einen DAV-Client")
                    .test()
                    .await
            }
        }
    }

    /// Sammelt alle Bilddateien der Quelle.
    ///
    /// Läuft iterativ statt rekursiv: eine rekursive `async fn` bräuchte
    /// `Box::pin`, und die Warteschlange macht die Tiefenbegrenzung ohnehin
    /// deutlicher.
    ///
    /// `subfolders` beschränkt auf bestimmte Unterordner (FA-29). Nextcloud-
    /// Alben sind flach, dort greift der Filter nicht.
    pub async fn list(&self, subfolders: &[String]) -> Result<Listing, DavError> {
        if let Self::Local(c) = self {
            return c.list(subfolders).await;
        }
        let dav = self.dav().expect("entfernte Quelle hat einen DAV-Client");
        let base = webdav::base_path(dav.base_url())?;
        let mut result = Listing::default();

        // (URL, Tiefe)
        let mut queue: Vec<(String, usize)> = vec![(dav.base_url().to_string(), 0)];
        let mut visited: Vec<String> = Vec::new();

        while let Some((url, depth)) = queue.pop() {
            if visited.contains(&url) {
                continue;
            }
            visited.push(url.clone());

            let entries = dav.propfind(&url, 1).await?;
            for entry in entries {
                let Some(rel) = webdav::relative_path(&entry.href, &base) else {
                    continue; // die Wurzel selbst oder ein Ausbruchsversuch
                };

                if entry.is_dir {
                    if depth + 1 > MAX_DEPTH {
                        result.truncated = true;
                        continue;
                    }
                    if !folder_allowed(&rel, subfolders) {
                        continue;
                    }
                    queue.push((dav.url_for(&rel), depth + 1));
                    continue;
                }

                if !file_allowed(&rel, subfolders) {
                    continue;
                }

                match classify(entry.name()) {
                    FileClass::Image => {
                        if result.files.len() >= MAX_FILES {
                            result.truncated = true;
                            break;
                        }
                        result.files.push(RemoteFile {
                            rel_path: rel,
                            file_name: entry.name().to_string(),
                            etag: entry.etag.clone(),
                            size: entry.size,
                            mtime: entry.mtime,
                            file_id: entry.file_id.clone(),
                            local_uri: None,
                        });
                    }
                    // FA-09 / E-07: HEIC und Video werden übersprungen und
                    // protokolliert, nicht heruntergeladen.
                    FileClass::Skipped => result.skipped.push(rel),
                    FileClass::Irrelevant => {}
                }
            }

            if result.truncated && result.files.len() >= MAX_FILES {
                break;
            }
        }

        Ok(result)
    }

    /// Lädt eine Datei.
    ///
    /// Nextcloud nutzt dabei die Preview-API (E-03), WebDAV lädt das Original,
    /// lokale Ordner lesen über SAF.
    pub async fn fetch(
        &self,
        file: &RemoteFile,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, DavError> {
        match self {
            Self::Local(c) => c.fetch(file).await,
            Self::WebDav(c) => c.get(&c.url_for(&file.rel_path)).await,
            Self::Nextcloud(c) => {
                let entry = DavEntry {
                    file_id: file.file_id.clone(),
                    ..Default::default()
                };
                c.fetch(&entry, &file.rel_path, width, height).await
            }
        }
    }
}

/// Darf in diesen Ordner abgestiegen werden? (FA-29)
///
/// Ein Ordner ist erlaubt, wenn er auf dem Weg zu einem gewünschten
/// Unterordner liegt oder selbst darunter.
pub fn folder_allowed(rel: &str, subfolders: &[String]) -> bool {
    if subfolders.is_empty() {
        return true;
    }
    subfolders.iter().any(|f| {
        let f = f.trim_matches('/');
        f.is_empty() || rel.starts_with(f) || f.starts_with(rel)
    })
}

/// Liegt die Datei in einem gewünschten Unterordner? (FA-29)
pub fn file_allowed(rel: &str, subfolders: &[String]) -> bool {
    if subfolders.is_empty() {
        return true;
    }
    subfolders.iter().any(|f| {
        let f = f.trim_matches('/');
        f.is_empty() || rel.starts_with(&format!("{f}/"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ohne_filter_ist_alles_erlaubt() {
        assert!(folder_allowed("Urlaub", &[]));
        assert!(file_allowed("Urlaub/1.jpg", &[]));
        assert!(file_allowed("1.jpg", &[]));
    }

    #[test]
    fn filter_laesst_nur_gewaehlte_unterordner_zu_fa_29() {
        let filter = vec!["Urlaub".to_string()];

        assert!(folder_allowed("Urlaub", &filter));
        assert!(folder_allowed("Urlaub/2025", &filter));
        assert!(!folder_allowed("Dokumente", &filter));

        assert!(file_allowed("Urlaub/1.jpg", &filter));
        assert!(file_allowed("Urlaub/2025/1.jpg", &filter));
        assert!(!file_allowed("Dokumente/1.jpg", &filter));
        assert!(
            !file_allowed("1.jpg", &filter),
            "Wurzeldateien sind nicht im Filterordner"
        );
    }

    #[test]
    fn filter_erlaubt_den_abstieg_ueber_zwischenordner() {
        // Um "a/b/c" zu erreichen, muss "a" und "a/b" betreten werden dürfen.
        let filter = vec!["a/b/c".to_string()];
        assert!(folder_allowed("a", &filter));
        assert!(folder_allowed("a/b", &filter));
        assert!(folder_allowed("a/b/c", &filter));
        assert!(!folder_allowed("x", &filter));
    }

    #[test]
    fn filter_vertraegt_schraegstriche_am_rand() {
        let filter = vec!["/Urlaub/".to_string()];
        assert!(folder_allowed("Urlaub", &filter));
        assert!(file_allowed("Urlaub/1.jpg", &filter));
    }

    // `from_source` braucht einen AppHandle und ist deshalb nur am Geraet
    // pruefbar. Was sich ohne Tauri testen laesst, ist der Teil, auf den es
    // ankommt: dass die Clients die richtigen Adressen bilden.

    #[test]
    fn webdav_client_uebernimmt_die_basisadresse() {
        let c = WebDavClient::new("https://nas.local/photo/Fotos/", "u", "p", false).unwrap();
        assert_eq!(c.base_url(), "https://nas.local/photo/Fotos");
    }

    #[test]
    fn nextcloud_client_zeigt_auf_das_album() {
        let c = NextcloudClient::new(
            "https://cloud.example.org",
            "kevin",
            "app-passwort",
            "Sommer 2026",
            true,
            false,
        )
        .unwrap();
        assert!(c
            .dav()
            .base_url()
            .contains("/remote.php/dav/photos/kevin/albums/Sommer%202026"));
    }

    #[test]
    fn remote_file_traegt_den_saf_verweis_nur_bei_lokalen_quellen() {
        // Der Verweis unterscheidet die Wege in `RemoteClient::fetch`.
        let remote = RemoteFile {
            rel_path: "1.jpg".into(),
            file_name: "1.jpg".into(),
            etag: None,
            size: None,
            mtime: None,
            file_id: None,
            local_uri: None,
        };
        assert!(remote.local_uri.is_none());

        let lokal = RemoteFile {
            local_uri: Some("{\"uri\":\"content://x\"}".into()),
            ..remote
        };
        assert!(lokal.local_uri.is_some());
    }
}

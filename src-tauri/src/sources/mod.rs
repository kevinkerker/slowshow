//! Entfernte Bildquellen (FA-21, FA-23).
//!
//! Vereinheitlicht lokale Ordner, WebDAV-NAS und Nextcloud hinter einer kleinen
//! Schnittstelle, damit `sync` alle drei gleich behandeln kann — inklusive
//! Fortschrittsmeldung und mitwachsender Playlist.
//!
//! Auch lokale SAF-Ordner laufen hierüber (siehe [`local`]): das Plugin
//! `tauri-plugin-android-fs` hat eine Rust-API, die Bilddaten müssen also nicht
//! durch die WebView (R-03, NF-13).

pub mod filter;
pub mod local;
pub mod nextcloud;
pub mod webdav;

pub use filter::{FolderFilter, SubfolderEntry};
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
    /// Angeschaltete Unterordner, die es an der Quelle nicht (mehr) gibt
    /// (E-70). Bei abgeschnittener Liste leer: dann ist das nicht entscheidbar.
    pub missing_subfolders: Vec<String>,
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
            // Postfaecher laufen nicht ueber list/fetch: eine Mail wird als
            // Ganzes geholt und ihre Anhaenge einzeln abgelegt, sonst laegen
            // bei 25 MB je Anhang hunderte Megabyte gleichzeitig im Speicher
            // (R-03). Der eigene Weg steht in `mail::imap` und `sync`.
            SourceKind::Mail { .. } => Ok(None),
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
    /// `filter` ist die Auswahl der Unterordner (FA-29, E-70). Nextcloud-Alben
    /// sind flach, dort greift sie nicht.
    pub async fn list(&self, filter: &FolderFilter) -> Result<Listing, DavError> {
        if let Self::Local(c) = self {
            return c.list(filter).await;
        }
        let all = FolderFilter::all();
        let filter = if matches!(self, Self::Nextcloud(_)) { &all } else { filter };
        let dav = self.dav().expect("entfernte Quelle hat einen DAV-Client");
        let base = webdav::base_path(dav.base_url())?;
        let mut result = Listing::default();

        // (URL, Tiefe)
        let mut queue: Vec<(String, usize)> = vec![(dav.base_url().to_string(), 0)];
        let mut visited: Vec<String> = Vec::new();
        // Betretene Unterordner, relativ — fuer die fehlenden (E-70).
        let mut seen_dirs = std::collections::HashSet::new();

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
                    if !filter.descend(&rel) {
                        continue;
                    }
                    queue.push((dav.url_for(&rel), depth + 1));
                    seen_dirs.insert(rel);
                    continue;
                }

                if !filter.file_allowed(&rel) {
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

        if !result.truncated {
            result.missing_subfolders = filter.missing(&seen_dirs);
        }
        Ok(result)
    }

    /// Die Unterordner eines Ordners der Quelle, fuer die Auswahl (E-70).
    ///
    /// Eine Ebene je Aufruf: die Liste im Dialog klappt Ebenen erst auf
    /// Wunsch auf. Den ganzen Baum vorab zu lesen hiesse bei einem NAS mit
    /// Fotoarchiv minutenlang warten, bevor sich der Dialog bedienen laesst.
    pub async fn subfolders(&self, path: &str) -> Result<Vec<SubfolderEntry>, DavError> {
        let mut entries = match self {
            Self::Local(c) => c.subfolders(path).await?,
            Self::Nextcloud(_) => {
                return Err(DavError::Url("Nextcloud-Alben haben keine Unterordner".into()))
            }
            Self::WebDav(dav) => {
                let base = webdav::base_path(dav.base_url())?;
                let url = if path.is_empty() {
                    dav.base_url().to_string()
                } else {
                    dav.url_for(path)
                };
                let own = path.trim_matches('/');
                let mut out = Vec::new();
                for entry in dav.propfind(&url, 1).await? {
                    let Some(rel) = webdav::relative_path(&entry.href, &base) else {
                        continue;
                    };
                    if !entry.is_dir || rel == own {
                        continue;
                    }
                    // Ein unlesbarer Unterordner soll die Liste nicht
                    // verhindern; er erscheint dann nur ohne Vermerke.
                    let children = dav.propfind(&dav.url_for(&rel), 1).await.unwrap_or_default();
                    let inner: Vec<(String, bool)> = children
                        .iter()
                        .filter(|c| webdav::relative_path(&c.href, &base).as_deref() != Some(&rel))
                        .map(|c| (c.name().to_string(), c.is_dir))
                        .collect();
                    out.push(SubfolderEntry::from_children(
                        entry.name(),
                        &rel,
                        inner.iter().map(|(n, d)| (n.as_str(), *d)),
                    ));
                }
                out
            }
        };
        filter::sort_entries(&mut entries);
        Ok(entries)
    }

    /// Muss die Laenge des Geladenen mit der Auflistung uebereinstimmen?
    ///
    /// Nur bei einer Netzuebertragung des Originals: dort bricht ein Download
    /// tatsaechlich ab (E-45). Nextcloud holt bevorzugt eine Vorschau in
    /// Zielgroesse (E-03) — deren Laenge ist eine andere. Ein lokaler Ordner
    /// kann nicht halb uebertragen werden; dort machte eine falsche Groesse
    /// des Dateianbieters aus lesbaren Bildern einen Totalausfall, gemessen
    /// am Emulator mit 295 von 300 (E-69). Der Decoder bleibt die Pruefung.
    pub fn checks_transfer_length(&self) -> bool {
        match self {
            Self::Local(_) => false,
            Self::WebDav(_) => true,
            Self::Nextcloud(c) => !c.uses_preview_api(),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn laenge_wird_nur_bei_netzuebertragungen_geprueft_e_69() {
        assert!(!RemoteClient::Local(LocalClient).checks_transfer_length());
        let dav = WebDavClient::new("https://nas.local/Fotos", "u", "p", false).unwrap();
        assert!(RemoteClient::WebDav(dav).checks_transfer_length());
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

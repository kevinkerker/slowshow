//! Lokale Ordner über das Storage Access Framework (FA-20).
//!
//! ## Warum das hier liegt und nicht im Frontend
//!
//! Ursprünglich lief der Ordnerdurchlauf im Frontend: die WebView las jede
//! Datei über das SAF-Plugin und schob die Bytes per IPC nach Rust. Das war
//! aus zwei Gründen falsch.
//!
//! Erstens verstößt es gegen die eigene Architekturregel (CLAUDE.md): Bilddaten
//! gehören nicht durch die WebView. Ein 5-MB-Foto durch die Brücke zu schieben
//! belastet genau den Speicher, den R-03 schützen soll.
//!
//! Zweitens funktionierte es schlicht nicht. Tauri lieferte den Rohkörper auf
//! Android nicht als `InvokeBody::Raw` aus, und jede Datei scheiterte mit
//! „ingest_image erwartet einen Rohkörper" — sichtbar nur in der
//! Browserkonsole, während die Oberfläche „Keine Änderungen" meldete.
//!
//! Das Plugin hat eine vollständige Rust-API. Damit läuft der lokale Ordner
//! jetzt über denselben Weg wie NAS und Nextcloud: listen, herunterladen,
//! dekodieren, ablegen — alles im Rust-Prozess.

use super::{DavError, Listing, RemoteFile};

/// Obergrenzen wie bei entfernten Quellen.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const MAX_DEPTH: usize = 8;
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const MAX_FILES: usize = 50_000;

#[cfg(target_os = "android")]
mod imp {
    use super::*;
    use crate::decode::{classify, FileClass};
    // Absolut, nicht ueber `super`: dieses Modul liegt in `sources::local::imp`,
    // `super` waere also `sources::local` und nicht `sources`.
    use crate::sources::{file_allowed, folder_allowed};
    use tauri::AppHandle;
    use tauri_plugin_android_fs::{AndroidFsExt, Entry, FsUri};

    pub struct LocalClient {
        app: AppHandle,
        root: FsUri,
    }

    impl LocalClient {
        /// `saf_uri` ist die in der Konfiguration abgelegte, serialisierte URI.
        pub fn new(app: AppHandle, saf_uri: &str) -> Result<Self, DavError> {
            let root: FsUri = serde_json::from_str(saf_uri)
                .map_err(|e| DavError::Url(format!("Ordnerfreigabe unlesbar: {e}")))?;
            Ok(Self { app, root })
        }

        /// Prüft, ob die Freigabe noch gilt (FA-20: dauerhaft gemerkt).
        pub async fn test(&self) -> Result<(), DavError> {
            let app = self.app.clone();
            let root = self.root.clone();
            blocking(move || {
                app.android_fs()
                    .read_dir(&root)
                    .map(|_| ())
                    .map_err(|e| DavError::Url(format!("Ordner nicht lesbar: {e}")))
            })
            .await
        }

        pub async fn list(&self, subfolders: &[String]) -> Result<Listing, DavError> {
            let app = self.app.clone();
            let root = self.root.clone();
            let subfolders = subfolders.to_vec();
            blocking(move || walk(&app, &root, &subfolders)).await
        }

        pub async fn fetch(&self, file: &RemoteFile) -> Result<Vec<u8>, DavError> {
            let raw = file
                .local_uri
                .clone()
                .ok_or_else(|| DavError::Url("Datei ohne SAF-Verweis".into()))?;
            let app = self.app.clone();

            blocking(move || {
                let uri: FsUri = serde_json::from_str(&raw)
                    .map_err(|e| DavError::Url(format!("SAF-Verweis unlesbar: {e}")))?;
                app.android_fs()
                    .read(&uri)
                    .map_err(|e| DavError::Url(format!("Datei nicht lesbar: {e}")))
            })
            .await
        }
    }

    /// Führt blockierende SAF-Aufrufe aus, ohne den Async-Runtime zu belegen.
    ///
    /// Das Lesen einer 5-MB-Datei dauert spürbar; direkt im Async-Kontext
    /// aufgerufen blockierte es einen Worker und damit auch die Diashow.
    async fn blocking<T, F>(f: F) -> Result<T, DavError>
    where
        F: FnOnce() -> Result<T, DavError> + Send + 'static,
        T: Send + 'static,
    {
        tauri::async_runtime::spawn_blocking(f)
            .await
            .map_err(|e| DavError::Url(format!("SAF-Aufruf abgebrochen: {e}")))?
    }

    /// Breitensuche durch den freigegebenen Ordner.
    fn walk(app: &AppHandle, root: &FsUri, subfolders: &[String]) -> Result<Listing, DavError> {
        let fs = app.android_fs();
        let mut result = Listing::default();
        let mut queue: Vec<(FsUri, String, usize)> = vec![(root.clone(), String::new(), 0)];

        while let Some((uri, prefix, depth)) = queue.pop() {
            let entries = match fs.read_dir(&uri) {
                Ok(e) => e,
                Err(e) => {
                    // Ein unlesbarer Unterordner darf den ganzen Lauf nicht
                    // scheitern lassen — der Rest wird trotzdem übernommen.
                    log::warn!("Ordner '{prefix}' nicht lesbar: {e}");
                    continue;
                }
            };

            for entry in entries {
                let name = entry.name().to_string();
                let rel = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{prefix}/{name}")
                };

                match entry {
                    Entry::Dir { uri, .. } => {
                        if depth + 1 > MAX_DEPTH {
                            result.truncated = true;
                            continue;
                        }
                        if !folder_allowed(&rel, subfolders) {
                            continue;
                        }
                        queue.push((uri, rel, depth + 1));
                    }
                    Entry::File {
                        uri,
                        last_modified,
                        len,
                        ..
                    } => {
                        if !file_allowed(&rel, subfolders) {
                            continue;
                        }
                        match classify(&name) {
                            FileClass::Image => {
                                if result.files.len() >= MAX_FILES {
                                    result.truncated = true;
                                    break;
                                }
                                result.files.push(RemoteFile {
                                    rel_path: rel,
                                    file_name: name,
                                    etag: None,
                                    size: Some(len),
                                    mtime: to_unix(last_modified),
                                    file_id: None,
                                    local_uri: serde_json::to_string(&uri).ok(),
                                });
                            }
                            // FA-09 / E-07: HEIC und Video überspringen.
                            FileClass::Skipped => result.skipped.push(rel),
                            FileClass::Irrelevant => {}
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    fn to_unix(t: std::time::SystemTime) -> Option<i64> {
        t.duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs() as i64)
    }
}

/// Auf dem Desktop gibt es kein SAF. Der Desktop-Build ist Nebenprodukt
/// (Lastenheft 1.3); lokale Quellen bleiben dort ohne Funktion.
#[cfg(not(target_os = "android"))]
mod imp {
    use super::*;

    pub struct LocalClient;

    impl LocalClient {
        pub fn new(_app: tauri::AppHandle, _saf_uri: &str) -> Result<Self, DavError> {
            Err(DavError::Url(
                "Lokale Ordner stehen nur in der Android-App zur Verfügung".into(),
            ))
        }

        pub async fn test(&self) -> Result<(), DavError> {
            Err(DavError::Url("SAF nicht verfügbar".into()))
        }

        pub async fn list(&self, _subfolders: &[String]) -> Result<Listing, DavError> {
            Err(DavError::Url("SAF nicht verfügbar".into()))
        }

        pub async fn fetch(&self, _file: &RemoteFile) -> Result<Vec<u8>, DavError> {
            Err(DavError::Url("SAF nicht verfügbar".into()))
        }
    }
}

pub use imp::LocalClient;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grenzen_entsprechen_den_entfernten_quellen() {
        // Beide Wege müssen gleich weit gehen, sonst zeigt derselbe Ordner je
        // nach Quellenart unterschiedlich viele Bilder.
        assert_eq!(MAX_DEPTH, 8);
        assert_eq!(MAX_FILES, 50_000);
    }
}

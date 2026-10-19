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

use super::{DavError, FolderFilter, Listing, RemoteFile, SubfolderEntry};

/// Obergrenzen wie bei entfernten Quellen.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const MAX_DEPTH: usize = 8;
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const MAX_FILES: usize = 50_000;

/// Fehler fuer eine unlesbare Wurzel der Ordnerfreigabe (E-40, E-45).
///
/// Vorher lief das als leerer Ordner durch: 0 Bilder, Erfolg, `last_sync`
/// gestempelt — und die Karte schwieg. Am Geraet nach der Neuinstallation
/// gemessen, mit genau der Meldung, die hier als Ursache angehaengt wird.
pub fn root_unreadable(cause: &str) -> DavError {
    DavError::Access(format!(
        "Ordnerfreigabe fehlt oder Ordner nicht erreichbar — unter „Bearbeiten“ den Ordner neu auswählen ({cause})"
    ))
}

/// Meldung, wenn Android dem Systemdienst „Externer Speicher“ die Bilder
/// sperrt (E-71).
pub const SYSTEM_STORAGE_BLOCKED: &str =
    "Android sperrt dem Systemdienst „Externer Speicher“ den Zugriff auf Bilder — in den Einstellungen unter „Spezieller App-Zugriff“ → „Zugriff auf alle Dateien“ die System-Apps einblenden, „Externer Speicher“ erlauben, dann neu laden";

/// Fehler fuer eine Datei, die sich nicht lesen laesst (E-71).
///
/// Ab Android 11 oeffnet der Dienst hinter SAF (`com.android.externalstorage`)
/// Bilder ueber den MediaProvider. Fehlt dem Dienst dort der Allzugriff,
/// scheitert jede Datei mit „com.android.externalstorage has no access to
/// content://media/…“ — gemessen an einem Testgeraet, 1387 von 1387 in zwei
/// Ordnern, obwohl das Auflisten ging. Die App kann das nicht beheben, nur
/// den Weg in die Einstellungen nennen; als `Access` beendet es den Lauf.
pub fn read_failed(cause: &str) -> DavError {
    if cause.contains("com.android.externalstorage has no access") {
        DavError::Access(SYSTEM_STORAGE_BLOCKED.into())
    } else {
        DavError::Url(format!("Datei nicht lesbar: {cause}"))
    }
}

#[cfg(target_os = "android")]
mod imp {
    use super::*;
    use crate::decode::{classify, FileClass};
    use crate::sources::filter::child_path;
    use std::collections::HashSet;
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

        pub async fn list(&self, filter: &FolderFilter) -> Result<Listing, DavError> {
            let app = self.app.clone();
            let root = self.root.clone();
            let filter = filter.clone();
            blocking(move || walk(&app, &root, &filter)).await
        }

        /// Die Unterordner eines Ordners, eine Ebene (E-70).
        pub async fn subfolders(&self, path: &str) -> Result<Vec<SubfolderEntry>, DavError> {
            let app = self.app.clone();
            let root = self.root.clone();
            let path = path.to_string();
            blocking(move || list_level(&app, &root, &path)).await
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
                    .map_err(|e| super::read_failed(&e.to_string()))
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

    /// Findet einen Ordner unterhalb der Freigabe ueber seinen Pfad.
    ///
    /// SAF kennt keine Pfade, nur Verweise; der Weg geht deshalb Name fuer
    /// Name von der Wurzel hinab.
    fn resolve(app: &AppHandle, root: &FsUri, path: &str) -> Result<FsUri, DavError> {
        let fs = app.android_fs();
        let mut current = root.clone();
        for segment in path.split('/').filter(|s| !s.is_empty()) {
            let entries = fs
                .read_dir(&current)
                .map_err(|e| super::root_unreadable(&e.to_string()))?;
            current = entries
                .into_iter()
                .find_map(|e| match e {
                    Entry::Dir { uri, name, .. } if name == segment => Some(uri),
                    _ => None,
                })
                .ok_or_else(|| DavError::Url(format!("Ordner „{path}“ nicht gefunden")))?;
        }
        Ok(current)
    }

    fn list_level(app: &AppHandle, root: &FsUri, path: &str) -> Result<Vec<SubfolderEntry>, DavError> {
        let fs = app.android_fs();
        let dir = resolve(app, root, path)?;
        let entries = fs
            .read_dir(&dir)
            .map_err(|e| super::root_unreadable(&e.to_string()))?;
        let mut out = Vec::new();
        for entry in entries {
            let Entry::Dir { uri, name, .. } = entry else {
                continue;
            };
            // Ein unlesbarer Unterordner erscheint ohne Vermerke, statt die
            // ganze Liste scheitern zu lassen.
            let inner: Vec<(String, bool)> = fs
                .read_dir(&uri)
                .map(|c| {
                    c.into_iter()
                        .map(|e| (e.name().to_string(), matches!(e, Entry::Dir { .. })))
                        .collect()
                })
                .unwrap_or_default();
            out.push(SubfolderEntry::from_children(
                &name,
                &child_path(path, &name),
                inner.iter().map(|(n, d)| (n.as_str(), *d)),
            ));
        }
        Ok(out)
    }

    /// Breitensuche durch den freigegebenen Ordner.
    fn walk(app: &AppHandle, root: &FsUri, filter: &FolderFilter) -> Result<Listing, DavError> {
        let fs = app.android_fs();
        let mut result = Listing::default();
        // Betretene Unterordner — fuer die fehlenden (E-70).
        let mut seen_dirs = HashSet::new();
        let mut queue: Vec<(FsUri, String, usize)> = vec![(root.clone(), String::new(), 0)];

        while let Some((uri, prefix, depth)) = queue.pop() {
            let entries = match fs.read_dir(&uri) {
                Ok(e) => e,
                Err(e) => {
                    // Die Wurzel selbst: ohne sie gibt es nichts zu listen, und
                    // ein leerer Erfolg verschleierte die fehlende Freigabe
                    // (E-40). Ein unlesbarer *Unterordner* dagegen darf den
                    // Lauf nicht scheitern lassen — der Rest wird uebernommen.
                    if prefix.is_empty() {
                        return Err(super::root_unreadable(&e.to_string()));
                    }
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
                        if !filter.descend(&rel) {
                            continue;
                        }
                        seen_dirs.insert(rel.clone());
                        queue.push((uri, rel, depth + 1));
                    }
                    Entry::File {
                        uri,
                        last_modified,
                        len,
                        ..
                    } => {
                        if !filter.file_allowed(&rel) {
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

        if !result.truncated {
            result.missing_subfolders = filter.missing(&seen_dirs);
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

        pub async fn list(&self, _filter: &FolderFilter) -> Result<Listing, DavError> {
            Err(DavError::Url("SAF nicht verfügbar".into()))
        }

        pub async fn subfolders(&self, _path: &str) -> Result<Vec<SubfolderEntry>, DavError> {
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
    fn unlesbare_wurzel_nennt_die_freigabe_und_den_weg_e_40() {
        // Der Android-Zweig von `walk` laeuft nur mit dem SAF-Plugin auf dem
        // Geraet; pruefbar ist hier die Meldung, die er zurueckgibt.
        let e = root_unreadable("No directory or permission");
        let text = e.to_string();
        assert!(text.contains("Ordnerfreigabe fehlt"), "{text}");
        assert!(text.contains("Bearbeiten"), "der Weg zur Behebung steht dabei");
        assert!(text.contains("No directory or permission"), "die Ursache bleibt lesbar");
    }

    #[test]
    fn gesperrter_speicherdienst_nennt_den_weg_in_die_einstellungen_e_71() {
        // Wortlaut vom Testgeraet, wie ihn das Plugin durchreicht.
        let e = read_failed(
            "com.android.externalstorage has no access to content://media/external_primary/images/media/76",
        );
        assert!(matches!(e, DavError::Access(_)), "beendet den Lauf");
        let text = e.to_string();
        assert!(text.contains("Zugriff auf alle Dateien"), "{text}");
        assert!(!text.contains("content://"), "keine Adresse in der Meldung");

        let e = read_failed("permission denied");
        assert!(matches!(e, DavError::Url(_)), "andere Fehler bleiben Einzelfehler");
        assert!(e.to_string().contains("Datei nicht lesbar: permission denied"));
    }

    #[test]
    fn grenzen_entsprechen_den_entfernten_quellen() {
        // Beide Wege müssen gleich weit gehen, sonst zeigt derselbe Ordner je
        // nach Quellenart unterschiedlich viele Bilder.
        assert_eq!(MAX_DEPTH, 8);
        assert_eq!(MAX_FILES, 50_000);
    }
}

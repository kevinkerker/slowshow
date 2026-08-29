//! Synchronisierung entfernter Quellen in den lokalen Cache (FA-26, FA-28, NF-14).
//!
//! Der Ablauf ist zweigeteilt, damit die Entscheidungslogik ohne Server prüfbar
//! bleibt: [`plan`] entscheidet rein rechnerisch, was zu tun ist, und
//! [`sync_source`] führt es aus.
//!
//! Sperrdisziplin: der Cache-Mutex wird nur für kurze Index-Zugriffe gehalten,
//! niemals über einen `await` hinweg. Andernfalls blockierte ein langsamer NAS
//! die Diashow — die genau denselben Cache liest.

use crate::cache::{Cache, DeltaState};
use crate::decode::{self, DecodeError};
use crate::model::{CacheConfig, Source};
use crate::sources::{Listing, RemoteClient, RemoteFile};
use std::collections::HashSet;
use std::sync::Mutex;

/// Zwischenstand eines laufenden Syncs.
///
/// Ohne diesen Rückkanal wäre die Oberfläche während eines Laufs über tausende
/// Bilder stumm — der Nutzer könnte nicht unterscheiden, ob synchronisiert wird
/// oder etwas hängt.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub source_id: String,
    pub source_name: String,
    /// Bereits abgearbeitete Dateien.
    pub done: usize,
    /// Insgesamt zu ladende Dateien. 0, solange noch gelistet wird.
    pub total: usize,
    /// Davon tatsächlich im Cache abgelegt.
    pub stored: usize,
    /// Zuletzt bearbeiteter Pfad.
    pub current: String,
}

/// Ergebnis eines Sync-Laufs — wird dem Frontend als Statusmeldung gereicht.
#[derive(Debug, Default, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub source_id: String,
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    /// An der Quelle gelöschte Bilder, die aus dem Cache entfernt wurden.
    pub removed: usize,
    /// HEIC/Video und Bilder unter der Mindestauflösung (FA-09, FA-29, E-07).
    pub skipped: usize,
    pub failed: usize,
    /// Vom Ringpuffer verdrängte Bilder (FA-27).
    pub evicted: usize,
    /// Die Quelle war größer als die Obergrenzen — Liste unvollständig.
    pub truncated: bool,
    pub error: Option<String>,
}

impl SyncReport {
    pub fn for_source(id: &str) -> Self {
        Self {
            source_id: id.to_string(),
            ..Default::default()
        }
    }

    /// Hat der Lauf den Bildbestand verändert? Nur dann muss die Playlist neu
    /// gebaut werden (FA-28: neue Bilder ohne App-Neustart).
    pub fn changed_anything(&self) -> bool {
        self.added + self.updated + self.removed + self.evicted > 0
    }
}

/// Was ein Sync-Lauf tun muss.
#[derive(Debug, Default)]
pub struct SyncPlan {
    /// Dateien, die geladen werden müssen (neu oder geändert).
    pub to_fetch: Vec<RemoteFile>,
    /// Wie viele davon neu sind — der Rest sind Aktualisierungen.
    pub new_count: usize,
    pub unchanged: usize,
    /// Alle an der Quelle vorhandenen Pfade — Grundlage für das Aufräumen.
    pub seen: HashSet<String>,
}

/// Entscheidet anhand des Cache-Index, was zu laden ist (NF-14).
///
/// Reine Funktion ohne IO — hier steckt die Logik, die der Delta-Sync
/// tatsächlich braucht, und genau die ist ohne NAS testbar.
pub fn plan(cache: &Cache, source_id: &str, files: &[RemoteFile]) -> SyncPlan {
    let index = cache.index();
    let mut plan = SyncPlan::default();

    for f in files {
        plan.seen.insert(f.rel_path.clone());
        match index.delta_state(source_id, &f.rel_path, f.etag.as_deref(), f.size, f.mtime) {
            DeltaState::Unchanged => plan.unchanged += 1,
            DeltaState::New => {
                plan.new_count += 1;
                plan.to_fetch.push(f.clone());
            }
            DeltaState::Changed => plan.to_fetch.push(f.clone()),
        }
    }
    plan
}

/// Synchronisiert eine entfernte Quelle vollständig.
///
/// `protected` enthält die aktuell angezeigten und vorgeladenen Bilder; sie
/// dürfen vom Ringpuffer nicht verdrängt werden (FA-27, FA-31).
/// `on_progress` wird nach jeder Datei gerufen — **niemals** während eine Sperre
/// gehalten wird, damit der Rückruf selbst auf den Cache zugreifen darf
/// (die Playlist wird von dort aus zwischendurch neu gebaut).
///
/// `Send + Sync` ist Pflicht: der Rückruf lebt über `await`-Punkte hinweg, und
/// ohne die Schranke wäre die gesamte Future nicht mehr `Send` — Tauri kann
/// sie dann nicht auf dem Async-Runtime ausführen.
#[allow(clippy::too_many_arguments)]
pub async fn sync_source(
    source: &Source,
    client: &RemoteClient,
    cache: &Mutex<Cache>,
    cfg: &CacheConfig,
    protected: &HashSet<String>,
    now: i64,
    on_progress: &(dyn Fn(SyncProgress) + Send + Sync),
) -> SyncReport {
    let mut report = SyncReport::for_source(&source.id);

    let listing: Listing = match client.list(&source.subfolders).await {
        Ok(l) => l,
        Err(e) => {
            // FA-26: ein Fehler beim Sync darf die Diashow nicht anhalten —
            // sie läuft aus dem Cache weiter.
            log::warn!("Sync von '{}' fehlgeschlagen: {e}", source.name);
            report.error = Some(e.to_string());
            return report;
        }
    };

    log::info!(
        "'{}': {} Bilddatei(en) gefunden, {} uebersprungen",
        source.name,
        listing.files.len(),
        listing.skipped.len()
    );
    report.truncated = listing.truncated;
    report.skipped += listing.skipped.len();
    for path in &listing.skipped {
        // FA-09: übersprungene HEIC-Dateien werden im Log vermerkt.
        log::info!(
            "'{}': übersprungen (nicht unterstütztes Format): {path}",
            source.name
        );
    }

    // Planung unter kurzer Sperre.
    let plan = {
        let guard = cache.lock().expect("Cache-Mutex vergiftet");
        plan(&guard, &source.id, &listing.files)
    };
    report.unchanged = plan.unchanged;

    let total = plan.to_fetch.len();
    let mut stored_count = 0usize;
    let mut fetched_new = 0usize;

    for (index, file) in plan.to_fetch.iter().enumerate() {
        // Zu Beginn der Runde melden, nicht am Ende: die Schleife verlässt
        // jeden Fehlerfall per `continue`. Am Ende gemeldet bliebe die Anzeige
        // bei einem Ordner voller HEIC-Dateien scheinbar stehen, obwohl
        // gearbeitet wird.
        on_progress(SyncProgress {
            source_id: source.id.clone(),
            source_name: source.name.clone(),
            done: index,
            total,
            stored: stored_count,
            current: file.rel_path.clone(),
        });

        // Download außerhalb jeder Sperre.
        let bytes = match client
            .fetch(file, cfg.target_width, cfg.target_height)
            .await
        {
            Ok(b) => b,
            Err(e) => {
                log::warn!(
                    "'{}': {} konnte nicht geladen werden: {e}",
                    source.name,
                    file.rel_path
                );
                report.failed += 1;
                continue;
            }
        };

        // NF-13: Dekodierung und Skalierung im Rust-Prozess, nicht in der WebView.
        let prepared = match decode::prepare(
            &bytes,
            cfg.target_width,
            cfg.target_height,
            cfg.jpeg_quality,
            source.min_width,
            source.min_height,
        ) {
            Ok(p) => p,
            Err(DecodeError::Unsupported(what)) => {
                log::info!("'{}': {} übersprungen ({what})", source.name, file.rel_path);
                report.skipped += 1;
                continue;
            }
            Err(DecodeError::TooSmall { width, height }) => {
                log::debug!(
                    "'{}': {} unter Mindestauflösung ({width}x{height})",
                    source.name,
                    file.rel_path
                );
                report.skipped += 1;
                continue;
            }
            Err(e) => {
                log::warn!(
                    "'{}': {} nicht dekodierbar: {e}",
                    source.name,
                    file.rel_path
                );
                report.failed += 1;
                continue;
            }
        };

        let was_new = {
            let guard = cache.lock().expect("Cache-Mutex vergiftet");
            guard
                .index()
                .by_source_path(&source.id, &file.rel_path)
                .is_none()
        };

        let stored = {
            let mut guard = cache.lock().expect("Cache-Mutex vergiftet");
            guard.store(
                &source.id,
                &file.rel_path,
                &file.file_name,
                prepared,
                file.etag.clone(),
                file.size,
                file.mtime,
                now,
            )
        };

        match stored {
            Ok(_) if was_new => {
                report.added += 1;
                fetched_new += 1;
                stored_count += 1;
            }
            Ok(_) => {
                report.updated += 1;
                stored_count += 1;
            }
            Err(e) => {
                log::error!(
                    "'{}': {} nicht speicherbar: {e}",
                    source.name,
                    file.rel_path
                );
                report.failed += 1;
            }
        }

        // Zweite Meldung nach dem Ablegen: erst hier stimmt `stored`, und der
        // Rückruf baut daran die Playlist mit. Außerhalb jeder Sperre — hier
        // gehalten wäre es ein Deadlock, weil der Rückruf den Cache braucht.
        on_progress(SyncProgress {
            source_id: source.id.clone(),
            source_name: source.name.clone(),
            done: index + 1,
            total,
            stored: stored_count,
            current: file.rel_path.clone(),
        });
    }
    debug_assert!(fetched_new <= plan.new_count);

    // Aufräumen und Ringpuffer — nur wenn die Liste vollständig war, sonst
    // würden abgeschnittene Ergebnisse fälschlich als „gelöscht" gelten.
    let mut guard = cache.lock().expect("Cache-Mutex vergiftet");
    if !listing.truncated {
        report.removed = guard.remove_missing(&source.id, &plan.seen);
    }
    report.evicted = guard.enforce_limit(cfg.max_bytes, protected);
    if let Err(e) = guard.flush() {
        log::error!("Cache-Index konnte nicht geschrieben werden: {e}");
    }

    report
}

/// Nimmt ein Bild einer lokalen SAF-Quelle entgegen (FA-20).
///
/// Wird vom Frontend aufgerufen, weil das Storage Access Framework nur über die
/// Android-Brücke erreichbar ist (R-06). Die Bytes durchlaufen ab hier exakt
/// denselben Weg wie entfernte Quellen — dekodieren, skalieren, ablegen —,
/// sodass auch lokale Bilder displaygerecht im Cache landen (NF-12).
#[allow(clippy::too_many_arguments)]
pub fn ingest_local(
    cache: &Mutex<Cache>,
    source: &Source,
    rel_path: &str,
    file_name: &str,
    bytes: &[u8],
    mtime: Option<i64>,
    cfg: &CacheConfig,
    now: i64,
) -> Result<bool, String> {
    let prepared = match decode::prepare(
        bytes,
        cfg.target_width,
        cfg.target_height,
        cfg.jpeg_quality,
        source.min_width,
        source.min_height,
    ) {
        Ok(p) => p,
        Err(DecodeError::Unsupported(what)) => {
            log::info!("'{}': {rel_path} übersprungen ({what})", source.name);
            return Ok(false);
        }
        Err(DecodeError::TooSmall { .. }) => return Ok(false),
        Err(e) => return Err(e.to_string()),
    };

    let mut guard = cache
        .lock()
        .map_err(|_| "Cache-Mutex vergiftet".to_string())?;
    guard
        .store(
            &source.id,
            rel_path,
            file_name,
            prepared,
            None,
            Some(bytes.len() as u64),
            mtime,
            now,
        )
        .map(|_| true)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::Prepared;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let mut p = std::env::temp_dir();
            p.push(format!("slowshow-sync-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn file(path: &str, etag: Option<&str>, size: Option<u64>, mtime: Option<i64>) -> RemoteFile {
        RemoteFile {
            rel_path: path.into(),
            file_name: path.rsplit('/').next().unwrap().into(),
            etag: etag.map(|s| s.to_string()),
            size,
            mtime,
            file_id: None,
            local_uri: None,
        }
    }

    fn prepared(size: usize) -> Prepared {
        Prepared {
            bytes: vec![1u8; size],
            width: 1920,
            height: 1080,
            taken_at: None,
        }
    }

    #[test]
    fn plan_laedt_beim_erstlauf_alles() {
        let dir = TempDir::new("plan-first");
        let cache = Cache::open(&dir.0).unwrap();
        let files = vec![
            file("1.jpg", Some("a"), Some(10), Some(1)),
            file("2.jpg", None, None, None),
        ];

        let p = plan(&cache, "s1", &files);
        assert_eq!(p.to_fetch.len(), 2);
        assert_eq!(p.new_count, 2);
        assert_eq!(p.unchanged, 0);
        assert_eq!(p.seen.len(), 2);
    }

    #[test]
    fn plan_ueberspringt_unveraenderte_dateien_nf_14() {
        let dir = TempDir::new("plan-delta");
        let mut cache = Cache::open(&dir.0).unwrap();
        cache
            .store(
                "s1",
                "1.jpg",
                "1.jpg",
                prepared(100),
                Some("a".into()),
                Some(10),
                Some(1),
                0,
            )
            .unwrap();

        let files = vec![file("1.jpg", Some("a"), Some(10), Some(1))];
        let p = plan(&cache, "s1", &files);

        assert!(p.to_fetch.is_empty(), "gleiches ETag -> kein Download");
        assert_eq!(p.unchanged, 1);
    }

    #[test]
    fn plan_erkennt_geaenderte_dateien() {
        let dir = TempDir::new("plan-changed");
        let mut cache = Cache::open(&dir.0).unwrap();
        cache
            .store(
                "s1",
                "1.jpg",
                "1.jpg",
                prepared(100),
                Some("alt".into()),
                Some(10),
                Some(1),
                0,
            )
            .unwrap();

        let p = plan(
            &cache,
            "s1",
            &[file("1.jpg", Some("neu"), Some(99), Some(2))],
        );
        assert_eq!(p.to_fetch.len(), 1);
        assert_eq!(p.new_count, 0, "eine Aktualisierung ist kein Neuzugang");
    }

    #[test]
    fn plan_trennt_quellen_voneinander() {
        let dir = TempDir::new("plan-sources");
        let mut cache = Cache::open(&dir.0).unwrap();
        cache
            .store(
                "s1",
                "1.jpg",
                "1.jpg",
                prepared(100),
                Some("a".into()),
                None,
                None,
                0,
            )
            .unwrap();

        // Gleicher Pfad, andere Quelle -> muss geladen werden.
        let p = plan(&cache, "s2", &[file("1.jpg", Some("a"), None, None)]);
        assert_eq!(p.to_fetch.len(), 1);
        assert_eq!(p.new_count, 1);
    }

    #[test]
    fn plan_sammelt_alle_pfade_fuer_das_aufraeumen() {
        let dir = TempDir::new("plan-seen");
        let cache = Cache::open(&dir.0).unwrap();
        let files = vec![
            file("a/1.jpg", None, None, None),
            file("b/2.jpg", None, None, None),
        ];
        let p = plan(&cache, "s1", &files);
        assert!(p.seen.contains("a/1.jpg"));
        assert!(p.seen.contains("b/2.jpg"));
    }

    #[test]
    fn report_erkennt_ob_sich_etwas_geaendert_hat() {
        let mut r = SyncReport::for_source("s");
        r.unchanged = 100;
        assert!(!r.changed_anything(), "nur unveraendert -> Playlist bleibt");

        r.added = 1;
        assert!(r.changed_anything());

        let mut r2 = SyncReport::for_source("s");
        r2.removed = 1;
        assert!(r2.changed_anything());
    }

    #[test]
    fn ingest_local_legt_bild_im_cache_ab_fa_20() {
        let dir = TempDir::new("ingest");
        let cache = Mutex::new(Cache::open(&dir.0).unwrap());
        let source = local_source();

        // Echtes JPEG erzeugen, damit der Dekodierpfad durchlaufen wird.
        let jpeg = test_jpeg(400, 300);
        let cfg = CacheConfig::default();
        let ok = ingest_local(
            &cache,
            &source,
            "DCIM/1.jpg",
            "1.jpg",
            &jpeg,
            Some(5),
            &cfg,
            100,
        )
        .unwrap();

        assert!(ok);
        let guard = cache.lock().unwrap();
        let e = guard
            .index()
            .by_source_path("lokal", "DCIM/1.jpg")
            .expect("Eintrag erwartet");
        assert_eq!(e.file_name, "1.jpg");
        assert_eq!(e.remote_mtime, Some(5));
    }

    #[test]
    fn ingest_local_ueberspringt_heic_ohne_fehler_fa_09() {
        let dir = TempDir::new("ingest-heic");
        let cache = Mutex::new(Cache::open(&dir.0).unwrap());
        let mut heic = vec![0, 0, 0, 0x18];
        heic.extend_from_slice(b"ftypheic");
        heic.extend_from_slice(b"0000");

        let ok = ingest_local(
            &cache,
            &local_source(),
            "1.HEIC",
            "1.HEIC",
            &heic,
            None,
            &CacheConfig::default(),
            0,
        )
        .unwrap();
        assert!(!ok, "uebersprungen, aber kein Fehler");
        assert_eq!(cache.lock().unwrap().index().len(), 0);
    }

    #[test]
    fn ingest_local_haelt_die_mindestaufloesung_ein_fa_29() {
        let dir = TempDir::new("ingest-small");
        let cache = Mutex::new(Cache::open(&dir.0).unwrap());
        let mut source = local_source();
        source.min_width = 1024;
        source.min_height = 768;

        let ok = ingest_local(
            &cache,
            &source,
            "klein.jpg",
            "klein.jpg",
            &test_jpeg(320, 240),
            None,
            &CacheConfig::default(),
            0,
        )
        .unwrap();
        assert!(!ok);
    }

    #[test]
    fn ingest_local_meldet_kaputte_datei_als_fehler() {
        let dir = TempDir::new("ingest-broken");
        let cache = Mutex::new(Cache::open(&dir.0).unwrap());
        let r = ingest_local(
            &cache,
            &local_source(),
            "1.jpg",
            "1.jpg",
            b"kein bild",
            None,
            &CacheConfig::default(),
            0,
        );
        assert!(r.is_err());
    }

    // ── Hilfen ──────────────────────────────────────────────────────────────

    fn local_source() -> Source {
        use crate::model::SourceKind;
        Source {
            id: "lokal".into(),
            name: "Tablet".into(),
            kind: SourceKind::Local {
                saf_uri: "{}".into(),
                display_path: "DCIM".into(),
            },
            enabled: true,
            subfolders: vec![],
            min_width: 0,
            min_height: 0,
            sync_interval_minutes: 60,
            last_sync: None,
        }
    }

    fn test_jpeg(w: u32, h: u32) -> Vec<u8> {
        use image::ImageEncoder;
        let img = image::RgbImage::new(w, h);
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85)
            .write_image(img.as_raw(), w, h, image::ExtendedColorType::Rgb8)
            .unwrap();
        out
    }
}

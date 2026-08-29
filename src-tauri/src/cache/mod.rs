//! Permanenter, rotierender Bild-Cache (FA-26, FA-27).
//!
//! Die Diashow liest ausschließlich aus diesem Cache. Dadurch läuft sie auch
//! bei Netzwerkausfall unterbrechungsfrei weiter (FA-26) und übersteht App- und
//! Geräteneustart (FA-27), weil sowohl die Bilddateien als auch der Index auf
//! dem Gerät liegen.
//!
//! Aufteilung: [`index`] hält die reine, testbare Datenstruktur, dieses Modul
//! das Dateisystem.

pub mod index;

pub use index::{CacheEntry, CacheIndex, DeltaState};

use crate::decode::Prepared;
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Cache-IO fehlgeschlagen: {0}")]
    Io(#[from] std::io::Error),
    #[error("Cache-Index ist beschädigt: {0}")]
    Corrupt(#[from] serde_json::Error),
}

/// Kennzahlen für die Einstellungsoberfläche (Fußzeile des Design-Entwurfs).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    pub images: usize,
    pub bytes: u64,
    pub max_bytes: u64,
    /// Wie viele Bilder aktuell aus der Diashow ausgeschlossen sind (FA-30).
    pub excluded: usize,
}

pub struct Cache {
    root: PathBuf,
    index: CacheIndex,
    /// Der Index wurde seit dem letzten Schreiben verändert.
    ///
    /// Nötig, weil `mark_shown` bei jedem Bildwechsel läuft — den kompletten
    /// Index dabei jedes Mal zu schreiben, wäre im Dauerbetrieb sinnlose
    /// Schreiblast auf dem Flash-Speicher (R-08).
    dirty: bool,
}

impl Cache {
    /// Öffnet den Cache unter `root` und lädt den Index.
    ///
    /// Ein fehlender oder beschädigter Index ist kein harter Fehler: dann
    /// startet die App mit leerem Index und baut ihn beim nächsten Sync neu auf
    /// (NF-02, Selbstheilung).
    pub fn open(root: impl AsRef<Path>) -> Result<Self, CacheError> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(root.join("images"))?;

        let index_path = root.join("index.json");
        let mut index = match std::fs::read(&index_path) {
            Ok(bytes) => serde_json::from_slice::<CacheIndex>(&bytes).unwrap_or_else(|e| {
                log::warn!("Cache-Index unlesbar, starte mit leerem Index: {e}");
                CacheIndex::new()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => CacheIndex::new(),
            Err(e) => return Err(e.into()),
        };
        index.rebuild_lookup();

        let mut cache = Self {
            root,
            index,
            dirty: false,
        };
        cache.drop_entries_without_file();
        Ok(cache)
    }

    /// Entfernt Indexeinträge, deren Bilddatei fehlt.
    ///
    /// Kann nach einem Absturz mitten im Schreiben auftreten. Ohne diesen
    /// Abgleich lieferte die Diashow tote Verweise und würde stocken (NF-02).
    fn drop_entries_without_file(&mut self) {
        let missing: Vec<String> = self
            .index
            .values()
            .filter(|e| !self.image_path(&e.id).exists())
            .map(|e| e.id.clone())
            .collect();
        if missing.is_empty() {
            return;
        }
        log::warn!("{} Cache-Einträge ohne Datei entfernt", missing.len());
        for id in missing {
            self.index.remove(&id);
        }
        self.dirty = true;
    }

    pub fn index(&self) -> &CacheIndex {
        &self.index
    }

    /// Pfad der Cache-Datei zu einer Id. Ausschließlich hier gebildet, damit
    /// keine vom Frontend gelieferte Id in einen Pfad außerhalb des Caches
    /// zeigen kann.
    pub fn image_path(&self, id: &str) -> PathBuf {
        self.root.join("images").join(format!("{id}.jpg"))
    }

    /// Liest eine Cache-Datei. Wird vom Asset-Protokoll bedient.
    ///
    /// Die Id wird gegen den Index geprüft — damit kann eine manipulierte URL
    /// keine beliebige Datei vom Gerät ausliefern.
    pub fn read_image(&self, id: &str) -> Option<Vec<u8>> {
        self.index.get(id)?;
        std::fs::read(self.image_path(id)).ok()
    }

    /// Legt ein aufbereitetes Bild ab und aktualisiert den Index.
    ///
    /// Ein bereits vorhandener Eintrag desselben Quellpfads wird überschrieben,
    /// nicht dupliziert (siehe [`CacheIndex::allocate_id`]).
    #[allow(clippy::too_many_arguments)]
    pub fn store(
        &mut self,
        source_id: &str,
        rel_path: &str,
        file_name: &str,
        prepared: Prepared,
        etag: Option<String>,
        remote_size: Option<u64>,
        remote_mtime: Option<i64>,
        now: i64,
    ) -> Result<CacheEntry, CacheError> {
        let id = self.index.allocate_id(source_id, rel_path);
        let path = self.image_path(&id);
        write_atomic(&path, &prepared.bytes)?;

        // Anzeigehistorie eines ersetzten Eintrags erhalten, damit ein Update
        // ein Bild nicht künstlich vor der Verdrängung schützt (FA-27).
        let last_shown = self.index.get(&id).and_then(|e| e.last_shown);
        let excluded = self.index.get(&id).map(|e| e.excluded).unwrap_or(false);

        let entry = CacheEntry {
            id,
            source_id: source_id.to_string(),
            rel_path: rel_path.to_string(),
            file_name: file_name.to_string(),
            etag,
            remote_size,
            remote_mtime,
            taken_at: prepared.taken_at,
            width: prepared.width,
            height: prepared.height,
            bytes: prepared.bytes.len() as u64,
            added_at: now,
            last_shown,
            excluded,
        };
        self.index.insert(entry.clone());
        self.dirty = true;
        Ok(entry)
    }

    /// Löscht Bild und Indexeintrag.
    pub fn remove(&mut self, id: &str) {
        if self.index.remove(id).is_some() {
            let _ = std::fs::remove_file(self.image_path(id));
            self.dirty = true;
        }
    }

    /// Entfernt alle Bilder einer Quelle — beim Löschen der Quelle.
    pub fn remove_source(&mut self, source_id: &str) -> usize {
        let ids = self.index.ids_for_source(source_id);
        let n = ids.len();
        for id in ids {
            self.remove(&id);
        }
        n
    }

    /// Entfernt Bilder, die an der Quelle nicht mehr existieren.
    pub fn remove_missing(&mut self, source_id: &str, seen: &HashSet<String>) -> usize {
        let ids = self.index.ids_missing_at_source(source_id, seen);
        let n = ids.len();
        for id in ids {
            self.remove(&id);
        }
        n
    }

    /// Setzt den Ringpuffer durch (FA-27). Gibt die Zahl verdrängter Bilder zurück.
    pub fn enforce_limit(&mut self, max_bytes: u64, protected: &HashSet<String>) -> usize {
        let victims = self.index.select_for_eviction(max_bytes, protected);
        let n = victims.len();
        for id in victims {
            self.remove(&id);
        }
        if n > 0 {
            log::info!("Ringpuffer: {n} Bilder verdrängt");
        }
        n
    }

    pub fn mark_shown(&mut self, id: &str, now: i64) {
        if self.index.mark_shown(id, now) {
            self.dirty = true;
        }
    }

    /// FA-30: Bild aus der Diashow nehmen, ohne es an der Quelle zu löschen.
    pub fn set_excluded(&mut self, id: &str, excluded: bool) -> bool {
        let ok = self.index.set_excluded(id, excluded);
        if ok {
            self.dirty = true;
        }
        ok
    }

    pub fn stats(&self, max_bytes: u64) -> CacheStats {
        CacheStats {
            images: self.index.len(),
            bytes: self.index.total_bytes(),
            max_bytes,
            excluded: self.index.values().filter(|e| e.excluded).count(),
        }
    }

    /// Schreibt den Index, falls er sich geändert hat.
    /// Wird periodisch, beim Pausieren der App und beim Beenden aufgerufen.
    pub fn flush(&mut self) -> Result<(), CacheError> {
        if !self.dirty {
            return Ok(());
        }
        let bytes = serde_json::to_vec(&self.index)?;
        write_atomic(&self.root.join("index.json"), &bytes)?;
        self.dirty = false;
        Ok(())
    }
}

/// Schreibt über eine temporäre Datei und benennt sie um.
///
/// Ohne das könnte ein Stromausfall mitten im Schreiben (R-08, im Dauerbetrieb
/// realistisch) einen halben Index oder ein halbes Bild hinterlassen.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Legt ein temporäres Cache-Verzeichnis an. Bewusst ohne `tempfile`-Crate,
    /// um die Abhängigkeitsliste klein zu halten (NF-10).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let mut p = std::env::temp_dir();
            p.push(format!("slowshow-test-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn prepared(size: usize) -> Prepared {
        Prepared {
            bytes: vec![0xAB; size],
            width: 1920,
            height: 1080,
            taken_at: Some(1700),
        }
    }

    #[test]
    fn store_legt_datei_und_indexeintrag_an() {
        let dir = TempDir::new("store");
        let mut cache = Cache::open(dir.path()).unwrap();

        let entry = cache
            .store(
                "s1",
                "a/1.jpg",
                "1.jpg",
                prepared(500),
                Some("tag".into()),
                Some(9),
                Some(7),
                100,
            )
            .unwrap();

        assert_eq!(entry.bytes, 500);
        assert_eq!(entry.taken_at, Some(1700));
        assert!(cache.image_path(&entry.id).exists());
        assert_eq!(cache.read_image(&entry.id).unwrap().len(), 500);
        assert_eq!(cache.index().total_bytes(), 500);
    }

    #[test]
    fn store_ueberschreibt_statt_zu_duplizieren() {
        let dir = TempDir::new("overwrite");
        let mut cache = Cache::open(dir.path()).unwrap();

        let a = cache
            .store("s1", "1.jpg", "1.jpg", prepared(500), None, None, None, 100)
            .unwrap();
        let b = cache
            .store("s1", "1.jpg", "1.jpg", prepared(300), None, None, None, 200)
            .unwrap();

        assert_eq!(a.id, b.id, "derselbe Quellpfad behält seine Cache-Datei");
        assert_eq!(cache.index().len(), 1);
        assert_eq!(cache.index().total_bytes(), 300, "Größe wurde aktualisiert");
    }

    #[test]
    fn store_erhaelt_anzeigehistorie_ueber_updates() {
        let dir = TempDir::new("history");
        let mut cache = Cache::open(dir.path()).unwrap();
        let a = cache
            .store("s1", "1.jpg", "1.jpg", prepared(100), None, None, None, 10)
            .unwrap();
        cache.mark_shown(&a.id, 555);
        cache.set_excluded(&a.id, true);

        let b = cache
            .store("s1", "1.jpg", "1.jpg", prepared(100), None, None, None, 20)
            .unwrap();
        assert_eq!(
            b.last_shown,
            Some(555),
            "sonst entkäme das Bild der Verdrängung"
        );
        assert!(b.excluded, "der Ausschluss aus FA-30 überlebt ein Update");
    }

    #[test]
    fn read_image_lehnt_unbekannte_id_ab() {
        let dir = TempDir::new("guard");
        let cache = Cache::open(dir.path()).unwrap();
        // Schutz gegen manipulierte Asset-URLs.
        assert!(cache.read_image("../../etc/passwd").is_none());
        assert!(cache.read_image("gibtsnicht").is_none());
    }

    #[test]
    fn remove_loescht_datei_und_eintrag() {
        let dir = TempDir::new("remove");
        let mut cache = Cache::open(dir.path()).unwrap();
        let e = cache
            .store("s1", "1.jpg", "1.jpg", prepared(100), None, None, None, 0)
            .unwrap();
        let path = cache.image_path(&e.id);

        cache.remove(&e.id);
        assert!(!path.exists());
        assert_eq!(cache.index().len(), 0);
    }

    #[test]
    fn remove_source_raeumt_nur_die_eigene_quelle() {
        let dir = TempDir::new("removesrc");
        let mut cache = Cache::open(dir.path()).unwrap();
        cache
            .store("s1", "1.jpg", "1.jpg", prepared(100), None, None, None, 0)
            .unwrap();
        cache
            .store("s1", "2.jpg", "2.jpg", prepared(100), None, None, None, 0)
            .unwrap();
        cache
            .store("s2", "3.jpg", "3.jpg", prepared(100), None, None, None, 0)
            .unwrap();

        assert_eq!(cache.remove_source("s1"), 2);
        assert_eq!(cache.index().len(), 1);
        assert_eq!(cache.index().count_for_source("s2"), 1);
    }

    #[test]
    fn enforce_limit_setzt_ringpuffer_durch_fa_27() {
        let dir = TempDir::new("ring");
        let mut cache = Cache::open(dir.path()).unwrap();
        for i in 0..5 {
            let e = cache
                .store(
                    "s1",
                    &format!("{i}.jpg"),
                    "x.jpg",
                    prepared(100),
                    None,
                    None,
                    None,
                    0,
                )
                .unwrap();
            cache.mark_shown(&e.id, i as i64);
        }
        assert_eq!(cache.index().total_bytes(), 500);

        let evicted = cache.enforce_limit(250, &HashSet::new());
        assert_eq!(evicted, 3);
        assert!(cache.index().total_bytes() <= 250);
        assert_eq!(cache.index().len(), 2);
    }

    #[test]
    fn cache_ueberlebt_neustart_fa_27() {
        let dir = TempDir::new("persist");
        let id = {
            let mut cache = Cache::open(dir.path()).unwrap();
            let e = cache
                .store(
                    "s1",
                    "1.jpg",
                    "1.jpg",
                    prepared(400),
                    Some("t".into()),
                    None,
                    None,
                    42,
                )
                .unwrap();
            cache.mark_shown(&e.id, 99);
            cache.flush().unwrap();
            e.id
        };

        // Zweiter Start — simuliert Geräteneustart.
        let cache = Cache::open(dir.path()).unwrap();
        let e = cache
            .index()
            .get(&id)
            .expect("Eintrag muss den Neustart überleben");
        assert_eq!(e.bytes, 400);
        assert_eq!(e.last_shown, Some(99));
        assert_eq!(e.etag.as_deref(), Some("t"));
        assert!(cache.read_image(&id).is_some());
        // Der Sekundärindex muss nach dem Laden wieder stehen.
        assert!(cache.index().by_source_path("s1", "1.jpg").is_some());
    }

    #[test]
    fn open_verwirft_eintraege_ohne_datei_nf_02() {
        let dir = TempDir::new("selfheal");
        let id = {
            let mut cache = Cache::open(dir.path()).unwrap();
            let e = cache
                .store("s1", "1.jpg", "1.jpg", prepared(100), None, None, None, 0)
                .unwrap();
            cache.flush().unwrap();
            e.id
        };
        // Absturz mitten im Schreiben simulieren: Datei weg, Index noch da.
        std::fs::remove_file(dir.path().join("images").join(format!("{id}.jpg"))).unwrap();

        let cache = Cache::open(dir.path()).unwrap();
        assert_eq!(cache.index().len(), 0, "toter Verweis muss verschwinden");
    }

    #[test]
    fn open_toleriert_kaputten_index_nf_02() {
        let dir = TempDir::new("corrupt");
        std::fs::create_dir_all(dir.path().join("images")).unwrap();
        std::fs::write(dir.path().join("index.json"), b"{ das ist kein json").unwrap();

        let cache = Cache::open(dir.path()).expect("darf nicht scheitern");
        assert_eq!(cache.index().len(), 0);
    }

    #[test]
    fn flush_schreibt_nur_bei_aenderung() {
        let dir = TempDir::new("dirty");
        let mut cache = Cache::open(dir.path()).unwrap();
        let index_path = dir.path().join("index.json");

        cache.flush().unwrap();
        assert!(!index_path.exists(), "ohne Änderung wird nicht geschrieben");

        cache
            .store("s1", "1.jpg", "1.jpg", prepared(10), None, None, None, 0)
            .unwrap();
        cache.flush().unwrap();
        assert!(index_path.exists());
    }

    #[test]
    fn stats_meldet_kennzahlen_fuer_die_oberflaeche() {
        let dir = TempDir::new("stats");
        let mut cache = Cache::open(dir.path()).unwrap();
        let a = cache
            .store("s1", "1.jpg", "1.jpg", prepared(100), None, None, None, 0)
            .unwrap();
        cache
            .store("s1", "2.jpg", "2.jpg", prepared(200), None, None, None, 0)
            .unwrap();
        cache.set_excluded(&a.id, true);

        let s = cache.stats(1000);
        assert_eq!(s.images, 2);
        assert_eq!(s.bytes, 300);
        assert_eq!(s.max_bytes, 1000);
        assert_eq!(s.excluded, 1);
    }
}

//! Cache-Index: Metadaten aller zwischengespeicherten Bilder.
//!
//! Bewusst eine reine In-Memory-Struktur mit JSON-Persistenz statt SQLite.
//! Bei der Zielgröße des Projekts (Abnahmekriterium: 5 000 Bilder) sind das
//! wenige MB, die einmal beim Start geladen werden. Das erspart die
//! Cross-Kompilierung von libsqlite3 für Android — dieselbe Aufwandsklasse,
//! die das Lastenheft bei HEIC (E-04) und SMB (E-02) bewusst meidet.
//!
//! Dieses Modul ist frei von Dateisystemzugriffen und dadurch vollständig
//! ohne Testverzeichnis prüfbar. Das IO liegt in `cache::Cache`.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Herkunft eines per Mail eingetroffenen Fotos (E-30).
///
/// Als eigener Teilsatz und nicht als vier lose Felder am Eintrag: Absender,
/// Betreff und Quarantäne haben nur bei Mail-Fotos einen Sinn, und ein
/// `Option` sagt das deutlicher als vier `Option`s nebeneinander.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MailMeta {
    /// Absenderadresse in Kleinschreibung.
    pub sender: String,
    pub subject: String,
    /// Hash der Message-ID — Schutz gegen Doppelimport (F2).
    pub message_id: String,
    /// Wartet das Foto auf Freigabe? (F4)
    ///
    /// Quarantäne statt Löschen: ein unbekannter Absender ist meist die Tante,
    /// die zum ersten Mal schickt, und nicht ein Angriff.
    #[serde(default)]
    pub quarantined: bool,
}

/// Ein Eintrag im Cache — beschreibt genau eine aufbereitete Bilddatei.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheEntry {
    /// Dateiname im Cache-Verzeichnis (ohne Endung).
    pub id: String,
    pub source_id: String,
    /// Pfad relativ zur Quellenwurzel — identifiziert die Datei an der Quelle.
    pub rel_path: String,
    pub file_name: String,
    /// ETag der Quelle, primäres Delta-Kriterium (NF-14).
    #[serde(default)]
    pub etag: Option<String>,
    /// Dateigröße an der Quelle, Delta-Kriterium falls kein ETag geliefert wird.
    #[serde(default)]
    pub remote_size: Option<u64>,
    /// Änderungsdatum an der Quelle als Unix-Zeitstempel (FA-03 Sortierung).
    #[serde(default)]
    pub remote_mtime: Option<i64>,
    /// EXIF-Aufnahmedatum (FA-03 Sortierung, FA-07 Overlay).
    #[serde(default)]
    pub taken_at: Option<i64>,
    /// Maße der Cache-Ablage nach Skalierung (NF-12).
    pub width: u32,
    pub height: u32,
    /// Größe der Cache-Datei in Bytes — Grundlage des Ringpuffers (FA-27).
    pub bytes: u64,
    pub added_at: i64,
    /// Zeitpunkt der letzten Anzeige. `None` = noch nie gezeigt.
    #[serde(default)]
    pub last_shown: Option<i64>,
    /// Wie oft das Bild bereits gezeigt wurde (E-29).
    ///
    /// Gilt quellenübergreifend. Fließt heute nur in die Statistik der Wartung
    /// ein; das Erweiterungspapier hält den Wert ausdrücklich als Vorhalt für
    /// spätere Gewichtungen vor.
    #[serde(default)]
    pub show_count: u32,
    /// Aus der Diashow entfernt, ohne an der Quelle zu löschen (FA-30).
    #[serde(default)]
    pub excluded: bool,
    /// Herkunft, falls das Bild per Mail kam (E-30).
    #[serde(default)]
    pub mail: Option<MailMeta>,
    /// Größe des Vorschaubilds, sobald eines erzeugt wurde (E-25).
    ///
    /// `None` heißt „noch keins" — Vorschaubilder entstehen beim ersten
    /// Betrachten im Bild-Browser, nicht beim Synchronisieren. Der Wert geht in
    /// die Cachegröße ein, damit die Anzeige in den Einstellungen nicht
    /// systematisch zu klein ist.
    #[serde(default)]
    pub thumb_bytes: Option<u64>,
}

impl CacheEntry {
    /// Wartet das Bild auf Freigabe? (F4)
    ///
    /// Quarantänefotos laufen nicht in der Diashow und zählen nicht in die
    /// Urne — sie sind noch nicht Teil des Bestands, nur schon im Cache.
    pub fn is_quarantined(&self) -> bool {
        self.mail.as_ref().is_some_and(|m| m.quarantined)
    }

    /// Ist das Bild im Hochformat? Grundlage des Paar-Modus (FA-08).
    pub fn is_portrait(&self) -> bool {
        self.height > self.width
    }

    /// Sortierschlüssel für `PlayOrder::TakenAt` — Aufnahmedatum mit Rückfall
    /// auf das Änderungsdatum und zuletzt auf den Aufnahmezeitpunkt im Cache.
    pub fn sort_time(&self) -> i64 {
        self.taken_at.or(self.remote_mtime).unwrap_or(self.added_at)
    }

    /// Verdrängungsschlüssel des Ringpuffers (FA-27): „älteste bzw. am
    /// längsten nicht angezeigte" Bilder zuerst. Noch nie gezeigte Bilder
    /// zählen ab ihrem Eintreffen, damit frisch synchronisierte Fotos nicht
    /// sofort wieder verdrängt werden.
    pub fn eviction_key(&self) -> i64 {
        self.last_shown.unwrap_or(self.added_at)
    }
}

/// Zusammengesetzter Schlüssel einer Quelldatei.
/// `\u{1}` als Trenner, weil es in Dateipfaden nicht vorkommt.
pub fn key_of(source_id: &str, rel_path: &str) -> String {
    format!("{source_id}\u{1}{rel_path}")
}

/// FNV-1a-64 über den Schlüssel — liefert den Basisnamen der Cache-Datei.
/// Kollisionen sind bei 5 000 Einträgen praktisch ausgeschlossen, werden von
/// [`CacheIndex::allocate_id`] aber trotzdem sauber aufgelöst.
fn fnv1a(key: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Ergebnis der Delta-Prüfung beim Sync (NF-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaState {
    /// Datei ist neu — herunterladen.
    New,
    /// Datei hat sich geändert — erneut herunterladen.
    Changed,
    /// Unverändert — überspringen, kein Netzverkehr.
    Unchanged,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheIndex {
    /// id -> Eintrag.
    entries: HashMap<String, CacheEntry>,
    /// Aus `entries` ableitbar, daher nicht persistiert.
    #[serde(skip)]
    by_key: HashMap<String, String>,
}

impl CacheIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Baut den Sekundärindex nach dem Laden aus JSON wieder auf.
    pub fn rebuild_lookup(&mut self) {
        self.by_key = self
            .entries
            .values()
            .map(|e| (key_of(&e.source_id, &e.rel_path), e.id.clone()))
            .collect();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&CacheEntry> {
        self.entries.get(id)
    }

    /// Veränderbarer Zugriff — gebraucht, um die Größe eines nachträglich
    /// erzeugten Vorschaubilds nachzutragen (E-25).
    pub fn get_mut(&mut self, id: &str) -> Option<&mut CacheEntry> {
        self.entries.get_mut(id)
    }

    /// Setzt Anzeigezeitpunkt und Zaehler zurueck (Wartung F3, E-29).
    ///
    /// Gibt zurueck, wie viele Eintraege sich geaendert haben.
    pub fn reset_history(&mut self, ids: &[String]) -> usize {
        let mut n = 0;
        for id in ids {
            if let Some(e) = self.entries.get_mut(id) {
                if e.last_shown.is_some() || e.show_count > 0 {
                    e.last_shown = None;
                    e.show_count = 0;
                    n += 1;
                }
            }
        }
        n
    }

    pub fn by_source_path(&self, source_id: &str, rel_path: &str) -> Option<&CacheEntry> {
        let id = self.by_key.get(&key_of(source_id, rel_path))?;
        self.entries.get(id)
    }

    pub fn values(&self) -> impl Iterator<Item = &CacheEntry> {
        self.entries.values()
    }

    /// Wie [`Self::values`], aber veraenderbar — fuer Reihenaenderungen an
    /// vielen Eintraegen (etwa alle Fotos eines Absenders, F4).
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut CacheEntry> {
        self.entries.values_mut()
    }

    /// Summe aller Cache-Dateien in Bytes — Vergleichswert für FA-27.
    pub fn total_bytes(&self) -> u64 {
        self.entries.values().map(|e| e.bytes).sum()
    }

    /// Summe der bereits erzeugten Vorschaubilder (E-25).
    ///
    /// Getrennt von [`Self::total_bytes`], weil der Ringpuffer (FA-27) über die
    /// Bilddateien verdrängt: flösse die Vorschau in denselben Wert ein, würde
    /// der Puffer früher verdrängen, als die Cachegröße es hergibt.
    pub fn thumb_bytes(&self) -> u64 {
        self.entries.values().filter_map(|e| e.thumb_bytes).sum()
    }

    pub fn count_for_source(&self, source_id: &str) -> usize {
        self.entries
            .values()
            .filter(|e| e.source_id == source_id)
            .count()
    }

    /// Vergibt einen freien Dateinamen für einen Schlüssel.
    ///
    /// Ein bereits vorhandener Eintrag für denselben Schlüssel behält seinen
    /// Namen, damit ein Update dieselbe Datei überschreibt statt zu duplizieren.
    pub fn allocate_id(&self, source_id: &str, rel_path: &str) -> String {
        let key = key_of(source_id, rel_path);
        if let Some(existing) = self.by_key.get(&key) {
            return existing.clone();
        }
        let base = fnv1a(&key);
        let mut candidate = format!("{base:016x}");
        let mut probe: u64 = 0;
        while self.entries.contains_key(&candidate) {
            probe += 1;
            candidate = format!("{:016x}", base.wrapping_add(probe));
        }
        candidate
    }

    /// Delta-Prüfung vor dem Download (NF-14).
    ///
    /// ETag hat Vorrang; liefert der Server keinen, entscheiden Größe und
    /// Änderungsdatum gemeinsam. Fehlt beides, gilt die Datei als geändert —
    /// lieber einmal zu viel laden als ein Update verpassen.
    pub fn delta_state(
        &self,
        source_id: &str,
        rel_path: &str,
        etag: Option<&str>,
        size: Option<u64>,
        mtime: Option<i64>,
    ) -> DeltaState {
        let Some(existing) = self.by_source_path(source_id, rel_path) else {
            return DeltaState::New;
        };
        if let (Some(new_tag), Some(old_tag)) = (etag, existing.etag.as_deref()) {
            return if new_tag == old_tag {
                DeltaState::Unchanged
            } else {
                DeltaState::Changed
            };
        }
        match (size, mtime, existing.remote_size, existing.remote_mtime) {
            (Some(s), Some(m), Some(os), Some(om)) if s == os && m == om => DeltaState::Unchanged,
            _ => DeltaState::Changed,
        }
    }

    pub fn insert(&mut self, entry: CacheEntry) {
        self.by_key
            .insert(key_of(&entry.source_id, &entry.rel_path), entry.id.clone());
        self.entries.insert(entry.id.clone(), entry);
    }

    pub fn remove(&mut self, id: &str) -> Option<CacheEntry> {
        let entry = self.entries.remove(id)?;
        self.by_key
            .remove(&key_of(&entry.source_id, &entry.rel_path));
        Some(entry)
    }

    /// Merkt die Anzeige eines Bildes — steuert die Verdrängung (FA-27).
    /// Gibt `true` zurück, wenn der Index dadurch verändert wurde.
    pub fn mark_shown(&mut self, id: &str, now: i64) -> bool {
        match self.entries.get_mut(id) {
            Some(e) => {
                e.last_shown = Some(now);
                // Nur bei tatsaechlicher Anzeige, nicht bei verworfenen
                // Ziehungen des Cluster-Filters (E-29) — `mark_shown` wird
                // ausschliesslich nach einem Bildwechsel gerufen.
                e.show_count = e.show_count.saturating_add(1);
                true
            }
            None => false,
        }
    }

    /// Nimmt ein Bild aus der Diashow, ohne es an der Quelle zu löschen (FA-30).
    pub fn set_excluded(&mut self, id: &str, excluded: bool) -> bool {
        match self.entries.get_mut(id) {
            Some(e) => {
                e.excluded = excluded;
                true
            }
            None => false,
        }
    }

    /// Alle Einträge einer Quelle — für das Entfernen einer Quelle.
    pub fn ids_for_source(&self, source_id: &str) -> Vec<String> {
        self.entries
            .values()
            .filter(|e| e.source_id == source_id)
            .map(|e| e.id.clone())
            .collect()
    }

    /// Einträge, deren Datei an der Quelle nicht mehr existiert.
    ///
    /// `seen` enthält die beim letzten vollständigen Listing gefundenen Pfade.
    /// Ohne diesen Schritt blieben an der Quelle gelöschte Fotos für immer im
    /// Cache und tauchten in der Diashow wieder auf.
    pub fn ids_missing_at_source(&self, source_id: &str, seen: &HashSet<String>) -> Vec<String> {
        self.entries
            .values()
            .filter(|e| e.source_id == source_id && !seen.contains(&e.rel_path))
            .map(|e| e.id.clone())
            .collect()
    }

    /// Wählt Einträge aus, die verdrängt werden müssen, damit der Cache unter
    /// `max_bytes` bleibt (FA-27, Ringpuffer).
    ///
    /// `protected` schützt die aktuell angezeigten und vorgeladenen Bilder —
    /// ohne das könnte der Ringpuffer das Bild löschen, das gerade auf dem
    /// Schirm ist. Ausgeschlossene Bilder (FA-30) fliegen zuerst raus, danach
    /// wird nach [`CacheEntry::eviction_key`] sortiert.
    pub fn select_for_eviction(&self, max_bytes: u64, protected: &HashSet<String>) -> Vec<String> {
        let mut total = self.total_bytes();
        if total <= max_bytes {
            return Vec::new();
        }

        let mut candidates: Vec<&CacheEntry> = self
            .entries
            .values()
            .filter(|e| !protected.contains(&e.id))
            .collect();

        candidates.sort_by(|a, b| {
            b.excluded
                .cmp(&a.excluded)
                .then(a.eviction_key().cmp(&b.eviction_key()))
                .then(a.id.cmp(&b.id))
        });

        let mut victims = Vec::new();
        for e in candidates {
            if total <= max_bytes {
                break;
            }
            total = total.saturating_sub(e.bytes);
            victims.push(e.id.clone());
        }
        victims
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, source: &str, path: &str, bytes: u64, added: i64) -> CacheEntry {
        CacheEntry {
            id: id.into(),
            source_id: source.into(),
            rel_path: path.into(),
            file_name: path.rsplit('/').next().unwrap_or(path).into(),
            etag: None,
            remote_size: None,
            remote_mtime: None,
            taken_at: None,
            width: 1920,
            height: 1080,
            bytes,
            added_at: added,
            last_shown: None,
            excluded: false,
            show_count: 0,
            mail: None,
            thumb_bytes: None,
        }
    }

    fn index_with(entries: Vec<CacheEntry>) -> CacheIndex {
        let mut idx = CacheIndex::new();
        for e in entries {
            idx.insert(e);
        }
        idx
    }

    #[test]
    fn insert_und_lookup_ueber_beide_schluessel() {
        let idx = index_with(vec![entry("a1", "src", "foto/1.jpg", 100, 0)]);
        assert_eq!(idx.len(), 1);
        assert!(idx.get("a1").is_some());
        assert_eq!(idx.by_source_path("src", "foto/1.jpg").unwrap().id, "a1");
        assert!(idx.by_source_path("src", "foto/2.jpg").is_none());
        assert!(idx.by_source_path("andere", "foto/1.jpg").is_none());
    }

    #[test]
    fn remove_raeumt_beide_indizes() {
        let mut idx = index_with(vec![entry("a1", "src", "foto/1.jpg", 100, 0)]);
        assert!(idx.remove("a1").is_some());
        assert!(idx.get("a1").is_none());
        assert!(idx.by_source_path("src", "foto/1.jpg").is_none());
        assert!(
            idx.remove("a1").is_none(),
            "zweites Entfernen ist ein No-op"
        );
    }

    #[test]
    fn allocate_id_ist_stabil_fuer_denselben_pfad() {
        let idx = CacheIndex::new();
        let a = idx.allocate_id("src", "foto/1.jpg");
        let b = idx.allocate_id("src", "foto/1.jpg");
        assert_eq!(a, b, "derselbe Pfad muss denselben Dateinamen ergeben");
        assert_ne!(a, idx.allocate_id("src", "foto/2.jpg"));
        assert_ne!(a, idx.allocate_id("andere", "foto/1.jpg"));
    }

    #[test]
    fn allocate_id_behaelt_namen_beim_update() {
        // Ein geändertes Bild muss dieselbe Cache-Datei überschreiben,
        // sonst wächst der Cache bei jedem Update.
        let idx = index_with(vec![entry("feste-id", "src", "foto/1.jpg", 100, 0)]);
        assert_eq!(idx.allocate_id("src", "foto/1.jpg"), "feste-id");
    }

    #[test]
    fn allocate_id_loest_kollision_durch_sondieren() {
        // Belegt den Namen, den der Hash für diesen Schlüssel liefern würde.
        let natural = CacheIndex::new().allocate_id("src", "foto/1.jpg");
        let idx = index_with(vec![entry(
            &natural,
            "andere-quelle",
            "kollision.jpg",
            10,
            0,
        )]);
        let alloc = idx.allocate_id("src", "foto/1.jpg");
        assert_ne!(
            alloc, natural,
            "belegter Name darf nicht erneut vergeben werden"
        );
    }

    #[test]
    fn delta_state_neu_wenn_unbekannt() {
        let idx = CacheIndex::new();
        assert_eq!(
            idx.delta_state("src", "1.jpg", Some("t"), None, None),
            DeltaState::New
        );
    }

    #[test]
    fn delta_state_bevorzugt_etag_nf_14() {
        let mut e = entry("a1", "src", "1.jpg", 100, 0);
        e.etag = Some("abc".into());
        let idx = index_with(vec![e]);
        assert_eq!(
            idx.delta_state("src", "1.jpg", Some("abc"), None, None),
            DeltaState::Unchanged
        );
        assert_eq!(
            idx.delta_state("src", "1.jpg", Some("xyz"), None, None),
            DeltaState::Changed
        );
    }

    #[test]
    fn delta_state_faellt_auf_groesse_und_datum_zurueck() {
        let mut e = entry("a1", "src", "1.jpg", 100, 0);
        e.remote_size = Some(5000);
        e.remote_mtime = Some(1700);
        let idx = index_with(vec![e]);
        assert_eq!(
            idx.delta_state("src", "1.jpg", None, Some(5000), Some(1700)),
            DeltaState::Unchanged
        );
        assert_eq!(
            idx.delta_state("src", "1.jpg", None, Some(5001), Some(1700)),
            DeltaState::Changed
        );
        assert_eq!(
            idx.delta_state("src", "1.jpg", None, Some(5000), Some(1800)),
            DeltaState::Changed
        );
    }

    #[test]
    fn delta_state_ohne_metadaten_gilt_als_geaendert() {
        // Lieber einmal zu viel laden als ein Update verpassen.
        let idx = index_with(vec![entry("a1", "src", "1.jpg", 100, 0)]);
        assert_eq!(
            idx.delta_state("src", "1.jpg", None, None, None),
            DeltaState::Changed
        );
    }

    #[test]
    fn eviction_leer_wenn_unter_der_grenze() {
        let idx = index_with(vec![entry("a", "s", "1.jpg", 100, 0)]);
        assert!(idx.select_for_eviction(1000, &HashSet::new()).is_empty());
        // Exakt auf der Grenze wird nicht verdrängt.
        assert!(idx.select_for_eviction(100, &HashSet::new()).is_empty());
    }

    #[test]
    fn eviction_verdraengt_am_laengsten_nicht_gezeigte_zuerst_fa_27() {
        let mut alt = entry("alt", "s", "1.jpg", 100, 0);
        alt.last_shown = Some(100);
        let mut neu = entry("neu", "s", "2.jpg", 100, 0);
        neu.last_shown = Some(900);
        let idx = index_with(vec![alt, neu]);

        let victims = idx.select_for_eviction(100, &HashSet::new());
        assert_eq!(
            victims,
            vec!["alt"],
            "das länger nicht gezeigte Bild fliegt raus"
        );
    }

    #[test]
    fn eviction_schuetzt_nie_gezeigte_neuzugaenge() {
        // Frisch synchronisiert (added_at hoch) darf nicht sofort wieder raus.
        let mut alt = entry("alt", "s", "1.jpg", 100, 0);
        alt.last_shown = Some(50);
        let frisch = entry("frisch", "s", "2.jpg", 100, 10_000);
        let idx = index_with(vec![alt, frisch]);

        assert_eq!(idx.select_for_eviction(100, &HashSet::new()), vec!["alt"]);
    }

    #[test]
    fn eviction_wirft_ausgeschlossene_bilder_zuerst_raus() {
        // FA-30: was ohnehin nicht gezeigt wird, belegt keinen Cache.
        let mut ausgeschlossen = entry("aus", "s", "1.jpg", 100, 9_000);
        ausgeschlossen.excluded = true;
        ausgeschlossen.last_shown = Some(9_000);
        let mut alt = entry("alt", "s", "2.jpg", 100, 0);
        alt.last_shown = Some(10);
        let idx = index_with(vec![ausgeschlossen, alt]);

        assert_eq!(idx.select_for_eviction(100, &HashSet::new()), vec!["aus"]);
    }

    #[test]
    fn eviction_verschont_geschuetzte_bilder() {
        // Das gerade angezeigte und die vorgeladenen Bilder (FA-31) dürfen
        // nicht unter der laufenden Diashow weggelöscht werden.
        let mut alt = entry("alt", "s", "1.jpg", 100, 0);
        alt.last_shown = Some(10);
        let mut neu = entry("neu", "s", "2.jpg", 100, 0);
        neu.last_shown = Some(999);
        let idx = index_with(vec![alt, neu]);

        let protected: HashSet<String> = ["alt".to_string()].into_iter().collect();
        assert_eq!(idx.select_for_eviction(100, &protected), vec!["neu"]);
    }

    #[test]
    fn eviction_verdraengt_so_lange_bis_die_grenze_haelt() {
        let mut entries = Vec::new();
        for i in 0..10 {
            let mut e = entry(&format!("e{i}"), "s", &format!("{i}.jpg"), 100, 0);
            e.last_shown = Some(i);
            entries.push(e);
        }
        let idx = index_with(entries);
        assert_eq!(idx.total_bytes(), 1000);

        let victims = idx.select_for_eviction(450, &HashSet::new());
        assert_eq!(victims.len(), 6, "1000 - 6*100 = 400 <= 450");
        assert_eq!(victims[0], "e0", "ältestes zuerst");
        assert_eq!(victims[5], "e5");
    }

    #[test]
    fn ids_missing_at_source_findet_geloeschte_quelldateien() {
        let idx = index_with(vec![
            entry("a", "s", "1.jpg", 10, 0),
            entry("b", "s", "2.jpg", 10, 0),
            entry("c", "andere", "3.jpg", 10, 0),
        ]);
        let seen: HashSet<String> = ["1.jpg".to_string()].into_iter().collect();
        let missing = idx.ids_missing_at_source("s", &seen);
        assert_eq!(
            missing,
            vec!["b"],
            "nur die fehlende Datei derselben Quelle"
        );
    }

    #[test]
    fn mark_shown_und_set_excluded_melden_unbekannte_ids() {
        let mut idx = index_with(vec![entry("a", "s", "1.jpg", 10, 0)]);
        assert!(idx.mark_shown("a", 500));
        assert_eq!(idx.get("a").unwrap().last_shown, Some(500));
        assert!(!idx.mark_shown("gibtsnicht", 500));

        assert!(idx.set_excluded("a", true));
        assert!(idx.get("a").unwrap().excluded);
        assert!(!idx.set_excluded("gibtsnicht", true));
    }

    #[test]
    fn sort_time_faellt_gestaffelt_zurueck() {
        let mut e = entry("a", "s", "1.jpg", 10, 42);
        assert_eq!(e.sort_time(), 42, "ohne alles gilt added_at");
        e.remote_mtime = Some(100);
        assert_eq!(e.sort_time(), 100, "mtime schlägt added_at");
        e.taken_at = Some(200);
        assert_eq!(e.sort_time(), 200, "EXIF schlägt alles");
    }

    #[test]
    fn is_portrait_erkennt_hochformat_fuer_fa_08() {
        let mut e = entry("a", "s", "1.jpg", 10, 0);
        assert!(!e.is_portrait());
        e.width = 1080;
        e.height = 1920;
        assert!(e.is_portrait());
        // Quadratisch zählt nicht als Hochformat.
        e.height = 1080;
        assert!(!e.is_portrait());
    }

    #[test]
    fn json_roundtrip_stellt_lookup_wieder_her() {
        let idx = index_with(vec![entry("a1", "src", "foto/1.jpg", 100, 0)]);
        let json = serde_json::to_string(&idx).unwrap();
        let mut back: CacheIndex = serde_json::from_str(&json).unwrap();
        // Vor dem Rebuild ist der Sekundärindex leer.
        assert!(back.by_source_path("src", "foto/1.jpg").is_none());
        back.rebuild_lookup();
        assert_eq!(back.by_source_path("src", "foto/1.jpg").unwrap().id, "a1");
    }
}

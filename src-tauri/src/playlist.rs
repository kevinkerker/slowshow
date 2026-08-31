//! Reihenfolge und Fortschritt der Diashow (FA-01, FA-03, FA-08, FA-30).
//!
//! Arbeitet ausschließlich auf dem Cache-Index — die Diashow kennt keine
//! Quellen und keine Netzwerkzustände, sie läuft deshalb bei Netzausfall
//! unverändert weiter (FA-26).
//!
//! Die Zufallsreihenfolge entsteht durch Sortieren nach einem seed-abhängigen
//! Hash, nicht durch Mischen. Der Unterschied ist im Dauerbetrieb wesentlich:
//! ein gemischtes Feld ordnet sich beim Einfügen eines einzigen Bildes komplett
//! neu, ein Hash-Schlüssel nicht. So bringt ein Sync neue Bilder in die laufende
//! Diashow, ohne die Reihenfolge der übrigen durcheinanderzubringen (FA-28).

use crate::cache::{CacheEntry, CacheIndex};
use crate::model::{PlaybackFilter, PlayOrder, TimeFilter};
use std::collections::HashSet;

/// Was in einem Anzeigeschritt auf dem Schirm steht.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Slide {
    Single {
        id: String,
    },
    /// Zwei Bilder gleichzeitig (FA-08).
    ///
    /// Im Querformat zwei Hochformatfotos nebeneinander, im Hochformat zwei
    /// Querformatfotos übereinander (E-26). Die Feldnamen bleiben `left` und
    /// `right`, weil die Anordnung Sache der Darstellung ist und ein zweiter
    /// Variantenname jede Stelle im Frontend verdoppeln würde.
    Pair {
        left: String,
        right: String,
    },
}

impl Slide {
    /// Wie viele Positionen die Playlist beim Weiterschalten überspringt.
    ///
    /// Bewusst nicht `len()`: ein Slide ist nie leer, eine `is_empty()`-Gegenprobe
    /// wäre also sinnlos — und der Name sagt hier ohnehin genauer, wofür der
    /// Wert gebraucht wird.
    pub fn step_size(&self) -> usize {
        match self {
            Slide::Single { .. } => 1,
            Slide::Pair { .. } => 2,
        }
    }

    pub fn ids(&self) -> Vec<&str> {
        match self {
            Slide::Single { id } => vec![id.as_str()],
            Slide::Pair { left, right } => vec![left.as_str(), right.as_str()],
        }
    }
}

/// Passt ein Bild zur Auswahl aus F5?
///
/// Reine Funktion mit hereingereichter Zeit: sonst wären „letzte 12 Monate"
/// und „dieses Jahr" nur am Kalender des Testrechners prüfbar.
pub fn matches_filter(entry: &CacheEntry, filter: &PlaybackFilter, now: i64) -> bool {
    if !filter.senders.is_empty() {
        let sender = entry.mail.as_ref().map(|m| m.sender.as_str());
        match sender {
            Some(s) if filter.senders.iter().any(|f| f.eq_ignore_ascii_case(s)) => {}
            // Ist nach Absendern gefiltert, fallen Bilder ohne Absender heraus
            // — bei einer Auswahl „von Oma" will niemand den Nextcloud-Ordner
            // dazwischen.
            _ => return false,
        }
    }

    let taken = entry.taken_at;
    if taken.is_none() {
        return filter.include_undated;
    }
    let taken = taken.unwrap_or(0);

    match &filter.time {
        TimeFilter::All => true,
        TimeFilter::Last12Months => taken >= now - 365 * 86_400,
        TimeFilter::ThisYear => year_of(taken) == year_of(now),
        TimeFilter::Years(years) => years.is_empty() || years.contains(&year_of(taken)),
    }
}

/// Kalenderjahr eines Zeitstempels in UTC.
pub fn year_of(ts: i64) -> i32 {
    use chrono::{Datelike, TimeZone, Utc};
    Utc.timestamp_opt(ts, 0)
        .single()
        .map(|d| d.year())
        .unwrap_or(0)
}

/// Sortierschlüssel eines Bildes für die Zufallsreihenfolge.
///
/// FNV-1a über Seed und Id, danach die Streuung aus splitmix64 — ohne die
/// landeten fortlaufende Ids auf benachbarten Hashes, und die „zufällige"
/// Reihenfolge wäre in Wahrheit die Einfügereihenfolge.
///
/// Kein `rand`-Crate: für eine Bilderreihenfolge braucht es keine
/// kryptografische Qualität, und jede eingesparte Abhängigkeit hilft NF-10.
pub fn order_hash(seed: u64, id: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64 ^ seed;
    for b in id.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h ^= h >> 30;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^ (h >> 31)
}

/// Stellt die Anzeigereihenfolge zusammen (FA-03).
///
/// Berücksichtigt nur Bilder aktiver Quellen (FA-25), die nicht ausgeschlossen
/// sind (FA-30). Sortierungen sind stabil über die Id, damit gleiche
/// Sortierschlüssel nicht bei jedem Neuaufbau die Reihenfolge ändern.
/// Baut die Reihenfolge für alle Modi außer [`PlayOrder::Smart`].
///
/// Die intelligente Mischung zieht statt zu sortieren und liegt deshalb in
/// `scheduler.rs`; hier landet sie als `Random`, damit ein Aufruf mit `Smart`
/// nicht ins Leere läuft — welcher Modus gilt, entscheidet `state.rs`.
pub fn build_order(
    index: &CacheIndex,
    enabled_sources: &HashSet<String>,
    order: PlayOrder,
    newest_first: bool,
    filter: &PlaybackFilter,
    now: i64,
    seed: u64,
) -> Vec<String> {
    let mut entries: Vec<&CacheEntry> = index
        .values()
        // Quarantaenefotos gehoeren noch nicht zum Bestand (F4, E-31): sie
        // liegen im Cache, warten aber auf Freigabe.
        .filter(|e| !e.excluded && !e.is_quarantined() && enabled_sources.contains(&e.source_id))
        .filter(|e| matches_filter(e, filter, now))
        .collect();

    match order {
        PlayOrder::FileName => entries.sort_by(|a, b| {
            a.file_name
                .to_lowercase()
                .cmp(&b.file_name.to_lowercase())
                .then(a.id.cmp(&b.id))
        }),
        PlayOrder::Chronological => {
            entries.sort_by(|a, b| a.sort_time().cmp(&b.sort_time()).then(a.id.cmp(&b.id)));
            if newest_first {
                entries.reverse();
            }
        }
        PlayOrder::Smart | PlayOrder::Random => entries.sort_by(|a, b| {
            order_hash(seed, &a.id)
                .cmp(&order_hash(seed, &b.id))
                .then(a.id.cmp(&b.id))
        }),
    }

    entries.into_iter().map(|e| e.id.clone()).collect()
}

/// Laufende Diashow: Reihenfolge plus aktuelle Position.
#[derive(Debug, Default)]
pub struct Playlist {
    order: Vec<String>,
    pos: usize,
    /// Seed des aktuellen Zufallsdurchlaufs; wird bei jedem Umlauf erneuert,
    /// damit die Endlosschleife (FA-01) nicht ewig dieselbe Folge zeigt.
    seed: u64,
}

impl Playlist {
    pub fn new() -> Self {
        Self::default()
    }

    /// Die Reihenfolge als eigene Zeichenketten.
    ///
    /// Die intelligente Mischung braucht die Kandidaten ueber die Sperre
    /// hinaus; geliehene Verweise wuerden die Playlist waehrend der ganzen
    /// Ziehung festhalten (E-29).
    pub fn ids_owned(&self) -> Vec<String> {
        self.order.clone()
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn position(&self) -> usize {
        self.pos
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Übernimmt eine neue Reihenfolge und behält nach Möglichkeit das gerade
    /// gezeigte Bild.
    ///
    /// Wichtig für FA-28: nach einem Sync erscheinen neue Bilder ohne
    /// App-Neustart — ohne diese Positionsrettung würde die Diashow bei jedem
    /// Sync an den Anfang springen.
    pub fn replace(&mut self, order: Vec<String>, seed: u64) {
        let current = self.order.get(self.pos).cloned();
        self.order = order;
        self.seed = seed;
        self.pos = current
            .and_then(|id| self.order.iter().position(|o| *o == id))
            .unwrap_or(0)
            .min(self.order.len().saturating_sub(1));
    }

    /// Das aktuell anzuzeigende Bild bzw. Bildpaar.
    ///
    /// `frame_portrait` beschreibt, wie der Rahmen hängt (E-26) — davon hängt
    /// ab, welches Seitenverhältnis überhaupt gepaart werden kann.
    pub fn current(&self, pair_mode: bool, frame_portrait: bool, index: &CacheIndex) -> Option<Slide> {
        self.slide_at(self.pos, pair_mode, frame_portrait, index)
    }

    fn slide_at(
        &self,
        pos: usize,
        pair_mode: bool,
        frame_portrait: bool,
        index: &CacheIndex,
    ) -> Option<Slide> {
        let first = self.order.get(pos)?;
        if pair_mode {
            // Gepaart wird immer das Format, das *nicht* zum Rahmen passt:
            // quer zwei Hochformatfotos nebeneinander (FA-08), hochkant zwei
            // Querformatfotos übereinander (E-26). Ein Bild im Format des
            // Rahmens füllt ihn allein und bleibt allein.
            let pairable = |e: &crate::cache::index::CacheEntry| e.is_portrait() != frame_portrait;
            if let (Some(a), Some(b)) = (index.get(first), self.order.get(pos + 1)) {
                if let Some(b_entry) = index.get(b) {
                    if pairable(a) && pairable(b_entry) {
                        return Some(Slide::Pair {
                            left: first.clone(),
                            right: b.clone(),
                        });
                    }
                }
            }
        }
        Some(Slide::Single { id: first.clone() })
    }

    /// Schaltet weiter (FA-41). `next_seed` wird nur beim Umlauf verwendet.
    pub fn advance(
        &mut self,
        pair_mode: bool,
        frame_portrait: bool,
        index: &CacheIndex,
        next_seed: u64,
    ) -> Option<Slide> {
        if self.order.is_empty() {
            return None;
        }
        let step = self
            .current(pair_mode, frame_portrait, index)
            .map(|s| s.step_size())
            .unwrap_or(1);
        let next = self.pos + step;
        if next >= self.order.len() {
            self.pos = 0;
            self.seed = next_seed;
        } else {
            self.pos = next;
        }
        self.current(pair_mode, frame_portrait, index)
    }

    /// Einen Schritt zurück (FA-41). Bewusst immer nur um eine Position,
    /// damit das Zurückwischen im Paar-Modus nicht springt.
    pub fn back(&mut self, pair_mode: bool, frame_portrait: bool, index: &CacheIndex) -> Option<Slide> {
        if self.order.is_empty() {
            return None;
        }
        self.pos = if self.pos == 0 {
            self.order.len() - 1
        } else {
            self.pos - 1
        };
        self.current(pair_mode, frame_portrait, index)
    }

    /// Springt gezielt auf ein Bild — z. B. nachdem das aktuelle Bild
    /// ausgeschlossen wurde (FA-30).
    pub fn seek_to(&mut self, id: &str) -> bool {
        match self.order.iter().position(|o| o == id) {
            Some(p) => {
                self.pos = p;
                true
            }
            None => false,
        }
    }

    /// Die nächsten `n` Bilder — Grundlage des Vorausladens (FA-31) und der
    /// Verdrängungssperre des Ringpuffers (FA-27).
    ///
    /// Enthält das aktuelle Bild an erster Stelle und läuft über das Listenende
    /// hinweg weiter, weil die Diashow endlos ist (FA-01).
    pub fn window(&self, n: usize) -> Vec<String> {
        if self.order.is_empty() {
            return Vec::new();
        }
        let take = (n + 1).min(self.order.len());
        (0..take)
            .map(|i| self.order[(self.pos + i) % self.order.len()].clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::CacheEntry;

    fn entry(id: &str, source: &str, name: &str) -> CacheEntry {
        CacheEntry {
            id: id.into(),
            source_id: source.into(),
            rel_path: name.into(),
            file_name: name.into(),
            etag: None,
            remote_size: None,
            remote_mtime: None,
            taken_at: None,
            width: 1920,
            height: 1080,
            bytes: 100,
            added_at: 0,
            last_shown: None,
            excluded: false,
            show_count: 0,
            mail: None,
            thumb_bytes: None,
        }
    }

    fn portrait(id: &str) -> CacheEntry {
        CacheEntry {
            width: 1080,
            height: 1920,
            ..entry(id, "s", id)
        }
    }

    #[test]
    fn hochformat_paart_querformatfotos_uebereinander_e_26() {
        // Das Spiegelbild von FA-08: haengt der Rahmen hochkant, sind die
        // Querformatfotos die schlecht passenden -- zwei davon fuellen ihn
        // genauso, wie zwei Hochformate ihn quer fuellen.
        let idx = index_of(vec![entry("a", "s", "a"), entry("b", "s", "b")]);
        let mut pl = Playlist::new();
        pl.replace(vec!["a".into(), "b".into()], 1);

        assert_eq!(
            pl.current(true, true, &idx),
            Some(Slide::Pair {
                left: "a".into(),
                right: "b".into()
            }),
            "hochkant werden Querformate gepaart"
        );
        assert_eq!(
            pl.current(true, false, &idx),
            Some(Slide::Single { id: "a".into() }),
            "quer bleibt ein Querformat allein"
        );
    }

    #[test]
    fn hochformat_laesst_hochformatfotos_allein_e_26() {
        // Ein Bild im Format des Rahmens fuellt ihn allein.
        let idx = index_of(vec![portrait("a"), portrait("b")]);
        let mut pl = Playlist::new();
        pl.replace(vec!["a".into(), "b".into()], 1);

        assert_eq!(
            pl.current(true, true, &idx),
            Some(Slide::Single { id: "a".into() })
        );
        assert_eq!(
            pl.current(true, false, &idx),
            Some(Slide::Pair {
                left: "a".into(),
                right: "b".into()
            }),
            "quer gilt weiterhin FA-08"
        );
    }

    #[test]
    fn gemischte_formate_werden_nie_gepaart_e_26() {
        // Ein Hoch- und ein Querformat nebeneinander ergaebe in beiden
        // Ausrichtungen einen unruhigen Schirm.
        let idx = index_of(vec![portrait("a"), entry("b", "s", "b")]);
        let mut pl = Playlist::new();
        pl.replace(vec!["a".into(), "b".into()], 1);
        for frame_portrait in [true, false] {
            assert_eq!(
                pl.current(true, frame_portrait, &idx),
                Some(Slide::Single { id: "a".into() }),
                "frame_portrait={frame_portrait}"
            );
        }
    }

    fn index_of(entries: Vec<CacheEntry>) -> CacheIndex {
        let mut idx = CacheIndex::new();
        for e in entries {
            idx.insert(e);
        }
        idx
    }

    fn sources(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    // ── Reihenfolge (FA-03) ─────────────────────────────────────────────────

    #[test]
    fn order_nach_dateiname_ist_alphabetisch_und_case_insensitiv() {
        let idx = index_of(vec![
            entry("1", "s", "Zebra.jpg"),
            entry("2", "s", "apfel.jpg"),
            entry("3", "s", "Bild.jpg"),
        ]);
        let o = build_order(&idx, &sources(&["s"]), PlayOrder::FileName, false, &PlaybackFilter::default(), 0, 1);
        assert_eq!(o, vec!["2", "3", "1"]);
    }

    #[test]
    fn order_nach_aufnahmedatum() {
        let mut a = entry("1", "s", "a.jpg");
        a.taken_at = Some(300);
        let mut b = entry("2", "s", "b.jpg");
        b.taken_at = Some(100);
        let mut c = entry("3", "s", "c.jpg");
        c.remote_mtime = Some(200); // kein EXIF -> Rückfall auf mtime
        let idx = index_of(vec![a, b, c]);

        let o = build_order(&idx, &sources(&["s"]), PlayOrder::Chronological, false, &PlaybackFilter::default(), 0, 1);
        assert_eq!(o, vec!["2", "3", "1"]);
    }

    #[test]
    fn order_nach_aenderungsdatum() {
        let mut a = entry("1", "s", "a.jpg");
        a.remote_mtime = Some(500);
        let mut b = entry("2", "s", "b.jpg");
        b.remote_mtime = Some(100);
        let idx = index_of(vec![a, b]);
        assert_eq!(
            build_order(&idx, &sources(&["s"]), PlayOrder::Chronological, false, &PlaybackFilter::default(), 0, 1),
            vec!["2", "1"]
        );
    }

    #[test]
    fn order_zufaellig_ist_bei_gleichem_seed_reproduzierbar() {
        let idx = index_of(
            (0..20)
                .map(|i| entry(&format!("{i:02}"), "s", "x.jpg"))
                .collect(),
        );
        let a = build_order(&idx, &sources(&["s"]), PlayOrder::Random, false, &PlaybackFilter::default(), 0, 12345);
        let b = build_order(&idx, &sources(&["s"]), PlayOrder::Random, false, &PlaybackFilter::default(), 0, 12345);
        let c = build_order(&idx, &sources(&["s"]), PlayOrder::Random, false, &PlaybackFilter::default(), 0, 99999);

        assert_eq!(a, b, "gleicher Seed -> gleiche Reihenfolge");
        assert_ne!(a, c, "anderer Seed -> andere Reihenfolge");
        assert_eq!(a.len(), 20, "es geht kein Bild verloren");
        assert_eq!(
            a.iter().collect::<HashSet<_>>().len(),
            20,
            "und keins doppelt"
        );
    }

    #[test]
    fn order_beruecksichtigt_nur_aktive_quellen_fa_25() {
        let idx = index_of(vec![
            entry("1", "aktiv", "a.jpg"),
            entry("2", "aus", "b.jpg"),
        ]);
        let o = build_order(&idx, &sources(&["aktiv"]), PlayOrder::FileName, false, &PlaybackFilter::default(), 0, 1);
        assert_eq!(o, vec!["1"]);
    }

    #[test]
    fn order_laesst_ausgeschlossene_bilder_weg_fa_30() {
        let mut b = entry("2", "s", "b.jpg");
        b.excluded = true;
        let idx = index_of(vec![entry("1", "s", "a.jpg"), b]);
        assert_eq!(
            build_order(&idx, &sources(&["s"]), PlayOrder::FileName, false, &PlaybackFilter::default(), 0, 1),
            vec!["1"]
        );
    }

    // ── Fortschritt (FA-01, FA-41) ──────────────────────────────────────────

    #[test]
    fn advance_laeuft_endlos_im_kreis_fa_01() {
        let idx = index_of(vec![entry("a", "s", "a"), entry("b", "s", "b")]);
        let mut pl = Playlist::new();
        pl.replace(vec!["a".into(), "b".into()], 1);

        assert_eq!(
            pl.current(false, false, &idx),
            Some(Slide::Single { id: "a".into() })
        );
        assert_eq!(
            pl.advance(false, false, &idx, 2),
            Some(Slide::Single { id: "b".into() })
        );
        assert_eq!(
            pl.advance(false, false, &idx, 3),
            Some(Slide::Single { id: "a".into() })
        );
        assert_eq!(pl.seed(), 3, "beim Umlauf wird neu gemischt");
    }

    #[test]
    fn back_laeuft_ueber_den_anfang_hinaus_zurueck() {
        let idx = index_of(vec![entry("a", "s", "a"), entry("b", "s", "b")]);
        let mut pl = Playlist::new();
        pl.replace(vec!["a".into(), "b".into()], 1);
        assert_eq!(pl.back(false, false, &idx), Some(Slide::Single { id: "b".into() }));
    }

    #[test]
    fn leere_playlist_liefert_nichts_statt_zu_paniken() {
        let idx = index_of(vec![]);
        let mut pl = Playlist::new();
        assert!(pl.current(false, false, &idx).is_none());
        assert!(pl.advance(false, false, &idx, 1).is_none());
        assert!(pl.back(false, false, &idx).is_none());
        assert!(pl.window(5).is_empty());
    }

    #[test]
    fn replace_haelt_das_aktuelle_bild_fest_fa_28() {
        let idx = index_of(vec![entry("a", "s", "a"), entry("b", "s", "b")]);
        let mut pl = Playlist::new();
        pl.replace(vec!["a".into(), "b".into()], 1);
        pl.advance(false, false, &idx, 1); // steht jetzt auf "b"

        // Sync bringt ein neues Bild -> Reihenfolge wird neu aufgebaut.
        pl.replace(vec!["neu".into(), "a".into(), "b".into()], 2);
        assert_eq!(
            pl.current(false, false, &idx),
            Some(Slide::Single { id: "b".into() }),
            "die Diashow darf nicht an den Anfang springen"
        );
    }

    #[test]
    fn replace_faengt_verschwundenes_bild_ab() {
        let mut pl = Playlist::new();
        pl.replace(vec!["a".into(), "b".into()], 1);
        pl.replace(vec!["c".into()], 2);
        assert_eq!(pl.position(), 0);

        // Auch die leere Liste darf keinen Index-Überlauf erzeugen.
        pl.replace(vec![], 3);
        assert_eq!(pl.position(), 0);
        assert!(pl.is_empty());
    }

    // ── Paar-Modus (FA-08) ──────────────────────────────────────────────────

    #[test]
    fn paar_modus_stellt_zwei_hochformatbilder_nebeneinander() {
        let idx = index_of(vec![portrait("p1"), portrait("p2")]);
        let mut pl = Playlist::new();
        pl.replace(vec!["p1".into(), "p2".into()], 1);

        assert_eq!(
            pl.current(true, false, &idx),
            Some(Slide::Pair {
                left: "p1".into(),
                right: "p2".into()
            })
        );
        // Ein Paar verbraucht zwei Positionen.
        assert_eq!(
            pl.advance(true, false, &idx, 2),
            Some(Slide::Pair {
                left: "p1".into(),
                right: "p2".into()
            })
        );
    }

    #[test]
    fn paar_modus_paart_kein_querformat() {
        let idx = index_of(vec![portrait("p1"), entry("q", "s", "q.jpg")]);
        let mut pl = Playlist::new();
        pl.replace(vec!["p1".into(), "q".into()], 1);
        assert_eq!(
            pl.current(true, false, &idx),
            Some(Slide::Single { id: "p1".into() })
        );
    }

    #[test]
    fn paar_modus_aus_zeigt_einzelbilder() {
        let idx = index_of(vec![portrait("p1"), portrait("p2")]);
        let mut pl = Playlist::new();
        pl.replace(vec!["p1".into(), "p2".into()], 1);
        assert_eq!(
            pl.current(false, false, &idx),
            Some(Slide::Single { id: "p1".into() })
        );
    }

    #[test]
    fn paar_modus_am_listenende_zeigt_einzelbild() {
        let idx = index_of(vec![portrait("p1")]);
        let mut pl = Playlist::new();
        pl.replace(vec!["p1".into()], 1);
        assert_eq!(
            pl.current(true, false, &idx),
            Some(Slide::Single { id: "p1".into() })
        );
    }

    // ── Prefetch-Fenster (FA-31) ────────────────────────────────────────────

    #[test]
    fn window_liefert_aktuelles_plus_n_folgende() {
        let mut pl = Playlist::new();
        pl.replace(vec!["a".into(), "b".into(), "c".into(), "d".into()], 1);
        assert_eq!(pl.window(2), vec!["a", "b", "c"]);
    }

    #[test]
    fn window_laeuft_ueber_das_listenende_hinweg() {
        let idx = index_of(vec![entry("a", "s", "a"), entry("b", "s", "b")]);
        let mut pl = Playlist::new();
        pl.replace(vec!["a".into(), "b".into()], 1);
        pl.advance(false, false, &idx, 1); // auf "b"
        assert_eq!(
            pl.window(2),
            vec!["b", "a"],
            "kürzt auf die Listenlänge, keine Dubletten"
        );
    }

    #[test]
    fn seek_to_findet_und_meldet_fehlschlag() {
        let mut pl = Playlist::new();
        pl.replace(vec!["a".into(), "b".into()], 1);
        assert!(pl.seek_to("b"));
        assert_eq!(pl.position(), 1);
        assert!(!pl.seek_to("gibtsnicht"));
        assert_eq!(pl.position(), 1, "Position bleibt bei Fehlschlag stehen");
    }

    // ── Zufallsreihenfolge ──────────────────────────────────────────────────

    #[test]
    fn order_hash_streut_aehnliche_ids() {
        // Fortlaufende Ids sind der Normalfall (Cache-Namen liegen dicht
        // beieinander). Ohne Streuung wäre die Reihenfolge die Einfügefolge.
        let a = order_hash(7, "0000000000000001");
        let b = order_hash(7, "0000000000000002");
        assert_ne!(a, b);
        assert!(a.abs_diff(b) > 1_000_000, "Hashes zu dicht: {a} vs {b}");
    }

    #[test]
    fn order_hash_haengt_am_seed() {
        assert_ne!(order_hash(1, "abc"), order_hash(2, "abc"));
        assert_eq!(order_hash(1, "abc"), order_hash(1, "abc"));
    }

    #[test]
    fn zufallsreihenfolge_bleibt_beim_hinzufuegen_stabil() {
        // Der eigentliche Grund für die Hash-Sortierung: ein Sync (FA-28) darf
        // die laufende Diashow nicht neu durchwürfeln.
        let vorher: Vec<CacheEntry> = (0..20)
            .map(|i| entry(&format!("{i:02}"), "s", "x.jpg"))
            .collect();
        let idx_vorher = index_of(vorher.clone());
        let alt = build_order(&idx_vorher, &sources(&["s"]), PlayOrder::Random, false, &PlaybackFilter::default(), 0, 4242);

        let mut nachher = vorher;
        nachher.push(entry("neu", "s", "neu.jpg"));
        let nachher = build_order(
            &index_of(nachher),
            &sources(&["s"]),
            PlayOrder::Random,
            false,
            &PlaybackFilter::default(),
            0,
            4242,
        );

        assert_eq!(nachher.len(), 21);
        let ohne_neues: Vec<&String> = nachher.iter().filter(|id| *id != "neu").collect();
        let erwartet: Vec<&String> = alt.iter().collect();
        assert_eq!(
            ohne_neues, erwartet,
            "die übrigen Bilder dürfen ihre Folge behalten"
        );
    }

    #[test]
    fn zufallsreihenfolge_weicht_von_der_id_folge_ab() {
        let idx = index_of(
            (0..30)
                .map(|i| entry(&format!("{i:02}"), "s", "x.jpg"))
                .collect(),
        );
        let zufall = build_order(&idx, &sources(&["s"]), PlayOrder::Random, false, &PlaybackFilter::default(), 0, 99);
        let sortiert: Vec<String> = (0..30).map(|i| format!("{i:02}")).collect();
        assert_ne!(
            zufall, sortiert,
            "sonst wäre die Reihenfolge gar nicht zufällig"
        );
    }

    #[test]
    fn seed_null_liefert_eine_brauchbare_reihenfolge() {
        let idx = index_of(
            (0..10)
                .map(|i| entry(&format!("{i:02}"), "s", "x.jpg"))
                .collect(),
        );
        let o = build_order(&idx, &sources(&["s"]), PlayOrder::Random, false, &PlaybackFilter::default(), 0, 0);
        assert_eq!(o.len(), 10);
        assert_eq!(o.iter().collect::<HashSet<_>>().len(), 10);
    }

    // ── Auswahl der Bilder (F5) ──────────────────────────────────────────────

    /// 1. Januar 2026, 12 Uhr UTC — fester Bezug statt `now()`.
    const HEUTE: i64 = 1_767_268_800;

    fn mit_datum(id: &str, taken_at: Option<i64>) -> CacheEntry {
        CacheEntry {
            taken_at,
            ..entry(id, "s", id)
        }
    }

    fn mit_absender(id: &str, sender: &str) -> CacheEntry {
        CacheEntry {
            mail: Some(crate::cache::index::MailMeta {
                sender: sender.into(),
                subject: "Hallo".into(),
                message_id: "x".into(),
                quarantined: false,
            }),
            ..entry(id, "s", id)
        }
    }

    /// Unix-Zeit fuer den 1. Juli des Jahres.
    fn im_jahr(year: i32) -> i64 {
        use chrono::{TimeZone, Utc};
        Utc.with_ymd_and_hms(year, 7, 1, 12, 0, 0).unwrap().timestamp()
    }

    #[test]
    fn voreinstellung_zeigt_alles() {
        let f = PlaybackFilter::default();
        assert!(matches_filter(&mit_datum("a", Some(im_jahr(1987))), &f, HEUTE));
        assert!(matches_filter(&mit_datum("b", Some(im_jahr(2025))), &f, HEUTE));
        assert!(
            matches_filter(&mit_datum("c", None), &f, HEUTE),
            "ohne Aufnahmedatum darf ein Bild nicht stillschweigend verschwinden"
        );
    }

    #[test]
    fn jahresauswahl_laesst_nur_die_gewaehlten_durch() {
        let f = PlaybackFilter {
            time: TimeFilter::Years(vec![1987, 1999]),
            include_undated: false,
            ..Default::default()
        };
        assert!(matches_filter(&mit_datum("a", Some(im_jahr(1987))), &f, HEUTE));
        assert!(matches_filter(&mit_datum("b", Some(im_jahr(1999))), &f, HEUTE));
        assert!(!matches_filter(&mit_datum("c", Some(im_jahr(2001))), &f, HEUTE));
    }

    #[test]
    fn leere_jahresauswahl_bedeutet_alle() {
        // Sonst waere ein versehentliches Abwaehlen aller Jahre eine leere
        // Diashow -- und niemand faende den Weg zurueck.
        let f = PlaybackFilter {
            time: TimeFilter::Years(vec![]),
            ..Default::default()
        };
        assert!(matches_filter(&mit_datum("a", Some(im_jahr(1987))), &f, HEUTE));
    }

    #[test]
    fn ohne_datum_laesst_sich_getrennt_zuschalten() {
        let aus = PlaybackFilter {
            time: TimeFilter::Years(vec![1987]),
            include_undated: false,
            ..Default::default()
        };
        let an = PlaybackFilter {
            include_undated: true,
            ..aus.clone()
        };
        assert!(!matches_filter(&mit_datum("a", None), &aus, HEUTE));
        assert!(matches_filter(&mit_datum("a", None), &an, HEUTE));
    }

    #[test]
    fn letzte_zwoelf_monate_sind_ein_rollendes_fenster() {
        let f = PlaybackFilter {
            time: TimeFilter::Last12Months,
            include_undated: false,
            ..Default::default()
        };
        assert!(matches_filter(&mit_datum("neu", Some(HEUTE - 30 * 86_400)), &f, HEUTE));
        assert!(!matches_filter(&mit_datum("alt", Some(HEUTE - 400 * 86_400)), &f, HEUTE));
    }

    #[test]
    fn dieses_jahr_meint_das_kalenderjahr() {
        // Unterschied zu "letzte 12 Monate": am 1. Januar ist das Fenster
        // fast leer, die Zwoelfmonatsauswahl dagegen voll.
        let f = PlaybackFilter {
            time: TimeFilter::ThisYear,
            include_undated: false,
            ..Default::default()
        };
        assert!(matches_filter(&mit_datum("a", Some(im_jahr(2026))), &f, HEUTE));
        assert!(!matches_filter(&mit_datum("b", Some(im_jahr(2025))), &f, HEUTE));
    }

    #[test]
    fn absenderauswahl_ignoriert_gross_und_kleinschreibung() {
        let f = PlaybackFilter {
            senders: vec!["Oma@Example.ORG".into()],
            ..Default::default()
        };
        assert!(matches_filter(&mit_absender("a", "oma@example.org"), &f, HEUTE));
        assert!(!matches_filter(&mit_absender("b", "opa@example.org"), &f, HEUTE));
    }

    #[test]
    fn absenderauswahl_schliesst_bilder_ohne_absender_aus() {
        // Bei einer Auswahl "von Oma" will niemand den Nextcloud-Ordner
        // dazwischen haben.
        let f = PlaybackFilter {
            senders: vec!["oma@example.org".into()],
            ..Default::default()
        };
        assert!(!matches_filter(&mit_datum("a", Some(im_jahr(2025))), &f, HEUTE));
    }

    #[test]
    fn build_order_wendet_die_auswahl_an() {
        let idx = index_of(vec![
            mit_datum("alt", Some(im_jahr(1987))),
            mit_datum("neu", Some(im_jahr(2026))),
        ]);
        let f = PlaybackFilter {
            time: TimeFilter::Years(vec![1987]),
            include_undated: false,
            ..Default::default()
        };
        let o = build_order(
            &idx,
            &sources(&["s"]),
            PlayOrder::FileName,
            false,
            &f,
            HEUTE,
            1,
        );
        assert_eq!(o, vec!["alt"]);
    }
}

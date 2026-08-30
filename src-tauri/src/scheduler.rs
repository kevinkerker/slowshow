//! Intelligente Zufallswiedergabe (E-29).
//!
//! Die Pipeline aus dem Erweiterungspapier, Stufe für Stufe:
//!
//! ```text
//! 1. Urne  →  2. Gewichtung  →  3. gewichtete Ziehung  →  4. Cluster-Filter
//! ```
//!
//! ## Warum eine Urne
//!
//! Reines Würfeln zeigt manche Bilder dreimal, bevor andere einmal drankommen.
//! Die Urne garantiert das Gegenteil: jedes Bild genau einmal je Durchlauf,
//! keine Wiederholung dazwischen. Erst darüber liegt die Gewichtung, die
//! bestimmt, *wann* innerhalb des Durchlaufs ein Bild kommt — nicht *ob*.
//!
//! ## Was das Papier offen ließ
//!
//! Zwei Dinge, hier entschieden und dokumentiert:
//!
//! **Zurückblättern.** Das Urnen-Modell kennt kein Zurück, FA-41 verlangt aber
//! Wischen nach rückwärts. Der Planer führt deshalb eine begrenzte Historie
//! der zuletzt gezogenen Bilder ([`HISTORY_LIMIT`]).
//!
//! **„Nie Gezeigte = Maximalfaktor".** Ohne Obergrenze wäre der Faktor für ein
//! nie gezeigtes Bild unendlich, und die Gewichtung entartete zu „zeige
//! zuerst alles Ungesehene". Gedeckelt wird bei [`LRS_MAX_DAYS`]; nie gezeigte
//! Bilder bekommen genau diesen Deckel.
//!
//! Der Planer ist frei von Zeit- und Zufallsquellen — beides wird
//! hereingereicht. Nur so sind Boost-Fenster, Cluster-Abstand und Gewichte
//! ohne Warten prüfbar.

use crate::cache::index::{CacheEntry, CacheIndex};
use crate::model::PlaybackConfig;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Faktor für frisch eingetroffene Bilder.
pub const BOOST_FACTOR: f64 = 5.0;
/// Wie lange ein Bild als „neu" gilt.
pub const BOOST_WINDOW_HOURS: i64 = 48;
/// Wie oft ein neues Bild je Durchlauf zusätzlich erscheinen darf.
pub const BOOST_MAX_SHOWS_PER_CYCLE: u8 = 3;
/// Mindestabstand zweier aufeinanderfolgender Aufnahmen (Serienbilder).
pub const CLUSTER_MIN_GAP_MINUTES: i64 = 2;
/// Nach so vielen Verwerfungen in Folge wird der Cluster-Filter übergangen.
pub const CLUSTER_MAX_RETRIES: u8 = 5;
/// Deckel für „lange nicht gezeigt" in Tagen.
pub const LRS_MAX_DAYS: f64 = 365.0;
/// Länge der Historie für das Zurückblättern (FA-41).
pub const HISTORY_LIMIT: usize = 50;

const SECONDS_PER_DAY: f64 = 86_400.0;

/// Zustand eines Durchlaufs. Überlebt Neustart und Stromausfall (R-08).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scheduler {
    /// Was im laufenden Durchlauf noch nicht gezogen wurde.
    bag: Vec<String>,
    /// Wie oft ein neues Bild in diesem Durchlauf schon erschienen ist.
    boost_shows: HashMap<String, u8>,
    /// Zuletzt gezogene Bilder, neuestes zuletzt.
    history: VecDeque<String>,
    /// Aufnahmezeit des zuletzt gezeigten Bildes — Bezug für den Cluster-Filter.
    last_taken_at: Option<i64>,
    /// Wie oft die Urne schon neu befüllt wurde. Nur für die Statistik.
    cycles: u64,
}

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Wie viele Bilder im laufenden Durchlauf noch offen sind (Wartung F1).
    pub fn remaining(&self) -> usize {
        self.bag.len()
    }

    pub fn cycles(&self) -> u64 {
        self.cycles
    }

    /// Leert die Urne, sodass die nächste Ziehung einen neuen Durchlauf beginnt
    /// (Wartung F2).
    pub fn restart(&mut self) {
        self.bag.clear();
        self.boost_shows.clear();
    }

    /// Nimmt ein Bild aus dem laufenden Durchlauf — gelöscht oder ausgeblendet.
    pub fn remove(&mut self, id: &str) {
        self.bag.retain(|b| b != id);
        self.boost_shows.remove(id);
        self.history.retain(|h| h != id);
    }

    /// Mischt ein neu eingetroffenes Bild in den laufenden Durchlauf ein.
    ///
    /// An zufälliger Stelle statt hinten: hinten angehängt käme es erst am
    /// Ende des Durchlaufs, und genau das soll der Neu-Boost verhindern.
    pub fn insert(&mut self, id: &str, rng: &mut impl RandomSource) {
        if self.bag.iter().any(|b| b == id) {
            return;
        }
        let at = if self.bag.is_empty() {
            0
        } else {
            (rng.next_f64() * self.bag.len() as f64) as usize
        };
        self.bag.insert(at.min(self.bag.len()), id.to_string());
    }

    /// Das zuletzt gezogene Bild, ohne neu zu ziehen.
    pub fn current(&self) -> Option<String> {
        self.history.back().cloned()
    }

    /// Vorheriges Bild aus der Historie (FA-41).
    ///
    /// Verbraucht den Eintrag: zweimal zurück führt zwei Bilder zurück. Das
    /// zurückgegebene Bild wandert *nicht* in die Urne zurück — es war in
    /// diesem Durchlauf bereits an der Reihe.
    pub fn back(&mut self) -> Option<String> {
        // Das letzte Element ist das gerade gezeigte; gebraucht wird das davor.
        self.history.pop_back()?;
        self.history.back().cloned()
    }

    /// Zieht das nächste Bild.
    ///
    /// `candidates` sind alle aktuell zulässigen Ids (aktive Quellen, nicht
    /// ausgeblendet). `now` ist die Unix-Zeit in Sekunden.
    pub fn draw(
        &mut self,
        index: &CacheIndex,
        candidates: &[String],
        cfg: &PlaybackConfig,
        now: i64,
        rng: &mut impl RandomSource,
    ) -> Option<String> {
        if candidates.is_empty() {
            return None;
        }

        if self.bag.is_empty() {
            self.refill(candidates);
        }

        // Verworfene dieser einen Ziehung. Sie bleiben in der Urne -- sie sind
        // nur gerade ungeschickt, nicht verbraucht --, duerfen aber im
        // naechsten Versuch nicht wieder gezogen werden. Ohne das haenge es
        // allein an der Zufallsquelle, ob ein zweiter Versuch etwas anderes
        // liefert; mit fester Quelle kam im Test immer dasselbe Bild zurueck.
        let mut rejected: Vec<String> = Vec::new();

        loop {
            let picked = {
                let skip: HashSet<&str> = rejected.iter().map(String::as_str).collect();
                let pool = self.pool(index, candidates, cfg, now, &|e| {
                    !skip.contains(e.id.as_str())
                });

                if pool.is_empty() {
                    if rejected.is_empty() {
                        // Alle Kandidaten sind zwischenzeitlich verschwunden.
                        self.refill(candidates);
                        if self.bag.is_empty() {
                            return None;
                        }
                        continue;
                    }
                    // Nur noch Verworfene uebrig: der Filter hat ausgereizt,
                    // was ging. Das zuerst Verworfene ist die beste Wahl -- und
                    // es wird ohne weitere Pruefung genommen. Ein erneuter
                    // Cluster-Test wuerde es zurueck in die Liste schieben, und
                    // die Schleife rotierte endlos durch dieselben Kandidaten.
                    let fallback = rejected.remove(0);
                    self.accept(index, &fallback, cfg, now);
                    return Some(fallback);
                } else {
                    weighted_pick(&pool, rng)?.to_string()
                }
            };

            if cfg.cluster_filter
                && (rejected.len() as u8) < CLUSTER_MAX_RETRIES
                && self.is_cluster(index, &picked)
            {
                rejected.push(picked);
                continue;
            }

            self.accept(index, &picked, cfg, now);
            return Some(picked);
        }
    }

    /// Zieht einen Partner für den Paar-Modus (FA-08, E-28).
    ///
    /// Anders als [`Self::draw`] füllt das die Urne **nicht** neu auf und
    /// verzichtet auf den Cluster-Filter: Es geht nicht um den nächsten
    /// Bildwechsel, sondern um die zweite Hälfte desselben. Findet sich kein
    /// passendes Bild, läuft das erste allein — besser als einen halben
    /// Durchlauf für eine Partnersuche zu verbrauchen.
    pub fn draw_partner(
        &mut self,
        index: &CacheIndex,
        candidates: &[String],
        cfg: &PlaybackConfig,
        now: i64,
        rng: &mut impl RandomSource,
        accept: &dyn Fn(&CacheEntry) -> bool,
    ) -> Option<String> {
        let picked = {
            let pool = self.pool(index, candidates, cfg, now, accept);
            weighted_pick(&pool, rng)?.to_string()
        };
        self.accept(index, &picked, cfg, now);
        Some(picked)
    }

    /// Trägt eine tatsächliche Anzeige ein.
    fn accept(&mut self, index: &CacheIndex, id: &str, cfg: &PlaybackConfig, now: i64) {
        self.bag.retain(|b| b != id);

        if cfg.new_boost && is_fresh(index.get(id), now) {
            *self.boost_shows.entry(id.to_string()).or_insert(0) += 1;
        }

        self.last_taken_at = index.get(id).and_then(|e| e.taken_at);

        self.history.push_back(id.to_string());
        while self.history.len() > HISTORY_LIMIT {
            self.history.pop_front();
        }
    }

    fn refill(&mut self, candidates: &[String]) {
        self.bag = candidates.to_vec();
        self.boost_shows.clear();
        self.cycles += 1;
    }

    /// Was gezogen werden darf, samt Gewicht.
    ///
    /// Das ist die Urne **plus** die frischen Bilder, die ihr Boost-Kontingent
    /// noch nicht verbraucht haben — Letztere „umgehen die Bag-Entnahme", wie
    /// das Papier es nennt.
    fn pool<'a>(
        &'a self,
        index: &'a CacheIndex,
        candidates: &'a [String],
        cfg: &PlaybackConfig,
        now: i64,
        accept: &dyn Fn(&CacheEntry) -> bool,
    ) -> Vec<(&'a str, f64)> {
        let usable = |id: &str| -> Option<&CacheEntry> {
            let e = index.get(id)?;
            accept(e).then_some(e)
        };

        let mut pool: Vec<(&str, f64)> = self
            .bag
            .iter()
            .filter_map(|id| usable(id).map(|e| (id.as_str(), self.weight(Some(e), cfg, now))))
            .collect();

        if cfg.new_boost {
            // Mengenpruefung statt linearer Suche: `bag.iter().any(...)` je
            // Kandidat ergab quadratischen Aufwand -- bei 10 000 Bildern rund
            // hundert Millionen Vergleiche und eine Ziehung von 400 ms.
            let in_bag: HashSet<&str> = self.bag.iter().map(String::as_str).collect();

            for id in candidates {
                if in_bag.contains(id.as_str()) {
                    continue;
                }
                let used = self.boost_shows.get(id).copied().unwrap_or(0);
                if used < BOOST_MAX_SHOWS_PER_CYCLE && is_fresh(index.get(id), now) {
                    if let Some(e) = usable(id) {
                        pool.push((id.as_str(), self.weight(Some(e), cfg, now)));
                    }
                }
            }
        }

        pool
    }

    fn weight(&self, entry: Option<&CacheEntry>, cfg: &PlaybackConfig, now: i64) -> f64 {
        let Some(entry) = entry else { return 1.0 };
        let mut w = 1.0;

        if cfg.new_boost && is_fresh(Some(entry), now) {
            w *= BOOST_FACTOR;
        }

        if cfg.least_recently_shown {
            let days = match entry.last_shown {
                // Nie gezeigt: der Deckel, nicht unendlich.
                None => LRS_MAX_DAYS,
                Some(t) => (((now - t).max(0)) as f64 / SECONDS_PER_DAY).min(LRS_MAX_DAYS),
            };
            w *= 1.0 + days / 30.0;
        }

        w
    }

    /// Liegt die Aufnahme zu dicht am zuletzt gezeigten Bild?
    ///
    /// Bilder ohne Aufnahmedatum sind ausgenommen — ohne Zeitstempel lässt
    /// sich keine Serie erkennen, und sie pauschal zu verwerfen hieße, sie nie
    /// zu zeigen.
    fn is_cluster(&self, index: &CacheIndex, id: &str) -> bool {
        let (Some(prev), Some(entry)) = (self.last_taken_at, index.get(id)) else {
            return false;
        };
        let Some(taken) = entry.taken_at else {
            return false;
        };
        (taken - prev).abs() < CLUSTER_MIN_GAP_MINUTES * 60
    }
}

/// Ist das Bild frisch genug für den Neu-Boost?
fn is_fresh(entry: Option<&CacheEntry>, now: i64) -> bool {
    let Some(entry) = entry else { return false };
    now - entry.added_at < BOOST_WINDOW_HOURS * 3600
}

/// Zufallsquelle, damit die Ziehung im Test vorhersagbar ist (E-29).
pub trait RandomSource {
    /// Gleichverteilt in `[0, 1)`.
    fn next_f64(&mut self) -> f64;
}

/// Zufallsquelle für den Betrieb.
///
/// `getrandom` statt eines eigenen Generators: die Quelle steht ohnehin schon
/// für die Seeds der übrigen Modi zur Verfügung, und ein selbstgebauter PRNG
/// waere hier nur zusaetzlicher Code ohne Gewinn.
pub struct SystemRandom;

impl RandomSource for SystemRandom {
    fn next_f64(&mut self) -> f64 {
        let mut buf = [0u8; 8];
        let bits = match getrandom::getrandom(&mut buf) {
            Ok(()) => u64::from_le_bytes(buf),
            // Schlägt die Systemquelle fehl, ist eine schlechte Zufallszahl
            // besser als eine Diashow, die stehenbleibt (NF-01).
            Err(_) => crate::state::now_ts() as u64,
        };
        // Obere 53 Bits: so viele traegt ein f64 verlustfrei.
        (bits >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Zieht gewichtet aus `pool`.
fn weighted_pick<'a>(pool: &[(&'a str, f64)], rng: &mut impl RandomSource) -> Option<&'a str> {
    let total: f64 = pool.iter().map(|(_, w)| w).sum();
    if total <= 0.0 {
        return pool.first().map(|(id, _)| *id);
    }

    let mut point = rng.next_f64() * total;
    for (id, w) in pool {
        point -= w;
        if point < 0.0 {
            return Some(id);
        }
    }
    // Rundungsfehler: das letzte Element ist die richtige Antwort.
    pool.last().map(|(id, _)| *id)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vorhersagbare Zufallsquelle: liefert die Folge der Reihe nach und
    /// wiederholt sie danach.
    struct FixedRandom {
        values: Vec<f64>,
        at: usize,
    }

    impl FixedRandom {
        fn new(values: &[f64]) -> Self {
            Self {
                values: values.to_vec(),
                at: 0,
            }
        }
    }

    impl RandomSource for FixedRandom {
        fn next_f64(&mut self) -> f64 {
            let v = self.values[self.at % self.values.len()];
            self.at += 1;
            v
        }
    }

    fn entry(id: &str, added_at: i64, last_shown: Option<i64>, taken_at: Option<i64>) -> CacheEntry {
        CacheEntry {
            id: id.into(),
            source_id: "s".into(),
            rel_path: id.into(),
            file_name: format!("{id}.jpg"),
            etag: None,
            remote_size: None,
            remote_mtime: None,
            taken_at,
            width: 1920,
            height: 1080,
            bytes: 100,
            added_at,
            last_shown,
            show_count: 0,
            excluded: false,
            thumb_bytes: None,
        }
    }

    fn index_of(entries: Vec<CacheEntry>) -> CacheIndex {
        let mut idx = CacheIndex::new();
        for e in entries {
            idx.insert(e);
        }
        idx
    }

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// Alle Gewichtungen aus -- damit die reine Urne geprueft wird.
    fn plain() -> PlaybackConfig {
        PlaybackConfig {
            new_boost: false,
            least_recently_shown: false,
            cluster_filter: false,
            newest_first: false,
        }
    }

    const NOW: i64 = 1_000_000;
    /// Aelter als das Boost-Fenster.
    const OLD: i64 = NOW - BOOST_WINDOW_HOURS * 3600 - 1;

    #[test]
    fn urne_zeigt_jedes_bild_genau_einmal_je_durchlauf() {
        let idx = index_of(vec![
            entry("a", OLD, None, None),
            entry("b", OLD, None, None),
            entry("c", OLD, None, None),
        ]);
        let candidates = ids(&["a", "b", "c"]);
        let mut sched = Scheduler::new();
        let mut rng = FixedRandom::new(&[0.1, 0.9, 0.5, 0.3]);

        let mut gezogen = Vec::new();
        for _ in 0..3 {
            gezogen.push(
                sched
                    .draw(&idx, &candidates, &plain(), NOW, &mut rng)
                    .unwrap(),
            );
        }
        gezogen.sort();
        assert_eq!(gezogen, vec!["a", "b", "c"], "jedes genau einmal");
        assert_eq!(sched.remaining(), 0, "Urne ist leer");
    }

    #[test]
    fn urne_fuellt_sich_nach_dem_durchlauf_neu() {
        let idx = index_of(vec![entry("a", OLD, None, None), entry("b", OLD, None, None)]);
        let candidates = ids(&["a", "b"]);
        let mut sched = Scheduler::new();
        let mut rng = FixedRandom::new(&[0.1]);

        for _ in 0..2 {
            sched.draw(&idx, &candidates, &plain(), NOW, &mut rng);
        }
        assert_eq!(sched.cycles(), 1);

        sched
            .draw(&idx, &candidates, &plain(), NOW, &mut rng)
            .unwrap();
        assert_eq!(
            sched.cycles(),
            2,
            "die dritte Ziehung beginnt einen neuen Durchlauf"
        );
    }

    #[test]
    fn neu_boost_hebt_das_gewicht_um_den_faktor_fuenf() {
        let cfg = PlaybackConfig {
            new_boost: true,
            ..plain()
        };
        let sched = Scheduler::new();
        let frisch = entry("neu", NOW - 3600, None, None);
        let alt = entry("alt", OLD, None, None);

        let w_neu = sched.weight(Some(&frisch), &cfg, NOW);
        let w_alt = sched.weight(Some(&alt), &cfg, NOW);
        assert_eq!(w_neu / w_alt, BOOST_FACTOR);
    }

    #[test]
    fn neu_boost_endet_nach_dem_fenster() {
        let cfg = PlaybackConfig {
            new_boost: true,
            ..plain()
        };
        let sched = Scheduler::new();
        // Genau an der Grenze -- schon zu alt.
        let knapp_zu_alt = entry("x", NOW - BOOST_WINDOW_HOURS * 3600, None, None);
        assert_eq!(sched.weight(Some(&knapp_zu_alt), &cfg, NOW), 1.0);
    }

    #[test]
    fn neu_boost_ist_je_durchlauf_begrenzt() {
        // Ohne Grenze koennte ein frisches Bild den ganzen Durchlauf beherrschen.
        let mut entries = vec![entry("neu", NOW - 3600, None, None)];
        let mut candidates = ids(&["neu"]);
        for i in 0..20 {
            let id = format!("alt{i}");
            entries.push(entry(&id, OLD, None, None));
            candidates.push(id);
        }
        let idx = index_of(entries);
        let cfg = PlaybackConfig {
            new_boost: true,
            ..plain()
        };
        let mut sched = Scheduler::new();
        // Immer das erste Element des Pools: erst die Urne, dann die
        // Boost-Nachzuegler.
        let mut rng = FixedRandom::new(&[0.0]);

        let mut treffer = 0;
        for _ in 0..21 {
            if let Some(id) = sched.draw(&idx, &candidates, &cfg, NOW, &mut rng) {
                if id == "neu" {
                    treffer += 1;
                }
            }
        }
        assert!(
            treffer <= BOOST_MAX_SHOWS_PER_CYCLE as usize,
            "hoechstens {BOOST_MAX_SHOWS_PER_CYCLE} je Durchlauf, hier waren es {treffer}"
        );
    }

    #[test]
    fn lange_nicht_gezeigte_wiegen_schwerer() {
        let cfg = PlaybackConfig {
            least_recently_shown: true,
            ..plain()
        };
        let sched = Scheduler::new();
        let gestern = entry("a", OLD, Some(NOW - 86_400), None);
        let vor_dreissig_tagen = entry("b", OLD, Some(NOW - 30 * 86_400), None);

        let w1 = sched.weight(Some(&gestern), &cfg, NOW);
        let w2 = sched.weight(Some(&vor_dreissig_tagen), &cfg, NOW);
        assert!((w1 - (1.0 + 1.0 / 30.0)).abs() < 1e-9);
        assert!(
            (w2 - 2.0).abs() < 1e-9,
            "dreissig Tage verdoppeln das Gewicht"
        );
    }

    #[test]
    fn nie_gezeigte_bekommen_den_deckel_statt_unendlich() {
        // Ohne Deckel entartete die Mischung zu "zeige zuerst alles Ungesehene".
        let cfg = PlaybackConfig {
            least_recently_shown: true,
            ..plain()
        };
        let sched = Scheduler::new();
        let nie = entry("a", OLD, None, None);
        let uralt = entry("b", OLD, Some(NOW - 10_000 * 86_400), None);

        let w_nie = sched.weight(Some(&nie), &cfg, NOW);
        assert_eq!(w_nie, 1.0 + LRS_MAX_DAYS / 30.0);
        assert_eq!(
            sched.weight(Some(&uralt), &cfg, NOW),
            w_nie,
            "auch zehntausend Tage werden gedeckelt"
        );
    }

    #[test]
    fn cluster_filter_trennt_serienaufnahmen() {
        let idx = index_of(vec![
            entry("serie1", OLD, None, Some(500_000)),
            entry("serie2", OLD, None, Some(500_060)),
            entry("weit", OLD, None, Some(900_000)),
        ]);
        let candidates = ids(&["serie1", "serie2", "weit"]);
        let cfg = PlaybackConfig {
            cluster_filter: true,
            ..plain()
        };
        let mut sched = Scheduler::new();
        let mut rng = FixedRandom::new(&[0.0]);

        let erstes = sched.draw(&idx, &candidates, &cfg, NOW, &mut rng).unwrap();
        let zweites = sched.draw(&idx, &candidates, &cfg, NOW, &mut rng).unwrap();
        let beide = [erstes.as_str(), zweites.as_str()];
        assert!(
            !(beide.contains(&"serie1") && beide.contains(&"serie2")),
            "die Serie darf nicht direkt hintereinander laufen: {beide:?}"
        );
    }

    #[test]
    fn cluster_filter_gibt_nach_wenigen_versuchen_auf() {
        // Nur Serienbilder: ohne Notausgang zoege der Planer endlos.
        let mut entries = Vec::new();
        let mut candidates = Vec::new();
        for i in 0..4 {
            let id = format!("s{i}");
            entries.push(entry(&id, OLD, None, Some(500_000 + i)));
            candidates.push(id);
        }
        let idx = index_of(entries);
        let cfg = PlaybackConfig {
            cluster_filter: true,
            ..plain()
        };
        let mut sched = Scheduler::new();
        let mut rng = FixedRandom::new(&[0.0, 0.3, 0.7, 0.5]);

        for _ in 0..4 {
            assert!(
                sched.draw(&idx, &candidates, &cfg, NOW, &mut rng).is_some(),
                "der Filter darf die Diashow nicht anhalten"
            );
        }
    }

    #[test]
    fn bilder_ohne_aufnahmedatum_umgehen_den_cluster_filter() {
        let idx = index_of(vec![
            entry("mit", OLD, None, Some(500_000)),
            entry("ohne", OLD, None, None),
        ]);
        let candidates = ids(&["mit", "ohne"]);
        let cfg = PlaybackConfig {
            cluster_filter: true,
            ..plain()
        };
        let mut sched = Scheduler::new();
        let mut rng = FixedRandom::new(&[0.0]);

        sched.draw(&idx, &candidates, &cfg, NOW, &mut rng);
        assert!(
            sched.draw(&idx, &candidates, &cfg, NOW, &mut rng).is_some(),
            "ohne Zeitstempel laesst sich keine Serie erkennen"
        );
    }

    #[test]
    fn neues_bild_wird_in_den_laufenden_durchlauf_eingemischt() {
        let idx = index_of(vec![entry("a", OLD, None, None), entry("b", OLD, None, None)]);
        let candidates = ids(&["a", "b"]);
        let mut sched = Scheduler::new();
        let mut rng = FixedRandom::new(&[0.0]);

        sched.draw(&idx, &candidates, &plain(), NOW, &mut rng);
        assert_eq!(sched.remaining(), 1);

        sched.insert("neu", &mut rng);
        assert_eq!(
            sched.remaining(),
            2,
            "das neue Bild wartet noch in dieser Runde"
        );
    }

    #[test]
    fn entferntes_bild_verlaesst_urne_und_historie() {
        let idx = index_of(vec![entry("a", OLD, None, None), entry("b", OLD, None, None)]);
        let candidates = ids(&["a", "b"]);
        let mut sched = Scheduler::new();
        let mut rng = FixedRandom::new(&[0.0]);

        let gezogen = sched
            .draw(&idx, &candidates, &plain(), NOW, &mut rng)
            .unwrap();
        sched.remove(&gezogen);
        assert_eq!(sched.current(), None, "auch aus der Historie");
        assert!(!sched.bag.contains(&gezogen));
    }

    #[test]
    fn zurueck_liefert_das_vorherige_bild() {
        // FA-41 verlangt Wischen nach rueckwaerts; das Urnen-Modell des
        // Papiers kennt das nicht.
        let idx = index_of(vec![
            entry("a", OLD, None, None),
            entry("b", OLD, None, None),
            entry("c", OLD, None, None),
        ]);
        let candidates = ids(&["a", "b", "c"]);
        let mut sched = Scheduler::new();
        let mut rng = FixedRandom::new(&[0.0]);

        let erstes = sched
            .draw(&idx, &candidates, &plain(), NOW, &mut rng)
            .unwrap();
        sched.draw(&idx, &candidates, &plain(), NOW, &mut rng).unwrap();

        assert_eq!(sched.back(), Some(erstes));
    }

    #[test]
    fn zurueck_am_anfang_liefert_nichts() {
        let mut sched = Scheduler::new();
        assert_eq!(sched.back(), None);
    }

    #[test]
    fn zustand_uebersteht_einen_neustart() {
        // Der Durchlauf soll einen Stromausfall ueberleben (R-08).
        let idx = index_of(vec![
            entry("a", OLD, None, None),
            entry("b", OLD, None, None),
            entry("c", OLD, None, None),
        ]);
        let candidates = ids(&["a", "b", "c"]);
        let mut sched = Scheduler::new();
        let mut rng = FixedRandom::new(&[0.0]);
        sched.draw(&idx, &candidates, &plain(), NOW, &mut rng);

        let json = serde_json::to_string(&sched).unwrap();
        let wieder: Scheduler = serde_json::from_str(&json).unwrap();

        assert_eq!(wieder.remaining(), sched.remaining());
        assert_eq!(wieder.current(), sched.current());
        assert_eq!(wieder.cycles(), sched.cycles());
    }

    #[test]
    fn durchlauf_neu_starten_leert_die_urne() {
        // Wartung F2.
        let idx = index_of(vec![entry("a", OLD, None, None), entry("b", OLD, None, None)]);
        let candidates = ids(&["a", "b"]);
        let mut sched = Scheduler::new();
        let mut rng = FixedRandom::new(&[0.0]);
        sched.draw(&idx, &candidates, &plain(), NOW, &mut rng);
        assert_eq!(sched.remaining(), 1);

        sched.restart();
        assert_eq!(sched.remaining(), 0);
    }

    #[test]
    fn partnersuche_liefert_nur_passende_formate() {
        // Paarbildung nach E-28: gezogen wird eines, der Partner muss das
        // gleiche (nicht zum Rahmen passende) Format haben.
        let mut hoch = entry("hoch", OLD, None, None);
        hoch.width = 1080;
        hoch.height = 1920;
        let idx = index_of(vec![hoch, entry("quer", OLD, None, None)]);
        let candidates = ids(&["hoch", "quer"]);
        let mut sched = Scheduler::new();
        let mut rng = FixedRandom::new(&[0.0]);

        sched.draw(&idx, &candidates, &plain(), NOW, &mut rng);
        let partner = sched.draw_partner(
            &idx,
            &candidates,
            &plain(),
            NOW,
            &mut rng,
            &|e: &CacheEntry| e.is_portrait(),
        );
        assert!(
            partner.is_none() || partner.as_deref() == Some("hoch"),
            "nur Hochformat kommt als Partner in Frage"
        );
    }

    #[test]
    fn ziehung_bleibt_schnell_bei_zehntausend_bildern() {
        // Nicht-funktionale Vorgabe aus 2.5: unter 50 ms.
        let mut entries = Vec::new();
        let mut candidates = Vec::new();
        for i in 0..10_000 {
            let id = format!("id{i}");
            entries.push(entry(&id, OLD, Some(NOW - i as i64), Some(i as i64 * 600)));
            candidates.push(id);
        }
        let idx = index_of(entries);
        let cfg = PlaybackConfig::default();
        let mut sched = Scheduler::new();
        let mut rng = FixedRandom::new(&[0.42, 0.13, 0.87]);

        let start = std::time::Instant::now();
        sched.draw(&idx, &candidates, &cfg, NOW, &mut rng).unwrap();
        let dauer = start.elapsed();
        assert!(
            dauer.as_millis() < 50,
            "Ziehung dauerte {dauer:?} bei 10.000 Bildern"
        );
    }
}

//! Protokoll der Postfach-Abrufe (Wartung F6).
//!
//! ## Warum ein Ringpuffer und keine Datei je Lauf
//!
//! Ein Rahmen ruft im Viertelstundentakt ab — das sind rund 35 000 Läufe im
//! Jahr. Die Frage, die jemand vor dem Gerät wirklich hat, lautet aber „lief
//! der Abruf, und was kam dabei heraus?", und die beantworten die letzten
//! Einträge. Das Papier nennt 50; alles darüber wäre Speicher für eine Frage,
//! die niemand stellt.
//!
//! ## Warum nicht das Android-Protokoll
//!
//! `logcat` überlebt keinen Neustart und ist ohne Kabel nicht lesbar. Der
//! Rahmen hängt an der Wand.

use serde::{Deserialize, Serialize};

/// Wie viele Läufe aufgehoben werden (Papier 3.4: hart auf 50 begrenzt).
pub const LOG_LIMIT: usize = 50;

/// Wodurch ein Abruf ausgelöst wurde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    /// Der Zeitgeber im Hintergrund (FA-28).
    Interval,
    /// Jemand hat am Gerät auf „Jetzt abrufen" getippt (F7).
    Manual,
    /// Vollständiger Neuabgleich des Postfachs (F8).
    Resync,
}

/// Ein Eintrag im Abruf-Protokoll (F6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchLogEntry {
    /// Unix-Sekunden des Laufs.
    pub at: i64,
    pub source_id: String,
    pub trigger: Trigger,
    /// Nachrichten im Ordner zum Zeitpunkt des Laufs.
    pub seen_in_folder: usize,
    /// Davon bereits bekannt (E-34, Stufe eins).
    pub already_known: usize,
    /// Vollständig geholte Nachrichten.
    pub checked: usize,
    pub added: usize,
    pub quarantined: usize,
    pub skipped: usize,
    pub failed: usize,
    /// Fehlertext im Klartext; `None` bei einem geglückten Lauf.
    pub error: Option<String>,
}

impl FetchLogEntry {
    /// Ist dieser Lauf schiefgegangen? Grundlage der Hervorhebung (F6).
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

/// Ringpuffer der letzten Läufe.
///
/// `Vec` statt `VecDeque`: bei 50 Einträgen ist das Verschieben beim Abschnei­den
/// nicht messbar, und ein `Vec` serialisiert ohne Umweg nach JSON — der Puffer
/// liegt neben dem Cache-Index auf der Platte.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FetchLog {
    entries: Vec<FetchLogEntry>,
}

impl FetchLog {
    /// Nimmt einen Lauf auf und wirft den ältesten weg, sobald es zu viele
    /// werden.
    pub fn push(&mut self, entry: FetchLogEntry) {
        self.entries.push(entry);
        if self.entries.len() > LOG_LIMIT {
            let ueberzaehlig = self.entries.len() - LOG_LIMIT;
            self.entries.drain(0..ueberzaehlig);
        }
    }

    /// Die Läufe, neueste zuerst — so wird die Liste auch gelesen.
    pub fn recent(&self) -> Vec<FetchLogEntry> {
        self.entries.iter().rev().cloned().collect()
    }

    /// Der jüngste Lauf einer Quelle, für die Statuszeile (F5).
    pub fn last_for(&self, source_id: &str) -> Option<&FetchLogEntry> {
        self.entries.iter().rev().find(|e| e.source_id == source_id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eintrag(at: i64, source: &str, fehler: Option<&str>) -> FetchLogEntry {
        FetchLogEntry {
            at,
            source_id: source.into(),
            trigger: Trigger::Interval,
            seen_in_folder: 3,
            already_known: 1,
            checked: 2,
            added: 1,
            quarantined: 1,
            skipped: 0,
            failed: 0,
            error: fehler.map(|s| s.to_string()),
        }
    }

    #[test]
    fn haelt_die_grenze_hart_ein() {
        // Papier 3.4: hart auf 50 begrenzt. Ein Rahmen ruft im
        // Viertelstundentakt ab -- ohne Grenze waeren das 35 000 Eintraege
        // im Jahr, die bei jedem Start mitgelesen wuerden.
        let mut log = FetchLog::default();
        for i in 0..120 {
            log.push(eintrag(i, "s", None));
        }
        assert_eq!(log.len(), LOG_LIMIT);
    }

    #[test]
    fn wirft_die_aeltesten_weg_nicht_die_neuesten() {
        let mut log = FetchLog::default();
        for i in 0..(LOG_LIMIT as i64 + 5) {
            log.push(eintrag(i, "s", None));
        }
        let neueste = log.recent();
        assert_eq!(neueste.first().map(|e| e.at), Some(LOG_LIMIT as i64 + 4));
        assert_eq!(neueste.last().map(|e| e.at), Some(5), "die ersten fuenf sind weg");
    }

    #[test]
    fn liefert_die_neuesten_zuerst() {
        // So wird die Liste gelesen: was zuletzt geschah, steht oben.
        let mut log = FetchLog::default();
        log.push(eintrag(10, "s", None));
        log.push(eintrag(20, "s", None));
        log.push(eintrag(30, "s", None));
        let zeiten: Vec<i64> = log.recent().iter().map(|e| e.at).collect();
        assert_eq!(zeiten, vec![30, 20, 10]);
    }

    #[test]
    fn findet_den_letzten_lauf_je_quelle() {
        // Zwei Postfaecher sind moeglich; die Statuszeile einer Quelle darf
        // nicht den Lauf der anderen zeigen.
        let mut log = FetchLog::default();
        log.push(eintrag(10, "a", None));
        log.push(eintrag(20, "b", None));
        log.push(eintrag(30, "a", None));

        assert_eq!(log.last_for("a").map(|e| e.at), Some(30));
        assert_eq!(log.last_for("b").map(|e| e.at), Some(20));
        assert!(log.last_for("gibtsnicht").is_none());
    }

    #[test]
    fn erkennt_fehlgeschlagene_laeufe() {
        assert!(!eintrag(1, "s", None).is_error());
        assert!(eintrag(1, "s", Some("Anmeldung abgelehnt")).is_error());
    }

    #[test]
    fn ueberlebt_das_speichern_und_laden() {
        // Der Puffer liegt neben dem Cache-Index auf der Platte -- ohne das
        // waere er nach jedem Neustart leer, und gerade dann sucht man ihn.
        let mut log = FetchLog::default();
        log.push(eintrag(10, "s", None));
        log.push(eintrag(20, "s", Some("kaputt")));

        let json = serde_json::to_string(&log).unwrap();
        let zurueck: FetchLog = serde_json::from_str(&json).unwrap();

        assert_eq!(zurueck.len(), 2);
        assert_eq!(zurueck.recent()[0].at, 20);
        assert_eq!(zurueck.recent()[0].error.as_deref(), Some("kaputt"));
    }

    #[test]
    fn leerer_puffer_meldet_sich_als_leer() {
        let log = FetchLog::default();
        assert!(log.is_empty());
        assert!(log.recent().is_empty());
        assert!(log.last_for("s").is_none());
    }
}

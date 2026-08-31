//! Gedächtnis für bereits verarbeitete Nachrichten (E-36).
//!
//! ## Warum der Cache-Index nicht reicht
//!
//! Der Doppelimport-Schutz (F2) erkennt eine Mail an ihrer Message-Id im
//! Cache-Index. Der Index kennt aber nur **Fotos**. Eine Nachricht ohne
//! brauchbaren Anhang — eine Antwort, eine Rechnung, ein Newsletter —
//! hinterlässt dort nichts und ist beim nächsten Lauf wieder unbekannt.
//!
//! Solange nur Ungelesenes geholt wurde, fiel das nicht auf: der
//! Gelesen-Vermerk nahm sie aus der Suche. Mit „auch gelesene" (E-34) bleibt
//! sie für immer darin und wird alle fünfzehn Minuten erneut vollständig
//! heruntergeladen. Am Gerät im Abruf-Protokoll aufgefallen — Stunde um
//! Stunde dieselbe Zeile: „1 geholt · 0 neu · 2 bekannt".
//!
//! ## Warum begrenzt
//!
//! Ein Postfach wächst unbegrenzt, der Rahmen hat 2 GB. Gemerkt werden
//! ausschließlich Nachrichten **ohne** Foto — die mit Foto stehen ohnehin im
//! Index —, und davon die jüngsten. Fällt eine aus dem Gedächtnis, wird sie
//! einmal zu viel geholt; das ist die günstigere Seite des Fehlers.

use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

/// Wie viele Kennungen aufgehoben werden.
///
/// 5 000 Hashes zu je 16 Zeichen sind rund 80 KB — neben 2 GB Bildern nichts,
/// und mehr photoreiche Post als ein Rahmen je sieht.
pub const SEEN_LIMIT: usize = 5_000;

/// Kennungen verarbeiteter Nachrichten, älteste zuerst verdrängt.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SeenMails {
    /// Reihenfolge für die Verdrängung. Die Menge daneben ist nur ein Index
    /// darauf und wird beim Laden neu aufgebaut.
    order: VecDeque<String>,
    #[serde(skip)]
    lookup: HashSet<String>,
}

impl SeenMails {
    /// Baut den Suchindex nach dem Laden auf.
    ///
    /// Ohne diesen Schritt wäre `contains` nach einem Neustart immer falsch,
    /// und das Gedächtnis nutzlos — der Fehler, gegen den es gebaut ist, käme
    /// still zurück.
    pub fn rebuild(&mut self) {
        self.lookup = self.order.iter().cloned().collect();
    }

    pub fn contains(&self, hash: &str) -> bool {
        self.lookup.contains(hash)
    }

    /// Merkt eine Kennung. Doppelte verschieben nichts.
    pub fn insert(&mut self, hash: String) {
        if self.lookup.contains(&hash) {
            return;
        }
        self.lookup.insert(hash.clone());
        self.order.push_back(hash);
        while self.order.len() > SEEN_LIMIT {
            if let Some(alt) = self.order.pop_front() {
                self.lookup.remove(&alt);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merkt_und_erkennt_wieder() {
        let mut s = SeenMails::default();
        assert!(!s.contains("a"));
        s.insert("a".into());
        assert!(s.contains("a"));
    }

    #[test]
    fn doppelte_verschieben_die_reihenfolge_nicht() {
        // Sonst haelt eine Mail, die bei jedem Lauf auftaucht, sich selbst
        // dauerhaft im Gedaechtnis und verdraengt die uebrigen.
        let mut s = SeenMails::default();
        s.insert("a".into());
        s.insert("b".into());
        s.insert("a".into());
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn verdraengt_die_aeltesten() {
        let mut s = SeenMails::default();
        for i in 0..(SEEN_LIMIT + 10) {
            s.insert(format!("id{i}"));
        }
        assert_eq!(s.len(), SEEN_LIMIT);
        assert!(!s.contains("id0"), "die aeltesten sind weg");
        assert!(s.contains(&format!("id{}", SEEN_LIMIT + 9)), "die juengste ist da");
    }

    #[test]
    fn ueberlebt_speichern_und_laden() {
        // Der eigentliche Zweck: nach einem Neustart darf die photolose Mail
        // nicht wieder unbekannt sein.
        let mut s = SeenMails::default();
        s.insert("abc".into());
        s.insert("def".into());

        let json = serde_json::to_string(&s).unwrap();
        let mut zurueck: SeenMails = serde_json::from_str(&json).unwrap();
        zurueck.rebuild();

        assert!(zurueck.contains("abc"));
        assert!(zurueck.contains("def"));
        assert_eq!(zurueck.len(), 2);
    }

    #[test]
    fn ohne_rebuild_faende_es_nichts() {
        // Haelt fest, warum `rebuild` noetig ist: die Suchmenge wird nicht
        // mitgespeichert. Wer das Laden einbaut und den Aufruf vergisst, hat
        // ein Gedaechtnis, das nichts erinnert -- und merkt es nie, weil der
        // Abruf trotzdem laeuft.
        let mut s = SeenMails::default();
        s.insert("abc".into());
        let json = serde_json::to_string(&s).unwrap();
        let ohne: SeenMails = serde_json::from_str(&json).unwrap();

        assert_eq!(ohne.len(), 1, "die Reihenfolge ist da");
        assert!(!ohne.contains("abc"), "die Suchmenge aber nicht");
    }

    #[test]
    fn leeres_gedaechtnis_meldet_sich_als_leer() {
        let s = SeenMails::default();
        assert!(s.is_empty());
        assert!(!s.contains("irgendwas"));
    }
}

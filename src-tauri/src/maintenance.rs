//! Wartung und Diagnose (Erweiterungspapier Teil 3).
//!
//! ## Warum ein eigenes Modul
//!
//! Die Auswertungen lesen den Cache-Index, rechnen und geben Zahlen zurück —
//! sie brauchen weder Tauri noch das Dateisystem. Als reine Funktionen sind
//! sie ohne laufende App prüfbar; in `commands.rs` wäre jede von ihnen an
//! einen `State` gebunden und damit nur am Gerät zu beurteilen.
//!
//! ## Warum kein eigener Navigationsbereich
//!
//! E-31: Statistik und Durchlauf stehen bei der Diashow, die Postfach-
//! Werkzeuge bei den Mail-Einstellungen, Speicher und Datenbank bei System.
//! Das Papier sah einen sechsten Punkt „Wartung & Diagnose" vor; der Entwurf
//! (E-13) kennt fünf, und eine Funktion findet man dort, wo man sie sucht.

use crate::cache::index::{CacheEntry, CacheIndex};
use serde::Serialize;

/// Wie viele Bilder die Bestenlisten führen (Papier 3.1: Top 10).
pub const TOP_LIMIT: usize = 10;

/// Ein Bild in einer der Bestenlisten.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TopEntry {
    pub id: String,
    pub file_name: String,
    pub show_count: u32,
    /// Unix-Sekunden der letzten Anzeige; `None`, wenn nie gezeigt.
    pub last_shown: Option<i64>,
}

impl From<&CacheEntry> for TopEntry {
    fn from(e: &CacheEntry) -> Self {
        Self {
            id: e.id.clone(),
            file_name: e.file_name.clone(),
            show_count: e.show_count,
            last_shown: e.last_shown,
        }
    }
}

/// Statistik der Zufallswiedergabe (F1).
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackStats {
    /// Alle Bilder im Cache, auch ausgeblendete und wartende.
    pub total: usize,
    /// Bilder, die in der Diashow laufen können — Bezugsgröße des Durchlaufs.
    pub eligible: usize,
    /// Davon noch nie gezeigt.
    pub never_shown: usize,
    /// Im laufenden Durchlauf noch offen („Bag: 342 von 1.850").
    pub bag_remaining: usize,
    /// Abgeschlossene Durchläufe seit dem letzten Zurücksetzen.
    pub cycles: u64,
    /// Meistgezeigte Bilder, absteigend.
    pub most_shown: Vec<TopEntry>,
    /// Am längsten nicht gezeigte, aufsteigend nach Zeitpunkt.
    ///
    /// Nie gezeigte stehen **nicht** darin: sie haben keinen Zeitpunkt, würden
    /// die Liste anführen und dabei nichts zeigen, was die eigene Kachel
    /// „Nie gezeigt" (F4) nicht besser zeigt.
    pub longest_unseen: Vec<TopEntry>,
}

/// Rechnet die Statistik aus dem Index (F1).
///
/// `eligible` entscheidet, welche Bilder überhaupt zum Bestand zählen — die
/// Diashow zieht nur aus diesen. Ausgeblendete und wartende Bilder wären
/// sonst im Nenner von „342 von 1.850" enthalten, und der Fortschritt käme
/// nie bei null an.
pub fn playback_stats(
    index: &CacheIndex,
    eligible: &dyn Fn(&CacheEntry) -> bool,
    bag_remaining: usize,
    cycles: u64,
) -> PlaybackStats {
    let alle: Vec<&CacheEntry> = index.values().collect();
    let bestand: Vec<&CacheEntry> = alle.iter().copied().filter(|e| eligible(e)).collect();

    let mut meist: Vec<&CacheEntry> = bestand.iter().copied().filter(|e| e.show_count > 0).collect();
    // Bei gleicher Zahl der Dateiname, damit die Liste zwischen zwei Aufrufen
    // nicht springt — `HashMap` gibt keine Reihenfolge vor.
    meist.sort_by(|a, b| {
        b.show_count
            .cmp(&a.show_count)
            .then(a.file_name.cmp(&b.file_name))
    });

    let mut laengst: Vec<&CacheEntry> = bestand
        .iter()
        .copied()
        .filter(|e| e.last_shown.is_some())
        .collect();
    laengst.sort_by(|a, b| {
        a.last_shown
            .cmp(&b.last_shown)
            .then(a.file_name.cmp(&b.file_name))
    });

    PlaybackStats {
        total: alle.len(),
        eligible: bestand.len(),
        never_shown: bestand.iter().filter(|e| e.show_count == 0).count(),
        bag_remaining,
        cycles,
        most_shown: meist.iter().take(TOP_LIMIT).map(|e| TopEntry::from(*e)).collect(),
        longest_unseen: laengst
            .iter()
            .take(TOP_LIMIT)
            .map(|e| TopEntry::from(*e))
            .collect(),
    }
}

/// Belegung nach Jahr oder Absender (F9).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageGroup {
    /// Jahreszahl oder Mailadresse; „—" für Bilder ohne Zuordnung.
    pub label: String,
    pub count: usize,
    pub bytes: u64,
}

/// Aufschlüsselung des Speichers (F9).
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StorageBreakdown {
    /// Nach Aufnahmejahr, neueste zuerst.
    pub by_year: Vec<StorageGroup>,
    /// Nach Absender, größte zuerst. Nur Fotos aus Postfächern.
    pub by_sender: Vec<StorageGroup>,
}

/// Beschriftung für Bilder ohne Jahr oder ohne Absender.
pub const UNKNOWN_LABEL: &str = "—";

/// Rechnet die Aufschlüsselung aus dem Index (F9).
///
/// `year_of` kommt von außen, weil die Umrechnung von Unix-Sekunden in ein
/// Kalenderjahr die Zeitzone braucht — dieselbe Funktion wie in `playlist`,
/// damit die Aufschlüsselung und der Jahresfilter nicht verschiedene Jahre
/// meinen.
pub fn storage_breakdown(
    index: &CacheIndex,
    year_of: &dyn Fn(i64) -> i32,
) -> StorageBreakdown {
    use std::collections::HashMap;

    let mut jahre: HashMap<String, (usize, u64)> = HashMap::new();
    let mut absender: HashMap<String, (usize, u64)> = HashMap::new();

    for e in index.values() {
        // Vorschaubilder zählen mit: sie liegen auf demselben Gerät, und wer
        // wissen will, wo der Platz bleibt, will sie nicht suchen müssen.
        let bytes = e.bytes + e.thumb_bytes.unwrap_or(0);

        let jahr = match e.taken_at {
            Some(t) => year_of(t).to_string(),
            None => UNKNOWN_LABEL.to_string(),
        };
        let eintrag = jahre.entry(jahr).or_insert((0, 0));
        eintrag.0 += 1;
        eintrag.1 += bytes;

        if let Some(m) = e.mail.as_ref() {
            let eintrag = absender
                .entry(m.sender.trim().to_lowercase())
                .or_insert((0, 0));
            eintrag.0 += 1;
            eintrag.1 += bytes;
        }
    }

    let mut by_year: Vec<StorageGroup> = jahre
        .into_iter()
        .map(|(label, (count, bytes))| StorageGroup {
            label,
            count,
            bytes,
        })
        .collect();
    // Neueste zuerst; „—" ans Ende, weil es kein Jahr ist und sonst je nach
    // Zeichenvergleich mitten in der Liste landete.
    by_year.sort_by(|a, b| match (a.label.as_str(), b.label.as_str()) {
        (UNKNOWN_LABEL, UNKNOWN_LABEL) => std::cmp::Ordering::Equal,
        (UNKNOWN_LABEL, _) => std::cmp::Ordering::Greater,
        (_, UNKNOWN_LABEL) => std::cmp::Ordering::Less,
        _ => b.label.cmp(&a.label),
    });

    let mut by_sender: Vec<StorageGroup> = absender
        .into_iter()
        .map(|(label, (count, bytes))| StorageGroup {
            label,
            count,
            bytes,
        })
        .collect();
    // Größte zuerst, bei Gleichstand alphabetisch — sonst spränge die Liste
    // zwischen zwei Aufrufen.
    by_sender.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.label.cmp(&b.label)));

    StorageBreakdown { by_year, by_sender }
}

/// Ergebnis der Datenbank-Prüfung (F10).
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseCheck {
    /// Einträge im Index, zu denen keine Bilddatei existiert.
    ///
    /// Entsteht, wenn eine Datei von außen verschwindet — ein abgebrochener
    /// Schreibvorgang, ein Aufräumer des Systems. Die Diashow zöge sie und
    /// zeigte nichts.
    pub missing_files: Vec<String>,
    /// Bilddateien ohne Eintrag im Index.
    ///
    /// Entsteht, wenn der Index verlorengeht oder ein Schreibvorgang zwischen
    /// Datei und Index abbricht. Sie belegen Platz, den niemand mehr zählt.
    pub orphan_files: Vec<String>,
    /// Vorschaubilder ohne zugehörigen Eintrag.
    pub orphan_thumbs: Vec<String>,
    /// Summe der Bytes, die ein Aufräumen freigäbe.
    pub reclaimable_bytes: u64,
}

impl DatabaseCheck {
    /// Ist alles in Ordnung? Grundlage der Meldung „nichts zu tun".
    pub fn is_clean(&self) -> bool {
        self.missing_files.is_empty()
            && self.orphan_files.is_empty()
            && self.orphan_thumbs.is_empty()
    }
}

/// Vergleicht Index und Dateibestand (F10).
///
/// Die Verzeichnislisten kommen von außen, damit die Auswertung ohne
/// Dateisystem prüfbar bleibt — der Aufrufer liest die Ordner, diese Funktion
/// entscheidet.
pub fn check_database(
    index: &CacheIndex,
    image_ids_on_disk: &[String],
    thumb_ids_on_disk: &[String],
    size_of: &dyn Fn(&str) -> u64,
) -> DatabaseCheck {
    use std::collections::HashSet;

    let vorhandene: HashSet<&str> = image_ids_on_disk.iter().map(|s| s.as_str()).collect();
    let bekannte: HashSet<String> = index.values().map(|e| e.id.clone()).collect();

    let mut missing_files: Vec<String> = index
        .values()
        .filter(|e| !vorhandene.contains(e.id.as_str()))
        .map(|e| e.id.clone())
        .collect();

    let mut orphan_files: Vec<String> = image_ids_on_disk
        .iter()
        .filter(|id| !bekannte.contains(*id))
        .cloned()
        .collect();

    let mut orphan_thumbs: Vec<String> = thumb_ids_on_disk
        .iter()
        .filter(|id| !bekannte.contains(*id))
        .cloned()
        .collect();

    // Sortiert, damit zwei Läufe dasselbe melden und die Liste im Bericht
    // nicht springt.
    missing_files.sort();
    orphan_files.sort();
    orphan_thumbs.sort();

    let reclaimable_bytes = orphan_files
        .iter()
        .chain(orphan_thumbs.iter())
        .map(|id| size_of(id))
        .sum();

    DatabaseCheck {
        missing_files,
        orphan_files,
        orphan_thumbs,
        reclaimable_bytes,
    }
}

/// Fassung des Sicherungsformats (F12, Papier: Pflichtfeld ab 1).
///
/// Wird hochgezählt, wenn eine Änderung alte Dateien unlesbar macht. Eine
/// Sicherung mit höherer Nummer lehnt der Import ab, statt sie halb zu
/// übernehmen — ein halb wiederhergestellter Rahmen ist schlimmer als einer,
/// der sagt „damit kann ich nichts anfangen".
pub const SCHEMA_VERSION: u32 = 1;

/// Umschlag einer Sicherung (F12).
///
/// Die Konfiguration steckt in einem Feld statt auf oberster Ebene, damit die
/// Version danebensteht und nicht mit einer Einstellung verwechselt werden
/// kann.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub schema_version: u32,
    /// Wann gesichert wurde, Unix-Sekunden — nur zur Anzeige.
    #[serde(default)]
    pub created_at: i64,
    pub config: crate::model::AppConfig,
}

/// Warum eine Sicherung nicht übernommen werden kann (F12).
#[derive(Debug, Clone, PartialEq)]
pub enum BackupError {
    /// Die Datei stammt aus einer neueren Fassung der App.
    TooNew { found: u32, supported: u32 },
    /// Kein gültiges Sicherungsformat.
    Malformed(String),
}

impl std::fmt::Display for BackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Ohne den Hinweis auf die Aktualisierung stünde da nur eine Zahl,
            // und niemand wüsste, was zu tun ist.
            Self::TooNew { found, supported } => write!(
                f,
                "Diese Sicherung stammt aus einer neueren Version von Slowshow \
                 (Format {found}, unterstützt wird {supported}). Bitte zuerst die App aktualisieren."
            ),
            Self::Malformed(why) => {
                write!(f, "Datei ist keine gültige Sicherung: {why}")
            }
        }
    }
}

/// Liest eine Sicherung und prüft ihre Fassung (F12).
///
/// Unbekannte Einzelfelder werden übergangen — so bleibt eine Sicherung aus
/// einer älteren App lesbar, auch wenn seither Einstellungen dazugekommen
/// sind. Eine *neuere* Fassung dagegen wird abgelehnt: dort könnte eine
/// Einstellung ihre Bedeutung geändert haben.
pub fn parse_backup(json: &str) -> Result<Backup, BackupError> {
    // Erst nur die Version lesen: ist sie zu neu, soll die Meldung das sagen
    // und nicht über ein unbekanntes Feld stolpern.
    #[derive(serde::Deserialize)]
    struct NurVersion {
        #[serde(rename = "schemaVersion")]
        schema_version: Option<u32>,
    }

    let probe: NurVersion =
        serde_json::from_str(json).map_err(|e| BackupError::Malformed(e.to_string()))?;

    match probe.schema_version {
        None => Err(BackupError::Malformed(
            "Feld „schemaVersion\" fehlt".into(),
        )),
        Some(v) if v > SCHEMA_VERSION => Err(BackupError::TooNew {
            found: v,
            supported: SCHEMA_VERSION,
        }),
        Some(_) => serde_json::from_str(json).map_err(|e| BackupError::Malformed(e.to_string())),
    }
}

/// Woraus der Diagnosebericht gebaut wird (F11).
///
/// Alles wird übergeben statt hier eingesammelt: so lässt sich der Bericht
/// gegen feste Werte prüfen — und gerade bei einer Datei, die das Gerät
/// verlässt, will man wissen, was drinsteht, ohne sie erst zu erzeugen.
pub struct DiagnosticInput<'a> {
    pub app_version: &'a str,
    pub android_release: &'a str,
    pub device_model: &'a str,
    pub config: &'a crate::model::AppConfig,
    pub stats: &'a PlaybackStats,
    pub storage: &'a StorageBreakdown,
    pub check: &'a DatabaseCheck,
    pub fetch_log: &'a [crate::mail::log::FetchLogEntry],
    pub cache_bytes: u64,
    pub cache_max_bytes: u64,
}

/// Wie viele Zeichen eines Serverfehlers in den Bericht dürfen (F11).
///
/// Der Text kommt wörtlich vom Server. GMX & Co. schreiben in eine Ablehnung
/// gelegentlich die Kontokennung hinein — das wäre der eine Weg, auf dem doch
/// etwas Persönliches in ein öffentliches Ticket rutschte. Die ersten 200
/// Zeichen tragen die Diagnose, der Rest ist Beiwerk.
pub const ERROR_SNIPPET: usize = 200;

/// Kürzt einen Serverfehler auf ein für die Weitergabe vertretbares Maß.
fn kurzfassung(text: &str) -> String {
    // Nur die erste Zeile: mehrzeilige Antworten enthalten oft eine
    // Sitzungskennung in der zweiten.
    let erste = text.lines().next().unwrap_or("").trim();
    if erste.chars().count() <= ERROR_SNIPPET {
        return erste.to_string();
    }
    let gekuerzt: String = erste.chars().take(ERROR_SNIPPET).collect();
    format!("{gekuerzt}…")
}

/// Ersetzt eine Mailadresse durch „Absender A", „Absender B", … (F11).
///
/// Die Zuordnung bleibt innerhalb eines Berichts stabil, damit man zwei
/// Zeilen desselben Absenders noch als zusammengehörig erkennt — ohne zu
/// erfahren, wer es ist.
fn pseudonym(sender: &str, map: &mut std::collections::HashMap<String, String>) -> String {
    let key = sender.trim().to_lowercase();
    let next = map.len();
    map.entry(key)
        .or_insert_with(|| {
            // A–Z, danach A1, B1, … — mehr als 26 Absender sind selten, und
            // eine durchlaufende Nummer wäre schwerer zu lesen.
            let letter = (b'A' + (next % 26) as u8) as char;
            let round = next / 26;
            if round == 0 {
                format!("Absender {letter}")
            } else {
                format!("Absender {letter}{round}")
            }
        })
        .clone()
}

/// Baut den Diagnosebericht (F11).
///
/// ## Was bewusst fehlt
///
/// Mailadressen, Zugangsdaten, Servernamen und Dateinamen. Der Bericht ist
/// für ein öffentliches Fehlerticket gedacht — er soll zeigen, *wie* die App
/// eingestellt ist und *was* schiefging, nicht *wer* dem Rahmen schreibt oder
/// wie die Urlaubsfotos heißen.
///
/// Absender erscheinen pseudonymisiert, damit die Aufschlüsselung „ein
/// Absender belegt 80 % des Speichers" noch lesbar bleibt.
pub fn diagnostic_report(input: &DiagnosticInput<'_>) -> String {
    use std::collections::HashMap;
    use std::fmt::Write;

    let mut map: HashMap<String, String> = HashMap::new();
    let mut out = String::new();

    let _ = writeln!(out, "Slowshow — Diagnosebericht");
    let _ = writeln!(out, "==========================");
    let _ = writeln!(out);
    let _ = writeln!(out, "App          {}", input.app_version);
    let _ = writeln!(out, "Android      {}", input.android_release);
    let _ = writeln!(out, "Geraet       {}", input.device_model);
    let _ = writeln!(out);

    let c = input.config;
    let _ = writeln!(out, "Einstellungen");
    let _ = writeln!(out, "-------------");
    let _ = writeln!(out, "Anzeigedauer      {} s", c.interval_seconds);
    let _ = writeln!(out, "Reihenfolge       {:?}", c.order);
    let _ = writeln!(out, "Anpassung         {:?}", c.fit_mode);
    let _ = writeln!(out, "Ausrichtung       {:?}", c.orientation);
    let _ = writeln!(out, "Paar-Modus        {}", c.pair_mode);
    let _ = writeln!(out, "Ueberblendung     {}", c.transition.enabled);
    let _ = writeln!(out, "Zeitplan          {}", c.schedule.enabled);
    let _ = writeln!(out, "Heimnetz          {}", c.remote.enabled);
    let _ = writeln!(out, "MQTT              {}", c.mqtt.enabled);
    let _ = writeln!(out);

    // Quellen ohne Adresse und ohne Benutzernamen: Art und Einstellungen
    // genuegen, um ein Verhalten nachzuvollziehen.
    let _ = writeln!(out, "Quellen ({})", c.sources.len());
    let _ = writeln!(out, "-------------");
    for (i, s) in c.sources.iter().enumerate() {
        let art = match &s.kind {
            crate::model::SourceKind::Local { .. } => "lokaler Ordner".to_string(),
            crate::model::SourceKind::WebDav { .. } => "WebDAV".to_string(),
            crate::model::SourceKind::Nextcloud { .. } => "Nextcloud".to_string(),
            crate::model::SourceKind::Mail {
                include_seen,
                quarantine_all,
                allowed_senders,
                max_mails_per_hour,
                ..
            } => format!(
                "Postfach (auch gelesene: {include_seen}, alles freigeben: {quarantine_all}, \
                 freigegebene Absender: {}, max {max_mails_per_hour}/h)",
                allowed_senders.len()
            ),
        };
        let _ = writeln!(
            out,
            "{}. {art} · aktiv: {} · alle {} min",
            i + 1,
            s.enabled,
            s.sync_interval_minutes
        );
    }
    let _ = writeln!(out);

    let st = input.stats;
    let _ = writeln!(out, "Bestand");
    let _ = writeln!(out, "-------");
    let _ = writeln!(out, "Fotos gesamt      {}", st.total);
    let _ = writeln!(out, "in der Diashow    {}", st.eligible);
    let _ = writeln!(out, "nie gezeigt       {}", st.never_shown);
    let _ = writeln!(
        out,
        "Durchlauf         noch {} von {}, {} abgeschlossen",
        st.bag_remaining, st.eligible, st.cycles
    );
    let _ = writeln!(
        out,
        "Cache             {} von {} Bytes",
        input.cache_bytes, input.cache_max_bytes
    );
    let _ = writeln!(out);

    let _ = writeln!(out, "Speicher nach Jahr");
    let _ = writeln!(out, "------------------");
    for g in &input.storage.by_year {
        let _ = writeln!(
            out,
            "{:>6}  {:>6} {}  {:>12} Bytes",
            g.label,
            g.count,
            // Der Bericht ist deutscher Fliesstext, auch wenn er technisch
            // ist — „1 Fotos" liest sich wie ein Fehler.
            if g.count == 1 { "Foto " } else { "Fotos" },
            g.bytes
        );
    }
    let _ = writeln!(out);

    if !input.storage.by_sender.is_empty() {
        let _ = writeln!(out, "Speicher nach Absender (pseudonymisiert)");
        let _ = writeln!(out, "---------------------------------------");
        for g in &input.storage.by_sender {
            let _ = writeln!(
                out,
                "{:<12}  {:>6} {}  {:>12} Bytes",
                pseudonym(&g.label, &mut map),
                g.count,
                if g.count == 1 { "Foto " } else { "Fotos" },
                g.bytes
            );
        }
        let _ = writeln!(out);
    }

    let ch = input.check;
    let _ = writeln!(out, "Datenbank");
    let _ = writeln!(out, "---------");
    let _ = writeln!(out, "Eintraege ohne Datei      {}", ch.missing_files.len());
    let _ = writeln!(out, "Dateien ohne Eintrag      {}", ch.orphan_files.len());
    let _ = writeln!(out, "verwaiste Vorschaubilder  {}", ch.orphan_thumbs.len());
    let _ = writeln!(out, "freigebbar                {} Bytes", ch.reclaimable_bytes);
    let _ = writeln!(out);

    let _ = writeln!(out, "Abruf-Protokoll (letzte {})", input.fetch_log.len());
    let _ = writeln!(out, "---------------------------");
    for e in input.fetch_log {
        // Der Zeitpunkt relativ zum jüngsten Lauf: ein Zeitstempel verriete
        // die Gewohnheiten des Haushalts.
        let ausloeser = match e.trigger {
            crate::mail::log::Trigger::Interval => "Zeitplan",
            crate::mail::log::Trigger::Manual => "von Hand",
            crate::mail::log::Trigger::Resync => "Neuabgleich",
        };
        match &e.error {
            Some(err) => {
                let _ = writeln!(out, "{ausloeser:<12} FEHLER: {}", kurzfassung(err));
            }
            None => {
                let _ = writeln!(
                    out,
                    "{ausloeser:<12} im Ordner {}, bekannt {}, geholt {}, neu {}, uebersprungen {}",
                    e.seen_in_folder, e.already_known, e.checked, e.added, e.skipped
                );
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::index::CacheIndex;

    fn bild(id: &str, name: &str, shows: u32, last: Option<i64>) -> CacheEntry {
        CacheEntry {
            id: id.into(),
            source_id: "s".into(),
            rel_path: name.into(),
            file_name: name.into(),
            etag: None,
            remote_size: None,
            remote_mtime: None,
            taken_at: None,
            width: 100,
            height: 100,
            bytes: 10,
            added_at: 0,
            last_shown: last,
            show_count: shows,
            excluded: false,
            mail: None,
            thumb_bytes: None,
        }
    }

    fn index_mit(entries: Vec<CacheEntry>) -> CacheIndex {
        let mut idx = CacheIndex::default();
        for e in entries {
            idx.insert(e);
        }
        idx
    }

    /// Alles zaehlt zum Bestand.
    fn alles(_: &CacheEntry) -> bool {
        true
    }

    // ── Diagnosebericht (F11) ────────────────────────────────────────────────

    fn beispielbericht() -> String {
        let mut cfg = crate::model::AppConfig {
            interval_seconds: 60,
            ..Default::default()
        };
        cfg.remote.enabled = true;
        cfg.sources = vec![
            crate::model::Source {
                id: "s1".into(),
                name: "2026-0607_Urlaub".into(),
                kind: crate::model::SourceKind::Local {
                    saf_uri: "content://geheim/pfad".into(),
                    display_path: "Urlaub".into(),
                },
                enabled: true,
                subfolders: vec![],
                min_width: 0,
                min_height: 0,
                sync_interval_minutes: 360,
                last_sync: Some(1),
            },
            crate::model::Source {
                id: "s2".into(),
                name: "Postfach".into(),
                kind: crate::model::SourceKind::Mail {
                    host: "imap.example.org".into(),
                    port: 993,
                    username: "rahmen@example.org".into(),
                    password_ref: "s2".into(),
                    folder: "INBOX".into(),
                    allowed_senders: vec!["tochter@example.org".into()],
                    quarantine_all: false,
                    max_attachment_bytes: 26_214_400,
                    max_mails_per_hour: 30,
                    quality: Default::default(),
                    include_seen: true,
                },
                enabled: true,
                subfolders: vec![],
                min_width: 0,
                min_height: 0,
                sync_interval_minutes: 15,
                last_sync: Some(2),
            },
        ];

        let stats = PlaybackStats {
            total: 597,
            eligible: 582,
            never_shown: 544,
            bag_remaining: 88,
            cycles: 3,
            most_shown: vec![],
            longest_unseen: vec![],
        };
        let storage = StorageBreakdown {
            by_year: vec![
                StorageGroup { label: "2026".into(), count: 106, bytes: 91_000_000 },
                StorageGroup { label: UNKNOWN_LABEL.into(), count: 2, bytes: 2_100_000 },
            ],
            by_sender: vec![
                StorageGroup { label: "tochter@example.org".into(), count: 2, bytes: 2_100_000 },
                StorageGroup { label: "oma@example.org".into(), count: 1, bytes: 900_000 },
            ],
        };
        let check = DatabaseCheck {
            missing_files: vec![],
            orphan_files: vec!["abc".into()],
            orphan_thumbs: vec![],
            reclaimable_bytes: 412_000,
        };
        let log = vec![
            crate::mail::log::FetchLogEntry {
                at: 100,
                source_id: "s2".into(),
                trigger: crate::mail::log::Trigger::Interval,
                seen_in_folder: 3,
                already_known: 3,
                checked: 0,
                added: 0,
                quarantined: 0,
                skipped: 0,
                failed: 0,
                error: None,
            },
            crate::mail::log::FetchLogEntry {
                at: 90,
                source_id: "s2".into(),
                trigger: crate::mail::log::Trigger::Manual,
                seen_in_folder: 3,
                already_known: 2,
                checked: 1,
                added: 1,
                quarantined: 1,
                skipped: 0,
                failed: 0,
                error: None,
            },
            crate::mail::log::FetchLogEntry {
                at: 80,
                source_id: "s2".into(),
                trigger: crate::mail::log::Trigger::Interval,
                seen_in_folder: 0,
                already_known: 0,
                checked: 0,
                added: 0,
                quarantined: 0,
                skipped: 0,
                failed: 0,
                error: Some("Anmeldung abgelehnt (Benutzername, Passwort oder IMAP-Zugriff im Konto pruefen)".into()),
            },
        ];

        diagnostic_report(&DiagnosticInput {
            app_version: "0.1.0",
            android_release: "15",
            device_model: "23043RP34G",
            config: &cfg,
            stats: &stats,
            storage: &storage,
            check: &check,
            fetch_log: &log,
            cache_bytes: 453_000_000,
            cache_max_bytes: 2_147_483_648,
        })
    }

    #[test]
    fn bericht_enthaelt_keine_personenbezogenen_daten() {
        // Der wichtigste Test von F11: die Datei ist fuer ein oeffentliches
        // Fehlerticket gedacht. Jeder Wert hier stand im Eingabematerial.
        let b = beispielbericht();
        for verboten in [
            "rahmen@example.org",  // Benutzername des Postfachs
            "tochter@example.org",     // Absender
            "oma@example.org",      // Absender
            "imap.example.org",         // Servername
            "content://geheim",     // Pfad der lokalen Quelle
            "2026-0607_Urlaub", // Name der Quelle
        ] {
            assert!(
                !b.contains(verboten),
                "„{verboten}\" darf nicht im Bericht stehen:\n{b}"
            );
        }
    }

    #[test]
    fn kuerzt_serverfehler_auf_die_erste_zeile() {
        // Mehrzeilige Antworten tragen die Diagnose in der ersten Zeile und
        // oft eine Sitzungskennung in der zweiten.
        assert_eq!(kurzfassung("Abgelehnt
Sitzung 4711-abc"), "Abgelehnt");
    }

    #[test]
    fn kuerzt_zu_lange_serverfehler() {
        let lang = "x".repeat(500);
        let k = kurzfassung(&lang);
        assert_eq!(k.chars().count(), ERROR_SNIPPET + 1, "200 Zeichen plus Auslassung");
        assert!(k.ends_with('…'));
    }

    #[test]
    fn laesst_kurze_fehler_unveraendert() {
        // Sonst stuende hinter jeder Meldung ein Auslassungszeichen, das
        // nahelegt, es fehle etwas.
        assert_eq!(kurzfassung("Anmeldung abgelehnt"), "Anmeldung abgelehnt");
    }

    #[test]
    fn bericht_pseudonymisiert_stabil() {
        // „Absender A belegt 80 %" bleibt lesbar, ohne zu verraten, wer das
        // ist. Zwei Zeilen desselben Absenders muessen denselben Namen tragen.
        let b = beispielbericht();
        assert!(b.contains("Absender A"), "{b}");
        assert!(b.contains("Absender B"), "{b}");
    }

    #[test]
    fn bericht_nennt_was_zur_fehlersuche_taugt() {
        let b = beispielbericht();
        for noetig in [
            "23043RP34G",            // Geraet
            "Android      15",       // Systemfassung
            "Postfach (auch gelesene: true", // Einstellung, die Verhalten erklaert
            "FEHLER: Anmeldung abgelehnt",   // der eigentliche Vorfall
            "nie gezeigt       544", // Bestand
        ] {
            assert!(b.contains(noetig), "„{noetig}\" fehlt im Bericht:\n{b}");
        }
    }

    /// Schreibt das Muster zum Ansehen heraus.
    ///
    /// Kein Teil der Pruefung — mit `--ignored` aufzurufen, wenn jemand den
    /// Bericht im Ganzen lesen will, ohne ihn am Geraet zu erzeugen.
    #[test]
    #[ignore = "gibt das Muster aus, statt etwas zu pruefen"]
    fn muster_ausgeben() {
        println!("{}", beispielbericht());
    }

    // ── Sicherung (F12) ──────────────────────────────────────────────────────

    /// Kleinste Sicherung, die sich lesen laesst.
    fn sicherung(version: &str) -> String {
        let cfg = serde_json::to_string(&crate::model::AppConfig::default()).unwrap();
        format!(r#"{{"schemaVersion":{version},"createdAt":0,"config":{cfg}}}"#)
    }

    #[test]
    fn liest_eine_sicherung_der_eigenen_fassung() {
        let b = parse_backup(&sicherung("1")).expect("muss lesbar sein");
        assert_eq!(b.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn lehnt_eine_neuere_fassung_verstaendlich_ab() {
        // Dort koennte eine Einstellung ihre Bedeutung geaendert haben. Ein
        // halb wiederhergestellter Rahmen waere schlimmer als eine Absage.
        let err = parse_backup(&sicherung("99")).expect_err("darf nicht durchgehen");
        assert_eq!(
            err,
            BackupError::TooNew {
                found: 99,
                supported: SCHEMA_VERSION
            }
        );
        let text = err.to_string();
        assert!(text.contains("99"), "die Zahl gehoert in die Meldung: {text}");
        assert!(
            text.contains("aktualisieren"),
            "und die naechste Handlung: {text}"
        );
    }

    #[test]
    fn lehnt_eine_datei_ohne_fassung_ab() {
        // Eine alte Konfigurationsdatei ohne Umschlag: sie sieht aehnlich aus,
        // ist aber keine Sicherung — und stillschweigend zu uebernehmen, was
        // man nicht einordnen kann, ist der falsche Weg.
        let err = parse_backup(r#"{"intervalSeconds":30}"#).expect_err("darf nicht durchgehen");
        assert!(matches!(err, BackupError::Malformed(_)));
    }

    #[test]
    fn lehnt_unsinn_mit_lesbarer_meldung_ab() {
        let err = parse_backup("kein json").expect_err("darf nicht durchgehen");
        assert!(err.to_string().contains("keine gültige Sicherung"));
    }

    #[test]
    fn uebergeht_unbekannte_felder() {
        // Eine Sicherung aus einer aelteren App bleibt lesbar, auch wenn
        // seither Einstellungen dazugekommen sind — und umgekehrt stoert ein
        // Feld nicht, das es hier nicht mehr gibt.
        let cfg = serde_json::to_string(&crate::model::AppConfig::default()).unwrap();
        let json = format!(
            r#"{{"schemaVersion":1,"createdAt":0,"nochWas":true,"config":{cfg}}}"#
        );
        assert!(parse_backup(&json).is_ok());
    }

    #[test]
    fn sicherung_traegt_keine_zugangsdaten() {
        // Die Passwoerter liegen im Schluesselspeicher, nicht in der
        // Konfiguration. Diese Probe haelt fest, dass das so bleibt — eine
        // Sicherung wandert per Definition aus dem Geraet heraus.
        let json = serde_json::to_string(&Backup {
            schema_version: SCHEMA_VERSION,
            created_at: 0,
            config: crate::model::AppConfig::default(),
        })
        .unwrap();
        let klein = json.to_lowercase();
        assert!(!klein.contains("\"password\""), "kein Passwortfeld: {json}");
    }

    /// Bild mit Jahr, Groesse und optionalem Absender — fuer F9.
    fn mit(id: &str, taken: Option<i64>, bytes: u64, sender: Option<&str>) -> CacheEntry {
        CacheEntry {
            taken_at: taken,
            bytes,
            thumb_bytes: Some(1),
            mail: sender.map(|s| crate::cache::index::MailMeta {
                sender: s.into(),
                subject: "x".into(),
                message_id: id.into(),
                quarantined: false,
            }),
            ..bild(id, &format!("{id}.jpg"), 0, None)
        }
    }

    /// Jahr aus Unix-Sekunden, ohne Zeitzonenkram: 0 = 1970, 1 = 1971, …
    fn jahr(t: i64) -> i32 {
        1970 + t as i32
    }

    // ── Speicher-Aufschluesselung (F9) ───────────────────────────────────────

    #[test]
    fn schluesselt_nach_jahr_auf_neueste_zuerst() {
        let idx = index_mit(vec![
            mit("a", Some(50), 100, None),
            mit("b", Some(55), 200, None),
            mit("c", Some(50), 300, None),
        ]);
        let b = storage_breakdown(&idx, &jahr);
        let jahre: Vec<&str> = b.by_year.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(jahre, vec!["2025", "2020"]);
        assert_eq!(b.by_year[1].count, 2, "2020 hat zwei Bilder");
        // Vorschaubilder zaehlen mit: 100 + 300 + zweimal 1 Byte Vorschau.
        assert_eq!(b.by_year[1].bytes, 402);
    }

    #[test]
    fn stellt_bilder_ohne_jahr_ans_ende() {
        // „—" ist kein Jahr. Im reinen Zeichenvergleich landete es je nach
        // Kodierung mitten in der Liste.
        let idx = index_mit(vec![
            mit("a", None, 10, None),
            mit("b", Some(50), 10, None),
            mit("c", Some(30), 10, None),
        ]);
        let b = storage_breakdown(&idx, &jahr);
        let jahre: Vec<&str> = b.by_year.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(jahre, vec!["2020", "2000", UNKNOWN_LABEL]);
    }

    #[test]
    fn schluesselt_nach_absender_auf_groesste_zuerst() {
        let idx = index_mit(vec![
            mit("a", Some(50), 100, Some("Oma@Example.ORG")),
            mit("b", Some(50), 900, Some("opa@example.org")),
            mit("c", Some(50), 100, Some("oma@example.org")),
            mit("d", Some(50), 500, None),
        ]);
        let b = storage_breakdown(&idx, &jahr);

        assert_eq!(b.by_sender.len(), 2, "Bilder ohne Absender zaehlen nicht mit");
        assert_eq!(b.by_sender[0].label, "opa@example.org");
        // Gross- und Kleinschreibung darf nicht trennen, sonst erschiene die
        // Oma zweimal mit je der Haelfte.
        assert_eq!(b.by_sender[1].label, "oma@example.org");
        assert_eq!(b.by_sender[1].count, 2);
    }

    #[test]
    fn aufschluesselung_bleibt_zwischen_aufrufen_stabil() {
        let idx = index_mit(vec![
            mit("a", Some(50), 100, Some("b@x")),
            mit("b", Some(50), 100, Some("a@x")),
        ]);
        let erst = storage_breakdown(&idx, &jahr);
        for _ in 0..5 {
            assert_eq!(storage_breakdown(&idx, &jahr), erst);
        }
        assert_eq!(b_labels(&erst), vec!["a@x", "b@x"], "bei Gleichstand alphabetisch");
    }

    fn b_labels(b: &StorageBreakdown) -> Vec<&str> {
        b.by_sender.iter().map(|g| g.label.as_str()).collect()
    }

    // ── Datenbank-Pruefung (F10) ─────────────────────────────────────────────

    fn eins() -> u64 {
        1
    }

    #[test]
    fn findet_eintraege_ohne_datei() {
        // Entsteht, wenn eine Datei von aussen verschwindet. Die Diashow zoege
        // den Eintrag und zeigte nichts.
        let idx = index_mit(vec![mit("a", None, 1, None), mit("b", None, 1, None)]);
        let c = check_database(&idx, &["a".into()], &[], &|_| eins());

        assert_eq!(c.missing_files, vec!["b"]);
        assert!(c.orphan_files.is_empty());
        assert!(!c.is_clean());
    }

    #[test]
    fn findet_dateien_ohne_eintrag() {
        // Sie belegen Platz, den niemand mehr zaehlt.
        let idx = index_mit(vec![mit("a", None, 1, None)]);
        let c = check_database(&idx, &["a".into(), "verwaist".into()], &[], &|_| 4096);

        assert_eq!(c.orphan_files, vec!["verwaist"]);
        assert_eq!(c.reclaimable_bytes, 4096);
    }

    #[test]
    fn zaehlt_verwaiste_vorschaubilder_mit() {
        let idx = index_mit(vec![mit("a", None, 1, None)]);
        let c = check_database(&idx, &["a".into()], &["a".into(), "alt".into()], &|_| 100);

        assert_eq!(c.orphan_thumbs, vec!["alt"]);
        assert_eq!(c.reclaimable_bytes, 100, "nur das verwaiste, nicht das gute");
    }

    #[test]
    fn meldet_einen_sauberen_bestand_als_sauber() {
        let idx = index_mit(vec![mit("a", None, 1, None)]);
        let c = check_database(&idx, &["a".into()], &["a".into()], &|_| eins());

        assert!(c.is_clean());
        assert_eq!(c.reclaimable_bytes, 0);
    }

    #[test]
    fn listen_sind_sortiert() {
        // Zwei Laeufe muessen dasselbe melden -- sonst sieht der Bericht nach
        // Veraenderung aus, wo keine ist.
        let idx = index_mit(vec![]);
        let c = check_database(&idx, &["z".into(), "a".into(), "m".into()], &[], &|_| eins());
        assert_eq!(c.orphan_files, vec!["a", "m", "z"]);
    }

    #[test]
    fn zaehlt_bestand_und_nie_gezeigte() {
        let idx = index_mit(vec![
            bild("a", "a.jpg", 3, Some(100)),
            bild("b", "b.jpg", 0, None),
            bild("c", "c.jpg", 0, None),
        ]);
        let s = playback_stats(&idx, &alles, 2, 7);

        assert_eq!(s.total, 3);
        assert_eq!(s.eligible, 3);
        assert_eq!(s.never_shown, 2);
        assert_eq!(s.bag_remaining, 2);
        assert_eq!(s.cycles, 7);
    }

    #[test]
    fn trennt_bestand_vom_gesamtbestand() {
        // „342 von 1.850" darf nur zaehlen, was die Diashow ueberhaupt ziehen
        // kann. Sonst erreichte der Fortschritt nie null, weil ausgeblendete
        // und wartende Bilder im Nenner blieben.
        let mut versteckt = bild("x", "x.jpg", 0, None);
        versteckt.excluded = true;
        let idx = index_mit(vec![bild("a", "a.jpg", 1, Some(10)), versteckt]);

        let s = playback_stats(&idx, &|e| !e.excluded, 1, 0);
        assert_eq!(s.total, 2, "gesamt zaehlt alles");
        assert_eq!(s.eligible, 1, "Bestand nur das Ziehbare");
        assert_eq!(s.never_shown, 0, "das ausgeblendete zaehlt nicht mit");
    }

    #[test]
    fn sortiert_die_meistgezeigten_absteigend() {
        let idx = index_mit(vec![
            bild("a", "a.jpg", 2, Some(1)),
            bild("b", "b.jpg", 9, Some(1)),
            bild("c", "c.jpg", 5, Some(1)),
        ]);
        let s = playback_stats(&idx, &alles, 0, 0);
        let namen: Vec<&str> = s.most_shown.iter().map(|e| e.file_name.as_str()).collect();
        assert_eq!(namen, vec!["b.jpg", "c.jpg", "a.jpg"]);
    }

    #[test]
    fn laesst_nie_gezeigte_aus_der_wartelisten_bestenliste() {
        // Ohne Zeitpunkt fuehrten sie die Liste an und sagten nichts aus --
        // dafuer gibt es die eigene Ansicht „Nie gezeigt" (F4).
        let idx = index_mit(vec![
            bild("alt", "alt.jpg", 1, Some(100)),
            bild("neu", "neu.jpg", 1, Some(900)),
            bild("nie", "nie.jpg", 0, None),
        ]);
        let s = playback_stats(&idx, &alles, 0, 0);
        let namen: Vec<&str> = s
            .longest_unseen
            .iter()
            .map(|e| e.file_name.as_str())
            .collect();
        assert_eq!(namen, vec!["alt.jpg", "neu.jpg"]);
    }

    #[test]
    fn bestenlisten_bleiben_zwischen_aufrufen_stabil() {
        // Der Index ist eine HashMap; ohne zweites Sortierkriterium sprangen
        // gleichrangige Eintraege bei jedem Aufruf, und die Liste flackerte.
        let idx = index_mit(vec![
            bild("a", "gleich-b.jpg", 4, Some(50)),
            bild("b", "gleich-a.jpg", 4, Some(50)),
            bild("c", "gleich-c.jpg", 4, Some(50)),
        ]);
        let erst = playback_stats(&idx, &alles, 0, 0);
        for _ in 0..5 {
            assert_eq!(playback_stats(&idx, &alles, 0, 0), erst);
        }
        let namen: Vec<&str> = erst.most_shown.iter().map(|e| e.file_name.as_str()).collect();
        assert_eq!(namen, vec!["gleich-a.jpg", "gleich-b.jpg", "gleich-c.jpg"]);
    }

    #[test]
    fn bestenlisten_enden_bei_zehn() {
        let entries: Vec<CacheEntry> = (0..25)
            .map(|i| bild(&format!("i{i}"), &format!("{i:02}.jpg"), i + 1, Some(i as i64)))
            .collect();
        let s = playback_stats(&index_mit(entries), &alles, 0, 0);
        assert_eq!(s.most_shown.len(), TOP_LIMIT);
        assert_eq!(s.longest_unseen.len(), TOP_LIMIT);
    }

    #[test]
    fn leerer_cache_liefert_nullen_statt_panik() {
        let s = playback_stats(&CacheIndex::default(), &alles, 0, 0);
        assert_eq!(s.total, 0);
        assert_eq!(s.never_shown, 0);
        assert!(s.most_shown.is_empty());
        assert!(s.longest_unseen.is_empty());
    }
}

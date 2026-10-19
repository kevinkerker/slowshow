//! Auswahl der Unterordner einer Quelle (FA-29, E-70).
//!
//! ## Die Regel
//!
//! Jede Entscheidung des Nutzers ist ein Pfad relativ zur Quellenwurzel,
//! entweder an (`subfolders`) oder aus (`excluded_subfolders`). Fuer einen
//! Ordner gilt die **naechstgelegene** Entscheidung auf seinem Weg nach oben:
//! `Familie` aus und `Familie/2025` an heisst „aus der Familie nur 2025".
//! Ohne Entscheidung gilt `include_new_subfolders` — so tauchen neue Ordner
//! bei „automatisch aufnehmen" von selbst in der Diashow auf.
//!
//! Die Bilder direkt im gewaehlten Ordner schaltet `include_root`. Die Wurzel
//! ist dabei kein Ahne im Sinne der Regel: sie abzuschalten nimmt den
//! Unterordnern nichts. Beim WhatsApp-Ordner liegen die Bilder genau dort,
//! und `Sent` soll trotzdem getrennt davon abschaltbar sein.
//!
//! ## Warum nicht mehr „nur diese"
//!
//! Bis E-70 gab es nur eine Positivliste, als freier Text. „Alles ausser
//! `Sent`" war damit unmoeglich, und ein Tippfehler ergab still 0 Fotos.
//! Alte Konfigurationen behalten ihre Bedeutung ueber die Rueckfaelle in
//! [`Source::include_root`] und [`Source::include_new_subfolders`].
//!
//! Dieselbe Regel steht im Frontend (`src/lib/subfolders.ts`), das danach die
//! Schalter setzt. Die Tests beider Seiten pruefen dieselben Faelle.

use crate::model::Source;
use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct FolderFilter {
    include: Vec<String>,
    exclude: Vec<String>,
    include_root: bool,
    include_new: bool,
}

/// Pfad ohne Leerraum und Schraegstriche am Rand — so stand es schon im
/// alten Textfeld, und so kommen die Eintraege alter Konfigurationen an.
fn clean(path: &str) -> String {
    path.trim().trim_matches('/').to_string()
}

/// Liegt `path` in `dir` oder ist es `dir` selbst?
///
/// Mit Trennzeichen verglichen: `Urlaubsfotos` liegt nicht in `Urlaub`. Der
/// alte Filter verglich den Abstieg ohne Trennzeichen und lief deshalb in
/// fremde Ordner hinein, auch wenn er deren Bilder dann verwarf.
fn within(path: &str, dir: &str) -> bool {
    path == dir
        || (path.len() > dir.len()
            && path.starts_with(dir)
            && path.as_bytes()[dir.len()] == b'/')
}

impl FolderFilter {
    /// Alles zaehlt — fuer Quellen ohne Ordner, etwa Nextcloud-Alben.
    pub fn all() -> Self {
        Self {
            include_root: true,
            include_new: true,
            ..Default::default()
        }
    }

    pub fn of(source: &Source) -> Self {
        let cleaned = |list: &[String]| {
            list.iter()
                .map(|p| clean(p))
                .filter(|p| !p.is_empty())
                .collect()
        };
        Self {
            include: cleaned(&source.subfolders),
            exclude: cleaned(&source.excluded_subfolders),
            include_root: source.include_root(),
            include_new: source.include_new_subfolders(),
        }
    }

    /// Die naechstgelegene Entscheidung fuer diesen Ordner, falls es eine gibt.
    fn decision(&self, dir: &str) -> Option<bool> {
        let nearest = |list: &[String]| {
            list.iter()
                .filter(|d| within(dir, d))
                .map(|d| d.len())
                .max()
        };
        match (nearest(&self.include), nearest(&self.exclude)) {
            (None, None) => None,
            (Some(_), None) => Some(true),
            (None, Some(_)) => Some(false),
            // Derselbe Pfad in beiden Listen entsteht nur in einer von Hand
            // bearbeiteten Datei; dann gewinnt „an", damit nichts still fehlt.
            (Some(i), Some(e)) => Some(i >= e),
        }
    }

    /// Zaehlen die Bilder direkt in diesem Ordner? `""` ist die Quellenwurzel.
    pub fn dir_on(&self, dir: &str) -> bool {
        if dir.is_empty() {
            return self.include_root;
        }
        self.decision(dir).unwrap_or(self.include_new)
    }

    /// Lohnt sich der Abstieg in diesen Ordner?
    ///
    /// Ja, wenn er selbst zaehlt oder darunter noch etwas angeschaltet ist —
    /// sonst waere `Familie/2025` unerreichbar, sobald `Familie` aus ist.
    pub fn descend(&self, dir: &str) -> bool {
        self.dir_on(dir) || self.include.iter().any(|i| i != dir && within(i, dir))
    }

    /// Zaehlt diese Datei? `rel_file` relativ zur Quellenwurzel.
    pub fn file_allowed(&self, rel_file: &str) -> bool {
        let dir = rel_file.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        self.dir_on(dir)
    }

    /// Angeschaltete Unterordner, die der Lauf nicht betreten hat (E-70).
    ///
    /// `seen_dirs` sind die tatsaechlich betretenen Ordner. Ein
    /// angeschalteter Ordner wird immer betreten, wenn es ihn gibt (siehe
    /// [`Self::descend`]); fehlt er in der Menge, fehlt er an der Quelle.
    pub fn missing(&self, seen_dirs: &HashSet<String>) -> Vec<String> {
        self.include
            .iter()
            .filter(|i| !seen_dirs.contains(*i))
            .cloned()
            .collect()
    }
}

/// Ein Unterordner fuer die Auswahl im Quellendialog (E-70).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubfolderEntry {
    pub name: String,
    /// Relativ zur Quellenwurzel, mit `/` getrennt.
    pub path: String,
    /// Liegt eine `.nomedia` darin? Android versteckt solche Ordner vor der
    /// Galerie — bei WhatsApp `Sent` und `Private`.
    pub nomedia: bool,
    /// Hat er selbst Unterordner? Nur dann bietet die Liste das Aufklappen an.
    pub has_children: bool,
}

impl SubfolderEntry {
    /// Baut den Eintrag aus dem Inhalt des Ordners: `(Name, ist Ordner)`.
    pub fn from_children<'a>(
        name: &str,
        path: &str,
        children: impl IntoIterator<Item = (&'a str, bool)>,
    ) -> Self {
        let mut nomedia = false;
        let mut has_children = false;
        for (child, is_dir) in children {
            if is_dir {
                has_children = true;
            } else if child == ".nomedia" {
                nomedia = true;
            }
        }
        Self {
            name: name.to_string(),
            path: path.to_string(),
            nomedia,
            has_children,
        }
    }
}

/// Pfad eines Unterordners aus dem Pfad seines Elternordners.
pub fn child_path(parent: &str, name: &str) -> String {
    let parent = clean(parent);
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

/// Reihenfolge der Liste: nach Namen, ohne Ruecksicht auf Gross- und
/// Kleinschreibung — „Urlaub" steht nicht hinter „Zelt", nur weil es gross
/// geschrieben ist.
pub fn sort_entries(entries: &mut [SubfolderEntry]) {
    entries.sort_by_key(|e| e.name.to_lowercase());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SourceKind;

    fn source(include: &[&str], exclude: &[&str], root: Option<bool>, new: Option<bool>) -> Source {
        Source {
            id: "s".into(),
            name: "S".into(),
            kind: SourceKind::Local {
                saf_uri: "{}".into(),
                display_path: "WhatsApp Images".into(),
            },
            enabled: true,
            subfolders: include.iter().map(|s| s.to_string()).collect(),
            excluded_subfolders: exclude.iter().map(|s| s.to_string()).collect(),
            include_root: root,
            include_new_subfolders: new,
            min_width: 0,
            min_height: 0,
            sync_interval_minutes: 60,
            last_sync: None,
        }
    }

    fn filter(include: &[&str], exclude: &[&str], root: bool, new: bool) -> FolderFilter {
        FolderFilter::of(&source(include, exclude, Some(root), Some(new)))
    }

    #[test]
    fn ohne_entscheidung_zaehlt_alles() {
        let f = filter(&[], &[], true, true);
        assert!(f.file_allowed("1.jpg"));
        assert!(f.file_allowed("Urlaub/2025/1.jpg"));
        assert!(f.descend("Urlaub"));
    }

    #[test]
    fn whatsapp_ohne_sent_und_private_e_70() {
        // Der Fall, der mit der alten Positivliste nicht ging: die Bilder
        // liegen in der Wurzel, und die sollte mit jedem Filter wegfallen.
        let f = filter(&[], &["Sent", "Private"], true, true);
        assert!(f.file_allowed("IMG-20241227-WA0001.jpg"));
        assert!(!f.file_allowed("Sent/IMG-20241227-WA0100.jpg"));
        assert!(!f.file_allowed("Private/x.jpg"));
        assert!(!f.descend("Sent"));
        assert!(f.file_allowed("Status/x.jpg"), "neue Ordner kommen automatisch dazu");
    }

    #[test]
    fn naechstgelegene_entscheidung_gewinnt() {
        let f = filter(&["Familie/2025"], &["Familie"], true, true);
        assert!(!f.file_allowed("Familie/1.jpg"));
        assert!(!f.file_allowed("Familie/2024/1.jpg"));
        assert!(f.file_allowed("Familie/2025/1.jpg"));
        assert!(f.file_allowed("Familie/2025/Sommer/1.jpg"), "vererbt nach unten");
        assert!(f.descend("Familie"), "sonst waere 2025 unerreichbar");
    }

    #[test]
    fn ohne_automatik_zaehlt_nur_das_angeschaltete() {
        let f = filter(&["Urlaub"], &[], false, false);
        assert!(f.file_allowed("Urlaub/1.jpg"));
        assert!(!f.file_allowed("Neu/1.jpg"), "neue Ordner bleiben draussen");
        assert!(!f.descend("Neu"));
        assert!(!f.file_allowed("1.jpg"));
    }

    #[test]
    fn wurzel_ist_kein_ahne() {
        // Die Wurzel abzuschalten nimmt den Unterordnern nichts.
        let f = filter(&[], &[], false, true);
        assert!(!f.file_allowed("1.jpg"));
        assert!(f.file_allowed("Urlaub/1.jpg"));
    }

    #[test]
    fn vergleicht_mit_trennzeichen() {
        let f = filter(&["Urlaub"], &[], false, false);
        assert!(!f.descend("Urlaubsfotos"));
        assert!(!f.file_allowed("Urlaubsfotos/1.jpg"));
    }

    #[test]
    fn alte_positivliste_behaelt_ihre_bedeutung_fa_29() {
        // Konfiguration von vor E-70: nur `subfolders`, keine neuen Felder.
        let alt = FolderFilter::of(&source(&["Urlaub", "/Familie/2025/"], &[], None, None));
        assert!(alt.file_allowed("Urlaub/1.jpg"));
        assert!(alt.file_allowed("Urlaub/2025/1.jpg"));
        assert!(alt.file_allowed("Familie/2025/1.jpg"), "Schraegstriche am Rand wie frueher");
        assert!(!alt.file_allowed("Familie/1.jpg"));
        assert!(!alt.file_allowed("Dokumente/1.jpg"));
        assert!(!alt.file_allowed("1.jpg"), "Wurzeldateien fielen mit Filter weg");

        let ohne = FolderFilter::of(&source(&[], &[], None, None));
        assert!(ohne.file_allowed("1.jpg"), "ohne Filter zaehlte alles");
        assert!(ohne.file_allowed("Neu/1.jpg"));
    }

    #[test]
    fn meldet_fehlende_angeschaltete_ordner_e_70() {
        let f = filter(&["Urlaub", "Familie/2025"], &["Sent"], true, true);
        let gesehen: HashSet<String> = ["Urlaub".to_string(), "Familie".to_string()].into();
        assert_eq!(f.missing(&gesehen), vec!["Familie/2025".to_string()]);
    }

    #[test]
    fn eintrag_erkennt_nomedia_und_unterordner() {
        let e = SubfolderEntry::from_children("Sent", "Sent", [(".nomedia", false), ("a.jpg", false)]);
        assert!(e.nomedia);
        assert!(!e.has_children);

        let e = SubfolderEntry::from_children("Urlaub", "Urlaub", [("2025", true), ("a.jpg", false)]);
        assert!(!e.nomedia);
        assert!(e.has_children);
    }

    #[test]
    fn pfade_und_reihenfolge() {
        assert_eq!(child_path("", "Urlaub"), "Urlaub");
        assert_eq!(child_path("Familie/", "2025"), "Familie/2025");

        let mut v = vec![
            SubfolderEntry::from_children("Zelt", "Zelt", []),
            SubfolderEntry::from_children("urlaub", "urlaub", []),
            SubfolderEntry::from_children("Auto", "Auto", []),
        ];
        sort_entries(&mut v);
        let names: Vec<_> = v.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Auto", "urlaub", "Zelt"]);
    }
}

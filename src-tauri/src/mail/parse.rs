//! Auswertung einer empfangenen Mail (E-30, Erweiterungspapier Teil 1).
//!
//! Bewusst frei von Netzzugriff: Was aus einer Mail wird — welche Anhänge
//! zählen, von wem sie kommt, welches Datum gilt — lässt sich so gegen ein
//! Literal im Test prüfen, ohne ein Postfach.
//!
//! Der Netzteil liegt in [`super::imap`].

use crate::decode::{classify, FileClass};
use mail_parser::{MessageParser, MimeHeaders};

/// Ein Bildanhang, so wie er in den Cache soll.
#[derive(Debug, Clone, PartialEq)]
pub struct MailPhoto {
    /// Dateiname des Anhangs, für die Bildunterschrift (FA-07).
    pub file_name: String,
    pub bytes: Vec<u8>,
}

/// Was eine Mail beigetragen hat.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedMail {
    /// Absenderadresse in Kleinschreibung — Grundlage der Freigabeliste (F4).
    pub sender: String,
    pub subject: String,
    /// Empfangszeitpunkt aus dem `Date`-Kopf, Unix-Sekunden.
    pub received_at: Option<i64>,
    /// Stabile Kennung gegen Doppelimport (F2).
    pub message_id: String,
    pub photos: Vec<MailPhoto>,
    /// Anhänge, die bewusst übersprungen wurden — für das Protokoll (FA-09).
    pub skipped: Vec<String>,
}

/// Zerlegt eine rohe RFC5322-Nachricht.
///
/// `max_attachment_bytes` greift **vor** dem Dekodieren: ein 200-MB-Anhang
/// soll gar nicht erst in den Speicher.
pub fn parse_mail(raw: &[u8], max_attachment_bytes: u64) -> Option<ParsedMail> {
    let msg = MessageParser::default().parse(raw)?;

    let sender = msg
        .from()
        .and_then(|a| a.first())
        .and_then(|a| a.address.as_ref())
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_default();

    let subject = msg.subject().unwrap_or_default().to_string();
    let received_at = msg.date().map(|d| d.to_timestamp());

    // Ohne Message-ID hilft nur der Inhalt. Ein Hash über Absender, Betreff
    // und Datum ist schwächer, aber besser als jede Mail doppelt zu holen.
    let message_id = match msg.message_id() {
        Some(id) if !id.trim().is_empty() => id.trim().to_string(),
        _ => format!("{sender}\u{1}{subject}\u{1}{}", received_at.unwrap_or(0)),
    };

    let mut photos = Vec::new();
    let mut skipped = Vec::new();

    // Eingebettete Bilder zählen mit: `attachments()` liefert jeden Binärteil,
    // ob `inline` oder als Anhang deklariert.
    for part in msg.attachments() {
        let raw_name = part
            .attachment_name()
            .map(|n| n.to_string())
            .unwrap_or_else(|| format!("anhang-{}", photos.len() + skipped.len() + 1));

        let data = part.contents();
        if data.len() as u64 > max_attachment_bytes {
            skipped.push(format!("{raw_name} (zu groß: {} Bytes)", data.len()));
            continue;
        }

        let ct = part.content_type();
        let (class, name) = classify_attachment(
            ct.map(|c| c.ctype()),
            ct.and_then(|c| c.subtype()),
            &raw_name,
        );

        match class {
            FileClass::Image => photos.push(MailPhoto {
                file_name: name,
                bytes: data.to_vec(),
            }),
            FileClass::Skipped => skipped.push(name),
            FileClass::Irrelevant => {}
        }
    }

    Some(ParsedMail {
        sender,
        subject,
        received_at,
        message_id,
        photos,
        skipped,
    })
}

/// MIME-Untertypen, die als Bild durchgehen — passend zu `IMAGE_EXTENSIONS`.
///
/// Die zweite Spalte ist die Endung, die einem Anhang **ohne Dateinamen**
/// gegeben wird — ein ins Textfeld eingefuegtes Bild traegt oft keinen.
/// `tiff` und `x-icon` stehen doppelt, weil beide Schreibweisen in freier
/// Wildbahn vorkommen (E-37).
const MIME_IMAGE: &[(&str, &str)] = &[
    ("jpeg", "jpg"),
    ("png", "png"),
    ("webp", "webp"),
    ("bmp", "bmp"),
    ("tiff", "tif"),
    ("x-icon", "ico"),
    ("vnd.microsoft.icon", "ico"),
    ("gif", "gif"),
];

/// Bild-Untertypen, die bewusst übersprungen werden (E-04).
///
/// `gif` stand hier bis E-37: es liefert jetzt sein erstes Einzelbild.
const MIME_SKIP: &[&str] = &["heic", "heif", "avif"];

/// Beurteilt einen Anhang nach seinem MIME-Typ, mit dem Dateinamen als Rückfall.
///
/// **Warum nicht nur der Dateiname:** Ein ins Textfeld eingefügtes Foto — der
/// Normalfall, wenn jemand am Telefon „Bild einfügen" tippt — trägt oft
/// keinen Dateinamen, sondern nur eine Content-ID. Die Erkennung allein über
/// die Endung hätte genau diese Bilder stillschweigend verworfen: kein
/// Fehler, kein Protokolleintrag, das Foto kommt einfach nie an.
///
/// Gibt neben der Einstufung den zu verwendenden Dateinamen zurück — bei
/// einem namenlosen Teil mit passender Endung, damit die Bildunterschrift
/// (FA-07) nicht „anhang-1" heißt.
///
/// Bei Widerspruch zwischen Endung und Inhaltstyp gewinnt die Endung, sofern
/// sie ausdrücklich ausgeschlossen ist — siehe unten.
fn classify_attachment(ctype: Option<&str>, subtype: Option<&str>, name: &str) -> (FileClass, String) {
    // Ausdrücklich ausgeschlossene Endungen gewinnen gegen den Inhaltstyp.
    //
    // Ein Absender, dessen Programm HEIC als `image/jpeg` deklariert, ist
    // häufiger als einer, der eine `.heic`-Datei mit JPEG-Inhalt schickt.
    // Andersherum entschieden, unterliefe eine falsch beschriftete Datei die
    // Ausschlüsse aus E-04 und E-07.
    if classify(name) == FileClass::Skipped {
        return (FileClass::Skipped, name.to_string());
    }

    let sub_lower = subtype.map(|s| s.to_ascii_lowercase());
    let sub_ref = sub_lower.as_deref();

    match ctype.map(|c| c.to_ascii_lowercase()).as_deref() {
        Some("image") => {
            if let Some(ext) = sub_ref.and_then(|st| {
                MIME_IMAGE
                    .iter()
                    .find(|(mime, _)| *mime == st)
                    .map(|(_, ext)| *ext)
            }) {
                let name = if classify(name) == FileClass::Image {
                    name.to_string()
                } else {
                    format!("{}.{ext}", name.trim_end_matches('.'))
                };
                return (FileClass::Image, name);
            }
            if sub_ref.is_some_and(|st| MIME_SKIP.contains(&st)) {
                return (FileClass::Skipped, name.to_string());
            }
            // Unbekannter Bild-Untertyp: der Dateiname darf noch entscheiden.
            (classify(name), name.to_string())
        }
        // Videos sind per E-07 kein Projektbestandteil.
        Some("video") => (FileClass::Skipped, name.to_string()),
        _ => (classify(name), name.to_string()),
    }
}

/// Stabile Kennung einer Mail für den Doppelimport-Schutz (F2).
///
/// FNV-1a wie im Cache-Index: dieselbe Begründung, dieselbe Kollisionslage.
/// Der Hash statt der rohen Message-ID, weil Letztere beliebige Zeichen
/// enthalten darf und als Teil eines Dateinamens landen soll.
pub fn message_id_hash(message_id: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in message_id.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Jahreszahl aus dem Betreff, für eingescannte Altfotos (F2).
///
/// „Urlaub 1987" liefert 1987. Gesucht wird eine vierstellige Zahl zwischen
/// 1826 — dem Jahr der ältesten erhaltenen Fotografie — und 2100. Ohne
/// Untergrenze würde „Bilder 0815" zu einem Aufnahmedatum.
pub fn year_from_subject(subject: &str) -> Option<i32> {
    let bytes = subject.as_bytes();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        let window = &bytes[i..i + 4];
        if window.iter().all(|b| b.is_ascii_digit()) {
            // Keine längere Zahl anschneiden: „12345" ist kein Jahr.
            let vor_ok = i == 0 || !bytes[i - 1].is_ascii_digit();
            let nach_ok = i + 4 == bytes.len() || !bytes[i + 4].is_ascii_digit();
            if vor_ok && nach_ok {
                let year: i32 = std::str::from_utf8(window).ok()?.parse().ok()?;
                if (1826..=2100).contains(&year) {
                    return Some(year);
                }
            }
        }
        i += 1;
    }
    None
}

/// Unix-Zeit für den 1. Januar des Jahres, 12 Uhr UTC.
///
/// Mittags und nicht um Mitternacht: ein gescanntes Foto aus „1987" soll bei
/// jeder Zeitzone im Jahr 1987 landen, nicht am 31. Dezember 1986.
pub fn year_to_timestamp(year: i32) -> i64 {
    use chrono::{TimeZone, Utc};
    Utc.with_ymd_and_hms(year, 1, 1, 12, 0, 0)
        .single()
        .map(|d| d.timestamp())
        .unwrap_or(0)
}

/// Aufnahmedatum nach der Rangfolge aus 1.3.
///
/// EXIF schlägt Mail-Empfang, Mail-Empfang schlägt Jahreszahl aus dem Betreff.
/// Die Jahreszahl steht bewusst zuletzt: sie ist die gröbste Angabe, aber bei
/// gescannten Altfotos die einzige — dort fehlt EXIF, und der Empfang wäre
/// heute statt 1987.
pub fn resolve_taken_at(exif: Option<i64>, received_at: Option<i64>, subject: &str) -> Option<i64> {
    if let Some(t) = exif {
        return Some(t);
    }
    if let Some(year) = year_from_subject(subject) {
        return Some(year_to_timestamp(year));
    }
    received_at
}

/// Darf dieser Absender ohne Quarantäne einliefern? (F4)
///
/// Vergleich in Kleinschreibung, weil Mailadressen im lokalen Teil zwar
/// theoretisch unterscheidend sind, in der Praxis aber niemand das erwartet.
pub fn is_allowed(sender: &str, allowed: &[String]) -> bool {
    let sender = sender.trim().to_lowercase();
    allowed.iter().any(|a| a.trim().to_lowercase() == sender)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kleinste vollstaendige Mail mit einem JPEG-Anhang.
    fn mail_with_attachment(name: &str, payload_b64: &str) -> Vec<u8> {
        format!(
            "From: Oma <oma@example.org>\r\n\
             Subject: Urlaub 1987\r\n\
             Date: Tue, 1 Jul 2025 10:00:00 +0000\r\n\
             Message-ID: <abc123@example.org>\r\n\
             MIME-Version: 1.0\r\n\
             Content-Type: multipart/mixed; boundary=\"XX\"\r\n\
             \r\n\
             --XX\r\n\
             Content-Type: text/plain\r\n\
             \r\n\
             Hallo!\r\n\
             --XX\r\n\
             Content-Type: image/jpeg\r\n\
             Content-Disposition: attachment; filename=\"{name}\"\r\n\
             Content-Transfer-Encoding: base64\r\n\
             \r\n\
             {payload_b64}\r\n\
             --XX--\r\n"
        )
        .into_bytes()
    }

    #[test]
    fn liest_absender_betreff_und_datum() {
        let raw = mail_with_attachment("foto.jpg", "AAAA");
        let m = parse_mail(&raw, 25_000_000).expect("Mail muss lesbar sein");
        assert_eq!(m.sender, "oma@example.org");
        assert_eq!(m.subject, "Urlaub 1987");
        assert!(m.received_at.is_some());
        assert_eq!(m.message_id, "abc123@example.org");
    }

    #[test]
    fn holt_den_bildanhang_und_ignoriert_den_text() {
        let raw = mail_with_attachment("foto.jpg", "AAAA");
        let m = parse_mail(&raw, 25_000_000).unwrap();
        assert_eq!(m.photos.len(), 1, "nur der Anhang, nicht der Fliesstext");
        assert_eq!(m.photos[0].file_name, "foto.jpg");
        assert!(!m.photos[0].bytes.is_empty(), "base64 wurde dekodiert");
    }

    #[test]
    fn ueberspringt_heic_wie_jede_andere_quelle() {
        // E-04 gilt unveraendert, auch fuer Mail (E-28). iPhone-Anhaenge
        // kommen deshalb teilweise nicht durch -- das ist bekannt und gewollt.
        let raw = mail_with_attachment("IMG_0001.heic", "AAAA");
        let m = parse_mail(&raw, 25_000_000).unwrap();
        assert!(m.photos.is_empty());
        assert_eq!(m.skipped, vec!["IMG_0001.heic"]);
    }

    #[test]
    fn ueberspringt_zu_grosse_anhaenge_vor_dem_dekodieren() {
        let raw = mail_with_attachment("gross.jpg", "AAAAAAAAAAAAAAAAAAAA");
        let m = parse_mail(&raw, 4).unwrap();
        assert!(m.photos.is_empty());
        assert!(m.skipped[0].contains("zu groß"), "{:?}", m.skipped);
    }

    #[test]
    fn faellt_ohne_message_id_auf_einen_inhaltsschluessel_zurueck() {
        let raw = b"From: a@b.de\r\nSubject: Test\r\n\r\nkein Anhang\r\n";
        let m = parse_mail(raw, 25_000_000).unwrap();
        assert!(
            m.message_id.contains("a@b.de"),
            "ohne Message-ID hilft nur der Inhalt: {}",
            m.message_id
        );
    }

    #[test]
    fn message_id_hash_ist_stabil_und_dateinamenstauglich() {
        let a = message_id_hash("<abc@example.org>");
        assert_eq!(a, message_id_hash("<abc@example.org>"));
        assert_ne!(a, message_id_hash("<xyz@example.org>"));
        assert_eq!(a.len(), 16);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn jahreszahl_aus_dem_betreff() {
        assert_eq!(year_from_subject("Urlaub 1987"), Some(1987));
        assert_eq!(year_from_subject("1999 Silvester"), Some(1999));
        assert_eq!(year_from_subject("Fotos von 2024!"), Some(2024));
    }

    #[test]
    fn jahreszahl_lehnt_unsinn_ab() {
        assert_eq!(year_from_subject("Bilder 0815"), None, "zu frueh fuer Fotos");
        assert_eq!(year_from_subject("Nummer 12345"), None, "keine laengere Zahl anschneiden");
        assert_eq!(year_from_subject("Ohne Zahl"), None);
        assert_eq!(year_from_subject("Jahr 3000"), None, "ueber der Obergrenze");
    }

    #[test]
    fn jahr_landet_mittags_im_richtigen_jahr() {
        // Um Mitternacht waere das Foto je nach Zeitzone im Vorjahr gelandet.
        use chrono::{Datelike, TimeZone, Utc};
        let ts = year_to_timestamp(1987);
        assert_eq!(Utc.timestamp_opt(ts, 0).unwrap().year(), 1987);
    }

    #[test]
    fn aufnahmedatum_folgt_der_rangfolge() {
        // EXIF gewinnt immer.
        assert_eq!(resolve_taken_at(Some(111), Some(222), "Urlaub 1987"), Some(111));
        // Ohne EXIF schlaegt die Jahreszahl den Empfang -- sonst waere ein
        // gescanntes Foto von 1987 auf heute datiert.
        let aus_betreff = resolve_taken_at(None, Some(222), "Urlaub 1987").unwrap();
        assert_eq!(aus_betreff, year_to_timestamp(1987));
        assert_ne!(aus_betreff, 222, "der Empfang darf die Jahreszahl nicht ueberstimmen");
        // Ohne beides bleibt der Empfang.
        assert_eq!(resolve_taken_at(None, Some(222), "ohne Jahr"), Some(222));
        assert_eq!(resolve_taken_at(None, None, "ohne Jahr"), None);
    }

    #[test]
    fn freigabeliste_vergleicht_ohne_gross_und_kleinschreibung() {
        let liste = vec!["Oma@Example.ORG".to_string()];
        assert!(is_allowed("oma@example.org", &liste));
        assert!(is_allowed("  OMA@EXAMPLE.ORG  ", &liste));
        assert!(!is_allowed("fremd@example.org", &liste));
        assert!(!is_allowed("oma@example.org", &[]), "leere Liste laesst niemanden durch");
    }

    /// Mail mit einem eingebetteten Bild ohne Dateinamen — so verschickt es
    /// ein Telefon, wenn jemand „Bild einfuegen" tippt statt anzuhaengen.
    fn mail_with_inline(content_type: &str) -> Vec<u8> {
        format!(
            "From: Oma <oma@example.org>\r\n\
             Subject: Schau mal\r\n\
             Message-ID: <inline@example.org>\r\n\
             MIME-Version: 1.0\r\n\
             Content-Type: multipart/related; boundary=\"YY\"\r\n\
             \r\n\
             --YY\r\n\
             Content-Type: text/html\r\n\
             \r\n\
             <p>Schau mal <img src=\"cid:bild1\"></p>\r\n\
             --YY\r\n\
             Content-Type: {content_type}\r\n\
             Content-ID: <bild1>\r\n\
             Content-Disposition: inline\r\n\
             Content-Transfer-Encoding: base64\r\n\
             \r\n\
             AAAA\r\n\
             --YY--\r\n"
        )
        .into_bytes()
    }

    #[test]
    fn erkennt_eingebettete_bilder_ohne_dateinamen() {
        // Der eigentliche "Oma-Modus": kein Anhang, sondern ein ins Textfeld
        // eingefuegtes Foto. Ueber die Dateiendung waere es unerkannt
        // verschwunden -- ohne Fehler, ohne Protokolleintrag.
        let m = parse_mail(&mail_with_inline("image/jpeg"), 25_000_000).unwrap();
        assert_eq!(m.photos.len(), 1, "eingebettetes Bild muss ankommen");
        assert!(
            m.photos[0].file_name.ends_with(".jpg"),
            "der Name braucht eine Endung fuer die Bildunterschrift: {}",
            m.photos[0].file_name
        );
    }

    #[test]
    fn eingebettetes_heic_wird_uebersprungen_nicht_verschluckt() {
        let m = parse_mail(&mail_with_inline("image/heic"), 25_000_000).unwrap();
        assert!(m.photos.is_empty());
        assert_eq!(m.skipped.len(), 1, "uebersprungenes gehoert ins Protokoll");
    }

    #[test]
    fn eingebettetes_png_und_webp_kommen_durch() {
        for (ct, ext) in [("image/png", ".png"), ("image/webp", ".webp")] {
            let m = parse_mail(&mail_with_inline(ct), 25_000_000).unwrap();
            assert_eq!(m.photos.len(), 1, "{ct}");
            assert!(m.photos[0].file_name.ends_with(ext), "{ct}");
        }
    }

    #[test]
    fn ausgeschlossene_endung_schlaegt_den_inhaltstyp() {
        // Ein Programm, das HEIC als image/jpeg deklariert, darf E-04 nicht
        // unterlaufen. Der umgekehrte Fall -- .heic mit echtem JPEG-Inhalt --
        // ist deutlich seltener.
        let (class, _) = classify_attachment(Some("image"), Some("jpeg"), "IMG_0001.heic");
        assert_eq!(class, FileClass::Skipped);
    }

    #[test]
    fn nimmt_die_neuen_bildtypen_an() {
        // E-37. Wichtig fuer eingebettete Anhaenge ohne Dateinamen: dort
        // entscheidet allein der Inhaltstyp, und die Endung wird ergaenzt.
        for (sub, erwartet) in [
            ("bmp", ".bmp"),
            ("tiff", ".tif"),
            ("x-icon", ".ico"),
            ("vnd.microsoft.icon", ".ico"),
            ("gif", ".gif"),
        ] {
            let (klasse, name) = classify_attachment(Some("image"), Some(sub), "anhang");
            assert_eq!(klasse, FileClass::Image, "{sub}");
            assert!(name.ends_with(erwartet), "{sub} -> {name}");
        }
    }

    #[test]
    fn haelt_heic_und_avif_weiter_draussen() {
        // Gegenprobe zu E-37: nur GIF ist herausgenommen worden.
        for sub in ["heic", "heif", "avif"] {
            let (klasse, _) = classify_attachment(Some("image"), Some(sub), "anhang.jpg");
            assert_eq!(klasse, FileClass::Skipped, "{sub}");
        }
    }

    #[test]
    fn mime_typ_schlaegt_eine_nichtssagende_endung() {
        // Manche Programme haengen Fotos als "bild.dat" an. Der Inhaltstyp
        // ist die verlaesslichere Angabe.
        assert_eq!(
            classify_attachment(Some("image"), Some("jpeg"), "bild.dat"),
            (FileClass::Image, "bild.dat.jpg".to_string())
        );
    }

    #[test]
    fn stimmige_endung_bleibt_unveraendert() {
        assert_eq!(
            classify_attachment(Some("image"), Some("jpeg"), "urlaub.jpg"),
            (FileClass::Image, "urlaub.jpg".to_string())
        );
    }

    #[test]
    fn videos_werden_am_inhaltstyp_erkannt() {
        let (class, _) = classify_attachment(Some("video"), Some("mp4"), "clip");
        assert_eq!(class, FileClass::Skipped, "E-07: keine Videos");
    }

    #[test]
    fn ohne_inhaltstyp_entscheidet_der_dateiname() {
        assert_eq!(
            classify_attachment(None, None, "foto.png"),
            (FileClass::Image, "foto.png".to_string())
        );
        let (class, _) = classify_attachment(None, None, "vertrag.pdf");
        assert_eq!(class, FileClass::Irrelevant, "PDF ist kein Bild und kein Video");
    }
}

//! Bilddekodierung, Ausrichtung und Skalierung — vollständig im Rust-Prozess.
//!
//! Umsetzung von NF-13 (Dekodierung im Backend statt in der WebView), NF-12
//! (skalierte Cache-Ablage) und FA-04 (EXIF-Orientierung). Das Frontend bekommt
//! nie eine Originaldatei zu sehen; das begrenzt den WebView-Speicher (R-03).
//!
//! ## Warum JPEG statt WebP in der Cache-Ablage
//!
//! NF-12 nennt WebP als Beispiel („z. B. WebP"). Ein verlustbehafteter
//! WebP-Encoder existiert in Rust nur als Bindung an libwebp (C) und müsste für
//! Android cross-kompiliert werden — genau die Aufwandsklasse, die das
//! Lastenheft bei HEIC (E-04) und SMB (E-02) bewusst vermeidet. Der
//! JPEG-Encoder des `image`-Crates ist reines Rust und liefert bei Qualität 85
//! auf Displayauflösung vergleichbare Dateigrößen. Dekodiert wird WebP
//! weiterhin, es ist also als Quellformat uneingeschränkt nutzbar (FA-04).

use image::{DynamicImage, ImageEncoder};
use std::io::Cursor;

/// Fehler beim Aufbereiten eines Bildes.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// Format wird bewusst nicht unterstützt (FA-09 HEIC, E-07 Video).
    /// Der Aufrufer überspringt die Datei und schreibt einen Log-Eintrag.
    #[error("nicht unterstütztes Format: {0}")]
    Unsupported(String),
    #[error("Bild konnte nicht dekodiert werden: {0}")]
    Decode(#[from] image::ImageError),
    #[error("Bild unterschreitet die Mindestauflösung ({width}x{height})")]
    TooSmall { width: u32, height: u32 },
}

/// Ein für den Cache aufbereitetes Bild.
#[derive(Debug)]
pub struct Prepared {
    /// Fertig kodierte JPEG-Bytes in Displayauflösung.
    pub bytes: Vec<u8>,
    /// Breite nach Orientierung und Skalierung.
    pub width: u32,
    /// Höhe nach Orientierung und Skalierung.
    pub height: u32,
    /// EXIF-Aufnahmedatum als Unix-Zeitstempel, falls vorhanden (FA-03, FA-07).
    pub taken_at: Option<i64>,
}

// ── Formaterkennung ──────────────────────────────────────────────────────────

/// Dateiendungen, die als Bildquelle in Frage kommen (FA-04).
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];

/// Endungen, die stillschweigend übersprungen werden.
/// Videos sind per E-07 kein Projektbestandteil, HEIC per E-04.
const SKIP_EXTENSIONS: &[&str] = &[
    "heic", "heif", "avif", "mp4", "mov", "avi", "mkv", "webm", "m4v", "3gp", "gif",
];

/// Klassifikation einer Datei allein anhand ihres Namens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileClass {
    /// Unterstütztes Bildformat (FA-04).
    Image,
    /// Bewusst ausgeschlossen — wird übersprungen und geloggt (FA-09, E-07).
    Skipped,
    /// Keine Mediendatei, ohne Log ignorieren.
    Irrelevant,
}

/// Ordnet eine Datei anhand ihrer Endung ein.
///
/// Läuft vor jedem Download, damit für ausgeschlossene Formate erst gar kein
/// Netzverkehr entsteht (NF-14).
pub fn classify(file_name: &str) -> FileClass {
    let ext = file_name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some(e) if IMAGE_EXTENSIONS.contains(&e) => FileClass::Image,
        Some(e) if SKIP_EXTENSIONS.contains(&e) => FileClass::Skipped,
        _ => FileClass::Irrelevant,
    }
}

/// Erkennt an den Magic Bytes ein Format, das wir nicht dekodieren können.
///
/// Nötig, weil Nextcloud-Previews und WebDAV-Server Dateien mit irreführender
/// Endung ausliefern können. Gibt den Markennamen für den Log-Eintrag zurück.
pub fn detect_unsupported(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() < 12 {
        return None;
    }
    // ISO-BMFF-Container: [size:4]["ftyp"][brand:4]
    if &bytes[4..8] == b"ftyp" {
        return match &bytes[8..12] {
            b"heic" | b"heix" | b"heim" | b"heis" | b"hevc" | b"hevm" | b"hevs" | b"mif1"
            | b"msf1" => Some("HEIC/HEIF"),
            b"avif" | b"avis" => Some("AVIF"),
            _ => Some("ISO-BMFF (Video)"),
        };
    }
    None
}

// ── Geometrie ────────────────────────────────────────────────────────────────

/// Zielgröße für das Einpassen in `max_w` x `max_h` unter Beibehaltung des
/// Seitenverhältnisses. Vergrößert nie — kleine Originale bleiben unangetastet,
/// sonst wüchse die Cache-Datei über das Original hinaus (NF-12).
pub fn fit_within(w: u32, h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if w == 0 || h == 0 {
        return (w, h);
    }
    if w <= max_w && h <= max_h {
        return (w, h);
    }
    let scale = f64::min(max_w as f64 / w as f64, max_h as f64 / h as f64);
    let nw = ((w as f64 * scale).round() as u32).max(1);
    let nh = ((h as f64 * scale).round() as u32).max(1);
    (nw, nh)
}

/// Wendet den EXIF-Orientierungswert (1..=8) auf das Bild an (FA-04).
///
/// Als eigene Funktion gehalten, weil sich die acht Fälle so ohne echte
/// Bilddateien testen lassen.
pub fn apply_orientation(img: DynamicImage, orientation: u32) -> DynamicImage {
    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        // 1 und alles Unbekannte: unverändert lassen.
        _ => img,
    }
}

// ── EXIF ─────────────────────────────────────────────────────────────────────

/// Aus EXIF gelesene Metadaten.
#[derive(Debug, Default, Clone, Copy)]
pub struct ExifInfo {
    /// Orientierungswert 1..=8, Standard 1.
    pub orientation: u32,
    /// DateTimeOriginal als Unix-Zeitstempel.
    pub taken_at: Option<i64>,
}

/// Liest Orientierung und Aufnahmedatum. Fehlendes oder defektes EXIF ist kein
/// Fehler — dann gelten die Standardwerte.
pub fn read_exif(bytes: &[u8]) -> ExifInfo {
    let mut cursor = Cursor::new(bytes);
    let Ok(exif) = exif::Reader::new().read_from_container(&mut cursor) else {
        return ExifInfo {
            orientation: 1,
            taken_at: None,
        };
    };

    let orientation = exif
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .filter(|v| (1..=8).contains(v))
        .unwrap_or(1);

    let taken_at = exif
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .or_else(|| exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY))
        .and_then(|f| match &f.value {
            exif::Value::Ascii(v) => v.first().cloned(),
            _ => None,
        })
        .and_then(|raw| parse_exif_datetime(&String::from_utf8_lossy(&raw)));

    ExifInfo {
        orientation,
        taken_at,
    }
}

/// Wandelt einen EXIF-Zeitstempel ("YYYY:MM:DD HH:MM:SS") in einen
/// Unix-Zeitstempel. EXIF kennt keine Zeitzone; wir interpretieren lokal, weil
/// Fotos üblicherweise in Ortszeit der Aufnahme gestempelt sind.
pub fn parse_exif_datetime(s: &str) -> Option<i64> {
    use chrono::{NaiveDateTime, TimeZone};
    let trimmed = s.trim().trim_end_matches('\0');
    let naive = NaiveDateTime::parse_from_str(trimmed, "%Y:%m:%d %H:%M:%S").ok()?;
    chrono::Local
        .from_local_datetime(&naive)
        .single()
        .map(|dt| dt.timestamp())
}

/// Lange Kante der Vorschaubilder in Pixeln (E-25).
///
/// 320 statt 192: das Referenzgerät hat 2,5-fache Pixeldichte, eine Zelle von
/// 128 CSS-Pixeln braucht also 320 echte. Kleiner wäre auf dem Pad sichtbar
/// weich.
pub const THUMB_EDGE: u32 = 320;

/// Qualität der Vorschaubilder.
///
/// Niedriger als bei der Cache-Ablage: bei 320 px fällt der Unterschied nicht
/// auf, und die Vorschaubilder gehen als Gesamtheit in die Cachegröße ein.
const THUMB_QUALITY: u8 = 78;

/// Erzeugt ein Vorschaubild aus einer bereits aufbereiteten Cache-Datei.
///
/// Bewusst aus dem Cache-Bild und nicht aus dem Original: das Original liegt
/// nach dem Sync nicht mehr vor (NF-12), und für 320 px ist der Unterschied
/// ohnehin nicht sichtbar. Dadurch lassen sich Vorschaubilder auch für Einträge
/// nachziehen, die vor E-25 in den Cache kamen.
///
/// `Triangle` statt `Lanczos3`: beim Verkleinern auf 320 px ist der Unterschied
/// nicht erkennbar, die Rechenzeit aber um ein Vielfaches kürzer — und diese
/// Funktion läuft im Zweifel für tausende Bilder hintereinander.
pub fn thumbnail(cached_bytes: &[u8], edge: u32) -> Result<Vec<u8>, DecodeError> {
    let img = image::load_from_memory(cached_bytes)?;
    let (tw, th) = fit_within(img.width(), img.height(), edge, edge);
    let img = img.resize_exact(tw, th, image::imageops::FilterType::Triangle);

    let rgb = img.into_rgb8();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, THUMB_QUALITY).write_image(
        rgb.as_raw(),
        tw,
        th,
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(out)
}

// ── Hauptpfad ────────────────────────────────────────────────────────────────

/// Dekodiert, richtet aus, skaliert und re-kodiert ein Bild für den Cache.
///
/// Das ist der einzige Weg, auf dem Bilddaten in den Cache gelangen — damit ist
/// garantiert, dass jede Cache-Datei bereits displaygerecht ist (NF-12) und die
/// Anzeige nie skalieren muss (NF-03).
pub fn prepare(
    bytes: &[u8],
    max_w: u32,
    max_h: u32,
    quality: u8,
    min_w: u32,
    min_h: u32,
) -> Result<Prepared, DecodeError> {
    if let Some(brand) = detect_unsupported(bytes) {
        return Err(DecodeError::Unsupported(brand.into()));
    }

    let exif = read_exif(bytes);
    let img = image::load_from_memory(bytes)?;
    let img = apply_orientation(img, exif.orientation);

    // Mindestauflösung erst nach der Orientierung prüfen: ein hochkant
    // aufgenommenes Bild hat vorher vertauschte Kantenlängen (FA-29).
    let (ow, oh) = (img.width(), img.height());
    if ow < min_w || oh < min_h {
        return Err(DecodeError::TooSmall {
            width: ow,
            height: oh,
        });
    }

    let (tw, th) = fit_within(ow, oh, max_w, max_h);
    let img = if (tw, th) == (ow, oh) {
        img
    } else {
        img.resize_exact(tw, th, image::imageops::FilterType::Lanczos3)
    };

    let rgb = img.into_rgb8();
    let mut out = Vec::with_capacity((tw as usize * th as usize) / 4);
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality).write_image(
        rgb.as_raw(),
        tw,
        th,
        image::ExtendedColorType::Rgb8,
    )?;

    Ok(Prepared {
        bytes: out,
        width: tw,
        height: th,
        taken_at: exif.taken_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    #[test]
    fn thumbnail_passt_in_die_vorgegebene_kante_e_25() {
        let src = test_jpeg(1920, 1080);
        let thumb = thumbnail(&src, THUMB_EDGE).expect("Vorschaubild");
        let img = image::load_from_memory(&thumb).unwrap();
        assert_eq!(img.width(), THUMB_EDGE, "lange Kante liegt auf dem Zielmass");
        assert_eq!(img.height(), 180, "Seitenverhaeltnis bleibt erhalten");
    }

    #[test]
    fn thumbnail_haelt_auch_hochformat_ein_e_25() {
        let thumb = thumbnail(&test_jpeg(1080, 1920), THUMB_EDGE).expect("Vorschaubild");
        let img = image::load_from_memory(&thumb).unwrap();
        assert_eq!(img.height(), THUMB_EDGE);
        assert_eq!(img.width(), 180);
    }

    #[test]
    fn thumbnail_vergroessert_kleine_bilder_nicht() {
        // `fit_within` skaliert nie hoch. Ein 200-px-Bild bliebe sonst als
        // aufgeblasenes 320-px-JPEG im Cache liegen, ohne besser auszusehen.
        let thumb = thumbnail(&test_jpeg(200, 150), THUMB_EDGE).expect("Vorschaubild");
        let img = image::load_from_memory(&thumb).unwrap();
        assert_eq!((img.width(), img.height()), (200, 150));
    }

    #[test]
    fn thumbnail_ist_deutlich_kleiner_als_die_cache_datei() {
        // Der Sinn der Sache: bei 5 000 Bildern zaehlt jedes Kilobyte.
        let src = test_jpeg(1920, 1080);
        let thumb = thumbnail(&src, THUMB_EDGE).expect("Vorschaubild");
        assert!(
            thumb.len() * 4 < src.len(),
            "Vorschau {} B gegen Original {} B",
            thumb.len(),
            src.len()
        );
    }

    fn test_jpeg(w: u32, h: u32) -> Vec<u8> {
        let mut img = RgbImage::new(w, h);
        // Farbverlauf statt Einfarbig, damit die Skalierung etwas zu tun hat.
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = Rgb([(x % 256) as u8, (y % 256) as u8, 128]);
        }
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90)
            .write_image(img.as_raw(), w, h, image::ExtendedColorType::Rgb8)
            .unwrap();
        out
    }

    #[test]
    fn classify_erkennt_bildformate_aus_fa_04() {
        assert_eq!(classify("urlaub.JPG"), FileClass::Image);
        assert_eq!(classify("a.jpeg"), FileClass::Image);
        assert_eq!(classify("a.png"), FileClass::Image);
        assert_eq!(classify("a.webp"), FileClass::Image);
    }

    #[test]
    fn classify_ueberspringt_heic_und_video() {
        // FA-09 / E-04: HEIC wird nicht dekodiert, sondern geloggt.
        assert_eq!(classify("IMG_0042.HEIC"), FileClass::Skipped);
        assert_eq!(classify("a.heif"), FileClass::Skipped);
        // E-07: Videodateien in Quellen werden ignoriert.
        assert_eq!(classify("clip.mp4"), FileClass::Skipped);
        assert_eq!(classify("clip.MOV"), FileClass::Skipped);
    }

    #[test]
    fn classify_ignoriert_fremddateien_ohne_log() {
        assert_eq!(classify("Thumbs.db"), FileClass::Irrelevant);
        assert_eq!(classify("notizen.txt"), FileClass::Irrelevant);
        assert_eq!(classify("ordner_ohne_endung"), FileClass::Irrelevant);
    }

    #[test]
    fn detect_unsupported_erkennt_heic_an_magic_bytes() {
        let mut heic = vec![0, 0, 0, 0x18];
        heic.extend_from_slice(b"ftypheic");
        heic.extend_from_slice(b"0000");
        assert_eq!(detect_unsupported(&heic), Some("HEIC/HEIF"));

        let mut avif = vec![0, 0, 0, 0x18];
        avif.extend_from_slice(b"ftypavif");
        avif.extend_from_slice(b"0000");
        assert_eq!(detect_unsupported(&avif), Some("AVIF"));
    }

    #[test]
    fn detect_unsupported_laesst_jpeg_durch() {
        assert_eq!(detect_unsupported(&test_jpeg(8, 8)), None);
        // Zu kurze Puffer dürfen nicht paniken.
        assert_eq!(detect_unsupported(&[0xFF, 0xD8]), None);
    }

    #[test]
    fn fit_within_verkleinert_seitenverhaeltnistreu() {
        assert_eq!(fit_within(4000, 3000, 2560, 1600), (2133, 1600));
        assert_eq!(fit_within(3000, 4000, 2560, 1600), (1200, 1600));
    }

    #[test]
    fn fit_within_vergroessert_nie() {
        // NF-12: kleine Originale bleiben unverändert, sonst wächst der Cache.
        assert_eq!(fit_within(800, 600, 2560, 1600), (800, 600));
        assert_eq!(fit_within(2560, 1600, 2560, 1600), (2560, 1600));
    }

    #[test]
    fn fit_within_ist_robust_bei_nullgroesse() {
        assert_eq!(fit_within(0, 0, 2560, 1600), (0, 0));
    }

    #[test]
    fn apply_orientation_dreht_kantenlaengen_bei_90_grad() {
        let img = DynamicImage::ImageRgb8(RgbImage::new(4, 2));
        for o in [6u32, 8, 5, 7] {
            let r = apply_orientation(img.clone(), o);
            assert_eq!(
                (r.width(), r.height()),
                (2, 4),
                "Orientierung {o} muss drehen"
            );
        }
        for o in [1u32, 2, 3, 4, 99] {
            let r = apply_orientation(img.clone(), o);
            assert_eq!(
                (r.width(), r.height()),
                (4, 2),
                "Orientierung {o} darf nicht drehen"
            );
        }
    }

    #[test]
    fn parse_exif_datetime_liest_standardformat() {
        let ts = parse_exif_datetime("2025:06:15 18:22:31").expect("gültiger Zeitstempel");
        assert!(ts > 1_700_000_000, "plausibler Unix-Zeitstempel, war {ts}");
        // Nullterminierung wie sie in EXIF-ASCII-Feldern vorkommt.
        assert!(parse_exif_datetime("2025:06:15 18:22:31\0").is_some());
        assert!(parse_exif_datetime("kein Datum").is_none());
        assert!(parse_exif_datetime("").is_none());
    }

    #[test]
    fn read_exif_ohne_exif_liefert_standardwerte() {
        let info = read_exif(&test_jpeg(16, 16));
        assert_eq!(info.orientation, 1);
        assert_eq!(info.taken_at, None);
    }

    #[test]
    fn prepare_skaliert_auf_zielgroesse_herunter() {
        let src = test_jpeg(1200, 900);
        let p = prepare(&src, 600, 600, 85, 0, 0).expect("muss aufbereiten");
        assert_eq!((p.width, p.height), (600, 450));
        // Die Ausgabe muss ein dekodierbares JPEG sein.
        let back = image::load_from_memory(&p.bytes).expect("Ausgabe ist JPEG");
        assert_eq!((back.width(), back.height()), (600, 450));
    }

    #[test]
    fn prepare_verkleinert_die_datei_nf_12() {
        let src = test_jpeg(2000, 1500);
        let p = prepare(&src, 800, 600, 80, 0, 0).unwrap();
        assert!(
            p.bytes.len() < src.len(),
            "Cache-Ablage muss kleiner sein als das Original: {} vs {}",
            p.bytes.len(),
            src.len()
        );
    }

    #[test]
    fn prepare_lehnt_heic_ab_statt_zu_paniken() {
        let mut heic = vec![0, 0, 0, 0x18];
        heic.extend_from_slice(b"ftypheic");
        heic.extend_from_slice(b"0000");
        let err = prepare(&heic, 800, 600, 85, 0, 0).unwrap_err();
        assert!(matches!(err, DecodeError::Unsupported(_)));
    }

    #[test]
    fn prepare_haelt_mindestaufloesung_ein_fa_29() {
        let src = test_jpeg(400, 300);
        let err = prepare(&src, 2560, 1600, 85, 1024, 768).unwrap_err();
        assert!(matches!(
            err,
            DecodeError::TooSmall {
                width: 400,
                height: 300
            }
        ));
        // Ohne Filter geht dasselbe Bild durch.
        assert!(prepare(&src, 2560, 1600, 85, 0, 0).is_ok());
    }

    #[test]
    fn prepare_meldet_fehler_statt_zu_paniken_bei_muell() {
        let err = prepare(b"das ist kein bild", 800, 600, 85, 0, 0).unwrap_err();
        assert!(matches!(err, DecodeError::Decode(_)));
    }
}

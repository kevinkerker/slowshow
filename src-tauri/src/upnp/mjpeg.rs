//! Teilbilder aus einem MJPEG-Strom (E-55, E-67).
//!
//! Home Assistant liefert Kameras als `multipart/x-mixed-replace`: ein Teil
//! nach dem anderen, jeder mit eigenen Kopfzeilen und einem JPEG als Inhalt,
//! ohne Ende. Das **erste vollstaendige Bild** haengt der Rahmen wie jedes
//! andere Fremdbild an die Wand (E-52, E-55); danach liest [`FrameReader`]
//! weiter und reicht jeweils das neueste Bild nach — das Livebild (E-67).
//! Genau das versucht Home Assistant, wenn jemand im Kamera-Dialog „Auf
//! Media-Player abspielen" waehlt.
//!
//! Reine Rechenlogik ueber einem Pufferstand: der Aufrufer liest Stueck fuer
//! Stueck vom Netz und fragt nach jedem Stueck, ob ein Bild fertig ist. So
//! laesst sich jeder Zwischenstand ohne Netz pruefen.
//!
//! Tolerant, weil Home Assistant den Trenner nicht nach RFC 2046 schreibt: im
//! Kopf steht `boundary=--frameboundary`, im Rumpf dann `--frameboundary` —
//! ein regelkonformer Server schriebe `----frameboundary`. Der Parser sucht
//! deshalb `--` plus den Trenner ohne fuehrende Striche und ueberliest danach
//! den Rest der Zeile; beide Schreibweisen treffen so.

/// Ist die Antwort ein Strom aus Teilbildern statt eines Bildes?
pub fn is_multipart_stream(content_type: &str) -> bool {
    content_type
        .trim()
        .to_ascii_lowercase()
        .starts_with("multipart/x-mixed-replace")
}

/// Der `boundary`-Parameter des Content-Type, ohne Anfuehrungszeichen.
pub fn boundary_of(content_type: &str) -> Option<String> {
    content_type.split(';').skip(1).find_map(|param| {
        let (name, value) = param.split_once('=')?;
        if !name.trim().eq_ignore_ascii_case("boundary") {
            return None;
        }
        let value = value.trim().trim_matches('"').trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

/// Ergebnis eines Blicks auf den bisher gelesenen Puffer.
#[derive(Debug, PartialEq, Eq)]
pub enum Part {
    /// Das erste Teilbild liegt vollstaendig vor.
    Complete {
        content_type: Option<String>,
        body: Vec<u8>,
    },
    /// Noch nicht genug gelesen — weiter vom Netz holen.
    Incomplete,
    /// Der Strom taugt nicht: kein Trenner, kein Bild, kaputte Kopfzeilen.
    Invalid(String),
}

/// Sucht `needle` in `haystack` ab `from`.
fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < from + needle.len() {
        return None;
    }
    haystack[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// Ende der Zeile ab `from`: Index hinter dem Zeilenumbruch.
fn after_line_end(buf: &[u8], from: usize) -> Option<usize> {
    let nl = find(buf, b"\n", from)?;
    Some(nl + 1)
}

/// Der Trenner, wie er im Rumpf gesucht wird: `--` plus Name ohne fuehrende
/// Striche. Trifft die regelkonforme und die Home-Assistant-Schreibweise.
fn needle_for(boundary: &str) -> Vec<u8> {
    let name = boundary.trim().trim_start_matches('-');
    let mut needle = b"--".to_vec();
    needle.extend_from_slice(name.as_bytes());
    needle
}

/// Erster Trenner im Puffer, wenn der Kopf keinen nennt: die erste Zeile, die
/// mit `--` beginnt.
fn guess_needle(buf: &[u8]) -> Option<(usize, Vec<u8>)> {
    let mut at = 0;
    while at < buf.len() {
        let end = find(buf, b"\n", at).map(|p| p + 1).unwrap_or(buf.len());
        let line = &buf[at..end];
        let trimmed: &[u8] = {
            let mut l = line;
            while let Some((last, rest)) = l.split_last() {
                if *last == b'\r' || *last == b'\n' {
                    l = rest;
                } else {
                    break;
                }
            }
            l
        };
        if trimmed.len() > 2 && trimmed.starts_with(b"--") {
            return Some((at, trimmed.to_vec()));
        }
        if end == buf.len() {
            break;
        }
        at = end;
    }
    None
}

/// Wert einer Kopfzeile (Name ohne Beachtung der Schreibung), getrimmt.
fn header_value<'a>(headers: &'a str, name: &str) -> Option<&'a str> {
    headers.lines().find_map(|line| {
        let (k, v) = line.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then(|| v.trim())
    })
}

/// Sieht nach, ob im Puffer das erste Teilbild vollstaendig ist.
///
/// `boundary` ist der Trenner aus dem Content-Type; fehlt er, wird die erste
/// `--`-Zeile genommen. Ein Teil mit `Content-Length` ist fertig, sobald so
/// viele Bytes nach den Kopfzeilen da sind; ohne Laenge endet er am naechsten
/// Trenner. Teile, die nichts Bildhaftes ankuendigen, gelten als ungeeignet.
pub fn first_part(buf: &[u8], boundary: Option<&str>) -> Part {
    match parse(buf, boundary) {
        Parsed::Complete {
            content_type, body, ..
        } => Part::Complete { content_type, body },
        Parsed::Incomplete => Part::Incomplete,
        Parsed::Invalid(why) => Part::Invalid(why),
    }
}

/// Wie [`Part`], mit dem, was der [`FrameReader`] zum Weiterlesen braucht.
enum Parsed {
    Complete {
        content_type: Option<String>,
        body: Vec<u8>,
        /// Index hinter dem Teil — ab hier beginnt der naechste.
        end: usize,
        /// Der tatsaechlich benutzte Trenner, auch wenn er geraten war.
        needle: Vec<u8>,
    },
    Incomplete,
    Invalid(String),
}

fn parse(buf: &[u8], boundary: Option<&str>) -> Parsed {
    // 1. Trenner finden und die Zeile dahinter ueberlesen.
    let (delim_at, needle) = match boundary {
        Some(b) => {
            let needle = needle_for(b);
            match find(buf, &needle, 0) {
                Some(at) => (at, needle),
                None => return Parsed::Incomplete,
            }
        }
        None => match guess_needle(buf) {
            Some(found) => found,
            None => return Parsed::Incomplete,
        },
    };
    let Some(headers_at) = after_line_end(buf, delim_at) else {
        return Parsed::Incomplete;
    };

    // 2. Kopfzeilen bis zur Leerzeile.
    let (headers_end, body_at) = match find(buf, b"\r\n\r\n", headers_at) {
        Some(p) => (p, p + 4),
        None => match find(buf, b"\n\n", headers_at) {
            Some(p) => (p, p + 2),
            None => return Parsed::Incomplete,
        },
    };
    let headers = String::from_utf8_lossy(&buf[headers_at..headers_end]);
    let content_type = header_value(&headers, "content-type").map(str::to_string);
    if let Some(ct) = &content_type {
        let ct = ct.to_ascii_lowercase();
        if !(ct.starts_with("image/") || ct.starts_with("application/octet-stream")) {
            return Parsed::Invalid(format!("Teil ist kein Bild: {ct}"));
        }
    }

    // 3. Rumpf: nach Laenge oder bis zum naechsten Trenner.
    let (body, end) = match header_value(&headers, "content-length").map(str::parse::<usize>) {
        Some(Ok(len)) => {
            if buf.len() < body_at + len {
                return Parsed::Incomplete;
            }
            (buf[body_at..body_at + len].to_vec(), body_at + len)
        }
        Some(Err(_)) => return Parsed::Invalid("Content-Length unlesbar".into()),
        None => match find(buf, &needle, body_at) {
            Some(next) => {
                let mut end = next;
                // Zeilenumbruch und ueberzaehlige Striche vor dem Trenner
                // gehoeren nicht zum Bild.
                while end > body_at && matches!(buf[end - 1], b'\r' | b'\n' | b'-') {
                    end -= 1;
                }
                // Weiter geht es *am* Trenner, nicht dahinter: er leitet den
                // naechsten Teil ein.
                (buf[body_at..end].to_vec(), next)
            }
            None => return Parsed::Incomplete,
        },
    };
    if body.is_empty() {
        return Parsed::Invalid("leerer Teil".into());
    }
    Parsed::Complete {
        content_type,
        body,
        end,
        needle,
    }
}

/// Liest einen Strom fortlaufend und liefert jeweils das neueste Bild (E-67).
///
/// Aeltere Bilder, die im selben Netzstueck mitkamen, fallen weg: ein Livebild
/// soll zeigen, was *jetzt* vor der Kamera ist, nicht aufholen, was der Rahmen
/// waehrend des Dekodierens verpasst hat. Der Puffer behaelt nur den
/// angefangenen Rest und waechst deshalb nicht mit der Laufzeit.
pub struct FrameReader {
    buf: Vec<u8>,
    /// Der Trenner, wie er im Rumpf gesucht wird. Nach dem ersten Teil steht
    /// er fest, auch wenn er anfangs geraten werden musste.
    boundary: Option<String>,
}

impl FrameReader {
    pub fn new(boundary: Option<String>) -> Self {
        Self {
            buf: Vec::new(),
            boundary,
        }
    }

    /// Haengt ein Netzstueck an.
    pub fn push(&mut self, chunk: &[u8]) {
        self.buf.extend_from_slice(chunk);
    }

    /// Wie viele Bytes auf den Rest eines angefangenen Teils warten.
    pub fn pending(&self) -> usize {
        self.buf.len()
    }

    /// Das neueste vollstaendige Bild seit dem letzten Aufruf, falls eines
    /// fertig wurde. `Err`, wenn der Strom nichts Bildhaftes mehr liefert.
    pub fn latest(&mut self) -> Result<Option<Vec<u8>>, String> {
        let mut newest = None;
        loop {
            match parse(&self.buf, self.boundary.as_deref()) {
                Parsed::Complete {
                    body, end, needle, ..
                } => {
                    if self.boundary.is_none() {
                        self.boundary = Some(String::from_utf8_lossy(&needle).into_owned());
                    }
                    self.buf.drain(..end);
                    newest = Some(body);
                }
                Parsed::Incomplete => return Ok(newest),
                Parsed::Invalid(why) => return Err(why),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xD9];

    /// Ein Teil, wie Home Assistant ihn schreibt (`camera_proxy_stream`).
    fn ha_part(body: &[u8]) -> Vec<u8> {
        let mut v = format!(
            "--frameboundary\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .into_bytes();
        v.extend_from_slice(body);
        v.extend_from_slice(b"\r\n");
        v
    }

    #[test]
    fn erkennt_den_strom_am_content_type() {
        assert!(is_multipart_stream("multipart/x-mixed-replace; boundary=--frameboundary"));
        assert!(is_multipart_stream("Multipart/X-Mixed-Replace;boundary=abc"));
        assert!(!is_multipart_stream("image/jpeg"));
        assert!(!is_multipart_stream("multipart/form-data; boundary=x"));
    }

    #[test]
    fn liest_den_trenner_aus_dem_content_type() {
        assert_eq!(
            boundary_of("multipart/x-mixed-replace; boundary=--frameboundary"),
            Some("--frameboundary".into())
        );
        assert_eq!(
            boundary_of("multipart/x-mixed-replace; charset=utf-8; Boundary=\"abc def\""),
            Some("abc def".into())
        );
        assert_eq!(boundary_of("multipart/x-mixed-replace"), None);
        assert_eq!(boundary_of("multipart/x-mixed-replace; boundary="), None);
    }

    #[test]
    fn nimmt_das_erste_bild_aus_einem_home_assistant_strom_e_55() {
        // Zwei Teile plus ein angefangener dritter: genau der Stand, den der
        // Leser nach ein paar Netzstuecken sieht.
        let mut buf = ha_part(JPEG);
        buf.extend(ha_part(&[0xFF, 0xD8, 0x00, 0xFF, 0xD9]));
        buf.extend_from_slice(b"--frameboundary\r\nContent-Type: image/jp");
        assert_eq!(
            first_part(&buf, Some("--frameboundary")),
            Part::Complete {
                content_type: Some("image/jpeg".into()),
                body: JPEG.to_vec(),
            }
        );
    }

    #[test]
    fn wartet_bis_der_teil_vollstaendig_ist() {
        let full = ha_part(JPEG);
        // Jeder Zwischenstand vor dem letzten Byte des Bildes heisst: weiter
        // lesen — nie ein halbes Bild abliefern.
        let body_end = full.len() - 2; // ohne das abschliessende CRLF
        for cut in 0..body_end {
            assert_eq!(
                first_part(&full[..cut], Some("--frameboundary")),
                Part::Incomplete,
                "Schnitt bei {cut}"
            );
        }
        assert!(matches!(
            first_part(&full[..body_end], Some("--frameboundary")),
            Part::Complete { .. }
        ));
    }

    #[test]
    fn versteht_den_regelkonformen_trenner_mit_vier_strichen() {
        // RFC 2046: im Rumpf steht `--` plus Trenner, hier also vier Striche.
        let mut buf = b"----frameboundary\r\nContent-Type: image/jpeg\r\nContent-Length: 4\r\n\r\n".to_vec();
        buf.extend_from_slice(JPEG);
        buf.extend_from_slice(b"\r\n----frameboundary\r\n");
        assert_eq!(
            first_part(&buf, Some("--frameboundary")),
            Part::Complete {
                content_type: Some("image/jpeg".into()),
                body: JPEG.to_vec(),
            }
        );
    }

    #[test]
    fn ohne_laenge_endet_der_teil_am_naechsten_trenner() {
        let mut buf = b"--abc\r\nContent-Type: image/jpeg\r\n\r\n".to_vec();
        buf.extend_from_slice(JPEG);
        // Noch kein zweiter Trenner: das Bild koennte weitergehen.
        assert_eq!(first_part(&buf, Some("abc")), Part::Incomplete);
        buf.extend_from_slice(b"\r\n--abc\r\nContent-Type: image/jpeg\r\n\r\n");
        assert_eq!(
            first_part(&buf, Some("abc")),
            Part::Complete {
                content_type: Some("image/jpeg".into()),
                body: JPEG.to_vec(),
            }
        );
    }

    #[test]
    fn findet_den_trenner_auch_ohne_angabe_im_kopf() {
        let mut buf = b"Vorspann, den manche Server schicken\r\n".to_vec();
        buf.extend(ha_part(JPEG));
        assert_eq!(
            first_part(&buf, None),
            Part::Complete {
                content_type: Some("image/jpeg".into()),
                body: JPEG.to_vec(),
            }
        );
        assert_eq!(first_part(b"nur Vorspann\r\n", None), Part::Incomplete);
    }

    #[test]
    fn ueberliest_einen_vorspann_vor_dem_ersten_trenner() {
        let mut buf = b"\r\nignorierter Vorspann\r\n".to_vec();
        buf.extend(ha_part(JPEG));
        assert!(matches!(
            first_part(&buf, Some("--frameboundary")),
            Part::Complete { .. }
        ));
    }

    #[test]
    fn lehnt_teile_ab_die_kein_bild_sind() {
        let buf = b"--abc\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhallo\r\n";
        assert!(matches!(first_part(buf, Some("abc")), Part::Invalid(_)));

        let buf = b"--abc\r\nContent-Type: image/jpeg\r\nContent-Length: vier\r\n\r\n";
        assert!(matches!(first_part(buf, Some("abc")), Part::Invalid(_)));

        let buf = b"--abc\r\nContent-Type: image/jpeg\r\nContent-Length: 0\r\n\r\n\r\n";
        assert!(matches!(first_part(buf, Some("abc")), Part::Invalid(_)));
    }

    #[test]
    fn kopfzeilen_werden_ohne_beachtung_der_schreibung_gelesen() {
        let mut buf = b"--abc\r\ncontent-type:IMAGE/JPEG\r\nCONTENT-LENGTH:  4 \r\n\r\n".to_vec();
        buf.extend_from_slice(JPEG);
        assert_eq!(
            first_part(&buf, Some("abc")),
            Part::Complete {
                content_type: Some("IMAGE/JPEG".into()),
                body: JPEG.to_vec(),
            }
        );
    }

    // ── Fortlaufendes Lesen (E-67) ──────────────────────────────────────────

    #[test]
    fn liefert_bild_fuer_bild_in_netzstuecken_e_67() {
        let mut reader = FrameReader::new(Some("--frameboundary".into()));
        let erstes = ha_part(&[0xFF, 0xD8, 1, 0xFF, 0xD9]);
        let zweites = ha_part(&[0xFF, 0xD8, 2, 0xFF, 0xD9]);

        // Das erste Bild in zwei Haelften: vor der zweiten gibt es nichts.
        let (a, b) = erstes.split_at(erstes.len() / 2);
        reader.push(a);
        assert_eq!(reader.latest(), Ok(None));
        reader.push(b);
        assert_eq!(reader.latest(), Ok(Some(vec![0xFF, 0xD8, 1, 0xFF, 0xD9])));

        reader.push(&zweites);
        assert_eq!(reader.latest(), Ok(Some(vec![0xFF, 0xD8, 2, 0xFF, 0xD9])));
        assert_eq!(reader.latest(), Ok(None), "kein Bild doppelt");
    }

    #[test]
    fn ueberspringt_aeltere_bilder_aus_demselben_stueck_e_67() {
        // Waehrend der Rahmen dekodiert, laeuft der Strom weiter. Aufholen
        // hiesse: das Livebild zeigt die Vergangenheit.
        let mut reader = FrameReader::new(Some("--frameboundary".into()));
        let mut stueck = Vec::new();
        for n in 1..=3u8 {
            stueck.extend(ha_part(&[0xFF, 0xD8, n, 0xFF, 0xD9]));
        }
        stueck.extend_from_slice(b"--frameboundary\r\nContent-Type: image/jp");
        reader.push(&stueck);
        assert_eq!(reader.latest(), Ok(Some(vec![0xFF, 0xD8, 3, 0xFF, 0xD9])));
        // Nur der angefangene vierte Teil bleibt im Puffer.
        assert!(
            reader.pending() < 45,
            "Puffer waechst: {}",
            reader.pending()
        );
    }

    #[test]
    fn liest_teile_ohne_laenge_fortlaufend_e_67() {
        // Ohne Content-Length endet ein Teil am naechsten Trenner — und der
        // gehoert dann schon zum naechsten Teil, darf also nicht verschluckt
        // werden. Ohne Trenner im Kopf wird er geraten und danach behalten.
        let mut reader = FrameReader::new(None);
        reader.push(b"--abc\r\nContent-Type: image/jpeg\r\n\r\n\xFF\xD8\x01\xFF\xD9\r\n");
        assert_eq!(reader.latest(), Ok(None));
        reader.push(b"--abc\r\nContent-Type: image/jpeg\r\n\r\n\xFF\xD8\x02\xFF\xD9\r\n");
        assert_eq!(reader.latest(), Ok(Some(vec![0xFF, 0xD8, 1, 0xFF, 0xD9])));
        reader.push(b"--abc\r\n");
        assert_eq!(reader.latest(), Ok(Some(vec![0xFF, 0xD8, 2, 0xFF, 0xD9])));
    }

    #[test]
    fn meldet_einen_strom_der_kein_bild_mehr_liefert_e_67() {
        let mut reader = FrameReader::new(Some("abc".into()));
        reader.push(b"--abc\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhallo\r\n");
        assert!(reader.latest().is_err());
    }

    #[test]
    fn kommt_mit_reinen_lf_zeilenenden_zurecht() {
        let mut buf = b"--abc\nContent-Type: image/jpeg\nContent-Length: 4\n\n".to_vec();
        buf.extend_from_slice(JPEG);
        assert!(matches!(first_part(&buf, Some("abc")), Part::Complete { .. }));
    }
}

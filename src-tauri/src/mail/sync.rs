//! Postfach abrufen und Fotos ablegen (Erweiterungspapier Teil 1).
//!
//! Eigener Weg statt `list`/`fetch` wie bei den übrigen Quellen: eine Mail
//! wird als Ganzes geholt und ihre Anhänge sofort abgelegt. Bei 25 MB je
//! Anhang lägen sonst hunderte Megabyte gleichzeitig im Speicher — genau die
//! Last, gegen die R-03 gerichtet ist.

use crate::cache::index::MailMeta;
use crate::cache::Cache;
use crate::decode::{self, DecodeError};
use crate::mail::imap::{self, MailboxConfig, MailError};
use crate::mail::parse::{is_allowed, message_id_hash, resolve_taken_at, ParsedMail};
use crate::model::{CacheConfig, MailQuality, Source, SourceKind};
use std::collections::HashSet;
use std::sync::Mutex;

/// Ergebnis eines Postfach-Abrufs.
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MailSyncReport {
    pub checked: usize,
    pub added: usize,
    pub quarantined: usize,
    pub skipped: usize,
    pub failed: usize,
    /// Wurde der Abruf durch das Rate-Limit gedeckelt? (F4)
    pub rate_limited: bool,
    /// Wie viele Nachrichten der Ordner enthielt (E-34).
    pub seen_in_folder: usize,
    /// Davon bereits im Cache, an der Message-Id erkannt (E-34).
    pub already_known: usize,
    pub error: Option<String>,
}

/// Wie viele Mails in dieser Stunde noch verarbeitet werden dürfen (F4).
///
/// Gezählt wird aus dem Cache-Index statt aus einem eigenen Zähler: die
/// Message-Ids der letzten Stunde stehen ohnehin dort, und eine zusätzliche
/// Datei müsste einen Neustart überleben, ohne dass sie etwas könnte, was der
/// Index nicht schon kann.
pub fn remaining_quota(
    index: &crate::cache::index::CacheIndex,
    source_id: &str,
    max_per_hour: u32,
    now: i64,
) -> u32 {
    if max_per_hour == 0 {
        return u32::MAX;
    }
    let seit = now - 3600;
    let letzte_stunde: HashSet<&str> = index
        .values()
        .filter(|e| e.source_id == source_id && e.added_at >= seit)
        .filter_map(|e| e.mail.as_ref().map(|m| m.message_id.as_str()))
        .collect();

    max_per_hour.saturating_sub(letzte_stunde.len() as u32)
}

/// Zugriff auf das Gedaechtnis verarbeiteter Nachrichten (E-36).
///
/// Als Buendel statt zweier Parameter: die beiden gehoeren zusammen, und
/// getrennt weitergereicht waren es acht Argumente je Aufruf — eine Zahl, bei
/// der niemand mehr sieht, was wohin gehoert.
pub struct MailMemory<'a> {
    /// Kennt das Gedaechtnis diese Kennung schon?
    pub seen: &'a (dyn Fn(&str) -> bool + Send + Sync),
    /// Nimm diese Kennung auf.
    pub remember: &'a (dyn Fn(String) + Send + Sync),
}

/// Wie viele Nachrichten ein Stapel des Neuabgleichs umfasst (Papier 3.2).
pub const RESYNC_BATCH_SIZE: usize = 50;

/// Pause zwischen zwei Stapeln, in Millisekunden (Papier 3.2).
///
/// Nicht der Höflichkeit gegenüber dem Server wegen — 50 Nachrichten sind für
/// GMX nichts —, sondern damit die Diashow weiterläuft: der Abruf entpackt und
/// skaliert Bilder, und ohne Pause belegt er den Rechner minutenlang am Stück.
pub const RESYNC_BATCH_PAUSE_MS: u64 = 2000;

/// Fortschritt eines laufenden Neuabgleichs (F8).
#[derive(Debug, Clone, Copy, PartialEq, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResyncProgress {
    /// Geprüfte Nachrichten.
    pub done: usize,
    /// Nachrichten im Ordner insgesamt.
    pub total: usize,
    /// Bisher neu abgelegte Fotos.
    pub added: usize,
}

/// Liest die Postfach-Angaben aus einer Quelle.
///
/// `None`, wenn die Quelle kein Postfach ist — der Aufrufer entscheidet dann,
/// dass hier nichts zu tun ist.
pub fn mailbox_config(source: &Source, password: &str) -> Option<MailboxConfig> {
    let SourceKind::Mail {
        host,
        port,
        username,
        folder,
        max_attachment_bytes,
        include_seen,
        ..
    } = &source.kind
    else {
        return None;
    };

    Some(MailboxConfig {
        host: host.clone(),
        port: *port,
        username: username.clone(),
        password: password.to_string(),
        folder: folder.clone(),
        max_attachment_bytes: *max_attachment_bytes,
        include_seen: *include_seen,
    })
}

/// Holt neue Mails und legt ihre Bilder ab.
///
/// Der Normalweg: Zugangsdaten und Vorgaben kommen aus der Quelle, die Menge
/// begrenzt das Stundenkontingent (F4).
pub async fn sync_mailbox(
    source: &Source,
    password: &str,
    cache: &Mutex<Cache>,
    cfg: &CacheConfig,
    now: i64,
    memory: &MailMemory<'_>,
) -> MailSyncReport {
    let mut report = MailSyncReport::default();

    let SourceKind::Mail {
        max_mails_per_hour, ..
    } = &source.kind
    else {
        return report;
    };

    let Some(mailbox) = mailbox_config(source, password) else {
        return report;
    };

    let quota = match cache.lock() {
        Ok(c) => remaining_quota(c.index(), &source.id, *max_mails_per_hour, now),
        Err(_) => {
            report.error = Some("Cache-Mutex vergiftet".into());
            return report;
        }
    };
    if quota == 0 {
        report.rate_limited = true;
        return report;
    }

    let mut r =
        sync_mailbox_batch(source, &mailbox, cache, cfg, now, quota as usize, memory).await;
    r.rate_limited = r.checked >= quota as usize;
    r
}

/// Ein Durchgang mit vorgegebenem Postfach und vorgegebener Menge.
///
/// Gemeinsamer Kern von Normalabruf und Neuabgleich (F8). Der Neuabgleich
/// setzt `include_seen` und eine eigene Stapelgroesse, macht sonst aber
/// dasselbe — ein zweiter Abrufweg koennte getrennt falsch werden.
pub async fn sync_mailbox_batch(
    source: &Source,
    mailbox: &MailboxConfig,
    cache: &Mutex<Cache>,
    cfg: &CacheConfig,
    now: i64,
    limit: usize,
    // Gedaechtnis fuer Nachrichten ohne Foto (E-36).
    memory: &MailMemory<'_>,
) -> MailSyncReport {
    let mut report = MailSyncReport::default();

    let SourceKind::Mail {
        allowed_senders,
        quarantine_all,
        quality,
        ..
    } = &source.kind
    else {
        return report;
    };

    // Bereits bekannte Message-Ids: der zweite Riegel gegen Doppelimport neben
    // dem Gelesen-Vermerk (F2). Greift auch, wenn jemand eine Mail von Hand
    // wieder als ungelesen markiert.
    let known: HashSet<String> = match cache.lock() {
        Ok(c) => c
            .index()
            .values()
            .filter_map(|e| e.mail.as_ref().map(|m| m.message_id.clone()))
            .collect(),
        Err(_) => HashSet::new(),
    };

    let result = imap::fetch_mails(
        mailbox,
        // Bekannt ist, was als Foto im Index liegt -- oder was schon einmal
        // verarbeitet wurde und kein Foto enthielt (E-36). Ohne den zweiten
        // Teil wuerde eine photolose Mail bei jedem Lauf erneut geholt.
        &|id: &str| {
            let hash = message_id_hash(id);
            known.contains(&hash) || (memory.seen)(&hash)
        },
        limit,
        |mail: ParsedMail| {
            let quarantine = *quarantine_all || !is_allowed(&mail.sender, allowed_senders);
            let hash = message_id_hash(&mail.message_id);
            match store_mail(cache, source, cfg, *quality, &mail, quarantine, now) {
                Ok(stats) => {
                    report.added += stats.0;
                    report.quarantined += stats.1;
                    report.skipped += stats.2;
                    // Auch merken, wenn kein Foto dabei war: sonst ist die
                    // Nachricht beim naechsten Lauf wieder unbekannt und wird
                    // erneut vollstaendig geholt (E-36). Bei Fotos ist es
                    // ueberfluessig, aber harmlos -- und die Unterscheidung
                    // waere eine Bedingung, die irgendwann falsch wird.
                    (memory.remember)(hash);
                    true
                }
                Err(e) => {
                    log::warn!("Mail von {} nicht ablegbar: {e}", mail.sender);
                    report.failed += 1;
                    // Nicht als gelesen markieren: beim naechsten Lauf noch
                    // einmal versuchen.
                    false
                }
            }
        },
    )
    .await;

    match result {
        Ok(fetched) => {
            report.checked = fetched.checked;
            report.seen_in_folder = fetched.seen_in_folder;
            report.already_known = fetched.already_known;

            // Bis hierher schrieb ein Postfach-Abruf nichts ins Protokoll:
            // ob ein Foto ankam, war nur am Bild-Browser zu sehen. Bei
            // eingeschaltetem „auch gelesene" ist die Zeile zudem die einzige
            // Erklaerung dafuer, warum ein Lauf ueber tausend Nachrichten drei
            // Fotos brachte (E-34).
            log::info!(
                concat!(
                    "Postfach '{}': {} Nachricht(en) im Ordner, {} bereits bekannt, ",
                    "{} geholt, {} Foto(s) abgelegt ({} Quarantaene), ",
                    "{} uebersprungen, {} fehlgeschlagen"
                ),
                source.name,
                report.seen_in_folder,
                report.already_known,
                report.checked,
                report.added,
                report.quarantined,
                report.skipped,
                report.failed
            );
        }
        Err(e) => {
            let text = friendly(&e);
            log::warn!("Postfach '{}': {text}", source.name);
            report.error = Some(text);
        }
    }

    report
}

/// Legt die Bilder einer Mail ab. Liefert (abgelegt, davon Quarantäne, übersprungen).
fn store_mail(
    cache: &Mutex<Cache>,
    source: &Source,
    cfg: &CacheConfig,
    quality: MailQuality,
    mail: &ParsedMail,
    quarantine: bool,
    now: i64,
) -> Result<(usize, usize, usize), String> {
    let (target_w, target_h, jpeg_quality) = quality.targets(cfg.target_width, cfg.target_height);
    let hash = message_id_hash(&mail.message_id);

    let mut added = 0;
    let mut quarantined = 0;
    let mut skipped = mail.skipped.len();

    for (nr, photo) in mail.photos.iter().enumerate() {
        let prepared = match decode::prepare(
            &photo.bytes,
            target_w,
            target_h,
            jpeg_quality,
            source.min_width,
            source.min_height,
        ) {
            Ok(p) => p,
            Err(DecodeError::Unsupported(what)) => {
                log::info!("Mail-Anhang {} übersprungen ({what})", photo.file_name);
                skipped += 1;
                continue;
            }
            Err(DecodeError::TooSmall { .. }) => {
                skipped += 1;
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };

        // Aufnahmedatum nach der Rangfolge aus 1.3 -- EXIF schlaegt die
        // Jahreszahl im Betreff, diese den Empfang.
        let taken_at = resolve_taken_at(prepared.taken_at, mail.received_at, &mail.subject);

        // Pfad aus Hash und laufender Nummer: eindeutig je Anhang, und beim
        // erneuten Abgleich derselben Mail derselbe Schluessel (F2, F8).
        let rel_path = format!("{hash}/{nr}");

        let mut guard = cache.lock().map_err(|_| "Cache-Mutex vergiftet".to_string())?;
        let entry = guard
            .store(
                &source.id,
                &rel_path,
                &photo.file_name,
                crate::decode::Prepared {
                    taken_at,
                    ..prepared
                },
                None,
                None,
                mail.received_at,
                now,
            )
            .map_err(|e| e.to_string())?;

        guard.set_mail_meta(
            &entry.id,
            MailMeta {
                sender: mail.sender.clone(),
                subject: mail.subject.clone(),
                message_id: hash.clone(),
                quarantined: quarantine,
            },
        );

        added += 1;
        if quarantine {
            quarantined += 1;
        }
    }

    Ok((added, quarantined, skipped))
}

/// Fehlertext, den auch jemand ohne Fachwissen einordnen kann (Teil 0).
fn friendly(e: &MailError) -> String {
    match e {
        MailError::Login(server) if !server.is_empty() => {
            format!("Anmeldung abgelehnt. Der Server sagt: {server}")
        }
        MailError::Login(_) => {
            "Anmeldung abgelehnt. Bitte Benutzername und Passwort prüfen.".into()
        }
        MailError::Connect { host, .. } => {
            format!("Keine Verbindung zu {host}. Ist der Rahmen im Netz?")
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::index::CacheEntry;

    fn entry(id: &str, source: &str, message_id: &str, added_at: i64) -> CacheEntry {
        CacheEntry {
            id: id.into(),
            source_id: source.into(),
            rel_path: id.into(),
            file_name: format!("{id}.jpg"),
            etag: None,
            remote_size: None,
            remote_mtime: None,
            taken_at: None,
            width: 100,
            height: 100,
            bytes: 10,
            added_at,
            last_shown: None,
            show_count: 0,
            mail: Some(MailMeta {
                sender: "oma@example.org".into(),
                subject: "Hallo".into(),
                message_id: message_id.into(),
                quarantined: false,
            }),
            excluded: false,
            thumb_bytes: None,
        }
    }

    const NOW: i64 = 1_000_000;

    fn index_of(entries: Vec<CacheEntry>) -> crate::cache::index::CacheIndex {
        let mut idx = crate::cache::index::CacheIndex::new();
        for e in entries {
            idx.insert(e);
        }
        idx
    }

    #[test]
    fn stapelgroesse_und_pause_folgen_dem_papier() {
        // Papier 3.2 nennt beide Zahlen ausdruecklich. Ein Test darauf ist
        // banal, haelt aber fest, dass sie nicht beilaeufig verstellt werden:
        // ein groesserer Stapel ohne Pause blockiert die Diashow minutenlang.
        assert_eq!(RESYNC_BATCH_SIZE, 50);
        assert_eq!(RESYNC_BATCH_PAUSE_MS, 2000);
    }

    #[test]
    fn fortschritt_beginnt_bei_null() {
        let p = ResyncProgress::default();
        assert_eq!((p.done, p.total, p.added), (0, 0, 0));
    }

    #[test]
    fn kontingent_zaehlt_nur_die_letzte_stunde() {
        let idx = index_of(vec![
            entry("a", "m", "id1", NOW - 60),
            entry("b", "m", "id2", NOW - 120),
            // Aelter als eine Stunde -- zaehlt nicht mehr mit.
            entry("c", "m", "id3", NOW - 7200),
        ]);
        assert_eq!(remaining_quota(&idx, "m", 30, NOW), 28);
    }

    #[test]
    fn kontingent_zaehlt_mails_nicht_fotos() {
        // Zwei Anhaenge derselben Mail sind eine Mail (F4 begrenzt Mails).
        let idx = index_of(vec![
            entry("a", "m", "gleich", NOW - 60),
            entry("b", "m", "gleich", NOW - 60),
        ]);
        assert_eq!(remaining_quota(&idx, "m", 10, NOW), 9);
    }

    #[test]
    fn kontingent_trennt_die_quellen() {
        let idx = index_of(vec![entry("a", "andere", "id1", NOW - 60)]);
        assert_eq!(remaining_quota(&idx, "m", 5, NOW), 5);
    }

    #[test]
    fn kontingent_null_bedeutet_unbegrenzt() {
        assert_eq!(remaining_quota(&index_of(vec![]), "m", 0, NOW), u32::MAX);
    }

    #[test]
    fn mailbox_config_nur_fuer_postfaecher() {
        let lokal = Source {
            id: "s".into(),
            name: "Ordner".into(),
            kind: SourceKind::Local {
                saf_uri: "{}".into(),
                display_path: "DCIM".into(),
            },
            enabled: true,
            subfolders: vec![],
            min_width: 0,
            min_height: 0,
            sync_interval_minutes: 60,
            last_sync: None,
        };
        assert!(mailbox_config(&lokal, "geheim").is_none());
    }

    #[test]
    fn fehlertexte_nennen_die_naechste_handlung() {
        let t = friendly(&MailError::Login(String::new()));
        assert!(t.contains("Passwort"), "{t}");

        // Sagt der Server, warum, muss das durchkommen -- am echten Postfach
        // war das der Unterschied zwischen "raten" und "wissen".
        let t = friendly(&MailError::Login("IMAP disabled".into()));
        assert!(t.contains("IMAP disabled"), "{t}");

        let t = friendly(&MailError::Connect {
            host: "imap.example.org".into(),
            port: 993,
            source: std::io::Error::other("weg"),
        });
        assert!(t.contains("imap.example.org"), "{t}");
        assert!(t.contains("Netz"), "{t}");
    }
}

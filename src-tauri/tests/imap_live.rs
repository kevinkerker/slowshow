//! Abruf gegen ein echtes Postfach (Erweiterungspapier Teil 1).
//!
//! Standardmäßig übersprungen: Der Test braucht einen erreichbaren Server und
//! gültige Zugangsdaten, beides gibt es auf einem Bau-Server nicht. Er ist
//! trotzdem versioniert, weil die Alternative — einmal von Hand prüfen und die
//! Schritte vergessen — bei der nächsten Änderung am IMAP-Pfad nichts wert
//! wäre.
//!
//! **Zugangsdaten stehen nirgends im Repository.** Sie kommen aus der
//! Umgebung, damit ein App-Passwort nicht versehentlich in einen Commit
//! gerät:
//!
//! ```text
//! SLOWSHOW_IMAP_HOST=imap.example.org
//! SLOWSHOW_IMAP_USER=jemand@example.org
//! SLOWSHOW_IMAP_PASS=…
//! SLOWSHOW_IMAP_PORT=993      (optional)
//! SLOWSHOW_IMAP_FOLDER=INBOX  (optional)
//! SLOWSHOW_IMAP_SEEN=1        (optional: auch gelesene, E-34)
//!
//! cargo test --test imap_live -- --ignored --nocapture
//! ```
//!
//! Der Test verändert das Postfach nicht: er ruft nur den Verbindungstest auf
//! und zählt, was ungelesen ist. Der eigentliche Abruf markiert Nachrichten
//! als gelesen — das gehört nicht in einen Test, den jemand versehentlich
//! startet.

use slowshow_lib::mail::imap::{self, MailboxConfig};

fn config_from_env() -> Option<MailboxConfig> {
    Some(MailboxConfig {
        host: std::env::var("SLOWSHOW_IMAP_HOST").ok()?,
        port: std::env::var("SLOWSHOW_IMAP_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(imap::DEFAULT_PORT),
        username: std::env::var("SLOWSHOW_IMAP_USER").ok()?,
        password: std::env::var("SLOWSHOW_IMAP_PASS").ok()?,
        folder: std::env::var("SLOWSHOW_IMAP_FOLDER")
            .unwrap_or_else(|_| imap::DEFAULT_FOLDER.to_string()),
        max_attachment_bytes: 25 * 1024 * 1024,
        // Der Live-Test soll das Postfach so wenig wie moeglich anfassen und
        // haelt sich deshalb an den Normalbetrieb (E-34).
        include_seen: std::env::var("SLOWSHOW_IMAP_SEEN").as_deref() == Ok("1"),
    })
}

#[tokio::test]
#[ignore = "braucht ein echtes Postfach — Zugangsdaten ueber Umgebungsvariablen"]
async fn verbindet_sich_und_findet_das_postfach() {
    let Some(cfg) = config_from_env() else {
        panic!("SLOWSHOW_IMAP_HOST, _USER und _PASS muessen gesetzt sein");
    };

    println!("Verbinde mit {}:{} als {}", cfg.host, cfg.port, cfg.username);

    match imap::test_connection(&cfg).await {
        Ok(unseen) => {
            println!("Verbindung steht. Ungelesen: {unseen}");
        }
        Err(e) => panic!("Verbindung fehlgeschlagen: {e}"),
    }
}

#[tokio::test]
#[ignore = "braucht ein echtes Postfach — meldet einen falschen Servernamen"]
async fn falscher_server_meldet_einen_lesbaren_fehler() {
    // Gegenprobe: der haeufigste Einrichtungsfehler ist ein Tippfehler im
    // Servernamen. Die Meldung muss das erkennen lassen, ohne Fachwissen.
    let cfg = MailboxConfig {
        host: "imap.gibtesnicht.invalid".into(),
        port: 993,
        username: "wer@example.org".into(),
        password: "egal".into(),
        folder: "INBOX".into(),
        max_attachment_bytes: 1024,
        include_seen: false,
    };

    let err = imap::test_connection(&cfg)
        .await
        .expect_err("ein erfundener Server darf nicht erreichbar sein");
    let text = err.to_string();
    println!("Fehlertext: {text}");
    assert!(
        text.contains("imap.gibtesnicht.invalid"),
        "der Text muss den Server nennen: {text}"
    );
}

/// Holt ungelesene Nachrichten und berichtet, was darin steckt.
///
/// **Verändert das Postfach**: verarbeitete Nachrichten werden als gelesen
/// markiert. Deshalb doppelt abgesichert — `--ignored` *und* eine eigene
/// Umgebungsvariable, damit ein versehentlicher Lauf der übrigen Live-Tests
/// nicht das Postfach anfasst.
#[tokio::test]
#[ignore = "veraendert das Postfach — zusaetzlich SLOWSHOW_IMAP_FETCH=1 noetig"]
async fn holt_ungelesene_nachrichten() {
    if std::env::var("SLOWSHOW_IMAP_FETCH").as_deref() != Ok("1") {
        println!("uebersprungen: SLOWSHOW_IMAP_FETCH=1 setzen, um wirklich abzurufen");
        return;
    }

    let Some(cfg) = config_from_env() else {
        panic!("SLOWSHOW_IMAP_HOST, _USER und _PASS muessen gesetzt sein");
    };

    let mut gesehen = Vec::new();
    let report = imap::fetch_mails(
        &cfg,
        &|_id: &str| false,
        5,
        |mail| {
            println!(
                "Von {} | Betreff {:?} | {} Bild(er), {} uebersprungen",
                mail.sender,
                mail.subject,
                mail.photos.len(),
                mail.skipped.len()
            );
            for p in &mail.photos {
                println!("    Anhang {} ({} Bytes)", p.file_name, p.bytes.len());
            }
            for s in &mail.skipped {
                println!("    uebersprungen: {s}");
            }
            gesehen.push(mail.sender.clone());
            // `false`: nicht als gelesen markieren. Ein Test soll den Zustand
            // des Postfachs so wenig wie moeglich veraendern.
            false
        },
    )
    .await
    .expect("Abruf muss durchlaufen");

    println!("Bericht: {report:?}");
    assert!(
        report.checked > 0,
        "es sollte mindestens eine ungelesene Nachricht geben"
    );
}

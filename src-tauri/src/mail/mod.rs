//! Fotos per E-Mail empfangen (Erweiterungspapier Teil 1, E-30).
//!
//! ## Aufteilung
//!
//! [`parse`] wertet eine rohe Nachricht aus und ist frei von Netzzugriff —
//! welche Anhänge zählen, von wem sie kommen, welches Datum gilt, lässt sich
//! damit gegen ein Literal prüfen statt gegen ein Postfach.
//!
//! [`imap`] holt die Nachrichten. Dort steckt alles, was ohne Server nicht
//! prüfbar ist, und entsprechend wenig Logik.
//!
//! ## Warum das Postfach die Quelle der Wahrheit ist
//!
//! Der lokale Bestand lässt sich jederzeit aus dem Postfach rekonstruieren.
//! Deshalb darf der Ringpuffer Mail-Fotos verdrängen wie alle anderen (E-28) —
//! verloren ist nichts, was ein erneuter Abgleich nicht zurückholt.

pub mod imap;
/// Protokoll der Abrufe, damit ein Ausfall auch ohne Kabel sichtbar ist (F6).
pub mod log;
pub mod parse;
/// Gedaechtnis fuer Nachrichten ohne Foto (E-36).
pub mod seen;
pub mod sync;

pub use parse::{
    is_allowed, message_id_hash, parse_mail, resolve_taken_at, year_from_subject, MailPhoto,
    ParsedMail,
};

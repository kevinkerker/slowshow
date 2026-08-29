//! Verschlüsselte Ablage der Zugangsdaten (NF-05, R-12).
//!
//! ## Stand der Umsetzung — bitte lesen
//!
//! NF-05 verlangt „Android Keystore oder gleichwertige Verschlüsselung".
//! Umgesetzt ist hier die Verschlüsselung (AES-256-GCM, reines Rust), **nicht**
//! die Keystore-Anbindung: der Schlüssel liegt als Datei im App-privaten
//! Verzeichnis neben den Daten.
//!
//! Das schützt gegen Auslesen aus einem Gerätebackup und gegen versehentliches
//! Mitkopieren der Konfiguration (FA-45 exportiert die Zugangsdaten nicht mit).
//! Es schützt **nicht** gegen jemanden mit Root-Zugriff auf das entsperrte
//! Gerät — dafür müsste der Schlüssel in den Android Keystore, was einen
//! eigenen Kotlin-Plugin-Code erfordert (Lastenheft Abschnitt 8, Zeile
//! „Zugangsdaten-Verschlüsselung", dort mit 🟡 bewertet).
//!
//! Die Trennlinie dafür ist [`KeyProvider`]: ein Keystore-gestützter Provider
//! lässt sich später einsetzen, ohne eine einzige Aufrufstelle zu ändern.
//! Bis dahin bleibt die Empfehlung aus R-12 gültig: für die App ein eigenes
//! NAS-Konto mit Nur-Lese-Rechten verwenden.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("Zugangsdaten-IO fehlgeschlagen: {0}")]
    Io(#[from] std::io::Error),
    #[error("Zugangsdaten konnten nicht entschlüsselt werden")]
    Decrypt,
    #[error("Schlüssel konnte nicht erzeugt werden: {0}")]
    Rand(String),
    #[error("Zugangsdaten sind kein gültiges JSON: {0}")]
    Parse(#[from] serde_json::Error),
}

/// Woher der Verschlüsselungsschlüssel kommt.
///
/// Die Abstraktion existiert allein deshalb, damit der Wechsel auf den Android
/// Keystore später eine reine Implementierungsfrage bleibt.
pub trait KeyProvider {
    fn key(&self) -> Result<[u8; KEY_LEN], SecretError>;
}

/// Übergangslösung: zufälliger Schlüssel als Datei im App-privaten Verzeichnis.
pub struct FileKeyProvider {
    path: PathBuf,
}

impl FileKeyProvider {
    pub fn new(dir: impl AsRef<Path>) -> Self {
        Self {
            path: dir.as_ref().join("secret.key"),
        }
    }
}

impl KeyProvider for FileKeyProvider {
    fn key(&self) -> Result<[u8; KEY_LEN], SecretError> {
        match std::fs::read(&self.path) {
            Ok(bytes) if bytes.len() == KEY_LEN => {
                let mut k = [0u8; KEY_LEN];
                k.copy_from_slice(&bytes);
                Ok(k)
            }
            _ => {
                let mut k = [0u8; KEY_LEN];
                getrandom::getrandom(&mut k).map_err(|e| SecretError::Rand(e.to_string()))?;
                write_private(&self.path, &k)?;
                Ok(k)
            }
        }
    }
}

/// Ablage der Passwörter, adressiert über `password_ref` aus [`crate::model::SourceKind`].
pub struct SecretStore {
    path: PathBuf,
    key: [u8; KEY_LEN],
    entries: HashMap<String, String>,
}

impl SecretStore {
    pub fn open(dir: impl AsRef<Path>, provider: &dyn KeyProvider) -> Result<Self, SecretError> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir)?;
        let path = dir.join("secrets.bin");
        let key = provider.key()?;

        let entries = match std::fs::read(&path) {
            Ok(blob) => match decrypt(&key, &blob) {
                Ok(plain) => serde_json::from_slice(&plain)?,
                Err(e) => {
                    // Ein verlorener Schlüssel darf den Start nicht verhindern —
                    // der Nutzer gibt die Passwörter dann neu ein.
                    log::error!("Zugangsdaten nicht entschlüsselbar, starte leer: {e}");
                    HashMap::new()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => return Err(e.into()),
        };

        Ok(Self { path, key, entries })
    }

    pub fn get(&self, reference: &str) -> Option<&str> {
        self.entries.get(reference).map(|s| s.as_str())
    }

    pub fn set(&mut self, reference: &str, password: &str) -> Result<(), SecretError> {
        self.entries
            .insert(reference.to_string(), password.to_string());
        self.persist()
    }

    pub fn remove(&mut self, reference: &str) -> Result<(), SecretError> {
        self.entries.remove(reference);
        self.persist()
    }

    /// Entfernt Passwörter, auf die keine Quelle mehr zeigt.
    pub fn retain_refs(&mut self, keep: &[String]) -> Result<(), SecretError> {
        let before = self.entries.len();
        self.entries.retain(|k, _| keep.contains(k));
        if self.entries.len() != before {
            self.persist()?;
        }
        Ok(())
    }

    fn persist(&self) -> Result<(), SecretError> {
        let plain = serde_json::to_vec(&self.entries)?;
        let blob = encrypt(&self.key, &plain)?;
        write_private(&self.path, &blob)
    }
}

/// Verschlüsselt zu `nonce || ciphertext`.
fn encrypt(key: &[u8; KEY_LEN], plain: &[u8]) -> Result<Vec<u8>, SecretError> {
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce_bytes).map_err(|e| SecretError::Rand(e.to_string()))?;
    let cipher = Aes256Gcm::new(key.into());
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plain)
        .map_err(|_| SecretError::Decrypt)?;

    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ct);
    Ok(out)
}

fn decrypt(key: &[u8; KEY_LEN], blob: &[u8]) -> Result<Vec<u8>, SecretError> {
    if blob.len() <= NONCE_LEN {
        return Err(SecretError::Decrypt);
    }
    let (nonce, ct) = blob.split_at(NONCE_LEN);
    Aes256Gcm::new(key.into())
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|_| SecretError::Decrypt)
}

/// Schreibt atomar und — wo das Betriebssystem es kennt — nur für den
/// Eigentümer lesbar. Auf Android ist das App-Verzeichnis ohnehin isoliert.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), SecretError> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let mut p = std::env::temp_dir();
            p.push(format!("slowshow-sec-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Fester Schlüssel — macht die Tests unabhängig von der Schlüsseldatei.
    struct FixedKey([u8; KEY_LEN]);

    impl KeyProvider for FixedKey {
        fn key(&self) -> Result<[u8; KEY_LEN], SecretError> {
            Ok(self.0)
        }
    }

    #[test]
    fn roundtrip_verschluesselt_und_entschluesselt() {
        let key = [7u8; KEY_LEN];
        let blob = encrypt(&key, b"geheimes passwort").unwrap();
        assert_ne!(
            &blob[NONCE_LEN..],
            b"geheimes passwort",
            "darf nicht im Klartext stehen"
        );
        assert_eq!(decrypt(&key, &blob).unwrap(), b"geheimes passwort");
    }

    #[test]
    fn falscher_schluessel_entschluesselt_nicht() {
        let blob = encrypt(&[7u8; KEY_LEN], b"geheim").unwrap();
        assert!(decrypt(&[8u8; KEY_LEN], &blob).is_err());
    }

    #[test]
    fn manipulierter_geheimtext_wird_erkannt() {
        // GCM ist authentifiziert — eine Änderung muss auffallen.
        let key = [7u8; KEY_LEN];
        let mut blob = encrypt(&key, b"geheim").unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0xFF;
        assert!(decrypt(&key, &blob).is_err());
    }

    #[test]
    fn zwei_verschluesselungen_erzeugen_verschiedene_nonces() {
        let key = [7u8; KEY_LEN];
        let a = encrypt(&key, b"gleich").unwrap();
        let b = encrypt(&key, b"gleich").unwrap();
        assert_ne!(a, b, "Nonce-Wiederverwendung wuerde GCM brechen");
    }

    #[test]
    fn zu_kurzer_blob_paniciert_nicht() {
        assert!(decrypt(&[7u8; KEY_LEN], &[]).is_err());
        assert!(decrypt(&[7u8; KEY_LEN], &[0u8; NONCE_LEN]).is_err());
    }

    #[test]
    fn passwoerter_ueberleben_neustart_nf_05() {
        let dir = TempDir::new("persist");
        let provider = FixedKey([3u8; KEY_LEN]);
        {
            let mut store = SecretStore::open(&dir.0, &provider).unwrap();
            store.set("nas", "hunter2").unwrap();
            store.set("cloud", "geheim").unwrap();
        }
        let store = SecretStore::open(&dir.0, &provider).unwrap();
        assert_eq!(store.get("nas"), Some("hunter2"));
        assert_eq!(store.get("cloud"), Some("geheim"));
        assert_eq!(store.get("gibtsnicht"), None);
    }

    #[test]
    fn passwoerter_stehen_nicht_im_klartext_auf_der_platte() {
        let dir = TempDir::new("plaintext");
        let mut store = SecretStore::open(&dir.0, &FixedKey([3u8; KEY_LEN])).unwrap();
        store.set("nas", "streng-geheimes-passwort").unwrap();

        let raw = std::fs::read(dir.0.join("secrets.bin")).unwrap();
        let as_text = String::from_utf8_lossy(&raw);
        assert!(!as_text.contains("streng-geheimes-passwort"));
        assert!(!as_text.contains("nas"));
    }

    #[test]
    fn remove_und_retain_raeumen_auf() {
        let dir = TempDir::new("cleanup");
        let provider = FixedKey([3u8; KEY_LEN]);
        let mut store = SecretStore::open(&dir.0, &provider).unwrap();
        store.set("a", "1").unwrap();
        store.set("b", "2").unwrap();
        store.set("c", "3").unwrap();

        store.remove("a").unwrap();
        assert_eq!(store.get("a"), None);

        store.retain_refs(&["b".to_string()]).unwrap();
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), None, "verwaistes Passwort wird entfernt");
    }

    #[test]
    fn verlorener_schluessel_blockiert_den_start_nicht() {
        let dir = TempDir::new("lostkey");
        {
            let mut store = SecretStore::open(&dir.0, &FixedKey([3u8; KEY_LEN])).unwrap();
            store.set("nas", "hunter2").unwrap();
        }
        // Anderer Schlüssel — z. B. nach Neuinstallation.
        let store = SecretStore::open(&dir.0, &FixedKey([9u8; KEY_LEN])).unwrap();
        assert_eq!(store.get("nas"), None, "leer statt Absturz");
    }

    #[test]
    fn file_key_provider_ist_stabil_ueber_aufrufe() {
        let dir = TempDir::new("keyfile");
        let p = FileKeyProvider::new(&dir.0);
        let a = p.key().unwrap();
        let b = p.key().unwrap();
        assert_eq!(
            a, b,
            "der Schluessel darf sich nicht bei jedem Start aendern"
        );
        assert_ne!(a, [0u8; KEY_LEN]);
    }
}

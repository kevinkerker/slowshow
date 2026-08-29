//! Persistenz der Konfiguration (FA-42, FA-45).
//!
//! Einstellungen und Quellen liegen als `config.json` im App-Konfigurationsordner
//! und überleben damit App- und Geräteneustart. Zugangsdaten stehen *nicht* in
//! dieser Datei, sondern verschlüsselt daneben (siehe `secrets`), damit ein
//! Konfigurations-Export (FA-45) keine Passwörter mitnimmt.

use crate::model::AppConfig;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Konfiguration konnte nicht gelesen/geschrieben werden: {0}")]
    Io(#[from] std::io::Error),
    #[error("Konfiguration ist kein gültiges JSON: {0}")]
    Parse(#[from] serde_json::Error),
}

pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn new(dir: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir)?;
        Ok(Self {
            path: dir.join("config.json"),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Lädt die Konfiguration.
    ///
    /// Eine fehlende Datei ergibt die Standardkonfiguration — das ist der
    /// Erststart. Eine kaputte Datei ergibt ebenfalls die Standardwerte statt
    /// eines Startabbruchs: ein Bilderrahmen, der wegen einer beschädigten
    /// Einstellungsdatei gar nicht mehr hochkommt, wäre das schlechtere
    /// Verhalten (NF-02).
    pub fn load(&self) -> AppConfig {
        let mut config = match std::fs::read(&self.path) {
            Ok(bytes) => serde_json::from_slice::<AppConfig>(&bytes).unwrap_or_else(|e| {
                log::error!("config.json unlesbar, starte mit Standardwerten: {e}");
                AppConfig::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => AppConfig::default(),
            Err(e) => {
                log::error!("config.json nicht lesbar: {e}");
                AppConfig::default()
            }
        };
        config.clamp();
        config
    }

    /// Schreibt die Konfiguration atomar.
    pub fn save(&self, config: &AppConfig) -> Result<(), ConfigError> {
        let mut config = config.clone();
        config.clamp();
        let bytes = serde_json::to_vec_pretty(&config)?;

        let tmp = self.path.with_extension("tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&bytes)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PlayOrder, Source, SourceKind};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let mut p = std::env::temp_dir();
            p.push(format!("slowshow-cfg-{name}-{}", std::process::id()));
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

    #[test]
    fn erststart_liefert_standardkonfiguration() {
        let dir = TempDir::new("first");
        let store = ConfigStore::new(&dir.0).unwrap();
        let c = store.load();
        assert_eq!(c.interval_seconds, 30);
        assert!(c.sources.is_empty());
    }

    #[test]
    fn einstellungen_ueberleben_neustart_fa_42() {
        let dir = TempDir::new("persist");
        let store = ConfigStore::new(&dir.0).unwrap();

        let mut c = AppConfig {
            interval_seconds: 120,
            order: PlayOrder::FileName,
            ..Default::default()
        };
        c.overlays.show_file_name = true;
        c.sources.push(Source {
            id: "nas".into(),
            name: "NAS Fotoarchiv".into(),
            kind: SourceKind::WebDav {
                url: "https://nas.local/dav".into(),
                username: "frame".into(),
                password_ref: "nas".into(),
                allow_insecure_tls: true,
            },
            enabled: true,
            subfolders: vec!["Urlaub".into()],
            min_width: 1024,
            min_height: 768,
            sync_interval_minutes: 60,
            last_sync: Some(1700),
        });
        store.save(&c).unwrap();

        // Zweiter Start.
        let store2 = ConfigStore::new(&dir.0).unwrap();
        let back = store2.load();
        assert_eq!(back.interval_seconds, 120);
        assert_eq!(back.order, PlayOrder::FileName);
        assert!(back.overlays.show_file_name);
        assert_eq!(back.sources.len(), 1);
        assert_eq!(back.sources[0].name, "NAS Fotoarchiv");
        assert_eq!(back.sources[0].subfolders, vec!["Urlaub"]);
        assert_eq!(back.sources[0].last_sync, Some(1700));
    }

    #[test]
    fn kaputte_datei_blockiert_den_start_nicht_nf_02() {
        let dir = TempDir::new("corrupt");
        let store = ConfigStore::new(&dir.0).unwrap();
        std::fs::write(store.path(), b"{{{ kaputt").unwrap();

        let c = store.load();
        assert_eq!(c.interval_seconds, 30, "faellt auf Standardwerte zurueck");
    }

    #[test]
    fn save_erzwingt_die_wertebereiche() {
        let dir = TempDir::new("clamp");
        let store = ConfigStore::new(&dir.0).unwrap();

        let c = AppConfig {
            interval_seconds: 2, // unter der Untergrenze aus FA-02
            ..Default::default()
        };
        store.save(&c).unwrap();

        assert_eq!(store.load().interval_seconds, 5);
    }

    #[test]
    fn load_klemmt_auch_manuell_editierte_werte() {
        let dir = TempDir::new("clamp2");
        let store = ConfigStore::new(&dir.0).unwrap();
        std::fs::write(store.path(), br#"{"intervalSeconds": 99999}"#).unwrap();
        assert_eq!(store.load().interval_seconds, 1800);
    }

    #[test]
    fn save_ist_atomar_und_laesst_keine_temp_datei_zurueck() {
        let dir = TempDir::new("atomic");
        let store = ConfigStore::new(&dir.0).unwrap();
        store.save(&AppConfig::default()).unwrap();
        assert!(store.path().exists());
        assert!(!store.path().with_extension("tmp").exists());
    }
}

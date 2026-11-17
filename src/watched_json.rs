// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// src/watched_json.rs
// Een instellingenbestand in JSON dat de applicatie bij de start laadt, zelf kan
// bijwerken én dat je met de hand mag aanpassen: wijzigingen van buitenaf worden
// binnen `check_interval` opgepikt. Generiek over het schema T (native only).
//
// Regels:
//   - ontbreekt het bestand, dan wordt het met de standaardwaarden aangemaakt —
//     een sjabloon om met de hand aan te passen;
//   - is het onleesbaar (tikfout), dan blijven de huidige waarden staan en wordt
//     het bestand NIET overschreven: je eigen werk gaat niet verloren;
//   - opslaan gaat via een tijdelijk bestand dat eerst volledig naar schijf gaat
//     (fsync), dan rename: na een stroomstoring staat er altijd de oude of de
//     nieuwe versie, nooit een half geschreven bestand.

use serde::{de::DeserializeOwned, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

pub struct WatchedJson<T> {
    path: PathBuf,
    value: T,
    modified: Option<SystemTime>,
    last_check: Instant,
    check_interval: Duration,
}

impl<T> WatchedJson<T>
where
    T: Serialize + DeserializeOwned + Default + PartialEq,
{
    /// Laadt het bestand, of maakt het aan met de standaardwaarden.
    pub fn open(path: PathBuf, check_interval: Duration) -> Self {
        let mut me = Self {
            path,
            value: T::default(),
            modified: None,
            last_check: Instant::now(),
            check_interval,
        };
        if me.path.exists() {
            match me.read() {
                Ok(v) => {
                    me.value = v;
                    crate::console_log!("⚙️ Instellingen geladen: {}", me.path.display());
                }
                Err(e) => crate::console_log!(
                    "⚠️ {} is onleesbaar ({}) — standaardwaarden tot het bestand klopt",
                    me.path.display(),
                    e
                ),
            }
            me.modified = modified_time(&me.path);
        } else {
            match me.write(&T::default()) {
                Ok(()) => crate::console_log!("⚙️ Instellingenbestand aangemaakt: {}", me.path.display()),
                Err(e) => crate::console_log!("⚠️ {} aanmaken mislukt: {}", me.path.display(), e),
            }
        }
        me
    }

    pub fn get(&self) -> &T {
        &self.value
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Elke frame aanroepen mag: kijkt hooguit eens per `check_interval` of het
    /// bestand van buitenaf veranderd is. true = de waarden zijn veranderd.
    pub fn poll(&mut self) -> bool {
        if self.last_check.elapsed() < self.check_interval {
            return false;
        }
        self.last_check = Instant::now();
        let modified = modified_time(&self.path);
        if modified.is_none() || modified == self.modified {
            return false; // ontbreekt (even) of ongewijzigd: niets doen
        }
        self.modified = modified;
        match self.read() {
            Ok(v) if v != self.value => {
                self.value = v;
                crate::console_log!("⚙️ Instellingen opnieuw geladen: {}", self.path.display());
                true
            }
            Ok(_) => false,
            Err(e) => {
                crate::console_log!("⚠️ {} is onleesbaar ({}) — vorige waarden blijven staan", self.path.display(), e);
                false
            }
        }
    }

    /// Nieuwe waarden overnemen en opslaan. true = er veranderde iets.
    pub fn set(&mut self, value: T) -> bool {
        if value == self.value {
            return false;
        }
        if let Err(e) = self.write(&value) {
            crate::console_log!("⚠️ {} opslaan mislukt: {}", self.path.display(), e);
        }
        self.value = value;
        true
    }

    fn read(&self) -> Result<T, String> {
        let text = fs::read_to_string(&self.path).map_err(|e| e.to_string())?;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    }

    fn write(&mut self, value: &T) -> Result<(), String> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let mut text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
        text.push('\n');
        // Eerst volledig naar schijf (sync_all), dan pas de naam omzetten, en ook de map
        // syncen: de installatie gaat vaak uit door de stekker eruit te trekken, en dan
        // moet de laatst opgeslagen versie er na het opstarten gewoon staan
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
            f.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            f.sync_all().map_err(|e| e.to_string())?;
        }
        fs::rename(&tmp, &self.path).map_err(|e| e.to_string())?;
        if let Some(dir) = self.path.parent() {
            if let Ok(d) = fs::File::open(dir) {
                let _ = d.sync_all();
            }
        }
        // Eigen schrijfactie niet bij de volgende poll() als wijziging van buitenaf zien
        self.modified = modified_time(&self.path);
        Ok(())
    }
}

fn modified_time(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// ~/.config/<app>/<file> (of $XDG_CONFIG_HOME/<app>/<file>)
pub fn config_path(app: &str, file: &str) -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(app).join(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Serialize, Deserialize, Default, PartialEq, Debug, Clone)]
    #[serde(default)]
    struct Demo {
        level: f32,
    }

    #[test]
    fn survives_restart_and_handles_edits() {
        let dir = std::env::temp_dir().join(format!("watched_json_test_{}", std::process::id()));
        let path = dir.join("settings.json");
        let _ = fs::remove_dir_all(&dir);

        // Eerste start: bestand wordt aangemaakt met standaardwaarden
        let mut w = WatchedJson::<Demo>::open(path.clone(), Duration::ZERO);
        assert!(path.exists());
        assert_eq!(w.get().level, 0.0);

        // Opslaan, dan "reboot": een nieuwe instantie leest de opgeslagen waarde
        assert!(w.set(Demo { level: -6.0 }));
        assert!(!w.poll(), "eigen schrijfactie is geen wijziging van buitenaf");
        let mut w = WatchedJson::<Demo>::open(path.clone(), Duration::ZERO);
        assert_eq!(w.get().level, -6.0);

        // Met de hand aangepast
        std::thread::sleep(Duration::from_millis(20));
        fs::write(&path, "{ \"level\": -3 }").unwrap();
        assert!(w.poll());
        assert_eq!(w.get().level, -3.0);

        // Tikfout: vorige waarde blijft, bestand wordt niet overschreven
        std::thread::sleep(Duration::from_millis(20));
        fs::write(&path, "{ \"level\": ").unwrap();
        assert!(!w.poll());
        assert_eq!(w.get().level, -3.0);
        let w2 = WatchedJson::<Demo>::open(path.clone(), Duration::ZERO);
        assert_eq!(w2.get().level, 0.0);
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ \"level\": ");

        let _ = fs::remove_dir_all(&dir);
    }
}

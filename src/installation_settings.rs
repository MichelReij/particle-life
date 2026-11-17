// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// src/installation_settings.rs
// Instellingen van de installatie (native simulatie op de Ubuntu) die je op locatie
// wilt kunnen bijstellen: het mengpaneel van de drone en het kleurenpalet van de
// deeltjestypes. De simulatie is de bron van waarheid; ze staan in
// ~/.config/origin-of-life/settings.json (zie watched_json.rs), bijvoorbeeld:
//
//   {
//     "audio": {
//       "mix_db": { "cellos": 0.0, "theremin": -3.0, ..., "master": 0.0 },
//       "lightning_crunch": 3
//     },
//     "colors": {
//       "types": [ "#71a58c", "#dbc27e", ... ],
//       "opacity": 0.8,
//       "screen_brightness": 1.0
//     }
//   }
//
// Dezelfde betekenis als het mengpaneel van de web-UI (src/audio-mixer.ts): dB per
// kanaal, -40 of lager = uit, max +12; crunch 0 = uit. Ontbrekende kanalen staan op
// 0 dB, ontbrekende secties op hun standaardwaarden — oudere bestanden blijven dus
// geldig als er later instellingen bijkomen. Zonder "colors" gelden de ingebouwde
// kleuren (DEFAULT_COLORS in particle_system.rs) en helderheid 1. Webpagina → hier:
// zie settings_protocol.rs.

use crate::dsp::mix;
use serde::de::Deserializer;
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct InstallationSettings {
    pub audio: AudioSettings,
    /// None = de ingebouwde kleuren
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<ColorSettings>,
}

/// Het kleurenpaneel: kleur per deeltjestype (sRGB als "#rrggbb", zoals de paletten
/// van de web-UI), de dekking van de deeltjes en de helderheid van het hele beeld
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ColorSettings {
    pub types: Vec<String>,
    pub opacity: f32,
    /// 1 = zoals ontworpen; lager in een donkere ruimte (omgevingslicht)
    #[serde(default = "default_brightness")]
    pub screen_brightness: f32,
}

fn default_brightness() -> f32 {
    1.0
}

impl ColorSettings {
    pub fn from_rgb(colors: &[[f32; 3]], opacity: f32, screen_brightness: f32) -> Self {
        Self { types: colors.iter().map(|c| rgb_to_hex(*c)).collect(), opacity, screen_brightness }
    }

    /// Kleur van type `i` (sRGB 0..1); None bij een ontbrekende of ongeldige waarde
    pub fn rgb(&self, i: usize) -> Option<[f32; 3]> {
        hex_to_rgb(self.types.get(i)?)
    }

    /// Korte samenvatting voor het log
    pub fn summary(&self) -> String {
        format!(
            "{}; dekking {:.2}; helderheid {:.2}",
            self.types.join(" "),
            self.opacity,
            self.screen_brightness
        )
    }
}

pub fn rgb_to_hex(c: [f32; 3]) -> String {
    let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", b(c[0]), b(c[1]), b(c[2]))
}

pub fn hex_to_rgb(hex: &str) -> Option<[f32; 3]> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let ch = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|v| v as f32 / 255.0);
    Some([ch(0)?, ch(2)?, ch(4)?])
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct AudioSettings {
    /// Faderstand per kanaal in dB
    pub mix_db: MixDb,
    /// Bliksem-crunch: 0 = uit, n = sample-and-hold over n+1 samples
    pub lightning_crunch: u32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            mix_db: MixDb::default(),
            lightning_crunch: mix::HOLD_DEFAULT - 1,
        }
    }
}

impl AudioSettings {
    /// Voor DroneHandle::set_mix: lineaire gains per kanaal + sample-and-hold
    pub fn gains(&self) -> [f32; mix::LEN + 1] {
        let mut g = [0.0; mix::LEN + 1];
        for (out, &db) in g.iter_mut().zip(self.mix_db.0.iter()) {
            *out = mix::db_to_gain(db);
        }
        g[mix::LIGHTNING_HOLD] = mix::crunch_to_hold(self.lightning_crunch);
        g
    }

    /// Korte samenvatting voor het log: alleen wat van 0 dB afwijkt
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = mix::KEYS
            .iter()
            .zip(self.mix_db.0.iter())
            .filter(|(_, &db)| db != 0.0)
            .map(|(k, &db)| {
                if db <= mix::MIN_DB {
                    format!("{k} uit")
                } else {
                    format!("{k} {db:+.1} dB")
                }
            })
            .collect();
        if parts.is_empty() {
            parts.push("alles 0 dB".into());
        }
        format!("{}; crunch {}", parts.join(", "), self.lightning_crunch)
    }
}

/// dB per mengpaneel-kanaal, in de volgorde van dsp::mix. In het bestand als
/// object met de kanaalnamen (mix::KEYS) als sleutels, in mengpaneelvolgorde.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MixDb(pub [f32; mix::LEN]);

impl Default for MixDb {
    fn default() -> Self {
        Self([0.0; mix::LEN])
    }
}

impl Serialize for MixDb {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(mix::LEN))?;
        for (key, db) in mix::KEYS.iter().zip(self.0.iter()) {
            map.serialize_entry(key, db)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for MixDb {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut read: HashMap<String, f32> = HashMap::deserialize(d)?;
        let mut db = MixDb::default();
        for (slot, key) in db.0.iter_mut().zip(mix::KEYS.iter()) {
            if let Some(v) = read.remove(*key) {
                *slot = v.clamp(mix::MIN_DB, mix::MAX_DB);
            }
        }
        if !read.is_empty() {
            let mut unknown: Vec<_> = read.into_keys().collect();
            unknown.sort();
            crate::console_log!(
                "⚠️ settings.json: onbekende kanalen {:?} genegeerd (bekend: {:?})",
                unknown,
                mix::KEYS
            );
        }
        Ok(db)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_defaults() {
        let mut s = InstallationSettings::default();
        s.audio.mix_db.0[mix::THEREMIN] = -3.5;
        s.audio.mix_db.0[mix::LIGHTNING] = mix::MIN_DB;
        let text = serde_json::to_string_pretty(&s).unwrap();
        // Sleutels in mengpaneelvolgorde, niet alfabetisch
        assert!(text.find("\"cellos\"").unwrap() < text.find("\"brainwaves\"").unwrap());
        let back: InstallationSettings = serde_json::from_str(&text).unwrap();
        assert_eq!(back, s);

        let g = back.audio.gains();
        assert_eq!(g[mix::CELLOS], 1.0);
        assert_eq!(g[mix::LIGHTNING], 0.0);
        assert!((g[mix::THEREMIN] - 10f32.powf(-3.5 / 20.0)).abs() < 1e-6);
        assert_eq!(g[mix::LIGHTNING_HOLD], mix::HOLD_DEFAULT as f32);
    }

    #[test]
    fn partial_file_keeps_defaults() {
        let s: InstallationSettings =
            serde_json::from_str(r#"{ "audio": { "mix_db": { "surf": -6, "bestaatniet": 3 } } }"#).unwrap();
        assert_eq!(s.audio.mix_db.0[mix::SURF], -6.0);
        assert_eq!(s.audio.mix_db.0[mix::CELLOS], 0.0);
        assert_eq!(s.audio.lightning_crunch, mix::HOLD_DEFAULT - 1);
        let empty: InstallationSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, InstallationSettings::default());
        assert!(!serde_json::to_string(&empty).unwrap().contains("colors"));
    }

    #[test]
    fn colors_hex() {
        let c = ColorSettings::from_rgb(&[[0.4437, 0.6471, 0.5490], [1.0, 0.0, 0.5]], 0.8, 1.0);
        assert_eq!(c.types, vec!["#71a58c", "#ff0080"]);
        assert_eq!(c.rgb(1), Some([1.0, 0.0, 128.0 / 255.0]));
        assert_eq!(c.rgb(2), None);
        assert_eq!(hex_to_rgb("71a58c"), None);
        assert_eq!(hex_to_rgb("#71a58"), None);
        assert_eq!(hex_to_rgb("#zz0000"), None);
        let s: InstallationSettings =
            serde_json::from_str(r##"{ "colors": { "types": ["#000000"], "opacity": 0.5 } }"##).unwrap();
        let colors = s.colors.unwrap();
        assert_eq!(colors.opacity, 0.5);
        assert_eq!(colors.screen_brightness, 1.0); // ontbreekt in het bestand → standaard
    }
}

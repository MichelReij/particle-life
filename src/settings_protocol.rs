// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// src/settings_protocol.rs
// Instellingen van de installatie over ESP-NOW: webpagina op de Mac → dev-server →
// OTA-master → ESP-NOW → liaison → UART-tunnel → deze simulatie, en het antwoord
// dezelfde weg terug (espnow-kit/tools/espnow_http.js, ESP32_UART_Protocol.h).
//
// Elk pakket (little-endian, zonder opvulling):
//   [msgType u16] [kind u8] [seq u8] [inhoud]
//   kind 1 = SET     pagina → simulatie: deze waarden gebruiken en bewaren
//        2 = STATE   simulatie → pagina: wat nu geldt (antwoord op SET en REQUEST)
//        3 = REQUEST pagina → simulatie: stuur je STATE (geen inhoud)
//   seq: gekozen door de pagina; het STATE-antwoord herhaalt hem
//
//   0x4F10 audio:   [kanalen n u8] [crunch u8] [dB × 10, i16] × n
//                   (kanalen in de volgorde van dsp::mix::KEYS)
//   0x4F11 kleuren: [types n u8] [dekking u8, 0..255] [schermhelderheid × 100, u8]
//                   [r u8] [g u8] [b u8] × n
//
// De liaison geeft alles met een msgType in TUNNEL_FIRST..=TUNNEL_LAST blind door
// (EspNowHelper.h): nieuwe instellingen vragen dus geen nieuwe liaison-firmware.
// Lengte nooit 24 bytes: dat is sizeof(EspNowMessage), wat de andere modules lezen.
// Tegenhanger op de pagina: src/installation-link.ts.

use crate::dsp::mix;
use crate::installation_settings::{AudioSettings, ColorSettings, MixDb};

pub const MSG_AUDIO: u16 = 0x4F10;
pub const MSG_COLORS: u16 = 0x4F11;
pub const TUNNEL_FIRST: u16 = 0x4F10;
pub const TUNNEL_LAST: u16 = 0x4F1F;

const KIND_SET: u8 = 1;
const KIND_STATE: u8 = 2;
const KIND_REQUEST: u8 = 3;
const HEADER: usize = 4;

#[derive(Debug, PartialEq)]
pub enum Incoming {
    SetAudio { seq: u8, audio: AudioSettings },
    SetColors { seq: u8, colors: ColorSettings },
    /// Stuur de STATE van `msg_type`
    Request { seq: u8, msg_type: u16 },
}

/// None = geen (geldig) instellingenpakket voor de simulatie, bijv. de STATE van
/// een andere installatie
pub fn parse(p: &[u8]) -> Option<Incoming> {
    if p.len() < HEADER {
        return None;
    }
    let msg_type = u16::from_le_bytes([p[0], p[1]]);
    let (kind, seq, body) = (p[2], p[3], &p[HEADER..]);
    match (msg_type, kind) {
        (MSG_AUDIO | MSG_COLORS, KIND_REQUEST) => Some(Incoming::Request { seq, msg_type }),
        (MSG_AUDIO, KIND_SET) => parse_audio(body).map(|audio| Incoming::SetAudio { seq, audio }),
        (MSG_COLORS, KIND_SET) => parse_colors(body).map(|colors| Incoming::SetColors { seq, colors }),
        _ => None,
    }
}

fn parse_audio(b: &[u8]) -> Option<AudioSettings> {
    let (&n, rest) = b.split_first()?;
    let (&crunch, values) = rest.split_first()?;
    if values.len() != n as usize * 2 {
        return None;
    }
    let mut mix_db = MixDb::default();
    for (slot, v) in mix_db.0.iter_mut().zip(values.chunks_exact(2)) {
        *slot = (i16::from_le_bytes([v[0], v[1]]) as f32 / 10.0).clamp(mix::MIN_DB, mix::MAX_DB);
    }
    Some(AudioSettings {
        mix_db,
        lightning_crunch: (crunch as u32).min(mix::HOLD_MAX as u32 - 1),
    })
}

fn parse_colors(b: &[u8]) -> Option<ColorSettings> {
    let (&n, rest) = b.split_first()?;
    let (&opacity, rest) = rest.split_first()?;
    let (&brightness, rgb) = rest.split_first()?;
    if rgb.len() != n as usize * 3 {
        return None;
    }
    Some(ColorSettings {
        types: rgb.chunks_exact(3).map(|c| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])).collect(),
        // Op 3 decimalen (leesbaar in settings.json); terug naar een byte geeft dezelfde waarde
        opacity: (opacity as f32 / 255.0 * 1000.0).round() / 1000.0,
        screen_brightness: (brightness as f32 / 100.0).min(MAX_BRIGHTNESS),
    })
}

/// Bovengrens van de schermhelderheid (1 = zoals ontworpen)
const MAX_BRIGHTNESS: f32 = 2.0;

fn header(msg_type: u16, kind: u8, seq: u8) -> Vec<u8> {
    let mut p = Vec::with_capacity(48);
    p.extend_from_slice(&msg_type.to_le_bytes());
    p.push(kind);
    p.push(seq);
    p
}

/// STATE-pakket met het mengpaneel zoals het nu geldt
pub fn audio_state(seq: u8, a: &AudioSettings) -> Vec<u8> {
    let mut p = header(MSG_AUDIO, KIND_STATE, seq);
    p.push(mix::LEN as u8);
    p.push(a.lightning_crunch.min(u8::MAX as u32) as u8);
    for db in a.mix_db.0 {
        p.extend_from_slice(&((db * 10.0).round() as i16).to_le_bytes());
    }
    p
}

/// STATE-pakket met de kleuren zoals ze nu gelden
pub fn colors_state(seq: u8, c: &ColorSettings) -> Vec<u8> {
    let mut p = header(MSG_COLORS, KIND_STATE, seq);
    p.push(c.types.len().min(u8::MAX as usize) as u8);
    p.push((c.opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
    p.push((c.screen_brightness.clamp(0.0, MAX_BRIGHTNESS) * 100.0).round() as u8);
    for i in 0..c.types.len().min(u8::MAX as usize) {
        for x in c.rgb(i).unwrap_or([0.0; 3]) {
            p.push((x * 255.0).round() as u8);
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Een STATE-pakket als SET terugsturen, zoals de pagina dat doet
    fn as_set(mut p: Vec<u8>, seq: u8) -> Vec<u8> {
        p[2] = KIND_SET;
        p[3] = seq;
        p
    }

    #[test]
    fn audio_roundtrip() {
        let mut a = AudioSettings::default();
        a.mix_db.0[mix::THEREMIN] = -3.5;
        a.mix_db.0[mix::LIGHTNING] = mix::MIN_DB;
        a.mix_db.0[mix::MASTER] = 12.0;
        a.lightning_crunch = 7;
        let state = audio_state(9, &a);
        assert_eq!(state.len(), 6 + 2 * mix::LEN);
        assert_ne!(state.len(), 24);
        assert_eq!(&state[..4], &[0x10, 0x4F, KIND_STATE, 9]);
        assert_eq!(parse(&state), None); // een STATE is niet voor de simulatie
        assert_eq!(parse(&as_set(state, 42)), Some(Incoming::SetAudio { seq: 42, audio: a }));
    }

    #[test]
    fn colors_roundtrip() {
        let c = ColorSettings {
            types: vec!["#71a58c".into(), "#ff0080".into()],
            opacity: 0.8,
            screen_brightness: 0.65,
        };
        let state = colors_state(3, &c);
        assert_eq!(state.len(), 7 + 3 * 2);
        assert_eq!(parse(&as_set(state, 4)), Some(Incoming::SetColors { seq: 4, colors: c }));
        // 11 types: nooit de 24 bytes van EspNowMessage
        let eleven = ColorSettings { types: vec!["#000000".into(); 11], opacity: 1.0, screen_brightness: 1.0 };
        assert_eq!(colors_state(0, &eleven).len(), 40);
        // Te hel wordt begrensd
        let mut bright = as_set(colors_state(0, &eleven), 1);
        bright[6] = 255;
        let Some(Incoming::SetColors { colors, .. }) = parse(&bright) else { panic!() };
        assert_eq!(colors.screen_brightness, MAX_BRIGHTNESS);
    }

    #[test]
    fn request_and_garbage() {
        assert_eq!(parse(&[0x11, 0x4F, KIND_REQUEST, 5]), Some(Incoming::Request { seq: 5, msg_type: MSG_COLORS }));
        assert_eq!(parse(&[0x10, 0x4F, KIND_SET]), None);
        assert_eq!(parse(&[0x10, 0x4F, KIND_SET, 1, 14, 3, 0]), None); // te kort voor 14 kanalen
        assert_eq!(parse(&[0x12, 0x4F, KIND_REQUEST, 1]), None);
        // Te hard en te zacht wordt begrensd
        let set = [0x10, 0x4F, KIND_SET, 1, 2, 200, 0xE8, 0x03, 0x18, 0xFC];
        let Some(Incoming::SetAudio { audio, .. }) = parse(&set) else { panic!() };
        assert_eq!(audio.mix_db.0[0], mix::MAX_DB);
        assert_eq!(audio.mix_db.0[1], mix::MIN_DB);
        assert_eq!(audio.mix_db.0[2], 0.0);
        assert_eq!(audio.lightning_crunch, 15);
    }
}

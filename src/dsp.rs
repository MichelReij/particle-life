// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// src/dsp.rs
// Ambient drone in C-mineur pentatonisch (C Eb F G Bb), opgebouwd uit funDSP-bouwstenen.
//
// Lagen:
//   1. Cello's      — twee gestreken zaagtanden met strijkruis, kastresonantie en
//                     boog-gestuurde tanh-saturatie (de rasp). Wisselen elkaar af
//                     met lange streken: grondtoon C/G laag, tweede stem beweegt.
//   2. Theremin     — sinus met portamento en vertraagd invallende vibrato, speelt
//                     losse frasen als random walk over de pentatoniek.
//   3. Brainwaves   — zachte sinus-pad C3/G3/Bb3 met binaurale zweving (L/R
//                     frequentieverschil), ademende amplitude per toon.
//   4. Kabbelen     — gestemde waterdruppels (opwaartse chirp, snel uitstervend) en
//                     een zacht ruisend stroompje.
//   5. Reverb       — grote FDN-hal (reverb_stereo) over alle lagen.
//
// De temperatuur (genormaliseerd 0..1) bepaalt het tempo: streeklengte, frasering,
// ademhaling, druppeldichtheid, vibrato-snelheid en de binaurale zweving
// (koud ≈ delta 2 Hz, heet ≈ alfa 9 Hz).
//
// Boven 3× zoom komen daar de clusterstatistieken bij (zie DroneParams):
//   chaos  → cello's strijken harder, ruwer en onvaster (rasp, haperende boog,
//            verstemming), theremin springt grillig, druppels raken uit de toon
//   motion → meer druppels en een voller stroompje
//   calm   → langere, zachtere theremin-frasen
//   pan    → cello's rond het zwaartepunt, theremin volgt het beweeglijkste type
//
// In de getijdenpoel-hypothese (WLP) komt er branding bij: golven die aanzwellen,
// breken en terugtrekken, met een periode die losjes het tempo volgt.

use fundsp::prelude32::*;

pub const SAMPLE_RATE: u32   = 44100;
pub const BLOCK_SIZE:  usize = 512;

const TAU: f32 = std::f32::consts::TAU;

/// Aantal samples tussen twee control-rate updates (envelopes, sequencing).
const CONTROL_INTERVAL: u32 = 32;

/// C-mineur pentatonisch, halve tonen boven C.
const PENTA: [i32; 5] = [0, 3, 5, 7, 10];
/// MIDI-noot van pentatonische trede 0 (C2).
const PENTA_BASE_MIDI: i32 = 36;

#[inline]
fn midi_hz(midi: f32) -> f32 {
    440.0 * 2.0f32.powf((midi - 69.0) / 12.0)
}

/// Pentatonische trede → frequentie. Trede 0 = C2, 5 = C3, 10 = C4, 15 = C5.
#[inline]
fn degree_hz(d: i32) -> f32 {
    let oct = d.div_euclid(5);
    let step = d.rem_euclid(5) as usize;
    midi_hz((PENTA_BASE_MIDI + 12 * oct + PENTA[step]) as f32)
}

/// Equal-power panning: -1 = links, 1 = rechts.
#[inline]
fn pan_gains(pan: f32) -> (f32, f32) {
    let a = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    (a.cos(), a.sin())
}

fn prepare(mut unit: Box<dyn AudioUnit>, sample_rate: f32, seed: u64) -> Box<dyn AudioUnit> {
    unit.set_sample_rate(sample_rate as f64);
    unit.ping(false, AttoHash::new(seed));
    unit.allocate();
    unit
}

// ─── Kleine deterministische RNG (xorshift32) ────────────────────────────────

struct Rng(u32);

impl Rng {
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// Uniform [0, 1)
    fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[(self.next_u32() as usize) % items.len()]
    }
}

// ─── Besturing ───────────────────────────────────────────────────────────────

/// Besturing vanuit de simulatie. Met alleen de temperatuur gezet en de rest op 0
/// klinkt de drone "algemeen"; de clusterwaarden mengt sonification.rs pas boven
/// 3× zoom in, geschaald met gpu_blend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DroneParams {
    /// Temperatuur [0, 1] → tempo
    pub temperature: f32,
    /// Energie zonder structuur [0, 1] → boogdruk en rasp op de cello's
    pub chaos: f32,
    /// Gemiddelde beweeglijkheid [0, 1] → meer druppels en stroming
    pub motion: f32,
    /// Stabiele clusters [0, 1] → langere, zachtere theremin-frasen
    pub calm: f32,
    /// Zwaartepunt van alle deeltjes in beeld [-1, 1] → panning cello's
    pub pan_center: f32,
    /// Positie van het beweeglijkste type [-1, 1] → panning theremin
    pub pan_lead: f32,
    /// Hypothese: 0 = diepzee (HTV), 1 = getijdenpoel (WLP) → branding
    pub surface: f32,
}

impl Default for DroneParams {
    fn default() -> Self {
        Self { temperature: 0.5, chaos: 0.0, motion: 0.0, calm: 0.0, pan_center: 0.0, pan_lead: 0.0, surface: 0.0 }
    }
}

impl DroneParams {
    pub const LEN: usize = 7;

    /// Volgorde zoals verstuurd naar de AudioWorklet.
    pub fn to_array(&self) -> [f32; Self::LEN] {
        [self.temperature, self.chaos, self.motion, self.calm, self.pan_center, self.pan_lead, self.surface]
    }

    pub fn from_slice(v: &[f32]) -> Option<Self> {
        let v: &[f32; Self::LEN] = v.get(..Self::LEN)?.try_into().ok()?;
        Some(Self {
            temperature: v[0],
            chaos:       v[1],
            motion:      v[2],
            calm:        v[3],
            pan_center:  v[4],
            pan_lead:    v[5],
            surface:     v[6],
        })
    }

    fn clamped(&self) -> Self {
        Self {
            temperature: self.temperature.clamp(0.0, 1.0),
            chaos:       self.chaos.clamp(0.0, 1.0),
            motion:      self.motion.clamp(0.0, 1.0),
            calm:        self.calm.clamp(0.0, 1.0),
            pan_center:  self.pan_center.clamp(-1.0, 1.0),
            pan_lead:    self.pan_lead.clamp(-1.0, 1.0),
            surface:     self.surface.clamp(0.0, 1.0),
        }
    }
}

/// Thread-safe handle om de parameters van een draaiende DroneSynth te zetten.
#[derive(Clone)]
pub struct DroneHandle([Shared; DroneParams::LEN]);

impl DroneHandle {
    fn new(p: &DroneParams) -> Self {
        Self(p.to_array().map(shared))
    }

    pub fn set(&self, p: &DroneParams) {
        for (s, v) in self.0.iter().zip(p.clamped().to_array()) {
            s.set_value(v);
        }
    }

    fn get(&self) -> DroneParams {
        DroneParams::from_slice(&self.0.each_ref().map(|s| s.value())).unwrap_or_default()
    }
}

// ─── 1. Cello ────────────────────────────────────────────────────────────────

/// Boogstreek-envelope over x ∈ [0, 1]: zwelt aan, zakt licht in, sterft uit.
fn bow_envelope(x: f32) -> f32 {
    if x < 0.4 {
        (x / 0.4 * std::f32::consts::FRAC_PI_2).sin().powi(2)
    } else if x < 0.7 {
        1.0 - 0.15 * (x - 0.4) / 0.3
    } else {
        0.85 * ((x - 0.7) / 0.3 * std::f32::consts::FRAC_PI_2).cos().powi(2)
    }
}

/// Basis-saturatie en strijkruis zonder chaos.
const CELLO_DRIVE: f32 = 1.4;
const CELLO_GRIT:  f32 = 0.35;

struct Cello {
    net:        Box<dyn AudioUnit>,
    freq:       Shared,
    bow:        Shared,
    drive:      Shared,
    grit:       Shared,
    /// Relatieve toonhoogte-afwijking door onvast strijken (chaos)
    wobble:     Shared,
    notes:      &'static [i32],
    stroke_pos: f32,
    stroke_len: f32,
    /// Basislengte van een streek in seconden bij tempo 1
    base_len:   f32,
    /// Stereopositie ten opzichte van het zwaartepunt van de deeltjes
    pan_offset: f32,
    chaos:      f32,
    /// Willekeurige doelen [-1, 1] voor haperende boogdruk en toonhoogte
    flutter:    f32,
    drift:      f32,
}

impl Cello {
    fn new(sample_rate: f32, seed: u64, notes: &'static [i32], base_len: f32, pan_offset: f32, phase: f32) -> Self {
        let freq  = shared(degree_hz(notes[0]));
        let bow   = shared(0.0);
        let drive = shared(CELLO_DRIVE);
        let grit  = shared(CELLO_GRIT);
        let wobble = shared(0.0);

        // Licht zwevende toonhoogte: trage vibrato + onregelmatige boog-jitter,
        // bij chaos aangevuld met grillig verstemmen (wobble)
        let pitch = (var(&freq) >> follow(0.3))
            * (1.0 + 0.003 * sine_hz(4.7) + 0.002 * (noise() >> lowpole_hz(6.0))
                + (var(&wobble) >> follow(0.06)));
        // Strijkruis: amplitude-ruis op de zaagtand geeft de korrel van hars op snaar
        let bowed = (pitch >> saw()) * (1.0 + (var(&grit) >> follow(0.5)) * (noise() >> lowpole_hz(1500.0)));
        // Kast: lucht-, hout- en boogresonantie. Net als bij een echte cello is de
        // grondtoon zwak (highpass) en dragen de boventonen de klank.
        let body = bowed
            >> lowpass_hz(2200.0, 0.6)
            >> bell_hz(250.0, 1.2, 2.2)
            >> bell_hz(560.0, 1.8, 1.6)
            >> bell_hz(1500.0, 1.5, 1.3)
            >> highpass_hz(110.0, 0.6);
        // Boogdruk stuurt zowel volume als saturatie — harder strijken = meer rasp
        let graph = body * (var(&bow) >> follow(0.05)) * (var(&drive) >> follow(0.5)) >> shape(Tanh(1.0));

        Self {
            net: prepare(Box::new(graph), sample_rate, seed),
            freq,
            bow,
            drive,
            grit,
            wobble,
            notes,
            stroke_pos: phase * base_len,
            stroke_len: base_len,
            base_len,
            pan_offset,
            chaos: 0.0,
            flutter: 0.0,
            drift: 0.0,
        }
    }

    /// Chaos in de simulatie = harder, ruwer en onvaster strijken.
    fn set_chaos(&mut self, chaos: f32) {
        self.chaos = chaos;
        self.drive.set_value(CELLO_DRIVE * (1.0 + 3.0 * chaos));
        self.grit.set_value(CELLO_GRIT + 1.0 * chaos);
    }

    /// Stereopositie: rond het zwaartepunt, met minder spreiding naarmate dat
    /// verder naar de rand ligt.
    fn pan(&self, center: f32) -> f32 {
        (center + self.pan_offset * (1.0 - 0.4 * center.abs())).clamp(-1.0, 1.0)
    }

    fn control(&mut self, dt: f32, tempo: f32, rng: &mut Rng) {
        let chaos = self.chaos;
        self.stroke_pos += dt;
        if self.stroke_pos >= self.stroke_len {
            self.stroke_pos = 0.0;
            // Chaos: kortere en grilliger streken
            let jitter = rng.range(0.8 - 0.4 * chaos, 1.25);
            self.stroke_len = self.base_len * jitter * (1.0 - 0.5 * chaos) / tempo;
            // Grondtoon blijft vaker liggen dan dat hij beweegt — tenzij het chaotisch is
            let note = if rng.unit() < 0.45 * (1.0 - chaos) { self.notes[0] } else { rng.pick(self.notes) };
            self.freq.set_value(degree_hz(note));
        }

        // Haperende boog: om de ~80 ms een nieuw willekeurig doel voor druk en toonhoogte
        if rng.unit() < dt * 12.0 {
            self.flutter = rng.range(-1.0, 1.0);
        }
        if rng.unit() < dt * 5.0 {
            self.drift = rng.range(-1.0, 1.0);
        }
        // ±25 cent verstemming bij volle chaos
        self.wobble.set_value(0.015 * chaos * self.drift);

        let env = bow_envelope(self.stroke_pos / self.stroke_len);
        self.bow.set_value(env * (1.0 + 0.5 * chaos * self.flutter).max(0.0));
    }
}

// ─── 2. Theremin ─────────────────────────────────────────────────────────────

/// Bereik van de theremin in pentatonische treden (G3 .. Eb5).
const THEREMIN_LOW:  i32 = 8;
const THEREMIN_HIGH: i32 = 16;

struct Theremin {
    net:        Box<dyn AudioUnit>,
    freq:       Shared,
    amp:        Shared,
    vib_depth:  Shared,
    vib_rate:   Shared,
    degree:     i32,
    notes_left: u32,
    timer:      f32,
    since_note: f32,
    pan_phase:  f32,
}

impl Theremin {
    fn new(sample_rate: f32, seed: u64) -> Self {
        let degree    = 10;
        let freq      = shared(degree_hz(degree));
        let amp       = shared(0.0);
        let vib_depth = shared(0.0);
        let vib_rate  = shared(5.2);

        // Portamento via follow(), vibrato-diepte zwelt pas na aankomst op de noot
        let pitch = (var(&freq) >> follow(0.16))
            * (1.0 + (var(&vib_depth) >> follow(0.5)) * (var(&vib_rate) >> sine()));
        // Zachte tanh op een sinus voegt wat oneven boventonen toe — warmer dan puur sinus
        let tone = pitch >> sine() >> shape(Tanh(1.6)) >> lowpass_hz(2800.0, 0.6);
        let graph = tone * (var(&amp) >> follow(0.35));

        Self {
            net: prepare(Box::new(graph), sample_rate, seed),
            freq,
            amp,
            vib_depth,
            vib_rate,
            degree,
            notes_left: 0,
            timer: 6.0,
            since_note: 0.0,
            pan_phase: 0.0,
        }
    }

    /// `calm`: stabiele clusters rekken noten en rusten op en maken de frasen zachter.
    /// `chaos`: grote sprongen, korte noten, wijde en snelle vibrato.
    fn control(&mut self, dt: f32, tempo: f32, warmth: f32, calm: f32, chaos: f32, rng: &mut Rng) {
        self.timer -= dt;
        self.since_note += dt;
        self.pan_phase = (self.pan_phase + dt * 0.03 * tempo * TAU) % TAU;
        self.vib_rate.set_value(4.6 + 1.6 * warmth + 2.5 * chaos);

        // Vertraagde vibrato: pas 0.4 s na een nieuwe noot invallen
        let depth = if self.amp.value() > 0.0 && self.since_note > 0.4 { 0.0055 * (1.0 + 2.0 * chaos) } else { 0.0 };
        self.vib_depth.set_value(depth);

        if self.timer > 0.0 {
            return;
        }

        if self.notes_left == 0 {
            // Einde van een rust → nieuwe frase beginnen
            if self.amp.value() == 0.0 {
                self.notes_left = 3 + rng.next_u32() % 5;
            } else {
                // Einde van een frase → rust
                self.amp.set_value(0.0);
                self.timer = rng.range(4.0, 11.0) * (1.0 + 0.5 * calm) * (1.0 - 0.5 * chaos) / tempo;
                return;
            }
        }

        // Random walk over de pentatoniek, met terugkaatsing aan de randen
        let step = if rng.unit() < 0.6 * chaos {
            rng.pick(&[-5, -4, -3, 3, 4, 5])
        } else {
            rng.pick(&[-2, -1, -1, 1, 1, 2])
        };
        let mut d = self.degree + step;
        if d < THEREMIN_LOW || d > THEREMIN_HIGH {
            d = self.degree - step;
        }
        self.notes_left -= 1;
        if self.notes_left == 0 {
            // Laatste noot van de frase landt op een akkoordtoon (C of G)
            while d.rem_euclid(5) != 0 && d.rem_euclid(5) != 3 {
                d += if d > 12 { -1 } else { 1 };
            }
        }
        self.degree = d.clamp(THEREMIN_LOW, THEREMIN_HIGH);
        self.freq.set_value(degree_hz(self.degree));
        self.amp.set_value(rng.range(0.75, 1.0) * (1.0 - 0.4 * calm));
        self.since_note = 0.0;

        let beats = rng.pick(&[1.5, 2.0, 2.0, 3.0, 4.0]);
        self.timer = beats * 1.3 * (1.0 + calm) * (1.0 - 0.5 * chaos) / tempo;
    }

    /// Volgt het beweeglijkste type; zonder clusterdata een trage zwerftocht rond het midden.
    fn pan(&self, lead: f32) -> f32 {
        (lead + 0.45 * (1.0 - lead.abs()) * self.pan_phase.sin()).clamp(-1.0, 1.0)
    }
}

// ─── 3. Brainwave-pad ────────────────────────────────────────────────────────

const PAD_MIDI:    [f32; 3] = [48.0, 55.0, 58.0]; // C3, G3, Bb3
const PAD_PERIODS: [f32; 3] = [17.0, 13.0, 21.0]; // ademperiode in s bij tempo 1

struct PadTone {
    net:     Box<dyn AudioUnit>,
    freq_l:  Shared,
    freq_r:  Shared,
    base:    f32,
    period:  f32,
    phase:   f32,
}

impl PadTone {
    fn new(sample_rate: f32, seed: u64, midi: f32, period: f32, phase: f32) -> Self {
        let base   = midi_hz(midi);
        let freq_l = shared(base);
        let freq_r = shared(base);
        let voice = |f: &Shared| var(f) >> follow(1.5) >> sine() >> shape(Tanh(1.2));
        let graph = voice(&freq_l) | voice(&freq_r);
        Self {
            net: prepare(Box::new(graph), sample_rate, seed),
            freq_l,
            freq_r,
            base,
            period,
            phase,
        }
    }

    /// Retourneert de huidige amplitude van deze toon.
    fn control(&mut self, dt: f32, tempo: f32, beat_hz: f32) -> f32 {
        self.phase = (self.phase + dt * tempo / self.period * TAU) % TAU;
        self.freq_l.set_value(self.base - beat_hz * 0.5);
        self.freq_r.set_value(self.base + beat_hz * 0.5);
        0.55 + 0.45 * self.phase.sin()
    }
}

// ─── 4. Kabbelen ─────────────────────────────────────────────────────────────

/// Bereik van de druppels in pentatonische treden (C5 .. G6).
const DROP_LOW:  i32 = 15;
const DROP_HIGH: i32 = 23;
const MAX_DROPS: usize = 8;

#[derive(Clone, Copy, Default)]
struct Drop {
    active: bool,
    phase:  f32,
    freq:   f32,
    chirp:  f32, // relatieve frequentiestijging per seconde
    amp:    f32,
    decay:  f32, // amplitude-factor per sample
    gain_l: f32,
    gain_r: f32,
}

// ─── 5. Branding (alleen getijdenpoel / WLP) ─────────────────────────────────

/// Golfvorm over x ∈ [0, 1]: (massa-amplitude, cutoff in Hz, schuim-amplitude).
/// Aanzwellen tot ~55% — een gelijkmatig crescendo van ~17 dB dat ook steeds
/// helderder wordt (600 → 2400 Hz), zodat je de golf hoort naderen — dan breken
/// (korte heldere piek met schuim) en terugtrekken (wash). De ondergrens blijft
/// hoorbaar: tussen de golven door ruist de zee zacht door.
fn wave_shape(x: f32) -> (f32, f32, f32) {
    const CRASH: f32 = 0.55;
    if x < CRASH {
        let t = (x / CRASH).powf(1.4);
        (0.25 + 0.75 * t, 600.0 * 4.0f32.powf(t), 0.1 * t)
    } else {
        let t = (x - CRASH) / (1.0 - CRASH);
        let hit  = (-t * 18.0).exp();
        let wash = (-t * 3.5).exp();
        (
            0.25 + 0.75 * wash + 0.4 * hit,
            600.0 * 4.0f32.powf(wash) + 2500.0 * hit,
            0.8 * hit + 0.5 * wash * (1.0 - hit),
        )
    }
}

struct Surf {
    net:      Box<dyn AudioUnit>,
    body_amp: Shared,
    cutoff:   Shared,
    foam_amp: Shared,
    pos:      f32,
    len:      f32,
    /// Grootte van de huidige golf [0.35, 1]
    size:     f32,
    pan:      f32,
}

impl Surf {
    fn new(sample_rate: f32, seed: u64) -> Self {
        let body_amp = shared(0.0);
        let cutoff   = shared(250.0);
        let foam_amp = shared(0.0);

        // Eén kanaal: golfmassa (roze ruis, meebewegende lowpass) plus sissend schuim
        // (witte ruis in de hoogte). Twee kanalen met eigen ruis = breed. De highpass
        // houdt de massa uit het register van cello's en pad, waar hij anders verdrinkt.
        let channel = || {
            let body = (pink() | (var(&cutoff) >> follow(0.15)) | dc(0.6)) >> lowpass()
                >> highpass_hz(220.0, 0.6);
            let foam = noise() >> highpass_hz(2500.0, 0.6) >> lowpass_hz(7000.0, 0.5);
            body * 6.0 * (var(&body_amp) >> follow(0.12)) + foam * (var(&foam_amp) >> follow(0.08))
        };
        let graph = channel() | channel();

        Self {
            net: prepare(Box::new(graph), sample_rate, seed),
            body_amp,
            cutoff,
            foam_amp,
            pos: 0.0,
            len: 9.0,
            size: 0.8,
            pan: 0.0,
        }
    }

    fn control(&mut self, dt: f32, tempo: f32, rng: &mut Rng) {
        self.pos += dt;
        if self.pos >= self.len {
            self.pos = 0.0;
            // De zee volgt het tempo maar slap: ~20 s per golf bij koud, ~8 s bij heet
            self.len = rng.range(7.0, 13.0) / tempo.powf(0.3);
            // Af en toe een kleiner golfje tussen de grote
            self.size = if rng.unit() < 0.25 { rng.range(0.35, 0.6) } else { rng.range(0.7, 1.0) };
            self.pan = rng.range(-0.6, 0.6);
        }
        let (body, cutoff, foam) = wave_shape(self.pos / self.len);
        self.body_amp.set_value(body * self.size);
        self.cutoff.set_value(cutoff * (0.7 + 0.3 * self.size));
        self.foam_amp.set_value(foam * self.size);
    }
}

// ─── DroneSynth ──────────────────────────────────────────────────────────────

/// Mengniveau van de branding.
const SURF_LEVEL: f32 = 0.22;

pub struct DroneSynth {
    sample_rate: f32,
    rng:         Rng,

    cellos:   [Cello; 2],
    theremin: Theremin,
    pad:      [PadTone; 3],
    pad_amp:  [f32; 3],
    drops:    [Drop; MAX_DROPS],
    murmur:     Box<dyn AudioUnit>,
    murmur_amp: Shared,
    surf:     Surf,
    reverb:   Box<dyn AudioUnit>,

    /// Doelwaarden, schrijfbaar vanuit andere threads
    handle: DroneHandle,
    /// Afgevlakte parameters — sprongen in de simulatie worden glijdende overgangen
    smooth: DroneParams,
    burst_phase: f32,

    control_counter: u32,
    fade_in:         f32,
    // Per-sample gains, bijgewerkt op control-rate
    cello_gains: [(f32, f32); 2],
    theremin_gain: (f32, f32),
}

impl DroneSynth {
    pub fn new(sample_rate: f32) -> Self {
        // Cello 1: grondtoon C2 / G1 / C3. Cello 2: beweeglijkere middenstem.
        const CELLO_LOW:  [i32; 3] = [0, -2, 5];
        const CELLO_HIGH: [i32; 5] = [3, 6, 4, 5, 7];

        let murmur_amp = shared(0.0);
        let murmur = (pink() >> lowpass_hz(900.0, 0.5) >> highpass_hz(180.0, 0.5))
            * (var(&murmur_amp) >> follow(1.0))
            >> split::<U2>();

        Self {
            sample_rate,
            rng: Rng(0x9E37_79B9),
            cellos: [
                Cello::new(sample_rate, 11, &CELLO_LOW,  10.0, -0.3, 0.0),
                Cello::new(sample_rate, 23, &CELLO_HIGH,  8.0,  0.35, 0.5),
            ],
            theremin: Theremin::new(sample_rate, 37),
            pad: std::array::from_fn(|i| {
                PadTone::new(sample_rate, 50 + i as u64, PAD_MIDI[i], PAD_PERIODS[i], i as f32 * 2.1)
            }),
            pad_amp: [0.0; 3],
            drops: [Drop::default(); MAX_DROPS],
            murmur: prepare(Box::new(murmur), sample_rate, 71),
            murmur_amp,
            surf: Surf::new(sample_rate, 97),
            // Highpass vóór de hal houdt de galmstaart helder in plaats van dreunend
            reverb: prepare(
                Box::new((highpass_hz(200.0, 0.6) | highpass_hz(200.0, 0.6)) >> reverb_stereo(22.0, 7.0, 0.55)),
                sample_rate,
                83,
            ),
            handle: DroneHandle::new(&DroneParams::default()),
            smooth: DroneParams::default(),
            burst_phase: 0.0,
            control_counter: 0,
            fade_in: 0.0,
            cello_gains: [(0.0, 0.0); 2],
            theremin_gain: (0.0, 0.0),
        }
    }

    /// Handle om de parameters thread-safe te zetten vanuit een andere thread.
    pub fn handle(&self) -> DroneHandle {
        self.handle.clone()
    }

    pub fn set_params(&mut self, p: &DroneParams) {
        self.handle.set(p);
    }

    /// Alleen de temperatuur zetten (0 = koudst, 1 = heetst); overige parameters blijven.
    pub fn set_temperature(&mut self, norm: f32) {
        let p = DroneParams { temperature: norm, ..self.handle.get() };
        self.handle.set(&p);
    }

    /// Tempofactor: 0.1 bij de koudste temperatuur, ~0.47 halverwege, 2.2 op het heetst.
    fn tempo(&self) -> f32 {
        0.1 * 22.0f32.powf(self.smooth.temperature)
    }

    fn control(&mut self) {
        let dt = CONTROL_INTERVAL as f32 / self.sample_rate;

        // Temperatuur traag volgen zodat tempowissels geleidelijk verlopen (~4 s);
        // clusterwaarden iets sneller (~1.5 s) zodat ze nog hoorbaar reageren.
        let target = self.handle.get();
        let (k_slow, k_fast) = ((dt / 4.0).min(1.0), (dt / 1.5).min(1.0));
        let s = &mut self.smooth;
        s.temperature += (target.temperature - s.temperature) * k_slow;
        s.chaos       += (target.chaos       - s.chaos)       * k_fast;
        s.motion      += (target.motion      - s.motion)      * k_fast;
        s.calm        += (target.calm        - s.calm)        * k_fast;
        s.pan_center  += (target.pan_center  - s.pan_center)  * k_fast;
        s.pan_lead    += (target.pan_lead    - s.pan_lead)    * k_fast;
        // Hypothesewissel: branding in ~3 s in- of uitfaden
        s.surface     += (target.surface     - s.surface)     * (dt / 3.0).min(1.0);
        let p = *s;
        let tempo = self.tempo();
        let warmth = p.temperature;

        for (i, cello) in self.cellos.iter_mut().enumerate() {
            cello.set_chaos(p.chaos);
            cello.control(dt, tempo, &mut self.rng);
            self.cello_gains[i] = pan_gains(cello.pan(p.pan_center));
        }

        self.theremin.control(dt, tempo, warmth, p.calm, p.chaos, &mut self.rng);
        self.theremin_gain = pan_gains(self.theremin.pan(p.pan_lead));

        // Binaurale zweving: delta (2 Hz) bij koud tot alfa (9 Hz) bij heet
        let beat_hz = 2.0 + 7.0 * warmth;
        for (i, tone) in self.pad.iter_mut().enumerate() {
            self.pad_amp[i] = tone.control(dt, tempo, beat_hz);
        }

        // Druppels komen in vlagen: een trage golf moduleert de dichtheid
        self.burst_phase = (self.burst_phase + dt * 0.07 * tempo * TAU) % TAU;
        let burst = 0.5 + 0.5 * self.burst_phase.sin();
        // Beweeglijke deeltjes geven tot 4× zoveel druppels en een voller stroompje
        let rate = (0.25 + 3.0 * warmth) * (0.2 + 1.6 * burst * burst) * (1.0 + 3.0 * p.motion);
        if self.rng.unit() < rate * dt {
            self.spawn_drop();
        }
        self.murmur_amp.set_value(0.25 + 0.75 * warmth * burst + 0.5 * p.motion);

        if p.surface > 0.001 {
            self.surf.control(dt, tempo, &mut self.rng);
        }

        if self.fade_in < 1.0 {
            self.fade_in = (self.fade_in + dt / 4.0).min(1.0);
        }
    }

    fn spawn_drop(&mut self) {
        let Some(slot) = self.drops.iter_mut().find(|d| !d.active) else { return };
        let degree = DROP_LOW + (self.rng.next_u32() % (DROP_HIGH - DROP_LOW + 1) as u32) as i32;
        let duration = self.rng.range(0.04, 0.14);
        let (l, r) = pan_gains(self.rng.range(-0.8, 0.8));
        // Chaos: tot een halve toon naast de toonladder, en grilliger op- én neerwaartse glijders
        let chaos = self.smooth.chaos;
        let detune = 2.0f32.powf(self.rng.range(-1.0, 1.0) * chaos / 12.0);
        *slot = Drop {
            active: true,
            phase:  0.0,
            freq:   degree_hz(degree) * detune,
            chirp:  self.rng.range(0.8 - 3.0 * chaos, 2.5 + 3.0 * chaos),
            amp:    self.rng.range(0.3, 1.0),
            decay:  (-1.0 / (duration * self.sample_rate)).exp(),
            gain_l: l,
            gain_r: r,
        };
    }

    #[inline]
    fn render_drops(&mut self) -> (f32, f32) {
        let dt = 1.0 / self.sample_rate;
        let (mut l, mut r) = (0.0, 0.0);
        for d in self.drops.iter_mut().filter(|d| d.active) {
            d.phase = (d.phase + d.freq * dt) % 1.0;
            d.freq *= 1.0 + d.chirp * dt;
            d.amp *= d.decay;
            let s = (d.phase * TAU).sin() * d.amp;
            l += s * d.gain_l;
            r += s * d.gain_r;
            if d.amp < 1e-4 {
                d.active = false;
            }
        }
        (l, r)
    }

    #[inline]
    pub fn render(&mut self) -> (f32, f32) {
        if self.control_counter == 0 {
            self.control();
        }
        self.control_counter = (self.control_counter + 1) % CONTROL_INTERVAL;

        // (dry_l, dry_r) en (send_l, send_r) naar de reverb
        let (mut dl, mut dr, mut sl, mut sr) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);

        // Meer drive bij chaos maakt de cello's ook luider — grotendeels compenseren,
        // zodat chaos vooral als rasp klinkt en niet als volume
        let cello_gain = 0.10 / (1.0 + 1.0 * self.smooth.chaos);
        for (i, cello) in self.cellos.iter_mut().enumerate() {
            let s = cello.net.get_mono() * cello_gain;
            let (gl, gr) = self.cello_gains[i];
            dl += s * gl; dr += s * gr;
            sl += s * gl * 0.5; sr += s * gr * 0.5;
        }

        let t = self.theremin.net.get_mono() * 0.20;
        let (gl, gr) = self.theremin_gain;
        dl += t * gl; dr += t * gr;
        sl += t * gl * 0.9; sr += t * gr * 0.9;

        for (i, tone) in self.pad.iter_mut().enumerate() {
            let (pl, pr) = tone.net.get_stereo();
            let a = self.pad_amp[i] * 0.07;
            dl += pl * a; dr += pr * a;
            sl += pl * a * 0.6; sr += pr * a * 0.6;
        }

        let (wl, wr) = self.render_drops();
        dl += wl * 0.035; dr += wr * 0.035;
        sl += wl * 0.09;  sr += wr * 0.09;

        let (ml, mr) = self.murmur.get_stereo();
        dl += ml * 0.02; dr += mr * 0.02;
        sl += ml * 0.03; sr += mr * 0.03;

        // Branding: alleen renderen als de getijdenpoel-hypothese (deels) actief is
        let surface = self.smooth.surface;
        if surface > 0.001 {
            let (bl, br) = self.surf.net.get_stereo();
            let (gl, gr) = pan_gains(self.surf.pan);
            let a = surface * SURF_LEVEL * std::f32::consts::SQRT_2;
            dl += bl * gl * a; dr += br * gr * a;
            sl += bl * gl * a * 0.25; sr += br * gr * a * 0.25;
        }

        let (rl, rr) = self.reverb.filter_stereo(sl, sr);

        let g = self.fade_in;
        let out_l = (dl + rl) * g;
        let out_r = (dr + rr) * g;
        // Zachte limiter: tanh houdt pieken onder 1 zonder hard te clippen
        (out_l.tanh(), out_r.tanh())
    }
}

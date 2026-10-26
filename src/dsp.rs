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
//
// Onweer volgt de bliksem in de simulatie: een laag gegrom dat aanzwelt met de
// donderwolk-verduistering, een knal op het moment van de flits en daarna een
// donderrol. Grotere bliksem = dichterbij; superbliksem krijgt een diepe dreun.
//
// In de diepzee (HTV) klinkt af en toe een losse sonar-ping met een kleine echo,
// en rommelen hydrothermale bronnen die af en toe uitbarsten (water en bellen).
//
// Ingezoomd is elke grote zichtbare klont een eigen geluidsbron (toon, borrelen,
// zoeven, op zijn plek), en klinkt wat er tussen klonten gebeurt (blobs.rs):
// naderen en samensmelten, uiteenvallen en uiteendrijven.

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
    /// Opbouw van de donderwolk [0, 1] — zelfde curve als de verduistering op scherm
    pub storm: f32,
    /// Teller van inslagen; een nieuwe waarde = nieuwe inslag
    pub strike_id: f32,
    /// Seconden sinds de laatste inslag (afgekapt op 10) — voorkomt dat een oude
    /// inslag alsnog klinkt als de synth later start
    pub strike_age: f32,
    /// Grootte van de bliksem [0, 1] → luider, helderder, dichterbij
    pub strike_power: f32,
    /// Horizontale positie van de inslag [-1, 1]
    pub strike_pan: f32,
    /// 1 = superbliksem → langere rol en een diepe dreun
    pub strike_super: f32,
    /// Seconden tot de volgende geplande inslag (10 = onbekend of ver weg) —
    /// laat de donder soms al vóór de flits beginnen te rollen
    pub strike_eta: f32,
    /// 1 = er kan bliksem zijn (getijdenpoel, elektrische activiteit > 0); bij 0
    /// zwijgt het onweer direct, ook als er nog donder aan het rollen was
    pub lightning: f32,
    /// Nacht [0, 1] (getijdenpoel): 1 = volle nacht → theremin zwijgt
    pub night: f32,
    /// Klonten samengesmolten (alleen ingezoomd, zie blobs.rs): teller, seconden
    /// sinds de laatste, sterkte [0, 1] en positie [-1, 1]
    pub merge_id: f32,
    pub merge_age: f32,
    pub merge_strength: f32,
    pub merge_pan: f32,
    /// Klont uiteengevallen: idem
    pub split_id: f32,
    pub split_age: f32,
    pub split_strength: f32,
    pub split_pan: f32,
    /// Doorlopend: klonten die naar elkaar toe bewegen (en nazinderen na het
    /// samensmelten), en klonten die uit elkaar drijven — activiteit [0, 1] en positie
    pub converge: f32,
    pub converge_pan: f32,
    pub diverge: f32,
    pub diverge_pan: f32,
    /// De grootste zichtbare klonten, elk met een eigen stem (alleen ingezoomd)
    pub voices: [BlobVoice; BLOB_VOICES],
}

/// Aantal klonten dat tegelijk een eigen stem heeft.
pub const BLOB_VOICES: usize = 6;

/// Eén zichtbare klont als geluidsbron (zie blobs.rs). Een stem met level 0 is leeg.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BlobVoice {
    /// Vast id zolang de klont gevolgd wordt
    pub id:        f32,
    /// Aanwezigheid [0, 1]: grootte van de klont, maal hoe ver ingezoomd
    pub level:     f32,
    /// Plaats [-1, 1]
    pub pan:       f32,
    /// Grootte [0, 1] → toonhoogte bij het ontstaan (groot = laag)
    pub size:      f32,
    /// Beweging als geheel [0, 1]
    pub motion:    f32,
    /// Onrust binnen de klont [0, 1]
    pub agitation: f32,
}

impl BlobVoice {
    const FIELDS: usize = 6;

    fn clamped(&self) -> Self {
        Self {
            id:        self.id,
            level:     self.level.clamp(0.0, 1.0),
            pan:       self.pan.clamp(-1.0, 1.0),
            size:      self.size.clamp(0.0, 1.0),
            motion:    self.motion.clamp(0.0, 1.0),
            agitation: self.agitation.clamp(0.0, 1.0),
        }
    }
}

impl Default for DroneParams {
    fn default() -> Self {
        Self {
            temperature: 0.5, chaos: 0.0, motion: 0.0, calm: 0.0, pan_center: 0.0, pan_lead: 0.0, surface: 0.0,
            storm: 0.0, strike_id: 0.0, strike_age: 10.0, strike_power: 0.0, strike_pan: 0.0, strike_super: 0.0,
            strike_eta: 10.0, lightning: 0.0, night: 0.0,
            merge_id: 0.0, merge_age: 10.0, merge_strength: 0.0, merge_pan: 0.0,
            split_id: 0.0, split_age: 10.0, split_strength: 0.0, split_pan: 0.0,
            converge: 0.0, converge_pan: 0.0, diverge: 0.0, diverge_pan: 0.0,
            voices: [BlobVoice::default(); BLOB_VOICES],
        }
    }
}

impl DroneParams {
    /// Losse waarden vóór de klont-stemmen.
    const SCALARS: usize = 28;
    pub const LEN: usize = Self::SCALARS + BLOB_VOICES * BlobVoice::FIELDS;

    /// Volgorde zoals verstuurd naar de AudioWorklet.
    pub fn to_array(&self) -> [f32; Self::LEN] {
        let scalars = [
            self.temperature, self.chaos, self.motion, self.calm, self.pan_center, self.pan_lead, self.surface,
            self.storm, self.strike_id, self.strike_age, self.strike_power, self.strike_pan, self.strike_super,
            self.strike_eta, self.lightning, self.night,
            self.merge_id, self.merge_age, self.merge_strength, self.merge_pan,
            self.split_id, self.split_age, self.split_strength, self.split_pan,
            self.converge, self.converge_pan, self.diverge, self.diverge_pan,
        ];
        let mut a = [0.0f32; Self::LEN];
        a[..Self::SCALARS].copy_from_slice(&scalars);
        for (chunk, v) in a[Self::SCALARS..].chunks_exact_mut(BlobVoice::FIELDS).zip(&self.voices) {
            chunk.copy_from_slice(&[v.id, v.level, v.pan, v.size, v.motion, v.agitation]);
        }
        a
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
            storm:        v[7],
            strike_id:    v[8],
            strike_age:   v[9],
            strike_power: v[10],
            strike_pan:   v[11],
            strike_super: v[12],
            strike_eta:   v[13],
            lightning:    v[14],
            night:        v[15],
            merge_id:       v[16],
            merge_age:      v[17],
            merge_strength: v[18],
            merge_pan:      v[19],
            split_id:       v[20],
            split_age:      v[21],
            split_strength: v[22],
            split_pan:      v[23],
            converge:       v[24],
            converge_pan:   v[25],
            diverge:        v[26],
            diverge_pan:    v[27],
            voices: std::array::from_fn(|i| {
                let c = &v[Self::SCALARS + i * BlobVoice::FIELDS..][..BlobVoice::FIELDS];
                BlobVoice { id: c[0], level: c[1], pan: c[2], size: c[3], motion: c[4], agitation: c[5] }
            }),
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
            storm:        self.storm.clamp(0.0, 1.0),
            strike_id:    self.strike_id,
            strike_age:   self.strike_age.clamp(0.0, 10.0),
            strike_power: self.strike_power.clamp(0.0, 1.0),
            strike_pan:   self.strike_pan.clamp(-1.0, 1.0),
            strike_super: self.strike_super.clamp(0.0, 1.0),
            strike_eta:   self.strike_eta.clamp(0.0, 10.0),
            lightning:    self.lightning.clamp(0.0, 1.0),
            night:        self.night.clamp(0.0, 1.0),
            merge_id:       self.merge_id,
            merge_age:      self.merge_age.clamp(0.0, 10.0),
            merge_strength: self.merge_strength.clamp(0.0, 1.0),
            merge_pan:      self.merge_pan.clamp(-1.0, 1.0),
            split_id:       self.split_id,
            split_age:      self.split_age.clamp(0.0, 10.0),
            split_strength: self.split_strength.clamp(0.0, 1.0),
            split_pan:      self.split_pan.clamp(-1.0, 1.0),
            converge:       self.converge.clamp(0.0, 1.0),
            converge_pan:   self.converge_pan.clamp(-1.0, 1.0),
            diverge:        self.diverge.clamp(0.0, 1.0),
            diverge_pan:    self.diverge_pan.clamp(-1.0, 1.0),
            voices:         self.voices.map(|v| v.clamped()),
        }
    }
}

// ─── Mengpaneel ──────────────────────────────────────────────────────────────

/// Kanalen van het mengpaneel, in de volgorde van set_mix().
pub mod mix {
    pub const CELLOS:     usize = 0;
    pub const THEREMIN:   usize = 1;
    pub const BRAINWAVES: usize = 2;
    pub const DROPLETS:   usize = 3;
    pub const STREAM:     usize = 4;
    pub const SURF:       usize = 5;
    pub const THUNDER:    usize = 6;
    pub const LIGHTNING:  usize = 7;
    pub const SONAR:      usize = 8;
    pub const VENTS:      usize = 9;
    pub const EVENTS:     usize = 10;
    pub const REVERB:     usize = 11;
    pub const MASTER:     usize = 12;
    /// Aantal gain-kanalen
    pub const LEN:        usize = 13;
    /// Geen gain maar een instelling, direct na de kanalen: sample-and-hold van de
    /// bliksem in samples (1 = uit, max 16)
    pub const LIGHTNING_HOLD: usize = 13;
    pub const HOLD_MAX:   f32   = 16.0;
}

/// Thread-safe handle om de parameters en de mix van een draaiende DroneSynth te zetten.
#[derive(Clone)]
pub struct DroneHandle {
    params: [Shared; DroneParams::LEN],
    /// Lineaire gain per mengpaneel-kanaal (1 = ongewijzigd)
    mix:    [Shared; mix::LEN],
    /// Sample-and-hold van de bliksem (mix::LIGHTNING_HOLD)
    hold:   Shared,
}

impl DroneHandle {
    fn new(p: &DroneParams) -> Self {
        Self {
            params: p.to_array().map(shared),
            mix:    [(); mix::LEN].map(|_| shared(1.0)),
            hold:   shared(LIGHTNING_HOLD_DEFAULT as f32),
        }
    }

    pub fn set(&self, p: &DroneParams) {
        for (s, v) in self.params.iter().zip(p.clamped().to_array()) {
            s.set_value(v);
        }
    }

    /// Gains per kanaal (zie `mix`), lineair, 0..4. Ontbrekende kanalen blijven staan.
    /// Een eventueel extra element op mix::LIGHTNING_HOLD zet de bliksem-crunch.
    pub fn set_mix(&self, gains: &[f32]) {
        for (s, &g) in self.mix.iter().zip(gains) {
            s.set_value(if g.is_finite() { g.clamp(0.0, 4.0) } else { 1.0 });
        }
        if let Some(&h) = gains.get(mix::LIGHTNING_HOLD) {
            if h.is_finite() {
                self.hold.set_value(h.round().clamp(1.0, mix::HOLD_MAX));
            }
        }
    }

    fn get(&self) -> DroneParams {
        DroneParams::from_slice(&self.params.each_ref().map(|s| s.value())).unwrap_or_default()
    }

    fn get_mix(&self) -> [f32; mix::LEN] {
        self.mix.each_ref().map(|s| s.value())
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
    /// Resterende adempauze (s) tussen twee streken; 0 = aan het strijken
    rest:       f32,
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
        // Strijkruis: amplitude-ruis op de zaagtand geeft de korrel van hars op snaar.
        // soft_saw heeft minder scherpe boventonen dan een kale zaagtand.
        let bowed = (pitch >> soft_saw()) * (1.0 + (var(&grit) >> follow(0.5)) * (noise() >> lowpole_hz(1500.0)));
        // Kast: warme houtresonantie laag, de nasale "honk" rond 560 Hz juist iets
        // teruggedrukt, en de grondtoon mag meeklinken — diep en rond in plaats van
        // een zoemende drone in het middengebied.
        let body = bowed
            >> lowpass_hz(1900.0, 0.6)
            >> bell_hz(180.0, 1.0, 1.4)
            >> bell_hz(560.0, 1.4, 0.7)
            >> bell_hz(1500.0, 1.5, 1.2)
            >> highpass_hz(55.0, 0.6);
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
            rest: 0.0,
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
        if self.rest > 0.0 {
            self.rest -= dt;
            self.bow.set_value(0.0);
            return;
        }
        self.stroke_pos += dt;
        if self.stroke_pos >= self.stroke_len {
            self.stroke_pos = 0.0;
            // Chaos: kortere en grilliger streken
            let jitter = rng.range(0.8 - 0.4 * chaos, 1.25);
            self.stroke_len = self.base_len * jitter * (1.0 - 0.5 * chaos) / tempo;
            // Adempauze: regelmatig even niet strijken (minder bij chaos), zodat de
            // cello's geen onafgebroken drone vormen
            if rng.unit() < 0.45 * (1.0 - chaos) {
                self.rest = self.stroke_len * rng.range(0.4, 1.2);
            }
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
//
// Naar het voorbeeld van assets/audio/brainwaves.mp3: een diepe, ronde basdrone
// (grondtoon 44–78 Hz) met zachte, donkere boventonen en twee veel zachtere
// akkoordtonen erboven. Vrijwel niets boven ~1 kHz, geen hoorbaar "wobbelen":
// links en rechts verschillen maar een fractie van een Hz (traag faseren, breed
// stereobeeld). Het volume ademt traag en beperkt; het akkoord wisselt langzaam
// binnen de pentatoniek. Bij een wissel glijden de stemmen niet (dat klonk als een
// glijbaan met te veel "intentie") maar faden ze na elkaar uit, springen in stilte
// naar hun nieuwe toon en zwellen weer aan.

/// Akkoorden (MIDI, laag → hoog), alle binnen C-mineur pentatonisch.
/// Het eerste is "thuis" en komt vaak terug.
const PAD_CHORDS: [[f32; 3]; 5] = [
    [36.0, 43.0, 51.0], // C2  G2  Eb3
    [31.0, 43.0, 46.0], // G1  G2  Bb2
    [34.0, 41.0, 48.0], // Bb1 F2  C3
    [39.0, 46.0, 55.0], // Eb2 Bb2 G3
    [29.0, 36.0, 51.0], // F1  C2  Eb3
];
/// Relatieve sterkte per stem: de grondtoon draagt, de akkoordtonen kleuren
/// (≈ −8 en −13 dB, zoals in het voorbeeld)
const PAD_WEIGHTS: [f32; 3] = [1.0, 0.4, 0.22];
const PAD_PERIODS:   [f32; 3] = [17.0, 13.0, 21.0]; // ademperiode in s bij tempo 1
const PAD_BRIGHT:    [f32; 3] = [41.0, 33.0, 47.0]; // periode van de klankkleur in s
/// Seconden per akkoord bij tempo 1 (bereik, willekeurig binnen)
const PAD_CHORD_LEN: (f32, f32) = (25.0, 50.0);
/// Uit- en infaden van een stem bij een akkoordwissel (s)
const PAD_FADE_OUT: f32 = 3.0;
const PAD_FADE_IN:  f32 = 5.0;
/// Vertraging per stem vóór die gaat wisselen (s, bereik), zodat ze na elkaar gaan
const PAD_STAGGER: [(f32, f32); 3] = [(0.0, 1.5), (3.0, 6.0), (6.0, 12.0)];

struct PadTone {
    net:     Box<dyn AudioUnit>,
    freq_l:  Shared,
    freq_r:  Shared,
    /// Zacht filter (twee one-poles per kanaal); coëfficiënt op control-rate —
    /// een funDSP-filter met variabele cutoff herberekent zich elk sample en was
    /// te zwaar voor de worklet
    lp_coef: f32,
    lp:      [(f32, f32); 2],
    sample_rate: f32,
    base:    f32,
    period:  f32,
    phase:   f32,
    bright_period: f32,
    bright_phase:  f32,
    /// Volgende toon (Hz) bij een akkoordwissel, de wachttijd daarvoor, en de
    /// fade (0..1) waarmee de stem uit- en weer infadet
    target:  f32,
    wait:    f32,
    fade:    f32,
}

impl PadTone {
    fn new(sample_rate: f32, seed: u64, midi: f32, period: f32, phase: f32, bright_period: f32) -> Self {
        let base   = midi_hz(midi);
        let freq_l = shared(base);
        let freq_r = shared(base);
        // Orgel-wavetable: grondtoon met octaaf-boventonen voor body, één opzoeking
        // per sample (drie losse sinussen per stem was te zwaar voor de worklet).
        // Geen aparte chorus-tweelingtoon: links en rechts staan door de binaurale
        // zweving al een paar Hz uit elkaar, wat zelf een breed, zwevend geluid geeft.
        // Korte follow: alleen klikvrij, geen hoorbaar glijden (toonwissels gebeuren
        // in stilte, zie control()).
        let voice = |f: &Shared| var(f) >> follow(0.05) >> organ();
        let graph = voice(&freq_l) | voice(&freq_r);
        Self {
            net: prepare(Box::new(graph), sample_rate, seed),
            freq_l,
            freq_r,
            lp_coef: 0.1,
            lp: [(0.0, 0.0); 2],
            sample_rate,
            base,
            period,
            phase,
            bright_period,
            bright_phase: phase * 1.7,
            target: base,
            wait: 0.0,
            fade: 1.0,
        }
    }

    /// Plant een wissel naar een nieuwe toon na `wait` seconden.
    fn change_to(&mut self, freq: f32, wait: f32) {
        self.target = freq;
        self.wait = wait;
    }

    /// Retourneert de huidige amplitude van deze toon.
    fn control(&mut self, dt: f32, tempo: f32, beat_hz: f32) -> f32 {
        // Akkoordwissel: wachten, uitfaden, in stilte springen, weer infaden
        if self.target != self.base {
            if self.wait > 0.0 {
                self.wait -= dt;
            } else {
                self.fade -= dt / PAD_FADE_OUT;
                if self.fade <= 0.0 {
                    self.fade = 0.0;
                    self.base = self.target;
                }
            }
        } else if self.fade < 1.0 {
            self.fade = (self.fade + dt / PAD_FADE_IN).min(1.0);
        }
        self.phase = (self.phase + dt * tempo / self.period * TAU) % TAU;
        self.bright_phase = (self.bright_phase + dt / self.bright_period * TAU) % TAU;
        self.freq_l.set_value(self.base - beat_hz * 0.5);
        self.freq_r.set_value(self.base + beat_hz * 0.5);
        // Donker: het filter volgt de toonhoogte (~3.5× de grondtoon, dus alleen de
        // eerste paar boventonen) en ademt daar heel langzaam ±25% omheen
        let cutoff = self.base * 3.5 * 1.25f32.powf(self.bright_phase.sin());
        self.lp_coef = 1.0 - (-TAU * cutoff / self.sample_rate).exp();
        // Volume ademt traag en beperkt (~8 dB), zoals in het voorbeeld; zacht
        // in- en uitfaden (sin²) bij een akkoordwissel
        let fade = (self.fade * std::f32::consts::FRAC_PI_2).sin().powi(2);
        (0.7 + 0.3 * self.phase.sin()) * fade
    }

    #[inline]
    fn render(&mut self) -> (f32, f32) {
        let (mut l, mut r) = self.net.get_stereo();
        let k = self.lp_coef;
        for stage in self.lp.iter_mut() {
            stage.0 += (l - stage.0) * k;
            stage.1 += (r - stage.1) * k;
            (l, r) = *stage;
        }
        // Kanalen deels samenvoegen: breed maar niet uit elkaar vallend (L/R-correlatie
        // ~0.7, zoals in het voorbeeld)
        let (l, r) = (0.85 * l + 0.35 * r, 0.85 * r + 0.35 * l);
        // Zachte verzadiging voor wat warmte
        ((1.1 * l).tanh(), (1.1 * r).tanh())
    }
}

/// Kiest wanneer en naar welk akkoord de pad gaat.
struct PadChords {
    current: usize,
    timer:   f32,
}

impl PadChords {
    /// Geeft Some(akkoord) als er gewisseld moet worden.
    fn control(&mut self, dt: f32, tempo: f32, rng: &mut Rng) -> Option<[f32; 3]> {
        self.timer -= dt;
        if self.timer > 0.0 {
            return None;
        }
        self.timer = rng.range(PAD_CHORD_LEN.0, PAD_CHORD_LEN.1) / tempo.powf(0.5);
        // Vanuit een ander akkoord vaak terug naar huis; vanuit huis ergens heen
        let next = if self.current != 0 && rng.unit() < 0.5 {
            0
        } else {
            let mut n = 1 + rng.next_u32() as usize % (PAD_CHORDS.len() - 1);
            if n == self.current {
                n = 1 + n % (PAD_CHORDS.len() - 1);
            }
            n
        };
        self.current = next;
        Some(PAD_CHORDS[next])
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
        // Uitrol na het breken: sneller weg dan eerder (3.5), minder lang naruisen
        let wash = (-t * 6.0).exp();
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

// ─── 6. Onweer (bliksem en donder, in sync met de simulatie) ─────────────────

/// Maximaal aantal knallen per inslag: de hoofdslag plus naflitsen.
const MAX_CRACKS: usize = 8;
/// Maximaal aantal gelijktijdige donderrollen (vooraf, hoofdrol, echo's).
const MAX_ROLLS: usize = 5;

/// Overstuurde vervorming van knal en geknetter: drive en asymmetrie (bias),
/// zoals een overbelaste microfoon bij een inslag dichtbij.
const LIGHTNING_DRIVE: f32 = 12.0;
const LIGHTNING_BIAS:  f32 = 0.4;
/// Lo-fi kraak: het vervormde signaal wordt maar om de zoveel samples bijgewerkt
/// (sample-and-hold), als een overbelaste opname. 1 = uit. Instelbaar via het
/// mengpaneel (mix::LIGHTNING_HOLD); dit is de standaard.
const LIGHTNING_HOLD_DEFAULT: u32 = 4;
/// Mengniveau van knal + geknetter ten opzichte van de donder.
const LIGHTNING_GAIN:  f32 = 2.2;
/// Terugkaatsingen van de knal tegen het landschap: (vertraging in s, sterkte).
/// Oneven echo's komen van de andere kant terug. De vertragingen worden per
/// inslag met een factor 0.8–1.3 geschaald (ander "landschap").
/// Op verzoek uitgezet (sterktes 0); eerder: 0.22, 0.14, 0.08.
const LIGHTNING_ECHOES: [(f32, f32); 3] = [(0.12, 0.0), (0.26, 0.0), (0.45, 0.0)];
/// Langste mogelijke echo-vertraging (0.45 s × 1.3) plus marge.
const LIGHTNING_ECHO_MAX: f32 = 0.7;
/// Galm-zending van de bliksem (de donder heeft 0.35).
const LIGHTNING_REVERB_SEND: f32 = 0.5;

/// Eén donderrol: begint op `at` (storm-klok), zwelt kort aan en sterft weg.
#[derive(Clone, Copy, Default)]
struct RollEvent {
    at:     f32,
    /// 0 = vrij slot
    amp:    f32,
    tau:    f32,
    /// Helderheid van het begin [0, 1]
    bright: f32,
}

struct Storm {
    net:        Box<dyn AudioUnit>,
    rumble_amp: Shared,
    rumble_cut: Shared,
    roll_amp:   Shared,
    roll_cut:   Shared,
    boom_amp:   Shared,
    boom_freq:  Shared,
    /// Laatst geziene strike_id; None tot de eerste parameters binnen zijn
    last_id:    Option<f32>,
    /// Lopende klok van de onweerslaag — donderrollen kunnen vóór de inslag
    /// beginnen, dus die worden niet ten opzichte van de inslag getimed
    clock:      f32,
    /// Seconden sinds de inslag die nu klinkt
    since:      f32,
    power:      f32,
    is_super:   bool,
    pan:        f32,
    /// Vertraging van de hoofdrol na de flits (ook het moment van de dreun)
    delay:      f32,
    /// (tijd na inslag, amplitude) per knal
    cracks:     [(f32, f32); MAX_CRACKS],
    rolls:      [RollEvent; MAX_ROLLS],
    /// Vorige strike_eta, om een nieuwe aanloop te herkennen
    last_eta:   f32,
    /// Bij hoeveel seconden vóór de flits een vooraf-rol inzet (None = deze keer niet)
    pre_roll:   Option<f32>,
    /// Onregelmatig rommelen en haperend knetteren: willekeurige doelen, glijdend gevolgd
    growl:      (f32, f32),
    roll:       (f32, f32),
    flicker:    (f32, f32),
    /// Eigen RNG: onweer verandert zo niets aan het verloop van de andere lagen
    rng:        Rng,
    /// Knal + geknetter: ruiskorrels (per sample gegenereerd) door een stereo
    /// filter met body, dan overstuurd (tanh), dan een DC-filter
    crackle:     Box<dyn AudioUnit>,
    dc_block:    Box<dyn AudioUnit>,
    /// Knal-envelope: doel (control-rate) en per sample gevolgde waarde
    crack_target: f32,
    crack_env:    f32,
    /// Totale duur van het geknetter van deze inslag (s) — daarna is het echt stil
    crackle_len: f32,
    /// Korrels per seconde bij de inslag — verschilt per bliksem
    dust_peak:   f32,
    /// Korrels per seconde en hun amplitude, bijgewerkt op control-rate
    dust_rate:   f32,
    dust_amp:    f32,
    /// Envelope van de huidige ruiskorrel per kanaal, en hoe snel die uitsterft
    grain:       (f32, f32),
    grain_decay: f32,
    /// Sample-and-hold van de vervormde bliksem: vastgehouden waarde en teller
    held:        (f32, f32),
    hold_count:  u32,
    /// Huidige sample-and-hold-lengte (1 = uit)
    hold:        u32,
    /// Echo's van de knal: ringbuffer per kanaal, schrijfpositie, schaal van de
    /// vertragingen (per inslag) en een lowpass die de echo's doffer maakt
    echo_buf:    Vec<(f32, f32)>,
    echo_pos:    usize,
    echo_scale:  f32,
    echo_lp:     (f32, f32),
    echo_lp_coef: f32,
    sample_dt:   f32,
}

impl Storm {
    fn new(sample_rate: f32, seed: u64) -> Self {
        let rumble_amp = shared(0.0);
        let rumble_cut = shared(100.0);
        let roll_amp   = shared(0.0);
        let roll_cut   = shared(400.0);
        let boom_amp   = shared(0.0);
        let boom_freq  = shared(60.0);

        // Per kanaal eigen ruis zodat het onweer breed klinkt:
        //   rumble — ver gegrom van de aanzwellende donderwolk (bruine ruis, heel laag)
        //   roll   — donderrollen, eerst helder, dan steeds donkerder
        // (knal en geknetter zitten apart, zie render())
        let channel = || {
            let rumble = (brown() | (var(&rumble_cut) >> follow(0.3)) | dc(0.7)) >> lowpass();
            let roll = (pink() | (var(&roll_cut) >> follow(0.05)) | dc(0.8)) >> lowpass();
            rumble * 2.0 * (var(&rumble_amp) >> follow(0.08))
                + roll * 20.0 * (var(&roll_amp) >> follow(0.03))
        };
        // Superbliksem: een diepe, wegzakkende dreun onder de rol
        let boom = (var(&boom_freq) >> follow(0.05) >> sine()) * 0.6 * (var(&boom_amp) >> follow(0.01));
        let graph = (channel() | channel()) + (boom >> split::<U2>());
        // Body vanaf ~180 Hz, nadruk rond 2.5 kHz, de allerhoogste sis eraf
        let crackle_filter = || highpass_hz(180.0, 0.6) >> bell_hz(2500.0, 0.7, 1.6) >> lowpass_hz(11000.0, 0.6);
        let crackle = crackle_filter() | crackle_filter();
        let dc_block = highpass_hz(40.0, 0.6) | highpass_hz(40.0, 0.6);

        Self {
            net: prepare(Box::new(graph), sample_rate, seed),
            rumble_amp,
            rumble_cut,
            roll_amp,
            roll_cut,
            boom_amp,
            boom_freq,
            last_id: None,
            clock: 0.0,
            since: 100.0,
            power: 0.0,
            is_super: false,
            pan: 0.0,
            delay: 0.3,
            cracks: [(0.0, 0.0); MAX_CRACKS],
            rolls: [RollEvent::default(); MAX_ROLLS],
            last_eta: 10.0,
            pre_roll: None,
            growl: (0.5, 0.5),
            roll: (0.5, 0.5),
            flicker: (1.0, 1.0),
            rng: Rng(0x2545_F491 ^ seed as u32),
            crackle: prepare(Box::new(crackle), sample_rate, seed + 1),
            dc_block: prepare(Box::new(dc_block), sample_rate, seed + 2),
            crack_target: 0.0,
            crack_env: 0.0,
            crackle_len: 0.5,
            dust_peak: 4000.0,
            dust_rate: 0.0,
            dust_amp: 0.0,
            grain: (0.0, 0.0),
            grain_decay: 0.99,
            held: (0.0, 0.0),
            hold_count: 0,
            hold: LIGHTNING_HOLD_DEFAULT,
            echo_buf: vec![(0.0, 0.0); (LIGHTNING_ECHO_MAX * sample_rate) as usize + 1],
            echo_pos: 0,
            echo_scale: 1.0,
            echo_lp: (0.0, 0.0),
            echo_lp_coef: 1.0 - (-TAU * 2500.0 / sample_rate).exp(),
            sample_dt: 1.0 / sample_rate,
        }
    }

    /// Plant een donderrol; overschrijft zo nodig de zwakste lopende.
    fn add_roll(&mut self, roll: RollEvent) {
        let clock = self.clock;
        let level = |r: &RollEvent| {
            let u = clock - r.at;
            if r.amp <= 0.0 { -1.0 } else if u < 0.0 { r.amp } else { r.amp * (-u / r.tau).exp() }
        };
        if let Some(slot) = self.rolls.iter_mut().min_by(|a, b| level(a).total_cmp(&level(b))) {
            *slot = roll;
        }
    }

    fn strike(&mut self, p: &DroneParams) {
        // Begin waar de simulatie al is, zodat geluid en beeld gelijk blijven lopen
        self.since    = p.strike_age;
        self.power    = p.strike_power.max(0.5);
        self.is_super = p.strike_super > 0.5;
        // Volle stereopositie: een inslag helemaal links klinkt ook helemaal links
        self.pan      = p.strike_pan;
        let power     = self.power;
        let strike_at = self.clock - p.strike_age;
        let rng = &mut self.rng;
        self.echo_scale = rng.range(0.8, 1.3);

        // Geknetter: lengte en dichtheid per bliksem anders; groot en super knetteren
        // wat langer, maar nooit langer dan ~1.5 s
        let super_len = if self.is_super { 1.4 } else { 1.0 };
        self.crackle_len = rng.range(0.3, 1.1) * (0.7 + 0.3 * power) * super_len;
        self.dust_peak   = rng.range(2500.0, 6000.0);

        // Hoofdknal plus naflitsen, verspreid over de hele knetterduur
        self.cracks = [(0.0, 0.0); MAX_CRACKS];
        self.cracks[0] = (0.0, rng.range(0.7, 1.0));
        let extra = 1 + rng.next_u32() as usize % (MAX_CRACKS - 1);
        let spread = 0.8 * self.crackle_len;
        for c in self.cracks.iter_mut().skip(1).take(extra) {
            *c = (rng.range(0.03, spread.max(0.04)), rng.range(0.2, 0.8));
        }

        // Hoofdrol: soms al tijdens het geknetter, soms pas wat later;
        // verre (kleine) bliksem gemiddeld later
        self.delay = rng.range(0.0, 0.25) + 0.45 * (1.0 - power) * rng.range(0.3, 1.3);
        let main = RollEvent {
            at:     strike_at + self.delay,
            amp:    power * rng.range(0.7, 1.0),
            tau:    rng.range(0.8, 2.0) * if self.is_super { 1.5 } else { 1.0 },
            bright: power * rng.range(0.6, 1.0),
        };
        // 0–2 echo's: naslagen die later nog over het landschap rollen
        let echoes = rng.next_u32() % 3;
        let echo_list: Vec<RollEvent> = (0..echoes).map(|_| RollEvent {
            at:     strike_at + rng.range(0.8, 3.5),
            amp:    power * rng.range(0.25, 0.6),
            tau:    rng.range(0.6, 1.6),
            bright: power * rng.range(0.2, 0.5),
        }).collect();
        self.add_roll(main);
        for e in echo_list {
            self.add_roll(e);
        }
    }

    fn control(&mut self, dt: f32, p: &DroneParams, build: f32) {
        self.clock += dt;

        // Nieuwe aanloop naar een inslag (eta springt omhoog, of wordt voor het eerst
        // bekend)? Dan beslissen of de donder deze keer al vóór de flits gaat rollen.
        let new_cycle = p.strike_eta > self.last_eta + 0.5
            || (self.last_eta >= 9.99 && p.strike_eta < 9.99);
        if new_cycle {
            self.pre_roll = (self.rng.unit() < 0.45).then(|| self.rng.range(0.4, 2.2));
        }
        self.last_eta = p.strike_eta;
        if let Some(lead) = self.pre_roll {
            if p.strike_eta <= lead {
                let roll = RollEvent {
                    at:     self.clock,
                    amp:    self.rng.range(0.35, 0.75),
                    tau:    self.rng.range(0.6, 1.4),
                    bright: self.rng.range(0.3, 0.7),
                };
                self.add_roll(roll);
                self.pre_roll = None;
            }
        }

        // Nieuwe inslag? Alleen als hij vers is — een id die al bestond bij het
        // opstarten van de synth (of een gemiste oude) mag niet alsnog knallen.
        if self.last_id != Some(p.strike_id) {
            if self.last_id.is_some() && p.strike_age < 0.5 {
                self.strike(p);
            }
            self.last_id = Some(p.strike_id);
        }
        self.since += dt;

        // Onregelmatig rommelen (~7 en ~10 doelen/s) en haperend knetteren (~25/s)
        if self.rng.unit() < dt * 7.0 {
            self.growl.1 = self.rng.range(0.3, 1.0);
        }
        if self.rng.unit() < dt * 10.0 {
            self.roll.1 = self.rng.range(0.35, 1.0);
        }
        if self.rng.unit() < dt * 25.0 {
            self.flicker.1 = self.rng.range(0.15, 1.4);
        }
        let k = (dt / 0.08).min(1.0);
        self.growl.0   += (self.growl.1   - self.growl.0)   * k;
        self.roll.0    += (self.roll.1    - self.roll.0)    * k;
        self.flicker.0 += (self.flicker.1 - self.flicker.0) * (dt / 0.015).min(1.0);

        // Donderwolk: ver gegrom dat met de verduistering aanzwelt
        self.rumble_amp.set_value(build * self.growl.0);
        self.rumble_cut.set_value(90.0 + 160.0 * build);

        // Knallen: een felle snap (8 ms) met wat body erachter (50 ms)
        let t = self.since;
        let crack: f32 = self.cracks.iter()
            .filter(|(at, a)| *a > 0.0 && t >= *at)
            .map(|(at, a)| a * (0.6 * (-(t - at) / 0.008).exp() + 0.4 * (-(t - at) / 0.05).exp()))
            .sum();
        self.crack_target = crack * self.power;

        // Geknetter: dicht bij de inslag en kwadratisch uitdunnend tot precies
        // crackle_len, met een nieuwe uitbarsting bij elke naflits en een haperend,
        // onregelmatig verloop
        let bursts: f32 = self.cracks.iter()
            .filter(|(at, a)| *a > 0.0 && t >= *at)
            .map(|(at, a)| a * (-(t - at) / 0.06).exp())
            .sum();
        let fade = (1.0 - t / self.crackle_len).clamp(0.0, 1.0);
        let e = ((fade + bursts) * fade).min(1.5);
        if e > 0.002 {
            self.dust_rate = self.dust_peak * e * self.flicker.0;
            self.dust_amp  = self.power * (0.3 + 0.7 * e.min(1.0));
        } else {
            self.dust_rate = 0.0;
        }

        // Donderrollen: som van alle lopende rollen; helderheid gewogen naar sterkte
        let clock = self.clock;
        let (mut amp, mut cut_weighted) = (0.0f32, 0.0f32);
        for r in self.rolls.iter_mut().filter(|r| r.amp > 0.0) {
            let u = clock - r.at;
            if u > 10.0 * r.tau {
                r.amp = 0.0;
            } else if u > 0.0 {
                let env = r.amp * (1.0 - (-u / 0.06).exp()) * (-u / r.tau).exp();
                amp += env;
                cut_weighted += env * (150.0 + 1600.0 * r.bright * (-u / 0.6).exp());
            }
        }
        self.roll_amp.set_value(amp * self.roll.0);
        self.roll_cut.set_value(if amp > 0.0 { cut_weighted / amp } else { 150.0 });

        let u = t - self.delay;
        if self.is_super && u > 0.0 && u < 12.0 {
            self.boom_amp.set_value((1.0 - (-u / 0.02).exp()) * (-u / 0.9).exp());
            self.boom_freq.set_value(38.0 + 32.0 * (-u / 0.5).exp());
        } else {
            self.boom_amp.set_value(0.0);
        }
    }

    /// Bliksem uitgeschakeld: alle lopende en geplande donder en geknetter wissen.
    fn silence(&mut self) {
        self.since = 100.0;
        self.cracks = [(0.0, 0.0); MAX_CRACKS];
        self.rolls = [RollEvent::default(); MAX_ROLLS];
        self.pre_roll = None;
        self.is_super = false;
        self.dust_rate = 0.0;
        self.crack_target = 0.0;
        self.crack_env = 0.0;
        self.grain = (0.0, 0.0);
    }

    /// Klinkt er nu iets? Anders hoeft het net niet gerenderd te worden.
    fn active(&self, build: f32) -> bool {
        build > 0.001
            || self.rolls.iter().any(|r| r.amp > 0.0)
            || self.since < self.crackle_len + 2.0
            || (self.is_super && self.since < self.delay + 10.0)
    }

    /// `thunder` en `lightning`: mengpaneel-gains voor donder (gegrom, rol, dreun)
    /// en bliksem (knal en geknetter).
    #[inline]
    /// Geeft (donder, bliksem) apart terug: de bliksem krijgt meer galm.
    fn render(&mut self, thunder: f32, lightning: f32) -> ((f32, f32), (f32, f32)) {
        let (l, r) = self.net.get_stereo();
        let thunder_out = (l * thunder, r * thunder);
        let (cl, cr) = self.render_lightning();
        let (el, er) = self.render_echoes(cl, cr);
        let g = LIGHTNING_GAIN * lightning;
        (thunder_out, ((cl + el) * g, (cr + er) * g))
    }

    /// Echo's van de knal tegen het landschap, afwisselend van links en rechts.
    #[inline]
    fn render_echoes(&mut self, l: f32, r: f32) -> (f32, f32) {
        let len = self.echo_buf.len();
        self.echo_buf[self.echo_pos] = (l, r);
        let sr = 1.0 / self.sample_dt;
        let (mut el, mut er) = (0.0f32, 0.0f32);
        for (i, &(delay, gain)) in LIGHTNING_ECHOES.iter().enumerate() {
            let back = Ord::min((delay * self.echo_scale * sr) as usize, len - 1);
            let (bl, br) = self.echo_buf[(self.echo_pos + len - back) % len];
            if i % 2 == 0 { el += bl * gain; er += br * gain; } else { el += br * gain; er += bl * gain; }
        }
        self.echo_pos = (self.echo_pos + 1) % len;
        self.echo_lp.0 += (el - self.echo_lp.0) * self.echo_lp_coef;
        self.echo_lp.1 += (er - self.echo_lp.1) * self.echo_lp_coef;
        self.echo_lp
    }

    /// Knal + geknetter, overstuurd.
    #[inline]
    fn render_lightning(&mut self) -> (f32, f32) {
        // Knal-envelope per sample volgen (~1 ms), zodat de snap niet trapsgewijs gaat
        self.crack_env += (self.crack_target - self.crack_env) * 0.03;
        let quiet = self.dust_rate <= 0.0 && self.crack_env < 1e-4
            && self.grain.0 + self.grain.1 < 1e-4;
        if quiet {
            return (0.0, 0.0);
        }

        // Nieuwe ruiskorrel: willekeurige sterkte, stereopositie en lengte (2–10 ms)
        if self.rng.unit() < self.dust_rate * self.sample_dt {
            let a = self.rng.range(0.3, 1.0) * self.dust_amp;
            let p = self.rng.unit();
            self.grain.0 = self.grain.0.max(a * (1.0 - p).sqrt());
            self.grain.1 = self.grain.1.max(a * p.sqrt());
            let len = self.rng.range(0.002, 0.010);
            self.grain_decay = (-self.sample_dt / len).exp();
        }
        let src_l = self.rng.range(-1.0, 1.0) * (self.grain.0 + self.crack_env);
        let src_r = self.rng.range(-1.0, 1.0) * (self.grain.1 + self.crack_env);
        self.grain.0 *= self.grain_decay;
        self.grain.1 *= self.grain_decay;

        // Zwaar overstuurd: harde tanh met asymmetrie voor rauwe, even boventonen,
        // daarna sample-and-hold voor digitale kraak
        let (fl, fr) = self.crackle.filter_stereo(src_l, src_r);
        let bias_off = LIGHTNING_BIAS.tanh();
        let drive = |x: f32| (LIGHTNING_DRIVE * x + LIGHTNING_BIAS).tanh() - bias_off;
        if self.hold_count == 0 {
            self.held = (drive(fl), drive(fr));
        }
        self.hold_count = (self.hold_count + 1) % Ord::max(self.hold, 1);
        self.dc_block.filter_stereo(self.held.0, self.held.1)
    }
}

// ─── 7. Sonar (alleen diepzee / HTV) ─────────────────────────────────────────
//
// Losse pings op onvoorspelbare momenten — geen vast ritme, dat zou een bedoeling
// suggereren — elk met één kleine echo op een wisselende tijd erna.
//
// Opbouw naar een referentie-opname: een vrijwel zuivere toon die snel inzakt en
// dan zacht nagalmt, met daaromheen een glinsterende wolk van licht verstemde
// tonen, zoals galm in water. (De lage dreun uit die opname is bewust weggelaten.)

/// Gelijktijdig klinkende pings en echo's.
const MAX_PINGS: usize = 4;
/// Pingtonen uit de toonladder: G3, Bb3, C4, Eb4, F4.
const SONAR_NOTES: [f32; 5] = [196.00, 233.08, 261.63, 311.13, 349.23];
/// Aantal verstemde tonen in de glinsterende naklank.
const SHIMMER: usize = 5;

/// Eén ping of echo.
#[derive(Clone, Copy, Default)]
struct Ping {
    active:  bool,
    /// Seconden tot deze ping/echo begint
    delay:   f32,
    t:       f32,
    freq:    f32,
    phase:   f32,
    amp:     f32,
    /// Lengte van de lange naklank (s)
    tail:    f32,
    /// Glinstering: frequentieverhouding en fase per verstemde toon
    shimmer: [(f32, f32); SHIMMER],
    /// One-pole lowpass (echo's doffer): coëfficiënt en toestand
    lp_coef: f32,
    lp:      f32,
    gain_l:  f32,
    gain_r:  f32,
}

struct Sonar {
    pings:       [Ping; MAX_PINGS],
    /// Index in SONAR_NOTES van de vorige ping
    last_note:   usize,
    /// Seconden tot de volgende ping
    timer:       f32,
    rng:         Rng,
    sample_rate: f32,
    sample_dt:   f32,
}

impl Sonar {
    fn new(sample_rate: f32, seed: u64) -> Self {
        Self {
            pings: [Ping::default(); MAX_PINGS],
            last_note: usize::MAX,
            timer: 10.0,
            rng: Rng(0x6C8E_9CF5 ^ seed as u32),
            sample_rate,
            sample_dt: 1.0 / sample_rate,
        }
    }

    fn lp_coef(&self, cutoff: f32) -> f32 {
        1.0 - (-TAU * cutoff / self.sample_rate).exp()
    }

    /// Plaatst een ping/echo in een vrij slot (of het zwakst klinkende).
    fn add(&mut self, ping: Ping) {
        let level = |p: &Ping| if !p.active { -1.0 } else { p.amp * (-p.t / p.tail).exp() };
        if let Some(slot) = self.pings.iter_mut().min_by(|a, b| level(a).total_cmp(&level(b))) {
            *slot = ping;
        }
    }

    fn shimmer(rng: &mut Rng) -> [(f32, f32); SHIMMER] {
        std::array::from_fn(|_| {
            let spread = rng.range(0.01, 0.07) * if rng.unit() < 0.5 { -1.0 } else { 1.0 };
            (1.0 + spread, rng.unit())
        })
    }

    /// Eén losse ping met één kleine echo op een wisselende tijd erna.
    fn ping(&mut self) {
        let rng = &mut self.rng;
        let mut note = rng.next_u32() as usize % SONAR_NOTES.len();
        if note == self.last_note {
            note = (note + 1 + rng.next_u32() as usize % (SONAR_NOTES.len() - 1)) % SONAR_NOTES.len();
        }
        self.last_note = note;
        let freq = SONAR_NOTES[note];
        let tail = rng.range(0.6, 0.8);
        // Elke ping uit een andere richting; de echo komt van de andere kant terug
        let pan = rng.range(-0.9, 0.9);
        let (l, r) = pan_gains(pan);
        let ping = Ping {
            active: true, delay: 0.0, t: 0.0, freq, phase: 0.0, amp: 1.0,
            tail, shimmer: Self::shimmer(rng),
            lp_coef: 1.0, lp: 0.0, gain_l: l, gain_r: r,
        };

        // Echo: klein, iets doffer, van de andere kant, op 0.4–2.2 s
        let echo_side = if pan > 0.0 { -1.0 } else { 1.0 };
        let (l, r) = pan_gains(echo_side * rng.range(0.3, 0.9));
        let echo = Ping {
            active: true, delay: rng.range(0.4, 2.2), t: 0.0,
            freq: freq * (1.0 + rng.range(-0.01, 0.01)), phase: 0.0,
            amp: rng.range(0.15, 0.3), tail: tail * 1.2,
            shimmer: Self::shimmer(rng), lp_coef: 0.0, lp: 0.0, gain_l: l, gain_r: r,
        };
        let echo = Ping { lp_coef: self.lp_coef(freq * 2.5), ..echo };
        self.add(ping);
        self.add(echo);
    }

    /// `depth`: 1 = diepzee (sonar actief), 0 = getijdenpoel.
    fn control(&mut self, dt: f32, tempo: f32, depth: f32) {
        if depth < 0.5 {
            // Bij terugkeer naar de diepzee niet meteen, maar wel binnen afzienbare tijd
            self.timer = self.timer.min(self.rng.range(6.0, 15.0));
            return;
        }
        self.timer -= dt;
        if self.timer <= 0.0 {
            self.ping();
            // Onregelmatig: soms vlak na elkaar, soms een hele tijd niets.
            // Volgt het tempo maar heel losjes.
            self.timer = self.rng.range(5.0, 25.0) / tempo.powf(0.3);
        }
    }

    fn active(&self) -> bool {
        self.pings.iter().any(|p| p.active)
    }

    #[inline]
    fn render(&mut self) -> (f32, f32) {
        let dt = self.sample_dt;
        let (mut l, mut r) = (0.0f32, 0.0f32);
        for p in self.pings.iter_mut().filter(|p| p.active) {
            if p.delay > 0.0 {
                p.delay -= dt;
                continue;
            }
            p.t += dt;
            let t = p.t;
            let attack = 1.0 - (-t / 0.004).exp();

            // Toon: snel inzakken (~12 dB in 0.2 s), dan een zachte naklank
            // (~-25 dB na 1 s, ~-38 dB na 2 s; de galm in de mix doet de rest)
            let tone_env = p.amp * attack * (0.8 * (-t / 0.1).exp() + 0.2 * (-t / p.tail).exp());
            p.phase = (p.phase + p.freq * dt) % 1.0;
            let mut s = (p.phase * TAU).sin() * tone_env;

            // Glinsterende wolk van licht verstemde tonen, zwelt kort aan en sterft traag
            let shimmer_env = p.amp * 0.035 * (1.0 - (-t / 0.15).exp()) * (-t / p.tail).exp();
            for (ratio, phase) in p.shimmer.iter_mut() {
                *phase = (*phase + p.freq * *ratio * dt) % 1.0;
                s += (*phase * TAU).sin() * shimmer_env;
            }

            p.lp += (s - p.lp) * p.lp_coef;
            l += p.lp * p.gain_l;
            r += p.lp * p.gain_r;

            if t > 10.0 * p.tail {
                p.active = false;
            }
        }

        (l, r)
    }
}

// ─── 8. Hydrothermale bronnen (alleen diepzee / HTV) ─────────────────────────
//
// Een bron die af en toe heet water omhoog spuit. Niet realistisch onder water,
// maar herkenbaar: een sissende straal (als stoom) die snel opkomt, een paar
// seconden aanhoudt en uitdooft, met daaronder zacht de brommende pijp-toon van de
// schoorsteen en af en toe een losse bel. Bewust zonder ritme — eerdere versies
// met stoten klonken als slagwerk. Daaronder een zacht, diep gerommel van de
// aardkorst. Hoe heter, hoe actiever.

/// Maximaal aantal gelijktijdig klinkende bellen.
const MAX_BUBBLES: usize = 32;
/// Pijp-toon in pentatonische treden (C3 .. C4) — boven de bas van pad en cello's,
/// die hem lager volledig maskeerden.
const VENT_PIPE_LOW:  i32 = 5;
const VENT_PIPE_HIGH: i32 = 10;
/// Toonhoogte van de bellen: van de kleinste (hoog) tot de grootste (laag), Hz.
/// Bewust niet gestemd — echte bellen hebben elke toonhoogte, naar hun grootte.
const BUBBLE_FREQ: (f32, f32) = (1500.0, 250.0);
/// Versterking van de pijp-toon (smalle banddoorlaat op ruis is van zichzelf zacht).
const VENT_PIPE_GAIN: f32 = 32.0;
/// Kwaliteit (smalheid) van de pijpresonantie: hoger = meer toon, minder gerommel.
const VENT_PIPE_Q: f32 = 12.0;
/// Bellen per seconde bij volle druk, en hun sterkte.
const VENT_BUBBLE_RATE: f32 = 80.0;
const VENT_BUBBLE_GAIN: f32 = 2.6;
/// Afstand van de bellen: afsnijfrequentie van het dempfilter (Hz), en hoeveel
/// direct geluid en galm ze krijgen (de rest van de vents: 1.0 en 0.4)
const VENT_BUBBLE_LP:     f32 = 1200.0;
const VENT_BUBBLE_DRY:    f32 = 0.35;
const VENT_BUBBLE_REVERB: f32 = 0.9;
/// Versterking van het gesis.
const VENT_HISS_GAIN: f32 = 0.5;
/// Volume per uitbarsting (dB, bereik), willekeurig binnen.
const VENT_LEVEL_DB: (f32, f32) = (-20.0, -5.0);

/// Banddoorlaat (RBJ, 0 dB piek); coëfficiënten op control-rate bijgewerkt.
#[derive(Clone, Copy, Default)]
struct Bandpass {
    b0: f32, a1: f32, a2: f32,
    x1: f32, x2: f32, y1: f32, y2: f32,
}

impl Bandpass {
    fn set(&mut self, freq: f32, q: f32, sample_rate: f32) {
        let w0 = TAU * freq / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let a0 = 1.0 + alpha;
        self.b0 = alpha / a0;
        self.a1 = -2.0 * w0.cos() / a0;
        self.a2 = (1.0 - alpha) / a0;
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        // b1 = 0, b2 = -b0
        let y = self.b0 * (x - self.x2) - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

#[derive(Clone, Copy, Default)]
struct Bubble {
    active: bool,
    /// Zachte aanzet (~1.5 ms, tegen klikjes): loopt van 1 naar 0
    rise:   f32,
    rise_coef: f32,
    phase:  f32,
    freq:   f32,
    /// Relatieve frequentiestijging per seconde (de bel krimpt tijdens het stijgen)
    chirp:  f32,
    amp:    f32,
    /// Amplitude-factor per sample
    decay:  f32,
    gain_l: f32,
    gain_r: f32,
}

/// Stroom luchtbellen naar een groottemodel, door een afstandsfilter. Gedeeld door de
/// vents en de klont-gebeurtenissen.
struct BubbleStream {
    bubbles:   [Bubble; MAX_BUBBLES],
    /// Afstandsfilter (one-pole lowpass per kanaal): verte dempt de hoogte
    lp:        (f32, f32),
    lp_coef:   f32,
    sample_rate: f32,
    sample_dt: f32,
}

impl BubbleStream {
    fn new(sample_rate: f32, lowpass_hz: f32) -> Self {
        Self {
            bubbles: [Bubble::default(); MAX_BUBBLES],
            lp: (0.0, 0.0),
            lp_coef: 1.0 - (-TAU * lowpass_hz / sample_rate).exp(),
            sample_rate,
            sample_dt: 1.0 / sample_rate,
        }
    }

    /// Eén bel rond `pan` (± `spread`), met sterkte `strength`. Grootte willekeurig:
    /// veel kleine en weinig grote bellen.
    fn spawn(&mut self, rng: &mut Rng, pan: f32, spread: f32, strength: f32) {
        let size = rng.unit().powi(2);
        self.spawn_sized(rng, pan, spread, strength, size);
    }

    /// Eén bel van een gegeven grootte 0..1. De grootte bepaalt toonhoogte (klein =
    /// hoog), luidheid (groot = harder) en duur.
    fn spawn_sized(&mut self, rng: &mut Rng, pan: f32, spread: f32, strength: f32, size: f32) {
        let Some(slot) = self.bubbles.iter_mut().find(|b| !b.active) else { return };
        let freq = BUBBLE_FREQ.0 * (BUBBLE_FREQ.1 / BUBBLE_FREQ.0).powf(size);
        // Een bel klinkt maar een handvol trillingen: kleine tikken kort, grote
        // "blubben" iets langer
        let cycles = rng.range(10.0, 25.0);
        let tau = cycles / (freq * 2.5);
        let (l, r) = pan_gains(pan + rng.range(-spread, spread));
        *slot = Bubble {
            active: true,
            rise:   1.0,
            rise_coef: (-1.0 / (0.0015 * self.sample_rate)).exp(),
            phase:  0.0,
            freq,
            // De toon stijgt licht (~10%) terwijl de bel krimpt — een grotere
            // stijging klonk als een laser ("space sound")
            chirp:  0.1 / (2.5 * tau) * rng.range(0.5, 1.5),
            amp:    (0.35 + 0.65 * size) * rng.range(0.6, 1.0) * strength,
            decay:  (-1.0 / (tau * self.sample_rate)).exp(),
            gain_l: l,
            gain_r: r,
        };
    }

    fn active(&self) -> bool {
        self.bubbles.iter().any(|b| b.active) || self.lp.0.abs() + self.lp.1.abs() > 1e-6
    }

    #[inline]
    fn render(&mut self, gain: f32) -> (f32, f32) {
        let dt = self.sample_dt;
        let (mut l, mut r) = (0.0f32, 0.0f32);
        for b in self.bubbles.iter_mut().filter(|b| b.active) {
            b.phase = (b.phase + b.freq * dt) % 1.0;
            b.freq *= 1.0 + b.chirp * dt;
            b.amp *= b.decay;
            b.rise *= b.rise_coef;
            let s = (b.phase * TAU).sin() * b.amp * (1.0 - b.rise) * gain;
            l += s * b.gain_l;
            r += s * b.gain_r;
            if b.amp < 1e-4 {
                b.active = false;
            }
        }
        let k = self.lp_coef;
        self.lp.0 += (l - self.lp.0) * k;
        self.lp.1 += (r - self.lp.1) * k;
        self.lp
    }
}

struct Vent {
    /// Zacht gerommel van de aardkorst (breed, per kanaal eigen ruis)
    rumble_net: Box<dyn AudioUnit>,
    rumble_amp: Shared,
    /// Sissende straal (ruis 1.8–7 kHz, per kanaal eigen ruis), gepand naar de bron
    hiss_net:   Box<dyn AudioUnit>,
    hiss_amp:   Shared,
    /// Pijpresonanties per kanaal: grondtoon en tweede modus
    pipe:       [[Bandpass; 2]; 2],
    bubbles:    BubbleStream,
    rng:        Rng,
    sample_rate: f32,
    /// Seconden tot de volgende uitbarsting
    timer:      f32,
    /// Lopende uitbarsting
    erupting:   bool,
    t:          f32,
    hold:       f32,
    release:    f32,
    strength:   f32,
    pipe_freq:  f32,
    pan:        f32,
    /// Druk [0, 1]: snel omhoog, houdt aan, dooft uit
    pressure:   f32,
    /// Trage, onregelmatige golving van de straal (geen ritme)
    flutter:    (f32, f32),
    breath:     f32,
}

impl Vent {
    fn new(sample_rate: f32, seed: u64) -> Self {
        let rumble_amp = shared(0.0);
        let hiss_amp   = shared(0.0);
        let rumble = || (brown() >> lowpass_hz(140.0, 0.7)) * 3.0 * (var(&rumble_amp) >> follow(0.2));
        let hiss = || {
            (noise() >> highpass_hz(1800.0, 0.6) >> lowpass_hz(7000.0, 0.6))
                * (var(&hiss_amp) >> follow(0.08))
        };
        Self {
            rumble_net: prepare(Box::new(rumble() | rumble()), sample_rate, seed),
            rumble_amp,
            hiss_net: prepare(Box::new(hiss() | hiss()), sample_rate, seed + 1),
            hiss_amp,
            pipe: [[Bandpass::default(); 2]; 2],
            bubbles: BubbleStream::new(sample_rate, VENT_BUBBLE_LP),
            rng: Rng(0x3C6E_F372 ^ seed as u32),
            sample_rate,
            timer: 12.0,
            erupting: false,
            t: 0.0,
            hold: 3.0,
            release: 2.0,
            strength: 0.0,
            pipe_freq: 80.0,
            pan: 0.0,
            pressure: 0.0,
            flutter: (1.0, 1.0),
            breath: 0.0,
        }
    }

    fn erupt(&mut self, warmth: f32) {
        let rng = &mut self.rng;
        self.erupting = true;
        self.t = 0.0;
        self.hold = rng.range(1.5, 4.5);
        self.release = rng.range(1.5, 3.0);
        // Per uitbarsting een eigen volume, gelijkmatig verdeeld tussen −20 en −5 dB:
        // de ene bron klinkt dichtbij, de andere ver weg
        let level_db = rng.range(VENT_LEVEL_DB.0, VENT_LEVEL_DB.1);
        self.strength = 10f32.powf(level_db / 20.0) * (0.6 + 0.4 * warmth);
        let degree = VENT_PIPE_LOW + (rng.next_u32() % (VENT_PIPE_HIGH - VENT_PIPE_LOW + 1) as u32) as i32;
        self.pipe_freq = degree_hz(degree);
        self.pan = rng.range(-0.8, 0.8);
    }

    /// `depth`: 1 = diepzee, 0 = getijdenpoel. `warmth`: temperatuur 0..1.
    fn control(&mut self, dt: f32, depth: f32, warmth: f32) {
        // Traag ademend achtergrondgerommel (~25 s)
        self.breath = (self.breath + dt / 25.0 * TAU) % TAU;
        let background = 0.06 + 0.025 * self.breath.sin();

        if depth < 0.5 {
            self.erupting = false;
            self.timer = self.timer.min(self.rng.range(6.0, 15.0));
        } else if !self.erupting {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.erupt(warmth);
                // Onregelmatig; hoe heter, hoe vaker (heet ~10–33 s, koud ~30–100 s)
                self.timer = self.rng.range(15.0, 50.0) / (0.5 + warmth);
            }
        }

        if self.erupting {
            self.t += dt;
            let t = self.t;
            // Snel maar zacht opkomen (~0.3 s), aanhouden, dan uitdoven
            let rise = 1.0 - (-t / 0.12).exp();
            let fall = if t < self.hold { 1.0 } else { (-(t - self.hold) / self.release).exp() };
            self.pressure = self.strength * rise * fall;
            if t > self.hold + 6.0 * self.release {
                self.erupting = false;
            }
        } else {
            self.pressure = 0.0;
        }
        let p = self.pressure;

        // Trage, onregelmatige golving van de straal (~2 nieuwe doelen per seconde,
        // glijdend gevolgd) — geen stoten
        if self.rng.unit() < dt * 2.0 {
            self.flutter.1 = self.rng.range(0.7, 1.1);
        }
        self.flutter.0 += (self.flutter.1 - self.flutter.0) * (dt / 0.4).min(1.0);

        // Pijp-toon kruipt ~4% mee omhoog met de druk
        let f = self.pipe_freq * (1.0 + 0.04 * p);
        for ch in self.pipe.iter_mut() {
            ch[0].set(f, VENT_PIPE_Q, self.sample_rate);
            ch[1].set(f * 2.03, VENT_PIPE_Q, self.sample_rate);
        }

        // Losse bellen, verspreid in de tijd
        // Bellen schalen minder sterk met de druk dan de rest (wortel): ook bij een
        // verre, zachte bron blijft er een hoorbare bellenstroom
        let bubbling = p.max(0.0).sqrt();
        if self.rng.unit() < VENT_BUBBLE_RATE * bubbling * dt {
            self.bubbles.spawn(&mut self.rng, self.pan, 0.15, bubbling);
        }

        self.rumble_amp.set_value(background + 0.2 * p);
        self.hiss_amp.set_value(p * self.flutter.0 * VENT_HISS_GAIN);
    }

    /// Geeft (gerommel + straal + pijp-toon, bellen) apart terug: de bellen krijgen
    /// meer galm en minder direct geluid, zodat ze verder weg klinken.
    #[inline]
    fn render(&mut self) -> ((f32, f32), (f32, f32)) {
        let (mut l, mut r) = self.rumble_net.get_stereo();

        let p = self.pressure;
        let (gl, gr) = pan_gains(self.pan);
        // Sissende straal vanaf de plek van de bron
        let (hl, hr) = self.hiss_net.get_stereo();
        l += hl * gl * std::f32::consts::SQRT_2;
        r += hr * gr * std::f32::consts::SQRT_2;

        // Pijp-toon: zacht en vloeiend onder het gesis
        if p > 1e-4 {
            let drive = p * self.flutter.0 * VENT_PIPE_GAIN;
            let mut out = [0.0f32; 2];
            for (o, ch) in out.iter_mut().zip(self.pipe.iter_mut()) {
                let n = self.rng.range(-1.0, 1.0);
                *o = ch[0].process(n) + 0.5 * ch[1].process(n);
            }
            l += out[0] * drive * gl;
            r += out[1] * drive * gr;
        }

        // Losse bellen, door een afstandsfilter (verte dempt de hoogte)
        ((l, r), self.bubbles.render(VENT_BUBBLE_GAIN))
    }
}

// ─── 9. Klonten (alleen ingezoomd, zie blobs.rs) ─────────────────────────────
//
// Elke grote zichtbare klont is een eigen geluidsbron, zodat je hoort wat je ziet:
//   - een zachte toon, toonhoogte naar de grootte bij het ontstaan (groot = laag) en
//     daarna vast; een lichte trilling naarmate de klont onrustiger is
//   - borrelende belletjes naar de onrust binnen de klont
//   - een zacht zoevend geluid naar zijn beweging als geheel (helderder = sneller)
//   - alles op de plek van de klont; een nieuwe klont zwelt in, een verdwenen klont
//     sterft weg — samensmelten en uiteenvallen hoor je zo vanzelf
// Daarbovenop wat er tússen klonten gebeurt: een opborreling op het moment van
// samensmelten of uiteenvallen, klotsend water zolang ze naderen of uiteendrijven, en
// bij samensmelten zwellen galm en pad even aan en gaat de pad naar een nieuw akkoord.

/// Klont-stemmen: toon, zoeven en borrelen
const VOICE_TONE_GAIN:   f32 = 0.05;
const VOICE_SWISH_GAIN:  f32 = 0.25;
const VOICE_BUBBLE_RATE: f32 = 30.0;
/// Toonhoogte van een stem in pentatonische treden: grootste klont .. kleinste
const VOICE_DEGREES: (i32, i32) = (5, 14);
/// In- en uitfaden van een stem (s)
const VOICE_FADE_IN:  f32 = 0.6;
const VOICE_FADE_OUT: f32 = 1.2;
/// Borrelen zolang klonten naderen of uiteendrijven (bewust bescheiden: dat gebeurt
/// vaak, en het moment van samensmelten of uiteenvallen moet erbovenuit komen)
const EVENT_BUBBLE_RATE:  f32 = 25.0;
/// Het moment zelf: een opborreling van zoveel bellen over EVENT_BURST_TIME s, een
/// "gloep" van een paar dikke bellen en een golf klotsend water die nadeint
const EVENT_BURST:        f32 = 50.0;
const EVENT_BURST_TIME:   f32 = 0.8;
const EVENT_WAVE_DECAY:   f32 = 1.5;
/// Afstandsfilter van de bellen (Hz)
const EVENT_BUBBLE_LP:    f32 = 1500.0;
const EVENT_BUBBLE_GAIN:  f32 = 2.2;
const EVENT_SLOSH_GAIN:   f32 = 1.0;

/// Geluid van één klont-stem.
#[derive(Clone, Copy, Default)]
struct VoiceState {
    /// In gebruik voor een klont (ook tijdens het wegsterven)
    live:      bool,
    /// De klont is nog in beeld
    present:   bool,
    id:        f32,
    fade:      f32,
    freq:      f32,
    phase:     f32,
    trill:     f32,
    /// Doel- en afgevlakte waarden: aanwezigheid, plaats, beweging, onrust
    target:    BlobVoice,
    level:     f32,
    pan:       f32,
    motion:    f32,
    agitation: f32,
    gains:     (f32, f32),
    swish:     [Bandpass; 2],
}

struct Happenings {
    voices:      [VoiceState; BLOB_VOICES],
    bubbles:     BubbleStream,
    rng:         Rng,
    sample_rate: f32,
    sample_dt:   f32,
    last_merge:  Option<f32>,
    last_split:  Option<f32>,
    /// Afgevlakte activiteit en positie: naar elkaar toe, uit elkaar
    converge:    (f32, f32),
    diverge:     (f32, f32),
    /// Opborreling: nog te maken bellen, en waar
    burst:       (f32, f32),
    /// Klotsen: banddoorlaten (L/R, eigen ruis) en een trage, onregelmatige golving
    slosh:       [Bandpass; 2],
    slosh_wave:  (f32, f32),
    /// Golf klotsend water na samensmelten of uiteenvallen: sterkte en plaats
    wave:        (f32, f32),
    /// Zwelling van galm en pad na samensmelten: hoogte en tijd sinds start
    swell:       (f32, f32),
    swell_t:     f32,
    /// Zet DroneSynth om bij samensmelten een nieuw pad-akkoord te kiezen
    chord_change: bool,
}

impl Happenings {
    fn new(sample_rate: f32, seed: u64) -> Self {
        Self {
            voices: [VoiceState::default(); BLOB_VOICES],
            bubbles: BubbleStream::new(sample_rate, EVENT_BUBBLE_LP),
            rng: Rng(0x1F83_D9AB ^ seed as u32),
            sample_rate,
            sample_dt: 1.0 / sample_rate,
            last_merge: None,
            last_split: None,
            converge: (0.0, 0.0),
            diverge: (0.0, 0.0),
            burst: (0.0, 0.0),
            slosh: [Bandpass::default(); 2],
            slosh_wave: (1.0, 1.0),
            wave: (0.0, 0.0),
            swell: (0.0, 0.0),
            swell_t: 100.0,
            chord_change: false,
        }
    }

    /// Klont-stemmen bijwerken: bekende klonten volgen, nieuwe laten inzwellen,
    /// verdwenen klonten laten wegsterven.
    fn update_voices(&mut self, dt: f32, incoming: &[BlobVoice; BLOB_VOICES]) {
        for st in self.voices.iter_mut() {
            st.present = false;
        }
        for v in incoming.iter().filter(|v| v.level > 0.001) {
            if let Some(st) = self.voices.iter_mut().find(|s| s.live && s.id == v.id) {
                st.present = true;
                st.target = *v;
            } else if let Some(st) = self.voices.iter_mut().find(|s| !s.live) {
                // Toonhoogte naar de grootte bij het ontstaan, daarna vast
                let span = (VOICE_DEGREES.1 - VOICE_DEGREES.0) as f32;
                let degree = VOICE_DEGREES.1 - (v.size * span).round() as i32;
                *st = VoiceState {
                    live: true, present: true, id: v.id, fade: 0.0,
                    freq: degree_hz(degree), target: *v,
                    level: v.level, pan: v.pan, motion: v.motion, agitation: v.agitation,
                    ..VoiceState::default()
                };
            }
        }

        let k = (dt / 0.3).min(1.0);
        for st in self.voices.iter_mut().filter(|s| s.live) {
            if st.present {
                st.fade = (st.fade + dt / VOICE_FADE_IN).min(1.0);
                st.level     += (st.target.level     - st.level)     * k;
                st.pan       += (st.target.pan       - st.pan)       * k;
                st.motion    += (st.target.motion    - st.motion)    * k;
                st.agitation += (st.target.agitation - st.agitation) * k;
            } else {
                st.fade -= dt / VOICE_FADE_OUT;
                if st.fade <= 0.0 {
                    st.live = false;
                    continue;
                }
            }
            st.gains = pan_gains(st.pan);
            let center = 500.0 + 2500.0 * st.motion;
            for bp in st.swish.iter_mut() {
                bp.set(center, 1.5, self.sample_rate);
            }
        }

        // Borrelen naar de onrust van elke klont
        let mut spawns = Vec::new();
        for st in self.voices.iter().filter(|s| s.live) {
            let rate = VOICE_BUBBLE_RATE * st.agitation * st.level * st.fade;
            if self.rng.unit() < rate * dt {
                spawns.push((st.pan, (0.4 + 0.6 * st.agitation) * st.level));
            }
        }
        for (pan, strength) in spawns {
            self.bubbles.spawn(&mut self.rng, pan, 0.1, strength);
        }
    }

    fn control(&mut self, dt: f32, p: &DroneParams) {
        self.update_voices(dt, &p.voices);

        // Samensmelten of uiteenvallen (alleen als het vers is, zie Storm::control):
        // een gloep van dikke bellen, een opborreling en een golf klotsend water;
        // samensmelten laat galm en pad opzwellen en kiest een nieuw pad-akkoord
        if self.last_merge != Some(p.merge_id) {
            if self.last_merge.is_some() && p.merge_age < 0.5 && p.merge_strength > 0.01 {
                let (s, pan) = (p.merge_strength, p.merge_pan);
                self.moment(s, pan, &[pan, pan, pan]);
                self.swell = (self.swell.0.max(s), 0.0);
                self.swell_t = 0.0;
                self.chord_change = true;
            }
            self.last_merge = Some(p.merge_id);
        }
        if self.last_split != Some(p.split_id) {
            if self.last_split.is_some() && p.split_age < 0.5 && p.split_strength > 0.01 {
                let (s, pan) = (p.split_strength, p.split_pan);
                // Twee dikke bellen die elk een kant op gaan
                self.moment(s, pan, &[pan - 0.3, pan + 0.3]);
                self.swell = (self.swell.0.max(0.6 * s), 0.0);
                self.swell_t = 0.0;
            }
            self.last_split = Some(p.split_id);
        }
        self.wave.0 *= (-dt / EVENT_WAVE_DECAY).exp();

        // Activiteit tussen klonten volgen: snel op (~0.3 s), rustig weg (~0.8 s)
        let follow = |cur: &mut (f32, f32), target: f32, pan: f32| {
            let k = if target > cur.0 { dt / 0.3 } else { dt / 0.8 };
            cur.0 += (target - cur.0) * k.min(1.0);
            if target > 0.01 {
                cur.1 += (pan - cur.1) * (dt / 0.3).min(1.0);
            }
        };
        follow(&mut self.converge, p.converge, p.converge_pan);
        follow(&mut self.diverge, p.diverge, p.diverge_pan);

        // Bellen bij naderen/uiteendrijven, plus de opborreling
        for (act, pan) in [self.converge, self.diverge] {
            if self.rng.unit() < EVENT_BUBBLE_RATE * act * dt {
                self.bubbles.spawn(&mut self.rng, pan, 0.25, (0.4 + 0.6 * act).min(1.0));
            }
        }
        if self.burst.0 >= 1.0 && self.rng.unit() < EVENT_BURST / EVENT_BURST_TIME * dt {
            self.burst.0 -= 1.0;
            self.bubbles.spawn(&mut self.rng, self.burst.1, 0.3, 1.0);
        }

        // Klotsen: trage, onregelmatige golving (~1.5 nieuwe doelen per seconde)
        if self.rng.unit() < dt * 1.5 {
            self.slosh_wave.1 = self.rng.range(0.4, 1.2);
        }
        self.slosh_wave.0 += (self.slosh_wave.1 - self.slosh_wave.0) * (dt / 0.25).min(1.0);
        let center = 250.0 + 250.0 * self.slosh_wave.0;
        for bp in self.slosh.iter_mut() {
            bp.set(center, 1.2, self.sample_rate);
        }

        // Zwelling: ~0.4 s aanzwellen, ~4 s wegebben
        self.swell_t += dt;
        let rise = 1.0 - (-self.swell_t / 0.4).exp();
        let fall = (-self.swell_t / 4.0).exp();
        self.swell.1 = self.swell.0 * rise * fall;
        if self.swell_t > 20.0 {
            self.swell.0 = 0.0;
        }
    }

    /// Het moment van samensmelten of uiteenvallen: dikke bellen op `gloops`,
    /// een opborreling en een golf klotsend water rond `pan`.
    fn moment(&mut self, strength: f32, pan: f32, gloops: &[f32]) {
        for &g in gloops {
            let size = self.rng.range(0.8, 1.0);
            self.bubbles.spawn_sized(&mut self.rng, g.clamp(-1.0, 1.0), 0.05, strength, size);
        }
        self.burst = (self.burst.0 + EVENT_BURST * strength, pan);
        self.wave = (self.wave.0.max(strength), pan);
    }

    /// Hoeveel de galm en de pad nu extra opzwellen [0, 1].
    fn swell(&self) -> f32 {
        self.swell.1
    }

    fn active(&self) -> bool {
        self.voices.iter().any(|v| v.live) || self.converge.0 > 1e-3 || self.diverge.0 > 1e-3
            || self.burst.0 >= 1.0 || self.wave.0 > 1e-3 || self.bubbles.active()
    }

    #[inline]
    fn render(&mut self) -> (f32, f32) {
        let (mut l, mut r) = self.bubbles.render(EVENT_BUBBLE_GAIN);
        let dt = self.sample_dt;

        // Klont-stemmen: toon (met trilling naar onrust) en zoeven (naar beweging)
        for st in self.voices.iter_mut().filter(|s| s.live) {
            st.trill = (st.trill + 5.0 * dt) % 1.0;
            let wobble = 1.0 + 0.004 * st.agitation * (st.trill * TAU).sin();
            st.phase = (st.phase + st.freq * wobble * dt) % 1.0;
            let a = st.phase * TAU;
            let tone = (a.sin() + 0.25 * (2.0 * a).sin()) * st.level * st.fade * VOICE_TONE_GAIN;
            let mut sl = tone;
            let mut sr = tone;
            if st.motion > 1e-3 {
                // Kwadratisch: alleen duidelijk bewegende klonten zoeven hoorbaar
                let s = st.motion * st.motion * st.level * st.fade * VOICE_SWISH_GAIN;
                sl += st.swish[0].process(self.rng.range(-1.0, 1.0)) * s;
                sr += st.swish[1].process(self.rng.range(-1.0, 1.0)) * s;
            }
            l += sl * st.gains.0;
            r += sr * st.gains.1;
        }

        // Klotsend water waar klonten naderen of uiteendrijven, en de golf na het
        // moment van samensmelten of uiteenvallen
        let (act, pan) = if self.converge.0 >= self.diverge.0 { self.converge } else { self.diverge };
        let (act, pan) = if self.wave.0 > act * self.slosh_wave.0 { (self.wave.0, self.wave.1) } else { (act * self.slosh_wave.0, pan) };
        if act > 1e-3 {
            let (gl, gr) = pan_gains(pan);
            let a = act * EVENT_SLOSH_GAIN;
            let (nl, nr) = (self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0));
            l += self.slosh[0].process(nl) * a * gl;
            r += self.slosh[1].process(nr) * a * gr;
        }
        (l, r)
    }
}

// ─── DroneSynth ──────────────────────────────────────────────────────────────

/// Mengniveau van de branding (−5 dB t.o.v. de eerdere 0.22).
const SURF_LEVEL: f32 = 0.124;
/// Mengniveau van het onweer.
const STORM_LEVEL: f32 = 0.3;
/// Mengniveau van de sonar.
const SONAR_LEVEL: f32 = 0.5;
/// Mengniveau van de hydrothermale bronnen.
const VENT_LEVEL: f32 = 0.55;

pub struct DroneSynth {
    sample_rate: f32,
    rng:         Rng,

    cellos:   [Cello; 2],
    theremin: Theremin,
    pad:      [PadTone; 3],
    pad_chords: PadChords,
    pad_amp:  [f32; 3],
    drops:    [Drop; MAX_DROPS],
    murmur:     Box<dyn AudioUnit>,
    murmur_amp: Shared,
    surf:     Surf,
    storm:    Storm,
    sonar:    Sonar,
    vent:     Vent,
    happenings: Happenings,
    reverb:   Box<dyn AudioUnit>,

    /// Doelwaarden, schrijfbaar vanuit andere threads
    handle: DroneHandle,
    /// Afgevlakte parameters — sprongen in de simulatie worden glijdende overgangen
    smooth: DroneParams,
    burst_phase: f32,

    control_counter: u32,
    fade_in:         f32,
    /// Afgevlakte mengpaneel-gains (zie `mix`)
    mix:             [f32; mix::LEN],
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
                PadTone::new(sample_rate, 50 + i as u64, PAD_CHORDS[0][i], PAD_PERIODS[i], i as f32 * 2.1, PAD_BRIGHT[i])
            }),
            pad_chords: PadChords { current: 0, timer: PAD_CHORD_LEN.0 },
            pad_amp: [0.0; 3],
            drops: [Drop::default(); MAX_DROPS],
            murmur: prepare(Box::new(murmur), sample_rate, 71),
            murmur_amp,
            surf: Surf::new(sample_rate, 97),
            storm: Storm::new(sample_rate, 113),
            sonar: Sonar::new(sample_rate, 131),
            vent: Vent::new(sample_rate, 151),
            happenings: Happenings::new(sample_rate, 173),
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
            mix: [1.0; mix::LEN],
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

    /// Mengpaneel: lineaire gain per kanaal, in de volgorde van `mix`.
    pub fn set_mix(&mut self, gains: &[f32]) {
        self.handle.set_mix(gains);
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
        // Mengpaneel-gains in ~50 ms volgen, zodat een fader niet klikt
        let k_mix = (dt / 0.05).min(1.0);
        for (m, t) in self.mix.iter_mut().zip(self.handle.get_mix()) {
            *m += (t - *m) * k_mix;
        }
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
        // Donderwolk volgt de verduistering op scherm vrijwel direct; inslag-velden
        // zijn gebeurtenissen en worden niet afgevlakt
        s.storm       += (target.storm       - s.storm)       * (dt / 0.15).min(1.0);
        s.strike_id    = target.strike_id;
        s.strike_age   = target.strike_age;
        s.strike_power = target.strike_power;
        s.strike_pan   = target.strike_pan;
        s.strike_super = target.strike_super;
        s.strike_eta   = target.strike_eta;
        // Klont-gebeurtenissen: ook gebeurtenissen, niet afvlakken (Happenings doet
        // zelf de in- en uitfade van de whoosh)
        s.merge_id       = target.merge_id;
        s.merge_age      = target.merge_age;
        s.merge_strength = target.merge_strength;
        s.merge_pan      = target.merge_pan;
        s.split_id       = target.split_id;
        s.split_age      = target.split_age;
        s.split_strength = target.split_strength;
        s.split_pan      = target.split_pan;
        s.voices         = target.voices;
        s.converge       = target.converge;
        s.converge_pan   = target.converge_pan;
        s.diverge        = target.diverge;
        s.diverge_pan    = target.diverge_pan;
        // Bliksem uit: onweer in ~0.5 s weg (aan gaat net zo snel, maar dan is er
        // toch nog niets te horen tot de eerste donderwolk)
        s.lightning   += (target.lightning   - s.lightning)   * (dt / 0.15).min(1.0);
        // De nacht valt al geleidelijk (10 s schemering); alleen sprongen afvlakken
        s.night       += (target.night       - s.night)       * (dt / 0.5).min(1.0);
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
        // Geen hoorbare zweving meer (bij lage tonen werd dat een wobbel): links en
        // rechts faseren alleen traag langs elkaar, iets sneller bij warmte
        let beat_hz = 0.2 + 0.4 * warmth;
        // Langzame akkoordwisselingen: de stemmen wisselen na elkaar, in willekeurige
        // volgorde, zonder te glijden (zie PadTone::control)
        if let Some(chord) = self.pad_chords.control(dt, tempo, &mut self.rng) {
            let first = self.rng.next_u32() as usize % 3;
            for (i, (tone, &midi)) in self.pad.iter_mut().zip(chord.iter()).enumerate() {
                let (lo, hi) = PAD_STAGGER[(i + 3 - first) % 3];
                tone.change_to(midi_hz(midi), self.rng.range(lo, hi));
            }
        }
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
        if target.lightning <= 0.0 && p.lightning < 0.01 {
            self.storm.silence();
        }
        self.storm.hold = self.handle.hold.value() as u32;
        self.storm.control(dt, &p, p.storm);
        self.sonar.control(dt, tempo, 1.0 - p.surface);
        self.vent.control(dt, 1.0 - p.surface, warmth);
        self.happenings.control(dt, &p);
        if std::mem::take(&mut self.happenings.chord_change) {
            // Bloei: de pad gaat (op zijn eigen rustige manier) naar een nieuw akkoord
            self.pad_chords.timer = 0.0;
        }

        if self.fade_in < 1.0 {
            self.fade_in = (self.fade_in + dt / 4.0).min(1.0);
        }
    }

    fn spawn_drop(&mut self) {
        let Some(slot) = self.drops.iter_mut().find(|d| !d.active) else { return };
        let degree = DROP_LOW + (self.rng.next_u32() % (DROP_HIGH - DROP_LOW + 1) as u32) as i32;
        let duration = self.rng.range(0.04, 0.14);
        // Elke druppel op een eigen plek in het volle stereobeeld
        let (l, r) = pan_gains(self.rng.range(-1.0, 1.0));
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
        let mx = self.mix;
        // In de getijdenpoel zwijgen de cello's: daar zorgt de branding al voor lange
        // klanken (in- en uitfaden met dezelfde ~3 s als de branding)
        let cello_gain = 0.10 / (1.0 + 1.0 * self.smooth.chaos) * mx[mix::CELLOS] * (1.0 - self.smooth.surface);
        for (i, cello) in self.cellos.iter_mut().enumerate() {
            let s = cello.net.get_mono() * cello_gain;
            let (gl, gr) = self.cello_gains[i];
            dl += s * gl; dr += s * gr;
            sl += s * gl * 0.5; sr += s * gr * 0.5;
        }

        // Theremin en druppels staan 10 dB lager dan oorspronkelijk (0.20 → 0.063 en
        // 0.035/0.09 → 0.011/0.0285), op verzoek vastgezet
        // 's Nachts zwijgt de theremin: zakt mee met de schemering, komt terug bij zonsopgang
        let t = self.theremin.net.get_mono() * 0.063 * mx[mix::THEREMIN] * (1.0 - self.smooth.night);
        let (gl, gr) = self.theremin_gain;
        dl += t * gl; dr += t * gr;
        sl += t * gl * 0.9; sr += t * gr * 0.9;

        for (i, tone) in self.pad.iter_mut().enumerate() {
            let (pl, pr) = tone.render();
            // Zwelt even mee aan bij een klont-gebeurtenis
            let a = self.pad_amp[i] * PAD_WEIGHTS[i] * 0.16 * mx[mix::BRAINWAVES]
                * (1.0 + 0.6 * self.happenings.swell());
            dl += pl * a; dr += pr * a;
            sl += pl * a * 0.6; sr += pr * a * 0.6;
        }

        let (wl, wr) = self.render_drops();
        let (wl, wr) = (wl * mx[mix::DROPLETS], wr * mx[mix::DROPLETS]);
        dl += wl * 0.011;  dr += wr * 0.011;
        sl += wl * 0.0285; sr += wr * 0.0285;

        let (ml, mr) = self.murmur.get_stereo();
        let (ml, mr) = (ml * mx[mix::STREAM], mr * mx[mix::STREAM]);
        dl += ml * 0.02; dr += mr * 0.02;
        sl += ml * 0.03; sr += mr * 0.03;

        // Branding: alleen renderen als de getijdenpoel-hypothese (deels) actief is
        let surface = self.smooth.surface;
        if surface > 0.001 {
            let (bl, br) = self.surf.net.get_stereo();
            let (gl, gr) = pan_gains(self.surf.pan);
            let a = surface * SURF_LEVEL * std::f32::consts::SQRT_2 * mx[mix::SURF];
            dl += bl * gl * a; dr += br * gr * a;
            sl += bl * gl * a * 0.25; sr += br * gr * a * 0.25;
        }

        // Onweer: alleen renderen tijdens donderwolk of nagalm van een inslag
        if self.storm.active(self.smooth.storm) {
            let ((tl, tr), (ll, lr)) = self.storm.render(mx[mix::THUNDER], mx[mix::LIGHTNING]);
            let (gl, gr) = pan_gains(self.storm.pan);
            let a = STORM_LEVEL * std::f32::consts::SQRT_2 * self.smooth.lightning;
            dl += tl * gl * a; dr += tr * gr * a;
            sl += tl * gl * a * 0.35; sr += tr * gr * a * 0.35;
            // Bliksem: wat meer galm dan de donder, zodat de knal niet kurkdroog is
            dl += ll * gl * a; dr += lr * gr * a;
            sl += ll * gl * a * LIGHTNING_REVERB_SEND; sr += lr * gr * a * LIGHTNING_REVERB_SEND;
        }

        // Sonar: alleen in de diepzee, met veel galm (onderwaterruimte)
        let depth = 1.0 - surface;
        if depth > 0.001 && self.sonar.active() {
            let (ol, or) = self.sonar.render();
            let a = depth * SONAR_LEVEL * mx[mix::SONAR];
            dl += ol * a; dr += or * a;
            sl += ol * a * 0.8; sr += or * a * 0.8;
        }

        // Hydrothermale bronnen: alleen in de diepzee
        if depth > 0.001 {
            let ((vl, vr), (bl, br)) = self.vent.render();
            let a = depth * VENT_LEVEL * mx[mix::VENTS];
            dl += vl * a; dr += vr * a;
            sl += vl * a * 0.4; sr += vr * a * 0.4;
            // Bellen: weinig direct geluid en veel galm, zodat ze op afstand klinken
            dl += bl * a * VENT_BUBBLE_DRY; dr += br * a * VENT_BUBBLE_DRY;
            sl += bl * a * VENT_BUBBLE_REVERB; sr += br * a * VENT_BUBBLE_REVERB;
        }

        // Klont-gebeurtenissen (alleen ingezoomd): veel galm, ze horen bij de ruimte
        if self.happenings.active() {
            let (el, er) = self.happenings.render();
            let a = mx[mix::EVENTS];
            dl += el * a; dr += er * a;
            sl += el * a * 0.7; sr += er * a * 0.7;
        }

        let (rl, rr) = self.reverb.filter_stereo(sl, sr);
        // De galm zwelt even aan bij een klont-gebeurtenis
        let reverb = mx[mix::REVERB] * (1.0 + 0.8 * self.happenings.swell());
        let (rl, rr) = (rl * reverb, rr * reverb);

        let g = self.fade_in * mx[mix::MASTER];
        let out_l = (dl + rl) * g;
        let out_r = (dr + rr) * g;
        // Zachte limiter: tanh houdt pieken onder 1 zonder hard te clippen
        (out_l.tanh(), out_r.tanh())
    }
}

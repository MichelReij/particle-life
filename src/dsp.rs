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
// Chaos (op elk zoomniveau) en boven 3× zoom de clusterstatistieken (zie DroneParams):
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
// Bij hoge chaos (deeltjes die alle kanten op schieten, op elk zoomniveau) komen er
// knetterende tikjes, kolkende ruis en een lage dissonant bij, raakt de pad
// uit de stemming en gaat het tempo omhoog.
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
    /// Deeltjes die alle kanten op schieten zonder orde [0, 1], op elk zoomniveau
    /// (sonification::ChaosMeter) → chaos-laag, valse pad, rasp op de cello's
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
    /// Hydrothermale uitbarsting (alleen diepzee, gepland door de simulatie, zie
    /// eruptions.rs): teller, seconden sinds het begin, sterkte [0, 1], positie
    /// [-1, 1], en hoe lang de straal aanhoudt en uitdooft (s)
    pub eruption_id: f32,
    pub eruption_age: f32,
    pub eruption_strength: f32,
    pub eruption_pan: f32,
    pub eruption_hold: f32,
    pub eruption_release: f32,
    /// Id van de klont-stem die bij de laatste samensmelting ontstond, en van de twee
    /// brokken na het laatste uiteenvallen (0 = geen) — die stemmen bloeien even op
    pub merge_voice: f32,
    pub split_voice_a: f32,
    pub split_voice_b: f32,
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
    /// Vaart over het scherm [0, 1]: 1 = zoeft snel langs
    pub rush:      f32,
    /// Onrust binnen de klont [0, 1]
    pub agitation: f32,
}

impl BlobVoice {
    const FIELDS: usize = 7;

    fn clamped(&self) -> Self {
        Self {
            id:        self.id,
            level:     self.level.clamp(0.0, 1.0),
            pan:       self.pan.clamp(-1.0, 1.0),
            size:      self.size.clamp(0.0, 1.0),
            motion:    self.motion.clamp(0.0, 1.0),
            rush:      self.rush.clamp(0.0, 1.0),
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
            eruption_id: 0.0, eruption_age: 10.0, eruption_strength: 0.0, eruption_pan: 0.0,
            eruption_hold: 0.0, eruption_release: 0.0,
            merge_voice: 0.0, split_voice_a: 0.0, split_voice_b: 0.0,
            voices: [BlobVoice::default(); BLOB_VOICES],
        }
    }
}

impl DroneParams {
    /// Losse waarden vóór de klont-stemmen.
    const SCALARS: usize = 37;
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
            self.eruption_id, self.eruption_age, self.eruption_strength, self.eruption_pan,
            self.eruption_hold, self.eruption_release,
            self.merge_voice, self.split_voice_a, self.split_voice_b,
        ];
        let mut a = [0.0f32; Self::LEN];
        a[..Self::SCALARS].copy_from_slice(&scalars);
        for (chunk, v) in a[Self::SCALARS..].chunks_exact_mut(BlobVoice::FIELDS).zip(&self.voices) {
            chunk.copy_from_slice(&[v.id, v.level, v.pan, v.size, v.motion, v.rush, v.agitation]);
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
            eruption_id:       v[28],
            eruption_age:      v[29],
            eruption_strength: v[30],
            eruption_pan:      v[31],
            eruption_hold:     v[32],
            eruption_release:  v[33],
            merge_voice:       v[34],
            split_voice_a:     v[35],
            split_voice_b:     v[36],
            voices: std::array::from_fn(|i| {
                let c = &v[Self::SCALARS + i * BlobVoice::FIELDS..][..BlobVoice::FIELDS];
                BlobVoice { id: c[0], level: c[1], pan: c[2], size: c[3], motion: c[4], rush: c[5], agitation: c[6] }
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
            eruption_id:       self.eruption_id,
            eruption_age:      self.eruption_age.clamp(0.0, 10.0),
            eruption_strength: self.eruption_strength.clamp(0.0, 1.0),
            eruption_pan:      self.eruption_pan.clamp(-1.0, 1.0),
            eruption_hold:     self.eruption_hold.clamp(0.0, 10.0),
            eruption_release:  self.eruption_release.clamp(0.0, 10.0),
            merge_voice:       self.merge_voice,
            split_voice_a:     self.split_voice_a,
            split_voice_b:     self.split_voice_b,
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
    pub const CHAOS:      usize = 11;
    pub const REVERB:     usize = 12;
    pub const MASTER:     usize = 13;
    /// Aantal gain-kanalen
    pub const LEN:        usize = 14;
    /// Geen gain maar een instelling, direct na de kanalen: sample-and-hold van de
    /// bliksem in samples (1 = uit, max 16)
    pub const LIGHTNING_HOLD: usize = 14;
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
    /// `sour`: verstemming als frequentieverhouding (1 = zuiver).
    fn control(&mut self, dt: f32, tempo: f32, beat_hz: f32, sour: f32) -> f32 {
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
        self.freq_l.set_value(self.base * sour - beat_hz * 0.5);
        self.freq_r.set_value(self.base * sour + beat_hz * 0.5);
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
// aardkorst, en nog dieper tektonisch gerommel dat los van de uitbarstingen
// onregelmatig aanzwelt en wegebt. Hoe heter, hoe actiever.

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
/// Meegeschaald met de wortel van de Q, zodat het niveau gelijk blijft.
const VENT_PIPE_GAIN: f32 = 50.0;
/// Kwaliteit (smalheid) van de pijpresonantie: hoger = meer toon, minder gerommel.
/// Was 12: dat bruisende, golvende gerommel plus de stijgende toon klonk als een
/// doortrekkende wc. Nu smaller, als een verre, rustige toon.
const VENT_PIPE_Q: f32 = 30.0;
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
/// Aardbevingsgerommel: zeer lage ruis (< 60 Hz) die bij aanzwellen door een
/// zachte verzadiging gaat, zodat er een schurend, malend randje bij komt (ook
/// hoorbaar op speakers zonder diepe bas). Drive vóór de verzadiging, niveau erna.
const QUAKE_DRIVE: f32 = 6.0;
const QUAKE_GAIN:  f32 = 0.25;
/// Onderstroom (altijd aanwezig) en de sterkte van een aanzwelling (bereik).
const QUAKE_FLOOR: f32 = 0.15;
const QUAKE_SWELL: (f32, f32) = (0.5, 1.0);

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
    /// Diep tektonisch gerommel dat los van de uitbarstingen aanzwelt en wegebt
    quake_net:  Box<dyn AudioUnit>,
    quake_amp:  Shared,
    /// Lopende aanzwelling: tijd erin, opkomst, uitdoving, piek; en tijd tot de volgende
    quake:      (f32, f32, f32, f32),
    quake_timer: f32,
    /// Sissende straal (ruis 1.8–7 kHz, per kanaal eigen ruis), gepand naar de bron
    hiss_net:   Box<dyn AudioUnit>,
    hiss_amp:   Shared,
    /// Pijpresonanties per kanaal: grondtoon en tweede modus
    pipe:       [[Bandpass; 2]; 2],
    bubbles:    BubbleStream,
    rng:        Rng,
    sample_rate: f32,
    /// Laatst geziene eruption_id; None tot de eerste parameters binnen zijn
    last_eruption: Option<f32>,
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
        let quake_amp = shared(0.0);
        let quake = || {
            ((brown() >> lowpass_hz(60.0, 0.9)) * QUAKE_DRIVE * (var(&quake_amp) >> follow(0.3))
                >> shape(Tanh(1.0)) >> lowpass_hz(200.0, 0.7)) * QUAKE_GAIN
        };
        let hiss = || {
            (noise() >> highpass_hz(1800.0, 0.6) >> lowpass_hz(7000.0, 0.6))
                * (var(&hiss_amp) >> follow(0.08))
        };
        Self {
            rumble_net: prepare(Box::new(rumble() | rumble()), sample_rate, seed),
            rumble_amp,
            quake_net: prepare(Box::new(quake() | quake()), sample_rate, seed + 2),
            quake_amp,
            quake: (0.0, 1.0, 1.0, 0.0),
            quake_timer: 5.0,
            hiss_net: prepare(Box::new(hiss() | hiss()), sample_rate, seed + 1),
            hiss_amp,
            pipe: [[Bandpass::default(); 2]; 2],
            bubbles: BubbleStream::new(sample_rate, VENT_BUBBLE_LP),
            rng: Rng(0x3C6E_F372 ^ seed as u32),
            sample_rate,
            last_eruption: None,
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

    /// Eén uitbarsting, gepland door de simulatie (zelfde moment als de bubbels
    /// op scherm en de lava op de neopixel-ring).
    fn erupt(&mut self, p: &DroneParams, warmth: f32) {
        let rng = &mut self.rng;
        self.erupting = true;
        self.t = 0.0;
        self.hold = p.eruption_hold.max(0.5);
        self.release = p.eruption_release.max(0.5);
        // Per uitbarsting een eigen volume tussen −20 en −5 dB: de ene bron klinkt
        // dichtbij, de andere ver weg
        let level_db = VENT_LEVEL_DB.0 + (VENT_LEVEL_DB.1 - VENT_LEVEL_DB.0) * p.eruption_strength;
        self.strength = 10f32.powf(level_db / 20.0) * (0.6 + 0.4 * warmth);
        let degree = VENT_PIPE_LOW + (rng.next_u32() % (VENT_PIPE_HIGH - VENT_PIPE_LOW + 1) as u32) as i32;
        self.pipe_freq = degree_hz(degree);
        self.pan = p.eruption_pan * 0.8;
    }

    /// `depth`: 1 = diepzee, 0 = getijdenpoel. `warmth`: temperatuur 0..1.
    fn control(&mut self, dt: f32, depth: f32, warmth: f32, p: &DroneParams) {
        // Traag ademend achtergrondgerommel (~25 s)
        self.breath = (self.breath + dt / 25.0 * TAU) % TAU;
        let background = 0.06 + 0.025 * self.breath.sin();

        // Nieuwe uitbarsting van de simulatie — maar niet een oude die al voorbij
        // was toen de synth (opnieuw) startte
        if self.last_eruption != Some(p.eruption_id) {
            if self.last_eruption.is_some() && p.eruption_age < 0.5 && depth >= 0.5 {
                self.erupt(p, warmth);
            }
            self.last_eruption = Some(p.eruption_id);
        }
        if depth < 0.5 {
            self.erupting = false;
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

        // Pijp-toon op vaste hoogte: meestijgen met de druk klonk als een
        // vollopend reservoir
        let f = self.pipe_freq;
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

        // Tektonisch gerommel: onregelmatige aanzwellingen (elke ~8–25 s, 1.5–4 s
        // opkomen, 4–9 s wegebben) boven een altijd aanwezige onderstroom; een
        // uitbarsting duwt het iets op
        self.quake_timer -= dt;
        if self.quake_timer <= 0.0 {
            let rng = &mut self.rng;
            self.quake = (0.0, rng.range(1.5, 4.0), rng.range(4.0, 9.0), rng.range(QUAKE_SWELL.0, QUAKE_SWELL.1));
            self.quake_timer = rng.range(8.0, 25.0);
        }
        let (qt, qrise, qfall, qpeak) = &mut self.quake;
        *qt += dt;
        let swell = if *qt < *qrise {
            0.5 - 0.5 * (std::f32::consts::PI * *qt / *qrise).cos()
        } else {
            (-(*qt - *qrise) / (*qfall * 0.4)).exp()
        };
        let quake = QUAKE_FLOOR * (1.0 + 0.3 * self.breath.cos()) + *qpeak * swell + 0.3 * p;
        self.quake_amp.set_value(quake);
        self.hiss_amp.set_value(p * self.flutter.0 * VENT_HISS_GAIN);
    }

    /// Geeft (gerommel + straal + pijp-toon, bellen) apart terug: de bellen krijgen
    /// meer galm en minder direct geluid, zodat ze verder weg klinken.
    #[inline]
    fn render(&mut self) -> ((f32, f32), (f32, f32)) {
        let (mut l, mut r) = self.rumble_net.get_stereo();
        let (ql, qr) = self.quake_net.get_stereo();
        l += ql;
        r += qr;

        let p = self.pressure;
        let (gl, gr) = pan_gains(self.pan);
        // Sissende straal vanaf de plek van de bron
        let (hl, hr) = self.hiss_net.get_stereo();
        l += hl * gl * std::f32::consts::SQRT_2;
        r += hr * gr * std::f32::consts::SQRT_2;

        // Pijp-toon: zacht en vloeiend onder het gesis. Zonder de golving van de
        // straal — die gaf de toon een gorgelend, doorspoelend karakter.
        if p > 1e-4 {
            let drive = p * VENT_PIPE_GAIN;
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
//   - vaart: een klont die snel over het scherm schiet zoeft hoorbaar langs — een
//     ruisvlaag die helderder wordt naarmate hij sneller gaat, een lage kielzog-rommel
//     voor grote klonten, een spoor belletjes en een helderder toon; de plek volgt
//     hem dan vlot, zodat je hem van de ene naar de andere kant hoort gaan
//   - alles op de plek van de klont; een nieuwe klont zwelt in, een verdwenen klont
//     sterft weg
// Daarbovenop wat er tússen klonten gebeurt: klotsend water zolang ze naderen of
// uiteendrijven, en het moment zelf:
//   - samensmelten: een "gloep" (een ronde, vloeibare toon die snel een stukje omhoog
//     buigt, als een dikke bel die bovenkomt) en een opborreling; de nieuwe klont bloeit
//     op (zijn toon zwelt aan en wordt voller) en galm en pad zwellen mee, de pad gaat
//     naar een nieuw akkoord
//   - uiteenvallen: een kort bruisen van fijne belletjes (scheuren), twee kleinere
//     gloeps kort na elkaar die elk een kant op gaan, en beide brokken bloeien op
//   - bij beide wijkt de rest van het geluid even (ducking), zodat het moment opvalt

/// Klont-stemmen: toon, zoeven en borrelen
const VOICE_TONE_GAIN:   f32 = 0.05;
const VOICE_SWISH_GAIN:  f32 = 0.25;
const VOICE_BUBBLE_RATE: f32 = 30.0;
/// Toonhoogte van een stem in pentatonische treden: grootste klont .. kleinste
const VOICE_DEGREES: (i32, i32) = (5, 14);
/// In- en uitfaden van een stem (s)
const VOICE_FADE_IN:  f32 = 0.6;
const VOICE_FADE_OUT: f32 = 1.2;
/// Opbloeien van een stem na samensmelten of uiteenvallen: zoveel keer luider op het
/// hoogtepunt, aanzwellen en wegebben (s)
const BLOOM_GAIN:  f32 = 6.0;
const BLOOM_RISE:  f32 = 0.12;
const BLOOM_DECAY: f32 = 2.5;
/// Vaart: zoevende ruisvlaag, kielzog-rommel, spoor belletjes, helderder toon
const RUSH_WHOOSH_GAIN: f32 = 2.0;
const RUSH_WHOOSH_FREQ: (f32, f32) = (350.0, 2200.0);
const RUSH_WAKE_GAIN:   f32 = 7.0;
const RUSH_BUBBLE_RATE: f32 = 35.0;
const RUSH_TONE:        f32 = 1.5;
/// Borrelen zolang klonten naderen of uiteendrijven (bewust bescheiden: dat gebeurt
/// vaak, en het moment van samensmelten of uiteenvallen moet erbovenuit komen)
const EVENT_BUBBLE_RATE:  f32 = 25.0;
/// Het moment zelf: een opborreling van zoveel bellen over EVENT_BURST_TIME s, een
/// "gloep" van een paar dikke bellen en een golf klotsend water die nadeint
const EVENT_BURST:        f32 = 70.0;
const EVENT_BURST_TIME:   f32 = 0.8;
const EVENT_WAVE_DECAY:   f32 = 1.5;
/// Gloep: begintoon (Hz, groot → laag), hoe ver en hoe snel hij omhoog buigt (snel:
/// een trage stijging klonk eerder als een laser), wegsterven (s) en sterkte. Een
/// doffe ruisdreun klonk als "poenk" en paste niet.
const GLOOP_FREQ:   (f32, f32) = (300.0, 170.0);
const GLOOP_BEND:   f32 = 1.55;
const GLOOP_GLIDE:  f32 = 0.035;
const GLOOP_DECAY:  f32 = 0.2;
const GLOOP_GAIN:   f32 = 3.0;
const MAX_GLOOPS:   usize = 4;
/// Bruisen bij uiteenvallen: zoveel fijne belletjes in FIZZ_TIME s
const FIZZ_BUBBLES: f32 = 30.0;
const FIZZ_TIME:    f32 = 0.35;
/// Ducking op het moment: zoveel zakt de rest weg bij volle sterkte (0.6 ≈ −8 dB),
/// en zo snel herstelt het (s)
const DUCK_DEPTH:   f32 = 0.6;
const DUCK_RECOVER: f32 = 0.9;
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
    /// Doel- en afgevlakte waarden: aanwezigheid, plaats, beweging, vaart, onrust
    target:    BlobVoice,
    level:     f32,
    pan:       f32,
    motion:    f32,
    rush:      f32,
    agitation: f32,
    /// Opbloeien: hoogte en tijd sinds de start
    bloom:     (f32, f32),
    /// Onregelmatige vlagen in het zoeven: huidig en doel
    gust:      (f32, f32),
    /// Op control-rate berekend: toonsterkte, boventonen (2e, 3e), zoeven, vaart-ruis, kielzog
    tone_amp:  f32,
    harmonics: (f32, f32),
    swish_amp: f32,
    whoosh_amp: f32,
    wake_amp:  f32,
    gains:     (f32, f32),
    swish:     [Bandpass; 2],
    whoosh:    [Bandpass; 2],
    wake:      [Bandpass; 2],
}

/// Eén gloep: een ronde toon (grondtoon + wat octaaf) die in de eerste tientallen
/// milliseconden een stukje omhoog buigt en dan wegsterft, met een zachte aanzet.
#[derive(Clone, Copy, Default)]
struct Gloop {
    active: bool,
    /// Samples wachten voor hij begint
    wait:   u32,
    phase:  f32,
    freq:   f32,
    target: f32,
    glide:  f32,
    amp:    f32,
    decay:  f32,
    rise:   f32,
    rise_coef: f32,
    gains:  (f32, f32),
}

impl Gloop {
    fn new(freq: f32, strength: f32, pan: f32, delay: f32, sample_rate: f32) -> Self {
        let (l, r) = pan_gains(pan);
        Self {
            active: true,
            wait:   (delay * sample_rate) as u32,
            phase:  0.0,
            freq,
            target: freq * GLOOP_BEND,
            glide:  1.0 - (-1.0 / (GLOOP_GLIDE * sample_rate)).exp(),
            amp:    strength * GLOOP_GAIN,
            decay:  (-1.0 / (GLOOP_DECAY * sample_rate)).exp(),
            rise:   1.0,
            rise_coef: (-1.0 / (0.004 * sample_rate)).exp(),
            gains:  (l, r),
        }
    }

    #[inline]
    fn render(&mut self, dt: f32) -> (f32, f32) {
        if self.wait > 0 {
            self.wait -= 1;
            return (0.0, 0.0);
        }
        self.freq += (self.target - self.freq) * self.glide;
        self.phase = (self.phase + self.freq * dt) % 1.0;
        self.amp *= self.decay;
        self.rise *= self.rise_coef;
        let a = self.phase * TAU;
        let s = (a.sin() + 0.3 * (2.0 * a).sin()) * self.amp * (1.0 - self.rise);
        if self.amp < 1e-4 {
            self.active = false;
        }
        (s * self.gains.0, s * self.gains.1)
    }
}

struct Happenings {
    voices:      [VoiceState; BLOB_VOICES],
    bubbles:     BubbleStream,
    rng:         Rng,
    sample_rate: f32,
    sample_dt:   f32,
    last_merge:  Option<f32>,
    last_split:  Option<f32>,
    /// Stemmen die moeten opbloeien maar (nog) niet klinken: (id, sterkte, wachttijd)
    blooms:      [(f32, f32, f32); 3],
    /// Afgevlakte activiteit en positie: naar elkaar toe, uit elkaar
    converge:    (f32, f32),
    diverge:     (f32, f32),
    /// Opborreling: nog te maken bellen, en waar
    burst:       (f32, f32),
    /// Bruisen bij uiteenvallen: nog te maken belletjes, en waar
    fizz:        (f32, f32),
    /// Gloeps op het moment van samensmelten of uiteenvallen
    gloops:      [Gloop; MAX_GLOOPS],
    /// Klotsen: banddoorlaten (L/R, eigen ruis) en een trage, onregelmatige golving
    slosh:       [Bandpass; 2],
    slosh_wave:  (f32, f32),
    /// Golf klotsend water na samensmelten of uiteenvallen: sterkte en plaats
    wave:        (f32, f32),
    /// Zwelling van galm en pad na samensmelten: hoogte en tijd sinds start
    swell:       (f32, f32),
    swell_t:     f32,
    /// Ducking: doel en huidige diepte (de rest van het geluid wijkt)
    duck:        (f32, f32),
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
            blooms: [(0.0, 0.0, 0.0); 3],
            converge: (0.0, 0.0),
            diverge: (0.0, 0.0),
            burst: (0.0, 0.0),
            fizz: (0.0, 0.0),
            gloops: [Gloop::default(); MAX_GLOOPS],
            slosh: [Bandpass::default(); 2],
            slosh_wave: (1.0, 1.0),
            wave: (0.0, 0.0),
            swell: (0.0, 0.0),
            swell_t: 100.0,
            duck: (0.0, 0.0),
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
                    level: v.level, pan: v.pan, motion: v.motion, rush: v.rush, agitation: v.agitation,
                    gust: (1.0, 1.0),
                    ..VoiceState::default()
                };
            }
        }

        let k = (dt / 0.3).min(1.0);
        for st in self.voices.iter_mut().filter(|s| s.live) {
            if st.present {
                // Een langszoevende klont is er snel: hij is ook zo weer weg
                st.fade = (st.fade + dt / VOICE_FADE_IN * (1.0 + 3.0 * st.rush)).min(1.0);
                st.level     += (st.target.level     - st.level)     * k;
                st.motion    += (st.target.motion    - st.motion)    * k;
                st.agitation += (st.target.agitation - st.agitation) * k;
                st.rush      += (st.target.rush      - st.rush)      * (dt / 0.15).min(1.0);
                // De plek volgt een snelle klont vlot, zodat je hem hoort passeren
                st.pan       += (st.target.pan       - st.pan)       * (dt / (0.3 - 0.22 * st.rush)).min(1.0);
            } else {
                st.fade -= dt / VOICE_FADE_OUT;
                if st.fade <= 0.0 {
                    st.live = false;
                    continue;
                }
            }
            st.gains = pan_gains(st.pan);

            // Opbloeien: snel aanzwellen, langzaam wegebben
            st.bloom.1 += dt;
            let bloom = st.bloom.0 * (1.0 - (-st.bloom.1 / BLOOM_RISE).exp()) * (-st.bloom.1 / BLOOM_DECAY).exp();
            if st.bloom.1 > 6.0 * BLOOM_DECAY {
                st.bloom.0 = 0.0;
            }
            let presence = st.level * st.fade;
            st.tone_amp = presence * VOICE_TONE_GAIN * (1.0 + BLOOM_GAIN * bloom) * (1.0 + RUSH_TONE * st.rush);
            st.harmonics = (0.25 + 0.5 * bloom + 0.3 * st.rush, 0.35 * bloom + 0.15 * st.rush);

            // Kwadratisch: alleen duidelijk bewegende klonten zoeven hoorbaar
            st.swish_amp = st.motion * st.motion * presence * VOICE_SWISH_GAIN;
            let center = 500.0 + 2500.0 * st.motion;
            for bp in st.swish.iter_mut() {
                bp.set(center, 1.5, self.sample_rate);
            }

            // Vaart: vlagen (~3 per seconde), helderder naarmate sneller
            if self.rng.unit() < dt * 3.0 {
                st.gust.1 = self.rng.range(0.5, 1.3);
            }
            st.gust.0 += (st.gust.1 - st.gust.0) * (dt / 0.12).min(1.0);
            // Ook een kleine klont moet je horen langsschieten
            let rush_presence = st.rush * st.fade * (0.5 + 0.5 * st.level);
            st.whoosh_amp = RUSH_WHOOSH_GAIN * rush_presence * st.gust.0;
            st.wake_amp = RUSH_WAKE_GAIN * rush_presence * (0.2 + 0.8 * st.target.size);
            let (lo, hi) = RUSH_WHOOSH_FREQ;
            let center = lo * (hi / lo).powf(st.rush * (0.85 + 0.15 * st.gust.0));
            for bp in st.whoosh.iter_mut() {
                bp.set(center, 0.8, self.sample_rate);
            }
            for bp in st.wake.iter_mut() {
                bp.set(100.0 + 80.0 * st.rush, 0.8, self.sample_rate);
            }
        }

        // Borrelen naar de onrust van elke klont, en een spoor belletjes achter een
        // langszoevende klont
        let mut spawns = [(0.0f32, 0.0f32, -1.0f32); 2 * BLOB_VOICES];
        for (i, st) in self.voices.iter().filter(|s| s.live).enumerate() {
            let rate = VOICE_BUBBLE_RATE * st.agitation * st.level * st.fade;
            if self.rng.unit() < rate * dt {
                spawns[2 * i] = (st.pan, (0.4 + 0.6 * st.agitation) * st.level, -1.0);
            }
            if self.rng.unit() < RUSH_BUBBLE_RATE * st.rush * st.fade * dt {
                spawns[2 * i + 1] = (st.pan, 0.7 * (0.5 + 0.5 * st.level), 0.4 * self.rng.unit().powi(2));
            }
        }
        for (pan, strength, size) in spawns {
            if strength > 0.0 {
                if size < 0.0 {
                    self.bubbles.spawn(&mut self.rng, pan, 0.1, strength);
                } else {
                    self.bubbles.spawn_sized(&mut self.rng, pan, 0.15, strength, size);
                }
            }
        }
    }

    /// Laat de stem van klont `id` opbloeien; klinkt hij nog niet, dan zodra hij er is.
    fn bloom(&mut self, id: f32, strength: f32) {
        if id <= 0.0 {
            return;
        }
        if let Some(slot) = self.blooms.iter_mut().min_by(|a, b| a.2.total_cmp(&b.2)) {
            *slot = (id, strength, 1.5);
        }
    }

    fn apply_blooms(&mut self, dt: f32) {
        for b in self.blooms.iter_mut().filter(|b| b.2 > 0.0) {
            if let Some(st) = self.voices.iter_mut().find(|s| s.live && s.present && s.id == b.0) {
                st.bloom = (st.bloom.0.max(b.1), 0.0);
                // Een afgesplitst brok is er meteen, niet pas na het inzwellen
                st.fade = st.fade.max(0.7);
                b.2 = 0.0;
            } else {
                b.2 -= dt;
            }
        }
    }

    fn control(&mut self, dt: f32, p: &DroneParams) {
        self.update_voices(dt, &p.voices);

        // Samensmelten of uiteenvallen (alleen als het vers is, zie Storm::control)
        if self.last_merge != Some(p.merge_id) {
            if self.last_merge.is_some() && p.merge_age < 0.5 && p.merge_strength > 0.01 {
                let (s, pan) = (p.merge_strength, p.merge_pan);
                // Een gloep op de plek van de nieuwe klont, groot = lager
                let size = self.voice_size(p.merge_voice).unwrap_or((s - 0.7) / 0.3);
                self.moment(s, pan, &[(pan, size, 0.0)]);
                self.duck.0 = self.duck.0.max(s * DUCK_DEPTH);
                self.bloom(p.merge_voice, s);
                self.swell = (self.swell.0.max(s), 0.0);
                self.swell_t = 0.0;
                self.chord_change = true;
            }
            self.last_merge = Some(p.merge_id);
        }
        if self.last_split != Some(p.split_id) {
            if self.last_split.is_some() && p.split_age < 0.5 && p.split_strength > 0.01 {
                let (s, pan) = (p.split_strength, p.split_pan);
                // Scheuren: fijn bruisen, dan twee kleinere gloeps kort na elkaar die
                // elk een kant op gaan
                self.fizz = (FIZZ_BUBBLES * s, pan);
                self.duck.0 = self.duck.0.max(0.8 * s * DUCK_DEPTH);
                let size_a = self.voice_size(p.split_voice_a).unwrap_or(0.5) * 0.7;
                let size_b = self.voice_size(p.split_voice_b).unwrap_or(0.3) * 0.7;
                self.moment(s, pan, &[(pan - 0.35, size_a, 0.05), (pan + 0.35, size_b, 0.16)]);
                self.bloom(p.split_voice_a, 0.8 * s);
                self.bloom(p.split_voice_b, 0.8 * s);
                self.swell = (self.swell.0.max(0.6 * s), 0.0);
                self.swell_t = 0.0;
            }
            self.last_split = Some(p.split_id);
        }
        self.apply_blooms(dt);
        self.wave.0 *= (-dt / EVENT_WAVE_DECAY).exp();
        // Ducking: snel weg (~20 ms, zonder klik), rustig terug
        self.duck.1 += (self.duck.0 - self.duck.1) * (dt / 0.02).min(1.0);
        self.duck.0 *= (-dt / DUCK_RECOVER).exp();

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

        // Bellen bij naderen/uiteendrijven, plus de opborreling en het bruisen
        for (act, pan) in [self.converge, self.diverge] {
            if self.rng.unit() < EVENT_BUBBLE_RATE * act * dt {
                self.bubbles.spawn(&mut self.rng, pan, 0.25, (0.4 + 0.6 * act).min(1.0));
            }
        }
        if self.burst.0 >= 1.0 && self.rng.unit() < EVENT_BURST / EVENT_BURST_TIME * dt {
            self.burst.0 -= 1.0;
            self.bubbles.spawn(&mut self.rng, self.burst.1, 0.3, 1.0);
        }
        let fizz_rate = FIZZ_BUBBLES / FIZZ_TIME * dt;
        let mut n = fizz_rate.floor() + if self.rng.unit() < fizz_rate.fract() { 1.0 } else { 0.0 };
        while self.fizz.0 >= 1.0 && n > 0.0 {
            self.fizz.0 -= 1.0;
            n -= 1.0;
            let size = 0.15 * self.rng.unit();
            self.bubbles.spawn_sized(&mut self.rng, self.fizz.1, 0.2, 1.0, size);
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

    /// Het moment van samensmelten of uiteenvallen: gloeps (plek, grootte 0..1,
    /// vertraging in s), een opborreling en een golf klotsend water rond `pan`.
    fn moment(&mut self, strength: f32, pan: f32, gloops: &[(f32, f32, f32)]) {
        for &(at, size, delay) in gloops {
            let freq = GLOOP_FREQ.0 * (GLOOP_FREQ.1 / GLOOP_FREQ.0).powf(size.clamp(0.0, 1.0))
                * self.rng.range(0.95, 1.05);
            if let Some(g) = self.gloops.iter_mut().find(|g| !g.active) {
                *g = Gloop::new(freq, strength, at.clamp(-1.0, 1.0), delay, self.sample_rate);
            }
        }
        self.burst = (self.burst.0 + EVENT_BURST * strength, pan);
        self.wave = (self.wave.0.max(strength), pan);
    }

    /// Grootte van de stem van klont `id`, als die klinkt.
    fn voice_size(&self, id: f32) -> Option<f32> {
        self.voices.iter().find(|v| v.live && v.id == id && id > 0.0).map(|v| v.target.size)
    }

    /// Hoeveel de galm en de pad nu extra opzwellen [0, 1].
    fn swell(&self) -> f32 {
        self.swell.1
    }

    /// Hoeveel de rest van het geluid nu wijkt [0, DUCK_DEPTH].
    fn duck(&self) -> f32 {
        self.duck.1
    }

    fn active(&self) -> bool {
        self.voices.iter().any(|v| v.live) || self.converge.0 > 1e-3 || self.diverge.0 > 1e-3
            || self.burst.0 >= 1.0 || self.fizz.0 >= 1.0 || self.gloops.iter().any(|g| g.active) || self.duck.1 > 1e-3
            || self.wave.0 > 1e-3 || self.bubbles.active()
    }

    #[inline]
    fn render(&mut self) -> (f32, f32) {
        let (mut l, mut r) = self.bubbles.render(EVENT_BUBBLE_GAIN);
        let dt = self.sample_dt;

        // Klont-stemmen: toon (met trilling naar onrust), zoeven (naar beweging) en
        // langszoeven (naar vaart)
        for st in self.voices.iter_mut().filter(|s| s.live) {
            st.trill = (st.trill + 5.0 * dt) % 1.0;
            let wobble = 1.0 + 0.004 * st.agitation * (st.trill * TAU).sin();
            st.phase = (st.phase + st.freq * wobble * dt) % 1.0;
            let a = st.phase * TAU;
            let tone = (a.sin() + st.harmonics.0 * (2.0 * a).sin() + st.harmonics.1 * (3.0 * a).sin())
                * st.tone_amp;
            let mut sl = tone;
            let mut sr = tone;
            if st.swish_amp > 1e-5 || st.whoosh_amp > 1e-5 {
                let (nl, nr) = (self.rng.range(-1.0, 1.0), self.rng.range(-1.0, 1.0));
                sl += st.swish[0].process(nl) * st.swish_amp;
                sr += st.swish[1].process(nr) * st.swish_amp;
                if st.whoosh_amp > 1e-5 {
                    sl += st.whoosh[0].process(nl) * st.whoosh_amp + st.wake[0].process(nl) * st.wake_amp;
                    sr += st.whoosh[1].process(nr) * st.whoosh_amp + st.wake[1].process(nr) * st.wake_amp;
                }
            }
            l += sl * st.gains.0;
            r += sr * st.gains.1;
        }

        // Gloeps
        for g in self.gloops.iter_mut().filter(|g| g.active) {
            let (gl, gr) = g.render(dt);
            l += gl;
            r += gr;
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

// ─── 10. Chaos ───────────────────────────────────────────────────────────────
//
// Bij hoge chaos (deeltjes die alle kanten op schieten zonder dat er orde ontstaat,
// zie sonification::ChaosMeter) verliest het geluid zijn rust:
//   - knetteren: korte droge tikjes en knispers (ruisstootjes zonder toonhoogte of
//     glijden), verspreid over het stereobeeld en in grillige vlagen, als botsende
//     deeltjes of hagel op het water; steeds dichter naarmate de chaos toeneemt.
//     (Eerder waren dit zoevende ruisstrepen met Doppler-val; die klonken te veel als
//     "space sounds".)
//   - kolken: rusteloze ruis waarvan de klankkleur en sterkte grillig verspringen
//   - spanning: een lage, schurende dissonant (kleine secunde en tritonus tegen de
//     C van de pad) die onregelmatig zweeft
// Daarnaast raakt de pad uit de stemming en gaat alles wat sneller (DroneSynth).
// Onder CHAOS_ONSET blijft deze laag stil: lichte onrust hoort bij het normale beeld.

const MAX_TICKS: usize = 24;
/// Tikjes per seconde bij volle chaos (gemiddeld; de vlagen variëren dat 0.2–1.8×)
const TICK_RATE: f32 = 220.0;
/// Middenfrequentie van een tikje (Hz, logaritmisch bereik)
const TICK_FREQ: (f32, f32) = (700.0, 6000.0);
/// Uitsterftijd van een tikje (s): kort en droog
const TICK_DECAY: (f32, f32) = (0.0006, 0.004);
const TICK_GAIN: f32 = 5.0;
const TURBULENCE_GAIN: f32 = 0.35;
/// Tonen van de spanning (Hz): Db2, F#2 en G2 een tikje te laag
const TENSION_FREQS: [f32; 3] = [69.3, 92.5, 97.0];
const TENSION_GAIN: f32 = 0.22;
/// Chaoswaarde waaronder deze laag zwijgt, en waarboven hij vol klinkt
const CHAOS_ONSET: (f32, f32) = (0.15, 0.8);
/// Maximale verstemming van de pad-tonen bij volle chaos (cent)
const PAD_SOUR_CENTS: f32 = 45.0;

#[derive(Clone, Copy, Default)]
struct Tick {
    active: bool,
    amp:    f32,
    /// Afname van de amplitude per sample
    decay:  f32,
    bp:     Bandpass,
    gains:  (f32, f32),
}

struct Turmoil {
    rng:     Rng,
    sample_rate: f32,
    /// Sterkte [0, 1] na de inzetdrempel
    level:   f32,
    ticks:   [Tick; MAX_TICKS],
    /// Vlagen: trage, grillige variatie van de tikdichtheid (huidig, doel)
    gust:    (f32, f32),
    /// Kolken: banddoorlaat per kanaal, huidige en doel-middenfrequentie, sterkte,
    /// en tijd tot de volgende sprong
    swirl:   [Bandpass; 2],
    swirl_freq: (f32, f32),
    swirl_amp:  (f32, f32),
    swirl_timer: f32,
    /// Spanning: fase, verstemming (huidig, doel) per toon
    tension: [(f32, f32, f32); 3],
    tension_timer: f32,
    /// Zacht laagdoorlaatfilter over de spanning
    tension_lp: f32,
}

impl Turmoil {
    fn new(sample_rate: f32, seed: u64) -> Self {
        Self {
            rng: Rng(0x5851_F42D ^ seed as u32),
            sample_rate,
            level: 0.0,
            ticks: [Tick::default(); MAX_TICKS],
            gust: (1.0, 1.0),
            swirl: [Bandpass::default(); 2],
            swirl_freq: (800.0, 800.0),
            swirl_amp: (0.0, 0.0),
            swirl_timer: 0.0,
            tension: [(0.0, 0.0, 0.0); 3],
            tension_timer: 0.0,
            tension_lp: 0.0,
        }
    }

    fn tick(&mut self) {
        let rng = &mut self.rng;
        let Some(t) = self.ticks.iter_mut().find(|t| !t.active) else { return };
        let (lo, hi) = TICK_FREQ;
        let q = rng.range(0.8, 2.5);
        let mut bp = Bandpass::default();
        bp.set(lo * (hi / lo).powf(rng.unit()), q, self.sample_rate);
        let (gl, gr) = pan_gains(rng.range(-1.0, 1.0));
        // Veel zachte, af en toe een harde
        let a = rng.unit().powi(2) * q.sqrt();
        *t = Tick {
            active: true,
            amp:    1.0,
            decay:  (-1.0 / (rng.range(TICK_DECAY.0, TICK_DECAY.1) * self.sample_rate)).exp(),
            bp,
            gains:  (gl * a, gr * a),
        };
    }

    fn control(&mut self, dt: f32, chaos: f32) {
        let x = ((chaos - CHAOS_ONSET.0) / (CHAOS_ONSET.1 - CHAOS_ONSET.0)).clamp(0.0, 1.0);
        self.level = x * x * (3.0 - 2.0 * x);
        let level = self.level;
        let sr = self.sample_rate;

        // Knetteren: dichtheid groeit kwadratisch, zodat het pas bij echte chaos druk
        // wordt, en komt in grillige vlagen
        if self.rng.unit() < TICK_RATE * self.gust.0 * level * level * dt {
            self.tick();
        }
        self.gust.0 += (self.gust.1 - self.gust.0) * (dt / 0.15).min(1.0);

        // Kolken: grillige sprongen in klankkleur en sterkte, sneller bij meer chaos
        self.swirl_timer -= dt;
        if self.swirl_timer <= 0.0 {
            let rng = &mut self.rng;
            self.swirl_timer = rng.range(0.06, 0.3) * (1.5 - level);
            self.swirl_freq.1 = 250.0 * 10f32.powf(rng.unit());
            self.swirl_amp.1 = rng.range(0.2, 1.0);
            self.gust.1 = rng.range(0.2, 1.8);
        }
        let k = (dt / 0.06).min(1.0);
        self.swirl_freq.0 += (self.swirl_freq.1 - self.swirl_freq.0) * k;
        self.swirl_amp.0 += (self.swirl_amp.1 - self.swirl_amp.0) * k;
        // Links en rechts net iets anders, zodat het rondkolkt in plaats van in het midden staat
        self.swirl[0].set(self.swirl_freq.0 * 0.9, 1.6, sr);
        self.swirl[1].set(self.swirl_freq.0 * 1.1, 1.6, sr);

        // Spanning: elke toon zweeft onregelmatig tot ±30 cent
        self.tension_timer -= dt;
        if self.tension_timer <= 0.0 {
            self.tension_timer = self.rng.range(0.4, 1.5);
            for t in self.tension.iter_mut() {
                t.2 = self.rng.range(-30.0, 30.0);
            }
        }
        let k = (dt / 0.8).min(1.0);
        for t in self.tension.iter_mut() {
            t.1 += (t.2 - t.1) * k;
        }
    }

    fn active(&self) -> bool {
        self.level > 1e-3 || self.ticks.iter().any(|t| t.active)
    }

    #[inline]
    fn render(&mut self) -> (f32, f32) {
        let dt = 1.0 / self.sample_rate;
        let (mut l, mut r) = (0.0f32, 0.0f32);

        let n = self.rng.range(-1.0, 1.0);
        for t in self.ticks.iter_mut().filter(|t| t.active) {
            let y = t.bp.process(n) * t.amp * TICK_GAIN;
            l += y * t.gains.0;
            r += y * t.gains.1;
            t.amp *= t.decay;
            if t.amp < 1e-3 {
                t.active = false;
            }
        }

        let level = self.level;
        if level > 1e-3 {
            let a = level * level.sqrt() * self.swirl_amp.0 * TURBULENCE_GAIN;
            l += self.swirl[0].process(self.rng.range(-1.0, 1.0)) * a;
            r += self.swirl[1].process(self.rng.range(-1.0, 1.0)) * a;

            let mut tone = 0.0f32;
            for (t, &f) in self.tension.iter_mut().zip(TENSION_FREQS.iter()) {
                t.0 = (t.0 + f * 2.0f32.powf(t.1 / 1200.0) * dt) % 1.0;
                let p = t.0 * TAU;
                tone += p.sin() + 0.35 * (2.0 * p).sin() + 0.15 * (3.0 * p).sin();
            }
            self.tension_lp += (tone - self.tension_lp) * 0.06;
            let y = self.tension_lp * level * level * TENSION_GAIN;
            l += y;
            r += y;
        }
        (l, r)
    }
}

// ─── DroneSynth ──────────────────────────────────────────────────────────────

/// Mengniveau van de branding (−5 dB t.o.v. de eerdere 0.22).
const SURF_LEVEL: f32 = 0.124;
/// Mengniveau van de klont-gebeurtenissen (−10 dB; op 1.0 waren ze veel te hard).
const EVENTS_LEVEL: f32 = 0.316;
/// Mengniveau van het onweer.
const STORM_LEVEL: f32 = 0.3;
/// Mengniveau van de sonar.
const SONAR_LEVEL: f32 = 0.5;
/// Mengniveau van de hydrothermale bronnen.
const VENT_LEVEL: f32 = 0.55;
/// Mengniveau van de chaos-laag.
const CHAOS_LEVEL: f32 = 0.2;

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
    turmoil:  Turmoil,
    /// Verstemming van de pad-tonen bij chaos: huidig en doel (cent), tijd tot een nieuw doel
    pad_sour: [(f32, f32); 3],
    pad_sour_timer: f32,
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
            turmoil: Turmoil::new(sample_rate, 191),
            pad_sour: [(0.0, 0.0); 3],
            pad_sour_timer: 0.0,
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

    /// Tempofactor: 0.1 bij de koudste temperatuur, ~0.47 halverwege, 2.2 op het heetst;
    /// chaos jaagt alles tot 1.6× zo snel op.
    fn tempo(&self) -> f32 {
        0.1 * 22.0f32.powf(self.smooth.temperature) * (1.0 + 0.6 * self.smooth.chaos)
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
        s.merge_voice    = target.merge_voice;
        s.split_voice_a  = target.split_voice_a;
        s.split_voice_b  = target.split_voice_b;
        s.voices         = target.voices;
        s.converge       = target.converge;
        s.converge_pan   = target.converge_pan;
        s.diverge        = target.diverge;
        s.diverge_pan    = target.diverge_pan;
        // Uitbarsting: ook een gebeurtenis
        s.eruption_id       = target.eruption_id;
        s.eruption_age      = target.eruption_age;
        s.eruption_strength = target.eruption_strength;
        s.eruption_pan      = target.eruption_pan;
        s.eruption_hold     = target.eruption_hold;
        s.eruption_release  = target.eruption_release;
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
        // Chaos: de pad-tonen dwalen elk een eigen kant op uit de stemming
        self.pad_sour_timer -= dt;
        if self.pad_sour_timer <= 0.0 {
            self.pad_sour_timer = self.rng.range(0.5, 2.0);
            for s in self.pad_sour.iter_mut() {
                s.1 = self.rng.range(-1.0, 1.0) * PAD_SOUR_CENTS * p.chaos;
            }
        }
        let k_sour = (dt / 0.7).min(1.0);
        for (i, tone) in self.pad.iter_mut().enumerate() {
            let s = &mut self.pad_sour[i];
            s.0 += (s.1 - s.0) * k_sour;
            self.pad_amp[i] = tone.control(dt, tempo, beat_hz, 2.0f32.powf(s.0 / 1200.0));
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
        self.vent.control(dt, 1.0 - p.surface, warmth, &p);
        self.happenings.control(dt, &p);
        self.turmoil.control(dt, p.chaos);
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
        // klanken (in- en uitfaden met dezelfde ~3 s als de branding). In de diepzee
        // +5 dB (was 0.10)
        let cello_gain = 0.178 / (1.0 + 1.0 * self.smooth.chaos) * mx[mix::CELLOS] * (1.0 - self.smooth.surface);
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

        // Klont-gebeurtenissen (alleen ingezoomd): veel galm, ze horen bij de ruimte.
        // Op het moment van samensmelten of uiteenvallen wijkt de rest even, zodat het
        // eruit springt
        if self.happenings.active() {
            let duck = 1.0 - self.happenings.duck() * mx[mix::EVENTS].min(1.0);
            dl *= duck; dr *= duck; sl *= duck; sr *= duck;
            let (el, er) = self.happenings.render();
            let a = EVENTS_LEVEL * mx[mix::EVENTS];
            dl += el * a; dr += er * a;
            sl += el * a * 0.7; sr += er * a * 0.7;
        }

        // Chaos: vrij droog en dichtbij, zodat het niet in de ruimte oplost
        if self.turmoil.active() {
            let (cl, cr) = self.turmoil.render();
            let a = CHAOS_LEVEL * mx[mix::CHAOS];
            dl += cl * a; dr += cr * a;
            sl += cl * a * 0.3; sr += cr * a * 0.3;
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

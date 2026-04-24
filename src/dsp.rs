// src/dsp.rs
// Supersaw synthesizer via fundsp — gedeeld tussen native en WASM.
//
// Signaalketen per stem:
//   4× saw_hz (gedetuned, anti-aliased) → mix × 0.18
//   + kwint-harmonische (sine, ×1.5 basisfreq) × 0.12
//   + white noise × noise_level
//   → butterpass lowpass (cutoff gestuurd door gate)
//   → tanh saturatie
//   → pan (stereo, statisch per stem-index)
//   → × amplitude × lfo_amp
//
// LFO-laag (per sample bijgewerkt in render()):
//   - Amplitude-LFO (0.05–0.15 Hz): ademt elke stem in/uit, faseverschil per stem
//   - Pitch-LFO (0.03–0.08 Hz): ±3–7 cent frequentiedrift, organisch gevoel
//   - Kwint-harmonische volgt de pitch-LFO mee
//
// Runtime parameter-updates verlopen via `Shared` variabelen (atomair).

use fundsp::prelude32::*;
use crate::sonification::StemState;

pub const SAMPLE_RATE: u32   = 44100;
pub const BLOCK_SIZE:  usize = 512;
pub const NUM_STEMS:   usize = 7;

// A-mineur pentatonisch: A1, C2, E2, G2, A2, C3, E3
// Harmonisch rustiger dan de vorige kwint-reeks; geschikt voor ambient drone.
pub const BASE_FREQS: [f32; NUM_STEMS] = [55.0, 65.4, 82.5, 98.0, 110.0, 130.8, 164.8];

const TAU:       f32 = std::f32::consts::TAU;
const SAMPLE_DT: f32 = 1.0 / SAMPLE_RATE as f32;

const GATE_CUTOFF_MIN: f32 = 80.0;
const GATE_CUTOFF_MAX: f32 = 8000.0;

#[inline]
fn gate_to_cutoff(gate: f32) -> f32 {
    GATE_CUTOFF_MIN * (GATE_CUTOFF_MAX / GATE_CUTOFF_MIN).powf(gate.clamp(0.0, 1.0))
}

// ─── StemGraph ────────────────────────────────────────────────────────────────

pub struct StemGraph {
    net:           Box<dyn AudioUnit>,

    // Gedeelde controls voor de fundsp-graph
    freq0:         Shared,
    freq1:         Shared,
    freq2:         Shared,
    freq3:         Shared,
    harmonic_freq: Shared,  // reine kwint (×1.5) boven basisfreq
    noise_level:   Shared,
    cutoff:        Shared,
    pan_pos:       Shared,  // bijgewerkt via update(), maar pan is statisch in de graph

    pub amplitude: f32,

    // Externe toestand (bijgewerkt via update())
    base_freq:     f32,
    detune_spread: f32,  // cents-spread per paar (halvering van totale spread)

    // LFO-toestand (per-sample voortgezet in render())
    lfo_amp_phase:   f32,  // radialen
    lfo_pitch_phase: f32,
    lfo_amp_rate:    f32,  // Hz — 0.05..0.15
    lfo_pitch_rate:  f32,  // Hz — 0.03..0.08
    lfo_pitch_depth: f32,  // cent — 3..7
    lfo_amp:         f32,  // huidige waarde [0.3, 1.0]
}

impl StemGraph {
    pub fn new(base_freq: f32, stem_index: usize) -> Self {
        // Gouden-ratio verdeling zodat elke stem unieke LFO-parameters en startfase heeft.
        // Geen twee stems lopen synchroon, wat een organisch, golvend effect geeft.
        const GOLDEN: f32 = 0.618_033_9;
        let fi = stem_index as f32;

        let phase_amp   = (fi * GOLDEN).fract() * TAU;
        let phase_pitch = (fi * GOLDEN * 1.618).fract() * TAU;
        let amp_rate    = 0.05 + (fi * GOLDEN * 0.618).fract() * 0.10;  // 0.05–0.15 Hz
        let pitch_rate  = 0.03 + (fi * GOLDEN * 0.382).fract() * 0.05;  // 0.03–0.08 Hz
        let pitch_depth = 3.0  + (fi * GOLDEN).fract() * 4.0;           // 3–7 cent

        // Statische stereopositie: verdeeld over −1..1, stem 0 links, stem 6 rechts
        let static_pan = if NUM_STEMS > 1 {
            (fi / (NUM_STEMS as f32 - 1.0)) * 2.0 - 1.0
        } else {
            0.0
        };

        let freq0         = shared(base_freq * cents(-15.0));
        let freq1         = shared(base_freq * cents(-5.0));
        let freq2         = shared(base_freq * cents(5.0));
        let freq3         = shared(base_freq * cents(15.0));
        let harmonic_freq = shared(base_freq * 1.5);
        let noise_level   = shared(0.05_f32);
        let cutoff        = shared(gate_to_cutoff(0.3));
        let pan_pos       = shared(static_pan);

        // Hoofdstem: 4 gedetunede zaaggolven
        let osc_mix =
            (var(&freq0) >> saw())
            + (var(&freq1) >> saw())
            + (var(&freq2) >> saw())
            + (var(&freq3) >> saw());

        // Zachte kwint-harmonische (sinus): muzikaler en minder agressief dan een extra saw
        let harmonic = var(&harmonic_freq) >> sine();

        let mixed = osc_mix * 0.18_f32 + harmonic * 0.12_f32 + white() * var(&noise_level);

        let graph = mixed
            >> (var(&cutoff) | pass())
            >> butterpass()
            >> shape(Tanh(1.0))
            >> pan(static_pan);

        let mut net = Box::new(graph) as Box<dyn AudioUnit>;
        net.set_sample_rate(SAMPLE_RATE as f64);
        net.allocate();

        Self {
            net,
            freq0,
            freq1,
            freq2,
            freq3,
            harmonic_freq,
            noise_level,
            cutoff,
            pan_pos,
            amplitude:     0.5,
            base_freq,
            detune_spread: 15.0,
            lfo_amp_phase:   phase_amp,
            lfo_pitch_phase: phase_pitch,
            lfo_amp_rate:    amp_rate,
            lfo_pitch_rate:  pitch_rate,
            lfo_pitch_depth: pitch_depth,
            lfo_amp:         0.65 + 0.35 * phase_amp.sin(),  // start op willekeurige waarde
        }
    }

    pub fn update(&mut self, s: &StemState) {
        self.base_freq     = s.frequency;
        self.detune_spread = s.detune * 20.0;
        self.noise_level.set_value(s.noise.max(0.03));
        self.cutoff.set_value(gate_to_cutoff(s.gate));
        self.pan_pos.set_value(s.pan.clamp(-1.0, 1.0));
        self.amplitude     = s.amplitude;
        // freq0..freq3 en harmonic_freq worden in render() bijgewerkt via de LFO
    }

    #[inline]
    pub fn render(&mut self) -> (f32, f32) {
        // ── LFO-voortgang (één sample per aanroep) ──────────────────────────
        self.lfo_amp_phase   = (self.lfo_amp_phase   + self.lfo_amp_rate   * SAMPLE_DT * TAU) % TAU;
        self.lfo_pitch_phase = (self.lfo_pitch_phase + self.lfo_pitch_rate * SAMPLE_DT * TAU) % TAU;

        // Amplitude-LFO: ademt tussen 0.30 en 1.0
        self.lfo_amp = 0.65 + 0.35 * self.lfo_amp_phase.sin();

        // Pitch-LFO: lichte frequentiedrift in cents
        let pitch_mul = cents(self.lfo_pitch_depth * self.lfo_pitch_phase.sin());
        let f = self.base_freq * pitch_mul;
        let sp = self.detune_spread;

        self.freq0.set_value(f * cents(-sp * 1.5));
        self.freq1.set_value(f * cents(-sp * 0.5));
        self.freq2.set_value(f * cents( sp * 0.5));
        self.freq3.set_value(f * cents( sp * 1.5));
        self.harmonic_freq.set_value(f * 1.5);  // kwint volgt pitch-drift mee

        let (l, r) = self.net.get_stereo();
        (l * self.amplitude * self.lfo_amp, r * self.amplitude * self.lfo_amp)
    }
}

/// Cent-ratio helper: cents → frequency multiplier
#[inline]
fn cents(c: f32) -> f32 {
    2.0f32.powf(c / 1200.0)
}

// src/dsp.rs
// Laag 1: drie zaagtand-oscillatoren op C2/Eb2/G2 — de drone-basis.
// Laag 2: drie sinusoscillatoren op C3/Eb3/G3 — theremin-achtige zweeftonen.
//
// Iedere stem heeft een eigen amplitude-LFO en (voor de sinussen) een pitch-LFO.

use fundsp::prelude32::*;

pub const SAMPLE_RATE: u32   = 44100;
pub const BLOCK_SIZE:  usize = 512;

const TAU:       f32 = std::f32::consts::TAU;
const SAMPLE_DT: f32 = 1.0 / SAMPLE_RATE as f32;

// ─── Zaagtand-laag (C2/Eb2/G2) ───────────────────────────────────────────────

const SAW_FREQS:   [f32; 3] = [65.41, 77.78, 98.00];
const SAW_AMP_PERIODS: [f32; 3] = [20.0, 15.0, 10.0];

pub struct SawVoice {
    net:       Box<dyn AudioUnit>,
    lfo_phase: f32,
    lfo_rate:  f32,
}

impl SawVoice {
    fn new(freq: f32, amp_period_s: f32) -> Self {
        let freq_node = shared(freq);
        let graph = (var(&freq_node) >> saw()) >> pan(0.0);
        let mut net = Box::new(graph) as Box<dyn AudioUnit>;
        net.set_sample_rate(SAMPLE_RATE as f64);
        net.allocate();
        Self { net, lfo_phase: 0.0, lfo_rate: 1.0 / amp_period_s }
    }

    #[inline]
    pub fn render(&mut self) -> (f32, f32) {
        self.lfo_phase = (self.lfo_phase + self.lfo_rate * SAMPLE_DT * TAU) % TAU;
        let amp = 0.65 + 0.35 * self.lfo_phase.sin();
        let (l, r) = self.net.get_stereo();
        (l * amp, r * amp)
    }
}

// ─── Sinus-laag (C3/Eb3/G3) ──────────────────────────────────────────────────

// Een octaaf hoger dan de zaagstanden.
const SINE_FREQS:       [f32; 3] = [130.81, 155.56, 196.00];
const SINE_AMP_PERIODS: [f32; 3] = [25.0,   18.0,   13.0];
const SINE_PIT_PERIODS: [f32; 3] = [11.0,    8.5,    9.7];   // pitch-drift periode (s)
const SINE_PIT_DEPTH:   f32      = 15.0;                       // ±cent

pub struct SineVoice {
    net:             Box<dyn AudioUnit>,
    freq_shared:     Shared,
    base_freq:       f32,
    amp_phase:       f32,
    amp_rate:        f32,
    pitch_phase:     f32,
    pitch_rate:      f32,
}

impl SineVoice {
    fn new(freq: f32, amp_period_s: f32, pitch_period_s: f32) -> Self {
        let freq_shared = shared(freq);
        let graph = (var(&freq_shared) >> sine()) >> pan(0.0);
        let mut net = Box::new(graph) as Box<dyn AudioUnit>;
        net.set_sample_rate(SAMPLE_RATE as f64);
        net.allocate();
        Self {
            net,
            freq_shared,
            base_freq:   freq,
            amp_phase:   0.0,
            amp_rate:    1.0 / amp_period_s,
            pitch_phase: 0.0,
            pitch_rate:  1.0 / pitch_period_s,
        }
    }

    #[inline]
    pub fn render(&mut self) -> (f32, f32) {
        self.amp_phase   = (self.amp_phase   + self.amp_rate   * SAMPLE_DT * TAU) % TAU;
        self.pitch_phase = (self.pitch_phase + self.pitch_rate * SAMPLE_DT * TAU) % TAU;

        // Amplitude ademt tussen 0.20 en 0.80 — zachter dan de zaagstanden
        let amp = 0.50 + 0.30 * self.amp_phase.sin();

        // Pitch zweeft ±15 cent
        let freq = self.base_freq * cents(SINE_PIT_DEPTH * self.pitch_phase.sin());
        self.freq_shared.set_value(freq);

        let (l, r) = self.net.get_stereo();
        (l * amp, r * amp)
    }
}

// ─── Factory-functies ─────────────────────────────────────────────────────────

pub fn create_saw_voices() -> Vec<SawVoice> {
    SAW_FREQS.iter()
        .zip(SAW_AMP_PERIODS.iter())
        .map(|(&f, &p)| SawVoice::new(f, p))
        .collect()
}

pub fn create_sine_voices() -> Vec<SineVoice> {
    SINE_FREQS.iter()
        .zip(SINE_AMP_PERIODS.iter())
        .zip(SINE_PIT_PERIODS.iter())
        .map(|((&f, &ap), &pp)| SineVoice::new(f, ap, pp))
        .collect()
}

/// Cent-ratio helper
#[inline]
fn cents(c: f32) -> f32 {
    2.0f32.powf(c / 1200.0)
}

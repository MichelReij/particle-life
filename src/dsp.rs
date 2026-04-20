// src/dsp.rs
// Supersaw synthesizer via fundsp — gedeeld tussen native en WASM.
//
// Signaalketen per stem:
//   4× saw_hz (gedetuned, anti-aliased) → mix * 0.25
//   + white noise * noise_level
//   → butterpass lowpass (cutoff gestuurd door gate)
//   → tanh saturatie (hardness gestuurd door saturation)
//   → pan (stereo)
//   → * amplitude
//
// Runtime parameter-updates verlopen via `Shared` variabelen, die atomair
// beschreven kunnen worden vanuit de render-thread zonder allocatie.

use fundsp::prelude32::*;
use crate::sonification::StemState;

pub const SAMPLE_RATE: u32  = 44100;
pub const BLOCK_SIZE:  usize = 512;
pub const NUM_STEMS:   usize = 7;

pub const BASE_FREQS: [f32; NUM_STEMS] = [55.0, 82.4, 123.5, 51.9, 73.4, 58.3, 69.3];

const GATE_CUTOFF_MIN: f32 = 80.0;
const GATE_CUTOFF_MAX: f32 = 8000.0;

#[inline]
fn gate_to_cutoff(gate: f32) -> f32 {
    GATE_CUTOFF_MIN * (GATE_CUTOFF_MAX / GATE_CUTOFF_MIN).powf(gate.clamp(0.0, 1.0))
}

// ─── StemGraph ────────────────────────────────────────────────────────────────

/// Één synthesizer-stem op basis van een fundsp Net.
/// Shared-variabelen sturen alle runtime-parameters aan.
pub struct StemGraph {
    net:         Box<dyn AudioUnit>,
    // Gedeelde controls (atomair schrijfbaar vanuit render-thread)
    freq0:       Shared,
    freq1:       Shared,
    freq2:       Shared,
    freq3:       Shared,
    noise_level: Shared,
    cutoff:      Shared,
    pan_pos:     Shared,
    // Amplitude wordt buiten de graph toegepast (simpelste aanpak)
    pub amplitude: f32,
}

impl StemGraph {
    pub fn new(base_freq: f32) -> Self {
        let freq0 = shared(base_freq * cents(-15.0));
        let freq1 = shared(base_freq * cents(-5.0));
        let freq2 = shared(base_freq * cents( 5.0));
        let freq3 = shared(base_freq * cents(15.0));
        let noise_level = shared(0.1_f32);
        let cutoff  = shared(gate_to_cutoff(0.3));
        let pan_pos = shared(0.0_f32);

        let osc_mix =
            (var(&freq0) >> saw())
            + (var(&freq1) >> saw())
            + (var(&freq2) >> saw())
            + (var(&freq3) >> saw());

        let with_noise = osc_mix * 0.25_f32 + white() * var(&noise_level);

        let graph = with_noise
            >> (var(&cutoff) | pass())
            >> butterpass()
            >> shape(Tanh(1.0))
            >> pan(0.0_f32);

        let mut net = Box::new(graph) as Box<dyn AudioUnit>;
        net.set_sample_rate(SAMPLE_RATE as f64);
        net.allocate();

        Self {
            net: net,
            freq0:       freq0,
            freq1:       freq1,
            freq2:       freq2,
            freq3:       freq3,
            noise_level: noise_level,
            cutoff:      cutoff,
            pan_pos:     pan_pos,
            amplitude:   0.5,
        }
    }

    pub fn update(&mut self, s: &StemState) {
        let spread = s.detune * 20.0;
        self.freq0.set_value(s.frequency * cents(-spread * 1.5));
        self.freq1.set_value(s.frequency * cents(-spread * 0.5));
        self.freq2.set_value(s.frequency * cents( spread * 0.5));
        self.freq3.set_value(s.frequency * cents( spread * 1.5));
        self.noise_level.set_value(s.noise);
        self.cutoff.set_value(gate_to_cutoff(s.gate));
        self.pan_pos.set_value(s.pan.clamp(-1.0, 1.0));
        self.amplitude = s.amplitude;
    }

    #[inline]
    pub fn render(&mut self) -> (f32, f32) {
        let (l, r) = self.net.get_stereo();
        (l * self.amplitude, r * self.amplitude)
    }

}

/// Cent-ratio helper: cents → frequency multiplier
#[inline]
fn cents(c: f32) -> f32 {
    2.0f32.powf(c / 1200.0)
}

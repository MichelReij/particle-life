// src/dsp.rs
// Drie zaagtand-oscillatoren op een C-mineur kwintakkoord (C2, Eb2, G2).
// Iedere stem heeft een eigen volume-LFO met lange periode (20s, 15s, 10s).

use fundsp::prelude32::*;

pub const SAMPLE_RATE: u32   = 44100;
pub const BLOCK_SIZE:  usize = 512;
pub const NUM_VOICES:  usize = 3;

// C mineur kwintakkoord: C2, Eb2, G2 (gelijkzwevend)
pub const VOICE_FREQS: [f32; NUM_VOICES] = [65.41, 77.78, 98.00];
const LFO_PERIODS_S:   [f32; NUM_VOICES] = [20.0,  15.0,  10.0];

const TAU:       f32 = std::f32::consts::TAU;
const SAMPLE_DT: f32 = 1.0 / SAMPLE_RATE as f32;

pub struct Voice {
    net:       Box<dyn AudioUnit>,
    lfo_phase: f32,
    lfo_rate:  f32,  // Hz
}

impl Voice {
    pub fn new(freq: f32, lfo_period_s: f32) -> Self {
        let freq_node = shared(freq);
        let graph = (var(&freq_node) >> saw()) >> pan(0.0);

        let mut net = Box::new(graph) as Box<dyn AudioUnit>;
        net.set_sample_rate(SAMPLE_RATE as f64);
        net.allocate();

        Self {
            net,
            lfo_phase: 0.0,
            lfo_rate:  1.0 / lfo_period_s,
        }
    }

    #[inline]
    pub fn render(&mut self) -> (f32, f32) {
        self.lfo_phase = (self.lfo_phase + self.lfo_rate * SAMPLE_DT * TAU) % TAU;
        let lfo_amp = 0.65 + 0.35 * self.lfo_phase.sin();  // ademt tussen 0.30 en 1.0

        let (l, r) = self.net.get_stereo();
        (l * lfo_amp, r * lfo_amp)
    }
}

pub fn create_voices() -> Vec<Voice> {
    VOICE_FREQS.iter()
        .zip(LFO_PERIODS_S.iter())
        .map(|(&freq, &period)| Voice::new(freq, period))
        .collect()
}

// src/dsp.rs
// Gedeelde DSP-primitieven voor de supersaw synthesizer.
// Gebruikt door audio_engine.rs (native/rodio) en audio_engine_wasm.rs (WASM/Web Audio).

pub const SAMPLE_RATE: u32  = 44100;
pub const BLOCK_SIZE:  usize = 512;
pub const NUM_STEMS:   usize = 7;

/// Basisfrequenties per particle-type (kwint-interval reeks, bas-register)
pub const BASE_FREQS: [f32; NUM_STEMS] = [55.0, 82.4, 123.5, 51.9, 73.4, 58.3, 69.3];

// ─── Sawtooth-oscillator ─────────────────────────────────────────────────────

pub struct SawOscillator {
    phase:     f32,
    phase_inc: f32,
}

impl SawOscillator {
    pub fn new(freq: f32, detune_cents: f32) -> Self {
        let ratio = 2.0f32.powf(detune_cents / 1200.0);
        Self { phase: 0.0, phase_inc: freq * ratio / SAMPLE_RATE as f32 }
    }

    pub fn set_freq(&mut self, freq: f32, detune_cents: f32) {
        let ratio = 2.0f32.powf(detune_cents / 1200.0);
        self.phase_inc = freq * ratio / SAMPLE_RATE as f32;
    }

    #[inline]
    pub fn next(&mut self) -> f32 {
        let s = 2.0 * self.phase - 1.0;
        self.phase = (self.phase + self.phase_inc).fract();
        s
    }
}

// ─── Biquad resonante lowpass filter (Direct Form II) ────────────────────────

pub struct BiquadLPF {
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    w1: f32, w2: f32,
}

impl BiquadLPF {
    pub fn new() -> Self {
        let mut f = Self { b0:1.0, b1:0.0, b2:0.0, a1:0.0, a2:0.0, w1:0.0, w2:0.0 };
        f.set_cutoff(200.0, 0.7);
        f
    }

    pub fn set_cutoff(&mut self, cutoff_hz: f32, q: f32) {
        let sr    = SAMPLE_RATE as f32;
        let fc    = cutoff_hz.clamp(20.0, sr * 0.49);
        let omega = 2.0 * std::f32::consts::PI * fc / sr;
        let sin_w = omega.sin();
        let cos_w = omega.cos();
        let alpha  = sin_w / (2.0 * q.max(0.1));
        let a0i    = 1.0 / (1.0 + alpha);
        self.b0 = (1.0 - cos_w) * 0.5 * a0i;
        self.b1 = (1.0 - cos_w) * a0i;
        self.b2 = self.b0;
        self.a1 = -2.0 * cos_w * a0i;
        self.a2 = (1.0 - alpha) * a0i;
    }

    /// Gate [0,1] → cutoff 80 Hz…8 kHz (exponentieel) + lichte resonantie
    pub fn set_gate(&mut self, gate: f32) {
        let g      = gate.clamp(0.0, 1.0);
        let cutoff = 80.0 * (8000.0f32 / 80.0).powf(g);
        let q      = 0.7 + g * 0.8;
        self.set_cutoff(cutoff, q);
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let w0 = x - self.a1 * self.w1 - self.a2 * self.w2;
        let y  = self.b0 * w0 + self.b1 * self.w1 + self.b2 * self.w2;
        self.w2 = self.w1;
        self.w1 = w0;
        y
    }
}

// ─── Witte ruis (xorshift32) ──────────────────────────────────────────────────

pub struct NoiseGen { pub state: u32 }

impl NoiseGen {
    pub fn new(seed: u32) -> Self { Self { state: seed | 1 } }

    #[inline]
    pub fn next(&mut self) -> f32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        (self.state as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

// ─── Stem ─────────────────────────────────────────────────────────────────────

pub struct Stem {
    pub oscs:        [SawOscillator; 4],
    pub filter:      BiquadLPF,
    pub noise:       NoiseGen,
    pub gate:        f32,
    pub noise_level: f32,
    pub saturation:  f32,
    pub amplitude:   f32,
    pub pan:         f32,
}

impl Stem {
    pub fn new(base_freq: f32, seed: u32) -> Self {
        Self {
            oscs: [
                SawOscillator::new(base_freq, -15.0),
                SawOscillator::new(base_freq,  -5.0),
                SawOscillator::new(base_freq,   5.0),
                SawOscillator::new(base_freq,  15.0),
            ],
            filter:      BiquadLPF::new(),
            noise:       NoiseGen::new(seed),
            gate:        0.3,
            noise_level: 0.1,
            saturation:  0.2,
            amplitude:   0.5,
            pan:         0.0,
        }
    }

    pub fn update(&mut self, s: &crate::sonification::StemState) {
        let spread = s.detune * 20.0; // max ±20 cent
        let detunes = [-spread * 1.5, -spread * 0.5, spread * 0.5, spread * 1.5];
        for (osc, &d) in self.oscs.iter_mut().zip(detunes.iter()) {
            osc.set_freq(s.frequency, d);
        }
        if (s.gate - self.gate).abs() > 0.001 {
            self.filter.set_gate(s.gate);
            self.gate = s.gate;
        }
        self.noise_level = s.noise;
        self.saturation  = s.saturation;
        self.amplitude   = s.amplitude;
        self.pan         = s.pan;
    }

    #[inline]
    pub fn render(&mut self) -> (f32, f32) {
        let raw = (self.oscs[0].next()
                 + self.oscs[1].next()
                 + self.oscs[2].next()
                 + self.oscs[3].next()) * 0.25;

        let noisy    = raw + self.noise.next() * self.noise_level;
        let filtered = self.filter.process(noisy);

        // tanh-saturatie
        let drive = 1.0 + self.saturation * 4.0;
        let sat   = (filtered * drive).tanh() / drive.tanh().max(1e-6);
        let out   = sat * self.amplitude;

        // Constant-power pan
        let angle = (self.pan.clamp(-1.0, 1.0) + 1.0) * 0.5 * std::f32::consts::FRAC_PI_2;
        (out * angle.cos(), out * angle.sin())
    }
}

// src/audio_engine.rs
// Native audio engine: supersaw synthesizer via rodio.
// DSP-primitieven (SawOscillator, BiquadLPF, NoiseGen, Stem) leven in dsp.rs.

use rodio::{OutputStream, Sink, Source};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::dsp::{StemGraph, BASE_FREQS, BLOCK_SIZE, NUM_STEMS, SAMPLE_RATE};
use crate::sonification::SonificationState;

// ─── SynthSource (rodio::Source) ─────────────────────────────────────────────

pub struct SynthSource {
    state_ref:  Arc<Mutex<SonificationState>>,
    stems:      Vec<StemGraph>,
    buffer:     Vec<f32>,
    buf_pos:    usize,
    master_amp: f32,
}

impl SynthSource {
    fn new(state_ref: Arc<Mutex<SonificationState>>) -> Self {
        let stems = BASE_FREQS.iter()
            .map(|&f| StemGraph::new(f))
            .collect();
        Self {
            state_ref,
            stems,
            buffer:     vec![0.0f32; BLOCK_SIZE * 2],
            buf_pos:    BLOCK_SIZE * 2,
            master_amp: 0.5,
        }
    }

    fn refill(&mut self) {
        if let Ok(state) = self.state_ref.try_lock() {
            self.master_amp = state.master_amplitude;
            for (i, stem) in self.stems.iter_mut().enumerate() {
                stem.update(&state.stems[i]);
            }
        }

        for i in 0..BLOCK_SIZE {
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for stem in self.stems.iter_mut() {
                let (sl, sr) = stem.render();
                l += sl;  r += sr;
            }
            let scale = self.master_amp / NUM_STEMS as f32;
            self.buffer[i * 2]     = (l * scale).clamp(-1.0, 1.0);
            self.buffer[i * 2 + 1] = (r * scale).clamp(-1.0, 1.0);
        }
        self.buf_pos = 0;
    }
}

impl Iterator for SynthSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.buf_pos >= self.buffer.len() { self.refill(); }
        let s = self.buffer[self.buf_pos];
        self.buf_pos += 1;
        Some(s)
    }
}

impl Source for SynthSource {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { 2 }
    fn sample_rate(&self) -> u32 { SAMPLE_RATE }
    fn total_duration(&self) -> Option<Duration> { None }
}

// ─── AudioEngine (publieke API) ───────────────────────────────────────────────

pub struct AudioEngine {
    _stream: OutputStream,
    sink:    Sink,
    state:   Arc<Mutex<SonificationState>>,
}

impl AudioEngine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        crate::console_log!("🎵 AudioEngine: initialiseren (supersaw synthesizer)...");

        let (_stream, handle) = OutputStream::try_default()
            .map_err(|e| format!("Audio output fout: {}", e))?;

        let state       = Arc::new(Mutex::new(SonificationState::default()));
        let state_clone = Arc::clone(&state);

        let sink = Sink::try_new(&handle)
            .map_err(|e| format!("Sink fout: {}", e))?;

        sink.append(SynthSource::new(state_clone));
        sink.play();

        crate::console_log!("✅ AudioEngine actief — 7 stemmen @ 44.1 kHz, stereo");

        Ok(Self { _stream, sink, state })
    }

    /// Bijwerk de SonificationState vanuit de render-loop (non-blocking).
    pub fn update(&self, new_state: SonificationState) {
        if let Ok(mut g) = self.state.try_lock() {
            *g = new_state;
        }
    }

    pub fn set_master_volume(&self, v: f32) {
        self.sink.set_volume(v.clamp(0.0, 1.0));
    }

    pub fn set_paused(&self, paused: bool) {
        if paused { self.sink.pause(); } else { self.sink.play(); }
    }
}

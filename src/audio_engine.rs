// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// src/audio_engine.rs
// Native audio engine — ambient drone (dsp::DroneSynth) via rodio.

use rodio::{OutputStream, Sink, Source};
use std::time::Duration;

use crate::dsp::{DroneHandle, DroneParams, DroneSynth, BLOCK_SIZE, SAMPLE_RATE};

pub struct SynthSource {
    synth:   DroneSynth,
    buffer:  Vec<f32>,
    buf_pos: usize,
}

impl SynthSource {
    pub fn new(synth: DroneSynth) -> Self {
        Self {
            synth,
            buffer:  vec![0.0f32; BLOCK_SIZE * 2],
            buf_pos: BLOCK_SIZE * 2,
        }
    }

    fn refill(&mut self) {
        for frame in self.buffer.chunks_exact_mut(2) {
            let (l, r) = self.synth.render();
            frame[0] = l;
            frame[1] = r;
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

pub struct AudioEngine {
    _stream: OutputStream,
    sink:    Sink,
    drone:   DroneHandle,
}

impl AudioEngine {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        crate::console_log!("🎵 AudioEngine: ambient drone (cello, theremin, brainwaves, kabbelen)...");

        let (_stream, handle) = OutputStream::try_default()
            .map_err(|e| format!("Audio output fout: {}", e))?;

        let sink = Sink::try_new(&handle)
            .map_err(|e| format!("Sink fout: {}", e))?;

        let synth = DroneSynth::new(SAMPLE_RATE as f32);
        let drone = synth.handle();
        sink.append(SynthSource::new(synth));
        sink.play();

        crate::console_log!("✅ AudioEngine actief — drone @ 44.1 kHz, stereo");

        Ok(Self { _stream, sink, drone })
    }

    /// Besturing vanuit de simulatie (temperatuur → tempo, clusterstatistieken).
    pub fn set_drone_params(&self, p: &DroneParams) {
        self.drone.set(p);
    }

    pub fn set_master_volume(&self, v: f32) {
        self.sink.set_volume(v.clamp(0.0, 1.0));
    }

    pub fn set_paused(&self, paused: bool) {
        if paused { self.sink.pause(); } else { self.sink.play(); }
    }
}

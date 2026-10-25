// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// src/audio_engine.rs
// Native audio engine — ambient drone (fundsp + rodio), of mp3 met feature demo_audio

#[cfg(not(feature = "demo_audio"))]
mod synth {
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
}

#[cfg(not(feature = "demo_audio"))]
pub use synth::AudioEngine;

// ─── Demo-implementatie (feature = "demo_audio") ──────────────────────────────

#[cfg(feature = "demo_audio")]
mod demo {
    use rodio::{Decoder, OutputStream, Sink, Source};
    use std::fs::File;
    use std::io::BufReader;

    const MP3_PATH: &str = "assets/audio/brainwaves.mp3";

    pub struct AudioEngine {
        _stream: OutputStream,
        sink:    Sink,
    }

    impl AudioEngine {
        pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
            crate::console_log!("🎵 AudioEngine: demo-modus ({})", MP3_PATH);

            let (_stream, handle) = OutputStream::try_default()
                .map_err(|e| format!("Audio output fout: {}", e))?;

            let sink = Sink::try_new(&handle)
                .map_err(|e| format!("Sink fout: {}", e))?;

            let file = File::open(MP3_PATH)
                .map_err(|e| format!("Kan {} niet openen: {}", MP3_PATH, e))?;
            let source = Decoder::new(BufReader::new(file))
                .map_err(|e| format!("MP3 decode fout: {}", e))?;

            sink.append(source.repeat_infinite());
            sink.set_volume(0.75);
            sink.play();

            crate::console_log!("✅ AudioEngine demo actief — {} in loop", MP3_PATH);

            Ok(Self { _stream, sink })
        }

        /// De mp3 reageert niet op de simulatie.
        pub fn set_drone_params(&self, _p: &crate::dsp::DroneParams) {}

        pub fn set_master_volume(&self, v: f32) {
            self.sink.set_volume(v.clamp(0.0, 1.0));
        }

        pub fn set_paused(&self, paused: bool) {
            if paused { self.sink.pause(); } else { self.sink.play(); }
        }
    }
}

#[cfg(feature = "demo_audio")]
pub use demo::AudioEngine;

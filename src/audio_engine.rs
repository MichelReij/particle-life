// src/audio_engine.rs
// Native audio engine — twee implementaties via Cargo feature:
//   default:     supersaw synthesizer (fundsp + rodio)
//   demo_audio:  brainwaves.mp3 afspelen in loop (rodio)

// ─── Synthesizer-implementatie (default) ─────────────────────────────────────

#[cfg(not(feature = "demo_audio"))]
mod synth {
    use rodio::{OutputStream, Sink, Source};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use crate::dsp::{StemGraph, BASE_FREQS, BLOCK_SIZE, NUM_STEMS, SAMPLE_RATE};
    use crate::sonification::SonificationState;

    pub struct SynthSource {
        state_ref:  Arc<Mutex<SonificationState>>,
        stems:      Vec<StemGraph>,
        buffer:     Vec<f32>,
        buf_pos:    usize,
        master_amp: f32,
    }

    impl SynthSource {
        pub fn new(state_ref: Arc<Mutex<SonificationState>>) -> Self {
            let stems = BASE_FREQS.iter()
                .enumerate()
                .map(|(i, &f)| StemGraph::new(f, i))
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
}

#[cfg(not(feature = "demo_audio"))]
pub use synth::AudioEngine;

// ─── Demo-implementatie (feature = "demo_audio") ──────────────────────────────

#[cfg(feature = "demo_audio")]
mod demo {
    use rodio::{Decoder, OutputStream, Sink, Source};
    use std::fs::File;
    use std::io::BufReader;

    use crate::sonification::SonificationState;

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

        pub fn update(&self, _new_state: SonificationState) {
            // Demo-modus reageert niet op simulatieparameters
        }

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

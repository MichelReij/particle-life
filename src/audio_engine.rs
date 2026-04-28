// src/audio_engine.rs
// Native audio engine — supersaw synthesizer (fundsp + rodio)

#[cfg(not(feature = "demo_audio"))]
mod synth {
    use rodio::{OutputStream, Sink, Source};
    use std::time::Duration;

    use crate::dsp::{create_saw_voices, create_sine_voices, SawVoice, SineVoice, BLOCK_SIZE, SAMPLE_RATE};

    pub struct SynthSource {
        saw:     Vec<SawVoice>,
        sine:    Vec<SineVoice>,
        buffer:  Vec<f32>,
        buf_pos: usize,
    }

    impl SynthSource {
        pub fn new() -> Self {
            Self {
                saw:     create_saw_voices(),
                sine:    create_sine_voices(),
                buffer:  vec![0.0f32; BLOCK_SIZE * 2],
                buf_pos: BLOCK_SIZE * 2,
            }
        }

        fn refill(&mut self) {
            let n_saw  = self.saw.len()  as f32;
            let n_sine = self.sine.len() as f32;
            for i in 0..BLOCK_SIZE {
                let (mut l, mut r) = (0.0f32, 0.0f32);
                for v in self.saw.iter_mut()  { let (vl, vr) = v.render(); l += vl; r += vr; }
                let (sl, sr) = (l / n_saw, r / n_saw);
                let (mut tl, mut tr) = (0.0f32, 0.0f32);
                for v in self.sine.iter_mut() { let (vl, vr) = v.render(); tl += vl; tr += vr; }
                let mix = 0.35;
                self.buffer[i * 2]     = ((sl + tl / n_sine * mix) * 0.65).clamp(-1.0, 1.0);
                self.buffer[i * 2 + 1] = ((sr + tr / n_sine * mix) * 0.65).clamp(-1.0, 1.0);
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
    }

    impl AudioEngine {
        pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
            crate::console_log!("🎵 AudioEngine: C-mineur kwintakkoord (3 stems, standalone)...");

            let (_stream, handle) = OutputStream::try_default()
                .map_err(|e| format!("Audio output fout: {}", e))?;

            let sink = Sink::try_new(&handle)
                .map_err(|e| format!("Sink fout: {}", e))?;

            sink.append(SynthSource::new());
            sink.play();

            crate::console_log!("✅ AudioEngine actief — C2/Eb2/G2 @ 44.1 kHz, stereo");

            Ok(Self { _stream, sink })
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

// src/audio_worklet_exports.rs
// WASM-exports die door het AudioWorklet-processor-script worden aangeroepen.

use std::cell::RefCell;
use wasm_bindgen::prelude::*;

use crate::dsp::{create_voices, Voice, NUM_VOICES};

struct WorkletSynth {
    voices:     Vec<Voice>,
    master_amp: f32,
}

impl WorkletSynth {
    fn new() -> Self {
        Self { voices: create_voices(), master_amp: 0.65 }
    }

    fn render_block(&mut self, left: &mut [f32], right: &mut [f32]) {
        let scale = self.master_amp / NUM_VOICES as f32;
        for i in 0..left.len() {
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for voice in self.voices.iter_mut() {
                let (vl, vr) = voice.render();
                l += vl; r += vr;
            }
            left[i]  = (l * scale).clamp(-1.0, 1.0);
            right[i] = (r * scale).clamp(-1.0, 1.0);
        }
    }
}

thread_local! {
    static SYNTH: RefCell<Option<WorkletSynth>> = RefCell::new(None);
}

#[wasm_bindgen]
pub fn worklet_synth_init() {
    SYNTH.with(|s| {
        *s.borrow_mut() = Some(WorkletSynth::new());
    });
    crate::console_log!("🎵 worklet_synth_init — C-mineur kwintakkoord klaar");
}

#[wasm_bindgen]
pub fn worklet_synth_update(_params: &[f32]) {
    // Standalone patch: geen externe parameters
}

#[wasm_bindgen]
pub fn worklet_synth_render(left: &mut [f32], right: &mut [f32]) {
    SYNTH.with(|s| {
        if let Some(synth) = s.borrow_mut().as_mut() {
            synth.render_block(left, right);
        }
    });
}

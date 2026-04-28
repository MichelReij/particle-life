// src/audio_worklet_exports.rs

use std::cell::RefCell;
use wasm_bindgen::prelude::*;

use crate::dsp::{create_saw_voices, create_sine_voices, SawVoice, SineVoice};

struct WorkletSynth {
    saw:        Vec<SawVoice>,
    sine:       Vec<SineVoice>,
    master_amp: f32,
}

impl WorkletSynth {
    fn new() -> Self {
        Self {
            saw:        create_saw_voices(),
            sine:       create_sine_voices(),
            master_amp: 0.65,
        }
    }

    fn render_block(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n_saw  = self.saw.len()  as f32;
        let n_sine = self.sine.len() as f32;
        for i in 0..left.len() {
            let (mut l, mut r) = (0.0f32, 0.0f32);

            for v in self.saw.iter_mut() {
                let (vl, vr) = v.render();
                l += vl; r += vr;
            }
            let (sl, sr) = (l / n_saw, r / n_saw);

            let (mut tl, mut tr) = (0.0f32, 0.0f32);
            for v in self.sine.iter_mut() {
                let (vl, vr) = v.render();
                tl += vl; tr += vr;
            }
            // Sinussen op 0.35 van het totale volume ten opzichte van de zaagstanden
            let sine_mix = 0.35;
            left[i]  = ((sl + tl / n_sine * sine_mix) * self.master_amp).clamp(-1.0, 1.0);
            right[i] = ((sr + tr / n_sine * sine_mix) * self.master_amp).clamp(-1.0, 1.0);
        }
    }
}

thread_local! {
    static SYNTH: RefCell<Option<WorkletSynth>> = RefCell::new(None);
}

#[wasm_bindgen]
pub fn worklet_synth_init() {
    SYNTH.with(|s| *s.borrow_mut() = Some(WorkletSynth::new()));
}

#[wasm_bindgen]
pub fn worklet_synth_update(_params: &[f32]) {}

#[wasm_bindgen]
pub fn worklet_synth_render(left: &mut [f32], right: &mut [f32]) {
    SYNTH.with(|s| {
        if let Some(synth) = s.borrow_mut().as_mut() {
            synth.render_block(left, right);
        }
    });
}

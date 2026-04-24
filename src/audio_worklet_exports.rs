// src/audio_worklet_exports.rs
// WASM-exports die door het AudioWorklet-processor-script worden aangeroepen.
// De AudioWorklet-thread heeft zijn eigen WASM-instantie; deze functies leven
// daarin als globale toestand (thread_local).
//
// Serialisatieformaat state-bericht (Float32Array, 50 floats):
//   [0]           master_amplitude
//   [1 + i*7 + 0] stem[i].frequency
//   [1 + i*7 + 1] stem[i].detune
//   [1 + i*7 + 2] stem[i].gate
//   [1 + i*7 + 3] stem[i].noise
//   [1 + i*7 + 4] stem[i].saturation
//   [1 + i*7 + 5] stem[i].pan
//   [1 + i*7 + 6] stem[i].amplitude

use std::cell::RefCell;
use wasm_bindgen::prelude::*;

use crate::dsp::{StemGraph, BASE_FREQS, NUM_STEMS};
use crate::sonification::StemState;

struct WorkletSynth {
    stems:      Vec<StemGraph>,
    master_amp: f32,
}

impl WorkletSynth {
    fn new() -> Self {
        let stems: Vec<StemGraph> = BASE_FREQS.iter().enumerate().map(|(i, &f)| StemGraph::new(f, i)).collect();
        Self { stems, master_amp: 0.65 }
    }

    fn apply_state_from_floats(&mut self, params: &[f32]) {
        if params.len() < 1 + NUM_STEMS * 7 { return; }
        self.master_amp = params[0];
        for i in 0..NUM_STEMS {
            let b = 1 + i * 7;
            self.stems[i].update(&StemState {
                frequency:  params[b],
                detune:     params[b + 1],
                gate:       params[b + 2],
                noise:      params[b + 3],
                saturation: params[b + 4],
                pan:        params[b + 5],
                amplitude:  params[b + 6],
            });
        }
    }

    fn render_block(&mut self, left: &mut [f32], right: &mut [f32]) {
        let scale = self.master_amp / NUM_STEMS as f32;
        for i in 0..left.len() {
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for stem in self.stems.iter_mut() {
                let (sl, sr) = stem.render();
                l += sl; r += sr;
            }
            left[i]  = (l * scale).clamp(-1.0, 1.0);
            right[i] = (r * scale).clamp(-1.0, 1.0);
        }
    }
}

thread_local! {
    static SYNTH: RefCell<Option<WorkletSynth>> = RefCell::new(None);
}

/// Initialiseer de synth in de AudioWorklet-thread.
#[wasm_bindgen]
pub fn worklet_synth_init() {
    SYNTH.with(|s| {
        *s.borrow_mut() = Some(WorkletSynth::new());
    });
    crate::console_log!("🎵 worklet_synth_init — synth klaar");
}

/// Bijwerk de synth-parameters vanuit een Float32Array-bericht.
#[wasm_bindgen]
pub fn worklet_synth_update(params: &[f32]) {
    SYNTH.with(|s| {
        if let Some(synth) = s.borrow_mut().as_mut() {
            synth.apply_state_from_floats(params);
        }
    });
}

/// Render één audioblok naar de gegeven output-buffers.
#[wasm_bindgen]
pub fn worklet_synth_render(left: &mut [f32], right: &mut [f32]) {
    SYNTH.with(|s| {
        if let Some(synth) = s.borrow_mut().as_mut() {
            synth.render_block(left, right);
        }
    });
}

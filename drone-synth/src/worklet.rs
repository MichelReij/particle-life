// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// drone-synth/src/worklet.rs — AudioWorklet-exports voor de losse drone-build (feature "worklet")

use std::cell::RefCell;
use wasm_bindgen::prelude::*;

use crate::{DroneParams, DroneSynth};

thread_local! {
    static SYNTH: RefCell<Option<DroneSynth>> = RefCell::new(None);
}

/// `sample_rate` is de globale `sampleRate` van de AudioWorkletGlobalScope —
/// vaak 48 kHz, dus niet aannemen dat het 44.1 kHz is.
#[wasm_bindgen]
pub fn worklet_synth_init(sample_rate: f32) {
    SYNTH.with(|s| *s.borrow_mut() = Some(DroneSynth::new(sample_rate)));
}

/// params = DroneParams::to_array() (temperatuur, chaos, beweging, rust, pan_center, pan_lead)
#[wasm_bindgen]
pub fn worklet_synth_update(params: &[f32]) {
    SYNTH.with(|s| {
        if let (Some(synth), Some(p)) = (s.borrow_mut().as_mut(), DroneParams::from_slice(params)) {
            synth.set_params(&p);
        }
    });
}

/// Mengpaneel: lineaire gain per kanaal (volgorde: dsp::mix)
#[wasm_bindgen]
pub fn worklet_synth_set_mix(gains: &[f32]) {
    SYNTH.with(|s| {
        if let Some(synth) = s.borrow_mut().as_mut() {
            synth.set_mix(gains);
        }
    });
}

#[wasm_bindgen]
pub fn worklet_synth_render(left: &mut [f32], right: &mut [f32]) {
    SYNTH.with(|s| {
        if let Some(synth) = s.borrow_mut().as_mut() {
            for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                (*l, *r) = synth.render();
            }
        }
    });
}

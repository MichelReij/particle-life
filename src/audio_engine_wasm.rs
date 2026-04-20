// src/audio_engine_wasm.rs
// WASM audio engine: supersaw synthesizer via Web Audio API (ScriptProcessorNode).
// DSP-primitieven (SawOscillator, BiquadLPF, NoiseGen, Stem) leven in dsp.rs.
//
// Gebruik:
//   WasmAudioEngine::new() aanroepen bij opstart. De AudioContext start in 'suspended'
//   state. Roep set_paused(false) aan vanuit een user-gesture (knopklik) om te starten.
//
// Noot: ScriptProcessorNode is deprecated maar werkt in alle browsers en is de
// simpelste manier om WASM-DSP-code direct audio te laten genereren zonder een
// aparte AudioWorklet-module. Geschikt voor testen en schaven.

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{AudioContext, AudioProcessingEvent, MediaStream, MediaStreamAudioDestinationNode};

use crate::dsp::{Stem, BASE_FREQS, BLOCK_SIZE, NUM_STEMS};
use crate::sonification::SonificationState;

// ─── Interne synthesizer-toestand ────────────────────────────────────────────

struct WasmSynth {
    stems:      Vec<Stem>,
    master_amp: f32,
}

impl WasmSynth {
    fn new() -> Self {
        let stems = BASE_FREQS.iter().enumerate()
            .map(|(i, &f)| Stem::new(f, (i as u32 + 1) * 98765))
            .collect();
        Self { stems, master_amp: 0.5 }
    }

    fn apply_state(&mut self, state: &SonificationState) {
        self.master_amp = state.master_amplitude;
        for (i, stem) in self.stems.iter_mut().enumerate() {
            stem.update(&state.stems[i]);
        }
    }

    fn render_block(&mut self, left: &mut [f32], right: &mut [f32]) {
        let scale = self.master_amp / NUM_STEMS as f32;
        for i in 0..left.len() {
            let (mut l, mut r) = (0.0f32, 0.0f32);
            for stem in self.stems.iter_mut() {
                let (sl, sr) = stem.render();
                l += sl;  r += sr;
            }
            left[i]  = (l * scale).clamp(-1.0, 1.0);
            right[i] = (r * scale).clamp(-1.0, 1.0);
        }
    }
}

// ─── WasmAudioEngine (publieke API) ──────────────────────────────────────────

pub struct WasmAudioEngine {
    ctx:              AudioContext,
    // Gedeelde toestand tussen render-loop en audio-callback
    state:            Rc<RefCell<SonificationState>>,
    // Closure moet leven zolang de engine leeft
    _audio_closure:   Closure<dyn FnMut(AudioProcessingEvent)>,
    // MediaStreamAudioDestinationNode voor opname via MediaRecorder
    stream_dest:      MediaStreamAudioDestinationNode,
    paused:           bool,
}

impl WasmAudioEngine {
    /// Maak de audio engine aan. AudioContext start in 'suspended' state.
    /// Roep set_paused(false) aan vanuit een user-gesture om te starten.
    pub fn new() -> Result<Self, JsValue> {
        crate::console_log!("🎵 WasmAudioEngine: initialiseren (supersaw synthesizer)...");

        let ctx = AudioContext::new()?;

        let state = Rc::new(RefCell::new(SonificationState::default()));

        // ScriptProcessorNode: 0 inputs, 2 output-kanalen, buffergrootte 512
        let processor = ctx.create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(
            BLOCK_SIZE as u32, 0, 2
        )?;

        // Synthesizer-toestand in de callback (apart van SonificationState)
        let synth = Rc::new(RefCell::new(WasmSynth::new()));
        let state_clone = Rc::clone(&state);
        let synth_clone = Rc::clone(&synth);

        let closure = Closure::wrap(Box::new(move |event: AudioProcessingEvent| {
            let output = match event.output_buffer() {
                Ok(buf) => buf,
                Err(_) => return,
            };
            let buf_len = output.length() as usize;

            // Haal nieuwe SonificationState op (non-blocking via try_borrow)
            if let Ok(s) = state_clone.try_borrow() {
                if let Ok(mut synth) = synth_clone.try_borrow_mut() {
                    synth.apply_state(&*s);
                }
            }

            // Render audioblok
            let mut left  = vec![0.0f32; buf_len];
            let mut right = vec![0.0f32; buf_len];
            if let Ok(mut synth) = synth_clone.try_borrow_mut() {
                synth.render_block(&mut left, &mut right);
            }

            // Schrijf naar Web Audio buffers
            let _ = output.copy_to_channel(&left,  0);
            let _ = output.copy_to_channel(&right, 1);
        }) as Box<dyn FnMut(AudioProcessingEvent)>);

        processor.set_onaudioprocess(Some(closure.as_ref().unchecked_ref()));

        // Verbind processor → speakers én → opname-destination (parallel)
        processor.connect_with_audio_node(&ctx.destination())?;
        let stream_dest = ctx.create_media_stream_destination()?;
        processor.connect_with_audio_node(&stream_dest)?;

        crate::console_log!("✅ WasmAudioEngine klaar — start via audio-knop in de UI");

        Ok(Self {
            ctx,
            state,
            _audio_closure: closure,
            stream_dest,
            paused: true, // Begint gepauzeerd; UI-knop roept set_paused(false) aan
        })
    }

    /// Bijwerk de SonificationState vanuit de render-loop.
    pub fn update(&self, new_state: SonificationState) {
        if let Ok(mut s) = self.state.try_borrow_mut() {
            *s = new_state;
        }
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        if paused {
            let _ = self.ctx.suspend();
        } else {
            let _ = self.ctx.resume();
        }
    }

    /// Geeft de MediaStream terug voor gebruik in MediaRecorder (video-opname met audio).
    pub fn get_media_stream(&self) -> MediaStream {
        self.stream_dest.stream()
    }

    pub fn set_master_volume(&self, v: f32) {
        // Volume-aanpassing via gain node is optioneel; voor nu via SonificationState.master_amplitude.
        // Kan later uitgebreid worden met een GainNode in de signaalketen.
        if let Ok(mut s) = self.state.try_borrow_mut() {
            s.master_amplitude = v.clamp(0.0, 1.0);
        }
    }
}

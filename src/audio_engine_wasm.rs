// src/audio_engine_wasm.rs
// WASM audio engine: supersaw synthesizer via Web Audio API (AudioWorkletNode).
// DSP-code (StemGraph, fundsp) draait in de AudioWorklet-thread (audio_worklet_exports.rs).
// De AudioWorklet-processor is gedefinieerd in public/audio-processor.js.
//
// Gebruik:
//   WasmAudioEngine::new() aanroepen bij opstart. De AudioContext start in 'suspended'
//   state. Roep set_paused(false) aan vanuit een user-gesture (knopklik) om te starten.

use std::cell::RefCell;
use std::rc::Rc;
use js_sys::Float32Array;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AudioContext, AudioWorkletNode, MediaStream, MediaStreamAudioDestinationNode,
};

use crate::dsp::NUM_STEMS;
use crate::sonification::SonificationState;

// ─── WasmAudioEngine (publieke API) ──────────────────────────────────────────

pub struct WasmAudioEngine {
    ctx:         AudioContext,
    node:        Rc<RefCell<Option<AudioWorkletNode>>>,
    stream_dest: Rc<MediaStreamAudioDestinationNode>,
    paused:      bool,
}

impl WasmAudioEngine {
    /// Maak de audio engine aan. AudioContext start in 'suspended' state.
    /// De AudioWorklet-module wordt asynchroon geladen; zodra klaar is het node-veld gevuld.
    /// Roep set_paused(false) aan vanuit een user-gesture om te starten.
    pub fn new() -> Result<Self, JsValue> {
        crate::console_log!("🎵 WasmAudioEngine v{} initialiseren...", env!("BUILD_ID"));

        let ctx = AudioContext::new()?;
        let node: Rc<RefCell<Option<AudioWorkletNode>>> = Rc::new(RefCell::new(None));
        let stream_dest = Rc::new(ctx.create_media_stream_destination()?);

        // Bepaal de URL van de processor-module relatief aan de huidige pagina.
        let processor_url = {
            let location = web_sys::window()
                .ok_or_else(|| JsValue::from_str("geen window"))?
                .location();
            let href = location.href()?;
            let base = web_sys::Url::new(&href)?;
            let abs  = web_sys::Url::new_with_base("audio-processor.js", &base.href())?;
            abs.href()
        };

        // addModule() is asynchroon — laad de processor en maak daarna de AudioWorkletNode aan.
        let node_clone        = Rc::clone(&node);
        let stream_dest_clone = Rc::clone(&stream_dest);
        let ctx_clone         = ctx.clone();

        wasm_bindgen_futures::spawn_local(async move {
            let worklet = match ctx_clone.audio_worklet() {
                Ok(w) => w,
                Err(e) => { crate::console_log!("❌ audio_worklet() fout: {:?}", e); return; }
            };

            let add_module_promise = match worklet.add_module(&processor_url) {
                Ok(p) => p,
                Err(e) => { crate::console_log!("❌ addModule() fout: {:?}", e); return; }
            };

            if let Err(e) = JsFuture::from(add_module_promise).await {
                crate::console_log!("❌ addModule() promise fout: {:?}", e);
                return;
            }

            let worklet_node = match AudioWorkletNode::new(&ctx_clone, "particle-life-processor") {
                Ok(n) => n,
                Err(e) => { crate::console_log!("❌ AudioWorkletNode::new() fout: {:?}", e); return; }
            };

            if let Err(e) = worklet_node.connect_with_audio_node(&ctx_clone.destination()) {
                crate::console_log!("❌ connect destination fout: {:?}", e);
                return;
            }
            if let Err(e) = worklet_node.connect_with_audio_node(&*stream_dest_clone) {
                crate::console_log!("❌ connect stream_dest fout: {:?}", e);
                return;
            }

            *node_clone.borrow_mut() = Some(worklet_node);
            crate::console_log!("✅ AudioWorkletNode klaar en verbonden");
        });

        crate::console_log!("✅ WasmAudioEngine aangemaakt — wacht op AudioWorklet...");

        Ok(Self {
            ctx,
            node,
            stream_dest,
            paused: true,
        })
    }

    /// Bijwerk de SonificationState vanuit de render-loop.
    /// Serialiseert de state naar een Float32Array en stuurt die naar de AudioWorklet.
    pub fn update(&self, new_state: SonificationState) {
        let node_ref = self.node.borrow();
        let Some(node) = node_ref.as_ref() else { return };
        let Ok(port) = node.port() else { return };

        let params = state_to_f32(&new_state);
        let arr = Float32Array::from(params.as_slice());
        let _ = port.post_message(&arr);
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
        if paused {
            if let Ok(promise) = self.ctx.suspend() {
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = JsFuture::from(promise).await;
                });
            }
        } else {
            match self.ctx.resume() {
                Ok(promise) => {
                    wasm_bindgen_futures::spawn_local(async move {
                        match JsFuture::from(promise).await {
                            Ok(_)  => crate::console_log!("🎵 AudioContext resumed"),
                            Err(e) => crate::console_log!("❌ resume rejected: {:?}", e),
                        }
                    });
                }
                Err(e) => crate::console_log!("❌ ctx.resume() fout: {:?}", e),
            }
        }
    }

    pub fn get_media_stream(&self) -> MediaStream {
        self.stream_dest.stream()
    }

    pub fn set_master_volume(&self, v: f32) {
        // Stuur een bijgewerkte state met aangepaste master_amplitude naar de worklet.
        // Omdat we geen lokale state bewaren, wordt dit afgehandeld door de render-loop
        // die set_master_volume doorgeeft via update(). Voor directe aanpassing:
        let node_ref = self.node.borrow();
        let Some(node) = node_ref.as_ref() else { return };
        let Ok(port) = node.port() else { return };
        // Stuur alleen master_amplitude (1 float, onderscheiden van de 50-float state)
        let arr = Float32Array::from([v.clamp(0.0, 1.0)].as_slice());
        let _ = port.post_message(&arr);
    }
}

/// Serialiseer SonificationState naar een platte Float32Array (50 floats).
fn state_to_f32(s: &SonificationState) -> Vec<f32> {
    let mut v = Vec::with_capacity(1 + NUM_STEMS * 7);
    v.push(s.master_amplitude);
    for stem in &s.stems {
        v.push(stem.frequency);
        v.push(stem.detune);
        v.push(stem.gate);
        v.push(stem.noise);
        v.push(stem.saturation);
        v.push(stem.pan);
        v.push(stem.amplitude);
    }
    v
}

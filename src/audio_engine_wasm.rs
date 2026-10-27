// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// src/audio_engine_wasm.rs
// WASM audio engine: synthesizer via Web Audio API (AudioWorkletNode).
//
// Initialisatieflow voor de WASM-module in de AudioWorklet:
//   1. main.ts roept set_audio_wasm_module(init.__wbindgen_wasm_module) aan
//      vlak na de wasm-bindgen init() call.
//   2. WasmAudioEngine::new() maakt de AudioWorkletNode aan.
//   3. Na addModule() stuurt de engine de opgeslagen module via port.postMessage.
//   4. audio-processor.js ontvangt de module, roept init() aan en start de synth.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AudioContext, AudioWorkletNode, MediaStream, MediaStreamAudioDestinationNode,
};

use crate::dsp::DroneParams;
use crate::sonification::SonificationState;

// ─── WASM-module opslag (injecteerbaar vanuit main.ts) ───────────────────────

thread_local! {
    static WASM_MODULE: RefCell<Option<JsValue>> = RefCell::new(None);
}

/// Sla de WebAssembly.Module op voor gebruik in de AudioWorklet.
/// Aanroepen vanuit main.ts direct na `await init(...)`:
///   set_audio_wasm_module((init as any).__wbindgen_wasm_module);
#[wasm_bindgen]
pub fn set_audio_wasm_module(module: JsValue) {
    WASM_MODULE.with(|m| *m.borrow_mut() = Some(module));
    crate::console_log!("✅ WASM-module opgeslagen voor AudioWorklet");
}

// ─── WasmAudioEngine (publieke API) ──────────────────────────────────────────

pub struct WasmAudioEngine {
    ctx:         AudioContext,
    node:        Rc<RefCell<Option<AudioWorkletNode>>>,
    stream_dest: Rc<MediaStreamAudioDestinationNode>,
    paused:      bool,
    /// Laatst verstuurde drone-parameters; None = nog niets verstuurd
    last_sent: Cell<Option<DroneParams>>,
    /// Mengpaneel-gains die nog naar de worklet moeten (de node bestaat pas
    /// asynchroon, dus wordt bij de eerstvolgende update() verstuurd)
    pending_mix: RefCell<Option<Vec<f32>>>,
}

impl WasmAudioEngine {
    /// `asset_base`: map waar audio-processor.js staat; None = naast de pagina.
    pub fn new(asset_base: Option<&str>) -> Result<Self, JsValue> {
        crate::console_log!("🎵 WasmAudioEngine v{} initialiseren...", env!("BUILD_ID"));

        let ctx = AudioContext::new()?;
        let node: Rc<RefCell<Option<AudioWorkletNode>>> = Rc::new(RefCell::new(None));
        let stream_dest = Rc::new(ctx.create_media_stream_destination()?);

        let processor_url = {
            let location = web_sys::window()
                .ok_or_else(|| JsValue::from_str("geen window"))?
                .location();
            let href = match asset_base {
                Some(b) => b.to_string(),
                None => location.href()?,
            };
            let base = web_sys::Url::new(&href)?;
            let abs  = web_sys::Url::new_with_base("audio-processor.js", &base.href())?;
            abs.href()
        };

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

            // Stuur de WASM-module via de port zodat de AudioWorklet init() kan aanroepen.
            // Pas nu ophalen (na addModule): in de embed worden de bytes op de
            // achtergrond geladen en kunnen ze net na de klik binnenkomen.
            let wasm_module = WASM_MODULE.with(|m| m.borrow().clone()).unwrap_or(JsValue::UNDEFINED);
            if wasm_module.is_undefined() {
                crate::console_log!("❌ Geen WASM-module voor de AudioWorklet (set_audio_wasm_module niet aangeroepen)");
            }
            if let Ok(port) = worklet_node.port() {
                let _ = port.post_message(&wasm_module);
            }

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
            last_sent: Cell::new(None),
            pending_mix: RefCell::new(None),
        })
    }

    /// Stuurt de drone-parameters naar de AudioWorklet, alleen bij merkbare
    /// verandering (update() wordt elke frame aangeroepen).
    pub fn update(&self, new_state: SonificationState) {
        self.flush_mix();
        let p = new_state.drone;
        if let Some(last) = self.last_sent.get() {
            let changed = p.to_array().iter().zip(last.to_array()).any(|(a, b)| (a - b).abs() >= 0.002);
            if !changed {
                return;
            }
        }
        if let Some(node) = self.node.borrow().as_ref() {
            if let Ok(port) = node.port() {
                let params = js_sys::Float32Array::from(&p.to_array()[..]);
                if port.post_message(&params).is_ok() {
                    self.last_sent.set(Some(p));
                }
            }
        }
    }

    /// Mengpaneel: lineaire gain per kanaal (volgorde: dsp::mix).
    pub fn set_mix(&self, gains: &[f32]) {
        *self.pending_mix.borrow_mut() = Some(gains.to_vec());
        self.flush_mix();
    }

    fn flush_mix(&self) {
        let Some(gains) = self.pending_mix.borrow().clone() else { return };
        let Some(node) = self.node.borrow().clone() else { return };
        let Ok(port) = node.port() else { return };
        let msg = js_sys::Object::new();
        let arr = js_sys::Float32Array::from(&gains[..]);
        if js_sys::Reflect::set(&msg, &JsValue::from_str("mix"), &arr).is_ok() && port.post_message(&msg).is_ok() {
            *self.pending_mix.borrow_mut() = None;
        }
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

    pub fn set_master_volume(&self, _v: f32) {
        // Standalone patch
    }
}

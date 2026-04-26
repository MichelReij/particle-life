// src/audio_engine_wasm.rs
// WASM audio engine: synthesizer via Web Audio API (AudioWorkletNode).
//
// Initialisatieflow voor de WASM-module in de AudioWorklet:
//   1. main.ts roept set_audio_wasm_module(init.__wbindgen_wasm_module) aan
//      vlak na de wasm-bindgen init() call.
//   2. WasmAudioEngine::new() maakt de AudioWorkletNode aan.
//   3. Na addModule() stuurt de engine de opgeslagen module via port.postMessage.
//   4. audio-processor.js ontvangt de module, roept init() aan en start de synth.

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AudioContext, AudioWorkletNode, MediaStream, MediaStreamAudioDestinationNode,
};

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
}

impl WasmAudioEngine {
    pub fn new() -> Result<Self, JsValue> {
        crate::console_log!("🎵 WasmAudioEngine v{} initialiseren...", env!("BUILD_ID"));

        let ctx = AudioContext::new()?;
        let node: Rc<RefCell<Option<AudioWorkletNode>>> = Rc::new(RefCell::new(None));
        let stream_dest = Rc::new(ctx.create_media_stream_destination()?);

        let processor_url = {
            let location = web_sys::window()
                .ok_or_else(|| JsValue::from_str("geen window"))?
                .location();
            let href = location.href()?;
            let base = web_sys::Url::new(&href)?;
            let abs  = web_sys::Url::new_with_base("audio-processor.js", &base.href())?;
            abs.href()
        };

        let node_clone        = Rc::clone(&node);
        let stream_dest_clone = Rc::clone(&stream_dest);
        let ctx_clone         = ctx.clone();

        // Haal de opgeslagen WASM-module op voor gebruik in de async closure.
        let wasm_module = WASM_MODULE.with(|m| m.borrow().clone()).unwrap_or(JsValue::UNDEFINED);

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
        })
    }

    pub fn update(&self, _new_state: SonificationState) {
        // Standalone patch: geen state-updates
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

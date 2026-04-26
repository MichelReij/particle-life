// public/audio-processor.js
// AudioWorklet processor voor de particle-life synthesizer.
//
// Initalisatieflow:
//   1. Rust stuurt de WebAssembly.Module via port.postMessage direct na addModule().
//   2. onmessage ontvangt de module, roept wasm-bindgen init() aan en start de synth.
//   3. process() wacht op `ready` voordat audio gerenderd wordt.

import init, { worklet_synth_init, worklet_synth_render }
    from './pkg/particle_life_wasm.js';

let ready = false;

class ParticleLifeProcessor extends AudioWorkletProcessor {
    constructor(options) {
        super(options);
        this.port.onmessage = async (e) => {
            const wasmModule = e.data;
            console.log('[AudioWorklet] module ontvangen:', wasmModule);
            try {
                await init({ module_or_path: wasmModule });
                worklet_synth_init();
                ready = true;
                console.log('[AudioWorklet] synth klaar');
            } catch (err) {
                console.error('[AudioWorklet] init mislukt:', err);
            }
        };
    }

    process(_inputs, outputs) {
        if (!ready) return true;
        const left  = outputs[0][0];
        const right = outputs[0][1];
        if (left && right) worklet_synth_render(left, right);
        return true;
    }
}

registerProcessor('particle-life-processor', ParticleLifeProcessor);

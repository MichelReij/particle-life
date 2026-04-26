// public/audio-processor.js
// AudioWorklet processor voor de particle-life synthesizer.
//
// De WASM-module wordt doorgegeven via processorOptions.wasmModule vanuit de
// main thread (audio_engine_wasm.rs). Dit omzeilt het ontbreken van `fetch`
// en `URL` in de AudioWorklet scope.

import init, { worklet_synth_init, worklet_synth_render }
    from './pkg/particle_life_wasm.js';

let ready = false;

class ParticleLifeProcessor extends AudioWorkletProcessor {
    constructor(options) {
        super(options);
        const wasmModule = options?.processorOptions?.wasmModule;
        init(wasmModule).then(() => {
            worklet_synth_init();
            ready = true;
        });
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

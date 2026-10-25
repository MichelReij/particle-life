// public/audio-processor.js

import init, { worklet_synth_init, worklet_synth_update, worklet_synth_set_mix, worklet_synth_render }
    from './pkg/particle_life_wasm.js';

let ready = false;

class ParticleLifeProcessor extends AudioWorkletProcessor {
    constructor(options) {
        super(options);
        this._tempL = null;
        this._tempR = null;
        // Parameters en mix die binnenkomen vóórdat de WASM-module klaar is
        this._pendingParams = null;
        this._pendingMix = null;
        this.port.onmessage = async (e) => {
            // Float32Array = parameter-update; {mix: Float32Array} = mengpaneel;
            // al het andere is de WASM-module
            if (e.data instanceof Float32Array) {
                if (ready) worklet_synth_update(e.data);
                else this._pendingParams = e.data;
                return;
            }
            if (e.data && e.data.mix instanceof Float32Array) {
                if (ready) worklet_synth_set_mix(e.data.mix);
                else this._pendingMix = e.data.mix;
                return;
            }
            try {
                await init({ module_or_path: e.data });
                worklet_synth_init(sampleRate);
                if (this._pendingParams) worklet_synth_update(this._pendingParams);
                if (this._pendingMix) worklet_synth_set_mix(this._pendingMix);
                ready = true;
            } catch (err) {
                console.error('[AudioWorklet] init mislukt:', err);
            }
        };
    }

    process(_inputs, outputs) {
        if (!ready) return true;
        const ch    = outputs[0];
        const left  = ch[0];
        if (!left) return true;

        // AudioWorklet kan 1 of 2 kanalen hebben afhankelijk van de node-configuratie.
        // Bij 1 kanaal renderen we naar tijdelijke buffers en mengen ze tot mono.
        const right = ch[1];
        if (right) {
            worklet_synth_render(left, right);
        } else {
            if (!this._tempL || this._tempL.length !== left.length) {
                this._tempL = new Float32Array(left.length);
                this._tempR = new Float32Array(left.length);
            }
            worklet_synth_render(this._tempL, this._tempR);
            for (let i = 0; i < left.length; i++) {
                left[i] = (this._tempL[i] + this._tempR[i]) * 0.5;
            }
        }
        return true;
    }
}

registerProcessor('particle-life-processor', ParticleLifeProcessor);

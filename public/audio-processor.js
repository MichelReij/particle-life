// public/audio-processor.js

import init, { worklet_synth_init, worklet_synth_render }
    from './pkg/particle_life_wasm.js';

let ready = false;

class ParticleLifeProcessor extends AudioWorkletProcessor {
    constructor(options) {
        super(options);
        this._tempL = null;
        this._tempR = null;
        this.port.onmessage = async (e) => {
            try {
                await init({ module_or_path: e.data });
                worklet_synth_init();
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

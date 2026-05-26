# Feature detection: WASM en WebGPU

## WASM

```js
const wasmSupported = typeof WebAssembly !== "undefined";
```

Werkt in alle moderne browsers. Geen async nodig.

Voor specifieke WASM-features (SIMD, threads, exceptions):
→ [`wasm-feature-detect`](https://www.npmjs.com/package/wasm-feature-detect)

```js
import { simd, threads } from "wasm-feature-detect";
const hasSIMD = await simd();
const hasThreads = await threads();
```

De particle-life simulatie gebruikt geen SIMD of threads — basischeck volstaat.

## WebGPU

```js
async function detectWebGPU() {
    if (!navigator.gpu) return false;          // API ontbreekt in browser
    const adapter = await navigator.gpu.requestAdapter();
    return adapter !== null;                   // null = geen compatibele GPU/driver
}
```

Twee gevallen onderscheiden:
- `navigator.gpu` ontbreekt → browser heeft geen WebGPU (bijv. Firefox zonder flag, oudere Safari)
- `requestAdapter()` geeft `null` → browser heeft de API maar de GPU/driver voldoet niet

## Terminologie

| Term | Wat het is |
|---|---|
| **WebGPU** | Browser-API — dít detecteer je |
| **WGSL** | Shadertaal die bij WebGPU hoort — werkt automatisch als WebGPU werkt |
| **wgpu** | Rust-library (wgpu-rs) die WebGPU implementeert — implementatiedetail, niet detecteerbaar |

## Andere WASM-projecten (zonder WebGPU)

Voor projecten die alleen WASM gebruiken (geen WebGPU):

```js
if (typeof WebAssembly === "undefined") {
    // Toon fallback
}
```

## Gecombineerde check (particle-life embed)

```js
async function canRunSimulation() {
    if (typeof WebAssembly === "undefined") return false;
    if (!navigator.gpu) return false;
    const adapter = await navigator.gpu.requestAdapter();
    return adapter !== null;
}
```

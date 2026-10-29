# CLAUDE.md — Particle Life / Origin of Life

## Project Overview

GPU-accelerated particle life simulation ("Origin of Life" artwork) with two deployment targets:

1. **Web** (WASM + WebGPU): HTML/TypeScript frontend for experimentation and parameter tuning
2. **Native** (Rust binary on Ubuntu): Final artwork running on Ubuntu PC with a round 1080×1080 pixel screen, receiving sensor data from an ESP32 network

## Repository Layout

```
src/
  lib.rs                   # Shared library entry (WASM + native)
  bin/native_minimal.rs    # Native binary entry point
  webgpu_renderer.rs       # Core WebGPU renderer (shared)
  particle_system.rs       # Core particle physics (CPU-side, init only)
  simulation_params.rs     # Parameter management + ESP32 mappings
  interaction_rules.rs     # Particle interaction rules + RuleEvolution (lerp)
  spatial_grid.rs          # Spatial partitioning (O(n) collision)
  buffer_utils.rs          # GPU buffer utilities
  shader_constants.rs      # Shader constants
  config.rs                # Configuration constants
  esp32_communication.rs   # ESP32 serial comm (native only)
  audio_engine.rs          # Native audio-uitvoer van de drone via rodio
  sonification.rs          # Simulatie → DroneParams (+ StormTracker voor onweer)
  stats_reader.rs          # GPU readback for particle-type statistics (native only)
  shaders/
    compute.wgsl           # Particle physics compute shader (GPU, runs every frame)
    stats_compute.wgsl     # NEW: per-type statistics shader for sonification (10 Hz)
    vert.wgsl / frag.wgsl  # Particle render shaders
    lightning_compute.wgsl / lightning_vert.wgsl / lightning_frag_buffer.wgsl
    background_vert.wgsl / background_frag.wgsl
    glow_frag.wgsl / fisheye_frag.wgsl / vignette_frag.wgsl
    zoom_frag.wgsl / grid_frag.wgsl / frag_flat.wgsl
    text_vert.wgsl / text_frag.wgsl / text_overlay.wgsl
  index.html / main.ts / ui.ts / config.ts / particle-life-types.ts  # Web UI
  embed.ts                 # WordPress embed entry point (minimal, zelfstandig)
  pkg/                     # Built WASM package (wasm-bindgen output)
```

## Technology Stack

| Layer | Web | Native |
|-------|-----|--------|
| Language | Rust (WASM) + TypeScript | Rust |
| GPU | WebGPU via wgpu | wgpu (Metal/Vulkan/DX12) |
| Shaders | WGSL (runtime loaded) | WGSL (compile-time embedded) |
| UI | HTML/TypeScript | winit window |
| Bundler | Webpack | — |
| External I/O | — | ESP32 via serial (115200 baud) |
| Audio | AudioWorklet (ambient drone) | rodio (ambient drone) |

## Build Commands

### Web (WASM)
```bash
./build-wasm.sh                        # Build WASM + bundle met webpack
npx webpack --mode production          # Production bundle (naar public/)
npm run dev                            # Dev server op poort 3000
```

### Native
```bash
cargo build               # Debug native binary
cargo build --release     # Release native binary
./run-native.sh           # Build + run
./target/debug/native_minimal  # Run directly
```

### Audio: ambient drone

Native en web spelen dezelfde zelf gesynthetiseerde ambient drone (`drone-synth/src/lib.rs`,
`DroneSynth`; een eigen crate die in particle-life als `dsp` beschikbaar is, zodat de Milgram
dezelfde klank kan draaien — daar via `Milgram/scripts/sync-drone.sh` als losse WASM met
feature `worklet`), aangestuurd vanuit de simulatie (`src/sonification.rs`). Native via rodio
(`src/audio_engine.rs`), web via een AudioWorklet (`src/audio_engine_wasm.rs`,
`src/audio_worklet_exports.rs`, `public/audio-processor.js`). Het mengpaneel staat in de
web-UI (`src/audio-mixer.ts`). Offline renderen voor metingen:
```bash
cargo run --release --example render_drone -- uit.wav [seconden] ...   # zie de example voor alle argumenten
```
De vroegere mp3-weergave (`demo_audio`, `assets/audio/brainwaves.mp3`) is verwijderd; het
mp3-bestand dient alleen nog als klankreferentie voor de brainwaves-pad.

## Simulation Parameters

- **Virtual world**: 3240×3240 units, rendered to 1080×1080 pixels
- **Particles**: 1600–4800 (pre-allocated at MAX=4800), 7 types
- **Zoom**: 1x–12x with GPU-side viewport culling
- **Physics**: Spatial grid partitioning, workgroup-aligned GPU dispatch (multiples of 64)
- **Rule evolution**: Continuous lerp between random rule sets; duration controlled by temperature

## ESP32 Protocol

17-byte binary packet @ ~60 FPS over serial:
```
[0xAA] [zoom_hi] [zoom_lo] [pan_x_hi] [pan_x_lo] [pan_y_hi] [pan_y_lo]
       [temp_hi] [temp_lo] [pressure_hi] [pressure_lo] [uv_hi] [uv_lo]
       [elec_hi] [elec_lo] [sleep] [0x55]
```
All u16 values are 0–4096 range. See `ESP32_API.md` for full parameter mapping.

## Conditional Compilation

- `#[cfg(target_arch = "wasm32")]` — web-only code
- `#[cfg(not(target_arch = "wasm32"))]` — native-only code (ESP32, audio, winit)

## Web Deployment (michelreij.nl)

Bestanden worden gepubliceerd naar `https://michelreij.nl/webapps/origin-of-life/`.

### Deploy commando
```bash
FTP_PASS=$(security find-internet-password -s mwp12009.hostingserver.nl -g 2>&1 | grep '^password:' | sed 's/password: "\(.*\)"/\1/')
FTP_USER="michelre"
bash deploy.sh "$FTP_USER" "$FTP_PASS"
```

`deploy.sh` uploadt via lftp (vereist `brew install lftp`):
- Top-level bestanden: `index.html`, `main.js`, `styles.css`, `styles.js`, `joy.js`
- Embed bestanden: `particle-life-embed.js`, `particle-life-embed.html`
- Mappen: `shaders/`, `pkg/` (incrementeel gesynchroniseerd)

### WordPress embed

`public/particle-life-embed.html` bevat het kant-en-klare snippet voor WordPress Custom HTML:

```html
<div id="ol-wrap"></div>
<script src="https://michelreij.nl/webapps/origin-of-life/particle-life-embed.js"></script>
```

De embed (`src/embed.ts`) is volledig zelfstandig:
- Injecteert zijn eigen CSS in `<head>`
- Bouwt de DOM op in `#ol-wrap`
- Leidt het WASM-pad af van `document.currentScript.src` — werkt op elke WordPress-pagina
- Bevat alleen de 4 hoofdsliders (temperatuur, pH, diepte (druk), elektrische activiteit)
- Zoom via muiswiel en pinch-to-zoom; pan via muisdrag en touch-drag

---

## Native Deployment (Ubuntu)

- Fullscreen, no titlebar
- Exit via Escape or Q
- Screen: round 1080×1080 pixels
- Binary: `./target/release/native_minimal`
- See `FULLSCREEN_UBUNTU.md` and `DEPLOYMENT-GUIDE.md`

---

## Sonification System

Native en web spelen dezelfde ambient drone (`dsp::DroneSynth`, funDSP), aangestuurd door
de simulatie. Geen stem per deeltjestype meer: een vaste set lagen in C-mineur pentatonisch.

### Architecture

```
SimulationParams ─┐
GpuTypeStats ─────┼→ sonification::compute_sonification() → SonificationState.drone
(alleen zoom > 3) │   + StormTracker (bliksemplanning via de simulatieklok)
LightningBolt ────┘                       ↓  DroneParams (temperatuur, chaos, beweging,
                                             rust, panning, hypothese, onweer, nacht)
             native: audio_engine::AudioEngine (rodio)  ·  web: AudioWorklet
                                          ↓
                                   dsp::DroneSynth → stereo
```

### Lagen (drone-synth/src/lib.rs)

| Laag | Wanneer | Kern |
|---|---|---|
| Cello's | diepzee (zwijgen in WLP) | soft_saw + kastresonanties, adempauzes; chaos → rasp |
| Theremin | altijd, zwijgt 's nachts (WLP) | portamento, random-walk frasen |
| Brainwaves | altijd | diepe basdrone naar `assets/audio/brainwaves.mp3`, langzame akkoordwissels (faden, niet glijden) |
| Druppels + stroompje | altijd | gestemde druppels; beweging → meer |
| Branding | WLP | aanzwellende golven + schuim |
| Onweer | WLP, in sync met de bliksem | donderwolk-gegrom, overstuurde knal + geknetter, donderrollen |
| Sonar | HTV | losse pings met een kleine echo, wisselende richting |
| Vents | HTV | sissende straal, pijp-toon, bellen (groottemodel), achtergrondgerommel |

Temperatuur bepaalt het tempo (0.1–2.2). Mengpaneel (web): `src/audio-mixer.ts`, volgorde
`dsp::mix`. Renders voor metingen: `examples/render_drone.rs`.

### Zoom-gestuurde sonische lens

- **zoom ≤ 3x**: alleen algemene simulatietoestand (temperatuur, hypothese, onweer, nacht)
- **zoom > 3x**: daarnaast `GpuTypeStats` → chaos/beweging/rust/panning, geschaald met `gpu_blend`

### GpuTypeStats (stats_compute.wgsl → stats_reader.rs)

Per particle-type: `vec4(viewport_count, energy, order, centroid_x)`; globaal (index 7):
`vec4(total_viewport_count, cluster_count, avg_cluster_size, 0)`.

### StatsReader (stats_reader.rs)

- Beheert `stats_compute.wgsl` pipeline + staging buffer (112 bytes)
- `maybe_dispatch()`: elke 6 frames een compute pass toevoegen aan encoder
- `read_stats()`: async GPU→CPU readback (pollster::block_on in render loop)
- Bind group wordt elke dispatch opnieuw aangemaakt (ping-pong buffer wisselt na elke render)

### Kritische implementatiedetails

**Ping-pong buffers**: Na `renderer.render()` is `current_buffer_index` al gewisseld. De output particles zitten in `1 - renderer.current_buffer_index`. De stats bind group moet altijd aan de output buffer gebonden worden.

**WebGpuRenderer publieke velden** (toegevoegd voor stats_reader):
- `pub sim_params_buffer: wgpu::Buffer`
- `pub particle_buffers: [wgpu::Buffer; 2]`
- `pub current_buffer_index: usize`
- `pub fn queue(&self) -> &wgpu::Queue`

**AudioEngine is non-blocking**: parameters gaan via `fundsp::Shared` (atomair) naar de audio-thread (`DroneHandle`); geen locks.

### Keyboard controls (native)

| Toets | Effect |
|-------|--------|
| M | Synthesizer aan (resume) |
| S | Synthesizer pauzeren |
| + | Volume 80% |
| - | Volume 30% |
| Q / Escape | Afsluiten |

---

## Key Design Decisions

- **Physics is GPU-only**: `particle_system.rs` bevat CPU-physics code maar die wordt NIET aangeroepen in de native binary. Alle physics loopt via `compute.wgsl`. De CPU-side `ParticleSystem` is alleen voor initialisatie en metadata.
- Pre-allocated GPU buffers (MAX_PARTICLES=4800) — no runtime reallocation to prevent GPU panics
- ESP32 runs in separate thread to avoid blocking render loop
- Shaders loaded at runtime for web, embedded at compile time for native
- `lib.rs` is both `cdylib` (for WASM) and `rlib` (for native binary dependency)
- `audio.rs` (old) is kept but unused — `audio_engine.rs` (new) is de actieve synthesizer

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
  audio.rs                 # OLD audio system — simple mp3 player (native only, kept for reference)
  audio_engine.rs          # NEW audio engine — supersaw synthesizer (native only)
  sonification.rs          # Parameter → StemState mapping (native only)
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
| Audio | — | rodio (supersaw synthesizer) |

## Build Commands

### Web (WASM)
```bash
./build-wasm.sh           # Build WASM + bundle with webpack
npm run build             # Webpack bundle only
npm run dev               # Dev server
```

### Native
```bash
cargo build               # Debug native binary
cargo build --release     # Release native binary
./run-native.sh           # Build + run
./target/debug/native_minimal  # Run directly
```

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

## Native Deployment (Ubuntu)

- Fullscreen, no titlebar
- Exit via Escape or Q
- Screen: round 1080×1080 pixels
- Binary: `./target/release/native_minimal`
- See `FULLSCREEN_UBUNTU.md` and `DEPLOYMENT-GUIDE.md`

---

## Sonification System (NEW — work in progress)

The native binary generates sound from the simulation state. This was added in April 2025 and was **not yet compiled/tested** when this CLAUDE.md was written. There will likely be compile errors to fix.

### Architecture

```
SimulationParams ──────────────────────────────────────────┐
                  → sonification::compute_sonification()   │
GpuTypeStats ─────→   (elke frame, EMA-smoothed)          │
(10 Hz readback         ↓                                  │
 bij zoom > 5x)    SonificationState { stems: [StemState; 7] }
                         ↓
                   audio_engine::AudioEngine
                   (rodio thread, non-blocking)
                         ↓
                   7× Stem: supersaw + biquad LPF + noise + tanh
                         ↓
                   stereo audio output
```

### Zoom-gestuurde sonische lens

- **zoom 1x–5x**: alleen `SimulationParams` → ambient, traag, globaal karakter
- **zoom 5x–12x**: `SimulationParams` + `GpuTypeStats` (readback 10 Hz) → rijker, reageert op clusters
- De GPU-blend schaalt lineair: `(zoom - 5) / 7`

### GpuTypeStats (stats_compute.wgsl → stats_reader.rs)

Per particle-type (7 total), berekend in `stats_compute.wgsl`:
```
stats_out[type] = vec4<f32>(centroid_x, centroid_y, mean_speed, density)
```
- `centroid_x/y`: gewogen positie in viewport [0, 1] → stereo panning
- `mean_speed`: gemiddelde snelheidsgrootte → noise niveau
- `density`: fractie tov verwacht gemiddelde (>1 = cluster in beeld) → gate opening + amplitude

**Dispatch**: 7 workgroups (één per type), elke 6 frames (~10 Hz). Slechts 112 bytes readback.

### StemState per particle-type

Elk van de 7 particle-types heeft een eigen "stem":
```
type 0 (blauw)  → A1  = 55.0 Hz
type 1 (geel)   → E2  = 82.4 Hz
type 2 (rood)   → B2  = 123.5 Hz  (klein, snel, hoog)
type 3 (paars)  → G#1 = 51.9 Hz
type 4 (groen)  → D2  = 73.4 Hz
type 5 (olijf)  → A#1 = 58.3 Hz
type 6 (oranje) → C#2 = 69.3 Hz
```

### Parameter mappings (sonification.rs)

| SimulationParams | → StemState |
|---|---|
| `inter_type_attraction_scale` [-1,3] | → `gate` [0.05, 0.9] — lowpass gate openheid |
| `r_smooth` [0.1, 20] | → `saturation` [0.7, 0.1] — rasp (tanh drive) |
| `lenia_growth_sigma` [0.02, 0.16] | → `noise` [0, 0.5] — witte ruis bijmenging |
| `friction` [0.01, 0.98] | → `frequency` +8% bij hoge temp — thermische pitch |
| `current_zoom_level` | → `master_amplitude` [0.35, 0.75] |

### Per-stem GPU correcties (bij zoom > 5x)

| GpuTypeStats veld | → StemState effect |
|---|---|
| `density > 1` | → gate verder open, meer amplitude |
| `mean_speed` | → noise omhoog, pitch +4% |
| `centroid_x` [0,1] | → stereo pan [-1, 1] |
| `density` | → saturation boost |

### AudioEngine (audio_engine.rs)

Per stem:
1. **4× SawOscillator** met spread ±20 cent (supersaw)
2. **BiquadLPF** — 2-pool resonante lowpass, cutoff 80 Hz..8 kHz via `gate`
3. **NoiseGen** (xorshift32) — witte ruis bijmenging via `noise`
4. **tanh-saturatie** — drive = 1 + saturation × 4 (de Plinky-achtige rasp)
5. **Stereo pan** — constant-power, aangestuurd door `pan`

Mix: 7 stems / NUM_STEMS, × master_amplitude, geleverd als rodio::Source (44.1 kHz, stereo, blokgrootte 512).

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

**AudioEngine is non-blocking**: update() gebruikt `try_lock()` — als de audio thread de lock heeft, wordt de update overgeslagen (geen klik, geen blokkering).

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

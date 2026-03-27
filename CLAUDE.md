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
  particle_system.rs       # Core particle physics
  simulation_params.rs     # Parameter management
  interaction_rules.rs     # Particle interaction rules
  spatial_grid.rs          # Spatial partitioning (O(n) collision)
  buffer_utils.rs          # GPU buffer utilities
  shader_constants.rs      # Shader constants
  config.rs                # Configuration
  esp32_communication.rs   # ESP32 serial comm (native only)
  audio.rs                 # Audio system (native only)
  shaders/                 # WGSL GPU shaders
    compute.wgsl           # Particle physics compute shader
    vert.wgsl / frag.wgsl  # Render shaders
    lightning_compute.wgsl / lightning_vert.wgsl / lightning_frag_buffer.wgsl
    background_vert.wgsl / background_frag.wgsl
    glow_frag.wgsl / fisheye_frag.wgsl / vignette_frag.wgsl
    zoom_frag.wgsl / grid_frag.wgsl / frag_flat.wgsl
    text_vert.wgsl / text_frag.wgsl / text_overlay.wgsl
  index.html               # Web UI
  main.ts                  # Web TypeScript entry
  ui.ts                    # Web UI controls
  config.ts                # Web config
  particle-life-types.ts   # TypeScript type definitions
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
| Audio | — | rodio |

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
./start-native.sh         # Start native
./target/debug/native_minimal  # Run directly
```

## Simulation Parameters

- **Virtual world**: 3240×3240 units, rendered to 1080×1080 pixels
- **Particles**: 1600–6400 (pre-allocated at MAX=6400), 7 types
- **Zoom**: 1x–50x with GPU-side viewport culling
- **Physics**: Spatial grid partitioning, workgroup-aligned GPU dispatch (multiples of 64)

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

## Key Design Decisions

- Pre-allocated GPU buffers (MAX_PARTICLES=6400) — no runtime reallocation to prevent GPU panics
- ESP32 runs in separate thread to avoid blocking render loop
- Shaders loaded at runtime for web, embedded at compile time for native
- `lib.rs` is both `cdylib` (for WASM) and `rlib` (for native binary dependency)

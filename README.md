# Particle Life Simulation with WebGPU

A GPU-accelerated particle life simulation built in Rust, compiled to WebAssembly for the browser and deployed as a native binary in the art installation **Origin of Life**.

The simulation runs particles in a virtual world of 3240×3240 units, rendered to a 1080×1080 pixel canvas (round screen) with up to 12x zoom.

## Two deployment targets

### Native — Origin of Life installation
The native binary (`native_minimal`) runs on a dedicated machine connected to a round 1080×1080 display. It receives sensor data from ESP32 hardware (temperature, pressure, pH, electrical activity) via serial/ESP-NOW, and uses that data to steer the simulation parameters in real time. Audio is produced via a supersaw synthesizer that maps simulation statistics to 7 voices, one per particle type.

### Web (WASM) — experimentation and development
The WASM build runs in any WebGPU-capable browser at `localhost:3000`. It is primarily used to experiment with parameters, visual effects, and new features before deploying to the installation. The web version does not include the ESP32 sensor integration.

## Features

- **WebGPU compute shaders**: GPU-accelerated particle physics and rendering
- **7 particle types**: Each with its own interaction rules, color, and sonic voice
- **Lenia-inspired growth dynamics**: Continuous cellular automaton forces alongside particle-life rules
- **12x perceptual zoom**: Exponential slider mapping (`zoom = 12^t`) for uniform feel across the full range
- **GPU viewport culling**: Only visible particles are processed at high zoom levels
- **Spatial grid partitioning**: O(n) force calculations
- **Visual effects**: Fish-eye distortion, dynamic background, glow, vignette, lightning
- **Sonification**: Simulation statistics mapped to a 7-voice synthesizer (see below)
- **Real-time parameter control**: All simulation parameters adjustable via UI sliders

## Architecture

```
rust/particle-life/
├── src/
│   ├── lib.rs                  # WASM entry point + public API
│   ├── bin/native_minimal.rs   # Native binary (installation)
│   ├── simulation_params.rs    # Central parameter struct (shared)
│   ├── particle_system.rs      # CPU-side particle state
│   ├── webgpu_renderer.rs      # Render pipeline
│   ├── sonification.rs         # Simulation stats → StemState mapping
│   ├── stats_reader.rs         # GPU readback of simulation statistics
│   ├── audio_engine.rs         # Supersaw synthesizer (native)
│   ├── esp32_communication.rs  # Sensor data ingestion (native)
│   └── shaders/
│       ├── compute.wgsl        # Particle physics
│       ├── stats_compute.wgsl  # Per-frame statistics compute pass
│       └── ...                 # Render shaders
└── src/
    ├── main.ts                 # Web app entry point
    └── ui.ts                   # Slider/joystick UI
```

## Zoom system

The zoom slider runs from 1x to 12x. The mapping is **exponential** (`zoom = 12^t`) so that every millimeter of slider travel corresponds to the same perceptual factor, rather than the same absolute number of zoom levels. This is implemented in `SimulationParams::slider_to_zoom()` in Rust and shared between the WASM build and the ESP32 hardware input.

## Sonification

The simulation is continuously sonified via a 7-voice synthesizer — one voice per particle type. Each voice is a supersaw oscillator with a lowpass gate, noise layer, and saturation.

### Simulation statistics (GPU readback)

A compute shader (`stats_compute.wgsl`) runs every ~6 frames and returns the following statistics for each particle type, plus one set of global statistics:

**Per particle type (7×):**
| Statistic | Description |
|---|---|
| `viewport_count` | Number of particles of this type currently visible |
| `energy` | Mean speed of particles of this type (world-units/s) |
| `order` | Clustering measure [0,1]: 1 = tightly clustered, 0 = uniformly dispersed. Computed via single-pass position variance: `1 - stddev / 0.408` |
| `centroid_x` | Weighted X position in viewport [0,1], used for stereo panning |

**Global (viewport-wide):**
| Statistic | Description |
|---|---|
| `total_viewport_count` | Total active particles visible |
| `cluster_count` | Number of non-empty cells in an 8×8 viewport grid (proxy for number of clusters) |
| `avg_cluster_size` | `total_viewport_count / cluster_count` |

### Statistics → sound mapping

The key insight is the interaction between `order` and `energy`:

| order | energy | situation | sound |
|---|---|---|---|
| high | low | stable cluster, little motion | deep, clean, calm |
| high | high | vibrating cluster | buzzing, tense, saturated |
| low | high | chaos, particles flying | noisy, wide, turbulent |
| low | low | sparse, inactive | quiet, ambient |

- `viewport_count` per type → voice amplitude (absent types go silent)
- `order × energy` (vibration) → saturation/buzz drive
- `energy` → pitch drift upward
- `order` → gate openness and amplitude
- `centroid_x` → stereo panning
- `cluster_count` + `avg_cluster_size` → global detune width (many small clusters = wide, few large clusters = tight)
- `total_viewport_count` → master amplitude (empty viewport = quieter)

GPU stats blend in linearly from zoom 2x onward, reaching full weight at zoom 12x.

## Performance

- 55–60 FPS at 12x zoom on target hardware
- Spatial grid partitioning for O(n) force calculations
- GPU-side viewport culling: particles outside the viewport skip force and render passes
- Stats readback at ~10 Hz (every 6 frames) to avoid GPU stall overhead

## Building

**WASM (web):**
```bash
./build-wasm.sh
npm start
```

**Native:**
```bash
cargo build --bin native_minimal --release
```

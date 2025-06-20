# Multi-Platform Architecture Plan

## Overview
Refactor the particle life simulation for both web (WASM) and native (desktop) deployment while maintaining a single core codebase.

## Architecture Layers

### 1. Core Simulation Layer (`core/`)
Platform-agnostic simulation logic:
- `particle_system.rs` - Core particle physics
- `simulation_params.rs` - Parameter management
- `interaction_rules.rs` - Particle interaction logic
- `spatial_grid.rs` - Spatial partitioning
- `buffer_utils.rs` - Buffer management utilities

### 2. Rendering Layer (`rendering/`)
Platform-agnostic GPU rendering:
- `renderer.rs` - Abstract renderer trait
- `shaders.rs` - Shader management and embedding
- `gpu_state.rs` - GPU resource management

### 3. Platform-Specific Layers

#### Web Platform (`platforms/web/`)
- `lib.rs` - WASM bindings and web-specific code
- `web_renderer.rs` - Web-specific WGPU implementation
- `canvas_integration.rs` - Canvas and DOM integration

#### Native Platform (`platforms/native/`)
- `main.rs` - Native application entry point
- `native_renderer.rs` - Native WGPU implementation
- `window_manager.rs` - Window creation and management
- `native_ui.rs` - Native UI (egui/iced)

### 4. Shared Assets
- `shaders/` - WGSL shaders (embedded in native, loaded in web)
- `assets/` - Other shared resources

## Build Targets

### Web Target
```toml
[lib]
name = "particle_life_web"
crate-type = ["cdylib"]
```

### Native Target
```toml
[[bin]]
name = "particle_life_native"
path = "src/platforms/native/main.rs"
```

## Cargo.toml Structure
```toml
[features]
default = []
web = ["wasm-bindgen", "web-sys", "js-sys"]
native = ["winit", "egui", "pollster"]

[dependencies]
# Core dependencies (always included)
wgpu = "25.0.2"
bytemuck = { version = "1.0", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
rand = "0.8"

# Web-specific dependencies
wasm-bindgen = { version = "0.2.100", optional = true }
web-sys = { version = "0.3.77", optional = true, features = [...] }
js-sys = { version = "0.3", optional = true }

# Native-specific dependencies
winit = { version = "0.29", optional = true }
egui = { version = "0.24", optional = true }
egui-wgpu = { version = "0.24", optional = true }
egui-winit = { version = "0.24", optional = true }
pollster = { version = "0.3", optional = true }
```

## Shader Management

### Web (Runtime Loading)
```rust
async fn load_shader(path: &str) -> String {
    // Fetch shader from URL
}
```

### Native (Compile-Time Embedding)
```rust
const COMPUTE_SHADER: &str = include_str!("../../shaders/compute.wgsl");
const VERTEX_SHADER: &str = include_str!("../../shaders/vert.wgsl");
```

## Build Scripts

### Web Build
```bash
# Build WASM module
cargo build --target wasm32-unknown-unknown --features web --release
wasm-bindgen --out-dir pkg --web target/wasm32-unknown-unknown/release/particle_life_web.wasm

# Bundle with webpack
npm run build
```

### Native Build
```bash
# Build native binary
cargo build --features native --release
```

## Deployment

### Web Deployment
- Static files: HTML, JS, WASM, shaders
- CDN-friendly with proper MIME types
- Service worker for offline capability

### Native Deployment
- Single executable with embedded assets
- Platform-specific installers (macOS .dmg, Windows .msi, Linux .AppImage)
- Auto-updater integration

## Migration Steps

1. **Phase 1**: Refactor core simulation code to remove web dependencies
2. **Phase 2**: Create abstract renderer trait and platform implementations
3. **Phase 3**: Implement native UI and window management
4. **Phase 4**: Set up build system and deployment pipelines
5. **Phase 5**: Testing and optimization for both platforms

## Benefits

1. **Code Reuse**: 90%+ code shared between platforms
2. **Performance**: Native version can use platform-specific optimizations
3. **Deployment Flexibility**: Web for easy access, native for performance
4. **Maintenance**: Single codebase with platform-specific variants
5. **Feature Parity**: Identical simulation behavior across platforms

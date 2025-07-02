# Particle Life Simulation with WebGPU

This project implements a sophisticated particle life simulation using WebGPU for high-performance parallel computation. The simulation runs particles in a virtual world of 3240×3240 units, rendered to a 1080×1080 pixel canvas with up to 6x zoom capability.

## Features

- **High-Performance WebGPU Rendering**: GPU-accelerated particle simulation with compute shaders
- **Configurable Particle System**: Support for multiple particle types with customizable interaction rules
- **Advanced Visual Effects**: Fish-eye distortion, dynamic backgrounds, and smooth particle animations
- **Spatial Optimization**: Grid-based spatial partitioning for efficient collision detection
- **Cross-Platform**: Web (WASM) and native (desktop) deployment support
- **Real-time Parameter Control**: Live adjustment of simulation parameters through UI controls
- **Advanced Zoom System**: 6x zoom with GPU-side viewport culling and zoom-adjusted drift for optimal performance

## Architecture

- **Virtual World**: 3240×3240 simulation space for particle physics
- **Canvas Rendering**: 1080×1080 display resolution with round screen viewport
- **GPU Viewport Culling**: Efficient rendering of only visible particles at high zoom levels
- **Zoom-Adjusted Drift**: Drift speed automatically scales with zoom level for better user experience
- **Multi-threaded GPU Compute**: Parallel particle updates using WebGPU compute shaders
- **Rust + TypeScript**: Clean architecture with Rust handling simulation core and TypeScript managing UI

## Performance

The simulation is optimized for real-time performance with:
- Spatial grid partitioning for O(n) collision detection
- GPU-accelerated force calculations and viewport culling
- Efficient memory layouts with proper 16-byte alignment for GPU uniform buffers
- Zoom-adjusted drift: particles stay in view longer when zoomed in
- Configurable FPS limiting (55-60 FPS even at 6x zoom)
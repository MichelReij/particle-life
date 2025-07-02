// TypeScript types for UI integration with Rust WASM implementation
// Note: The actual simulation parameters are now handled entirely in Rust
// This file only contains types needed for UI communication

export interface Particle {
    position: [number, number]; // vec2f
    velocity: [number, number]; // vec2f
    type: number; // u32 or f32, represents particle type index
}

export interface InteractionRule {
    // Attraction force (can be negative for repulsion)
    attraction: number; // f32
    // Minimum interaction radius (particles closer than this will repel strongly)
    minRadius: number; // f32
    // Maximum interaction radius (particles further than this do not interact)
    maxRadius: number; // f32
}

// Rules will be a matrix where rules[typeA][typeB] defines interaction
export type ParticleRules = InteractionRule[][];

// Simplified params interface for UI communication
// The actual struct with GPU layout is defined in Rust (simulation_params.rs)
export interface SimulationParams {
    deltaTime: number;
    friction: number;
    numParticles: number;
    numTypes: number;
    virtualWorldWidth: number;
    virtualWorldHeight: number;
    canvasRenderWidth: number;
    canvasRenderHeight: number;
    virtualWorldOffsetX: number;
    virtualWorldOffsetY: number;
    boundaryMode: 0 | 1 | 2; // 0: disappear/respawn, 1: wrap, 2: bounce
    particleRenderSize: number;
    forceScale: number;
    rSmooth: number;
    flatForce: boolean;
    driftXPerSecond: number;
    interTypeAttractionScale: number;
    interTypeRadiusScale: number;
    time: number;
    fisheyeStrength: number;
    backgroundColor: [number, number, number];
    leniaEnabled: boolean;
    leniaGrowthMu: number;
    leniaGrowthSigma: number;
    leniaKernelRadius: number;
    lightningFrequency: number;
    lightningIntensity: number;
    lightningDuration: number;
    // Note: Viewport, transition, and spatial grid parameters are handled internally by Rust
}

export const MAX_PARTICLE_TYPES = 16;

export enum BoundaryMode {
    Wrap = 0,
    Bounce = 1,
    Disappear = 2,
}

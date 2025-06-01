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

export interface SimulationParams {
    deltaTime: number; // Time step for the simulation
    friction: number; // Friction coefficient to slow down particles
    numParticles: number; // Total number of particles
    numTypes: number; // Number of particle types

    virtualWorldWidth: number;
    virtualWorldHeight: number;
    canvasRenderWidth: number;
    canvasRenderHeight: number;
    virtualWorldOffsetX: number;
    virtualWorldOffsetY: number;
    boundaryMode: 0 | 1 | 2; // 0: disappear/respawn, 1: wrap, 2: bounce (not implemented yet)
    particleRenderSize: number;
    forceScale: number; // Scales the overall force applied to particles
    rSmooth: number; // Smoothing factor for repulsion force calculation
    flatForce: boolean; // Keep as boolean, buffer conversion will handle it
    driftXPerSecond: number; // Horizontal drift speed
    interTypeAttractionScale: number; // Scales attraction between different types
    interTypeRadiusScale: number; // Scales interaction radii between different types
    time: number; // Time in seconds for animation
    fisheyeStrength: number; // Fisheye distortion strength
    backgroundColor: [number, number, number]; // RGB color
    // _padding1 is implicit in WGSL if needed after backgroundColor for alignment
}

// Size of the SimulationParams struct in bytes
// Must be a multiple of 16 for WGSL alignment
// deltaTime: f32 (4)
// friction: f32 (4)
// numParticles: u32 (4)
// numTypes: u32 (4)
// virtualWorldWidth: f32 (4)
// virtualWorldHeight: f32 (4)
// canvasRenderWidth: f32 (4)
// canvasRenderHeight: f32 (4)
// virtualWorldOffsetX: f32 (4)
// virtualWorldOffsetY: f32 (4)
// boundaryMode: u32 (4)
// particleRenderSize: f32 (4)
// forceScale: f32 (4)
// rSmooth: f32 (4)
// flatForce: u32 (4) -> will be read as f32 in buffer if not careful, but WGSL u32 is fine
// driftXPerSecond: f32 (4)
// interTypeAttractionScale: f32 (4)
// interTypeRadiusScale: f32 (4)
// time: f32 (4)
// fisheyeStrength: f32 (4)
// -- Total so far: 20 * 4 = 80 bytes --
// backgroundColor: vec3f (3 * 4 = 12 bytes)
// -- Total so far: 80 + 12 = 92 bytes --
// To make it a multiple of 16, we need 4 more bytes of padding (_padding1: f32).
// So, 92 + 4 = 96 bytes.
// This corresponds to 24 floats (20 actual params + 3 for color + 1 padding)
export const SIM_PARAMS_SIZE_BYTES = 24 * 4; // 96 bytes

export const MAX_PARTICLE_TYPES = 16;

export enum BoundaryMode { // Added export
    Wrap = 0,
    Bounce = 1,
}

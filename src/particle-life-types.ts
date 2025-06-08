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

    // Lenia-inspired parameters
    leniaEnabled: boolean; // Enable Lenia-style interactions
    leniaGrowthMu: number; // Lenia growth function center (μ)
    leniaGrowthSigma: number; // Lenia growth function spread (σ)
    leniaKernelRadius: number; // Lenia kernel radius

    // Lightning parameters
    lightningFrequency: number; // Lightning strikes per second when electrical activity is high
    lightningIntensity: number; // Lightning brightness/strength (0-1)
    lightningDuration: number; // Duration of each lightning flash in seconds

    // Padding to align to 128 bytes (32 floats) for WebGPU requirements
    _padding1?: number;
    _padding2?: number;
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
// leniaEnabled: u32 (4) - boolean as u32
// leniaGrowthMu: f32 (4)
// leniaGrowthSigma: f32 (4)
// leniaKernelRadius: f32 (4)
// lightningFrequency: f32 (4)
// lightningIntensity: f32 (4)
// lightningDuration: f32 (4)
// -- Total so far: 92 + 28 = 120 bytes --
// However, WebGPU may require more padding for the WGSL struct alignment
// Increasing to 128 bytes (32 floats) to meet WebGPU requirements
export const SIM_PARAMS_SIZE_BYTES = 32 * 4; // 128 bytes

export const MAX_PARTICLE_TYPES = 16;

export enum BoundaryMode { // Added export
    Wrap = 0,
    Bounce = 1,
}

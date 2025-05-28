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
    flatForce: 0 | 1; // Boolean (0 or 1) to use flat force or distance-based force
    driftXPerSecond: number; // Horizontal drift speed
    interTypeAttractionScale: number; // Scales attraction between different types
    interTypeRadiusScale: number; // Scales interaction radii between different types
    time: number; // Time in seconds for animation
}

export const SIM_PARAMS_SIZE_BYTES = 20 * 4; // 20 floats, each 4 bytes (includes new time field)

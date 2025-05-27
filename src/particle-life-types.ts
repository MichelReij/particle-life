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
    deltaTime: number; // f32
    friction: number; // f32
    numParticles: number; // u32
    numTypes: number; // u32
    virtualWorldWidth: number; // f32
    virtualWorldHeight: number; // f32
    canvasRenderWidth: number; // f32
    canvasRenderHeight: number; // f32
    virtualWorldOffsetX: number; // f32
    virtualWorldOffsetY: number; // f32
    boundaryMode: number; // u32 (0: disappear/respawn, 1: wrap)
    particleRenderSize: number; // f32
    forceScale: number; // f32
    rSmooth: number; // f32
    flatForce: number; // u32 (0: false, 1: true)
    driftXPerSecond: number; // f32 (new)
    // _padding_final is handled by buffer sizing and explicit write in main.ts
}

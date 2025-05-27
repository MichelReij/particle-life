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
    deltaTime: number;
    friction: number;
    numParticles: number;
    numTypes: number;
    canvasWidth: number; // Deprecated, use worldWidth from sim_params
    canvasHeight: number; // Deprecated, use worldHeight from sim_params
    worldWidth: number; // Actual simulation area width
    worldHeight: number; // Actual simulation area height
    forceScale: number; // To adjust overall force strength
    velocityScale: number; // To adjust max velocity or initial velocities // This seems unused in current WGSL
    rSmooth: number; // Smoothing factor for repulsion, from C++ RADIUS_SMOOTH
    flatForce: boolean; // Whether to use flat force model from C++
    particleRenderSize: number;
}

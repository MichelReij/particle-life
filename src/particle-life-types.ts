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
    virtualWorldWidth: number; // Renamed from worldWidth, e.g., 1000px
    virtualWorldHeight: number; // Renamed from worldHeight, e.g., 1000px
    canvasRenderWidth: number; // Actual renderable canvas width, e.g., 800px
    canvasRenderHeight: number; // Actual renderable canvas height, e.g., 800px
    virtualWorldOffsetX: number; // Offset of canvas within virtual world, e.g., 100px
    virtualWorldOffsetY: number; // Offset of canvas within virtual world, e.g., 100px
    boundaryMode: number; // 0: disappear, 1: wrap
    forceScale: number;
    rSmooth: number;
    flatForce: boolean;
    particleRenderSize: number;
    // Removed canvasWidth, canvasHeight, velocityScale as they were deprecated or unused
}

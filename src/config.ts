// Central configuration constants for the particle life simulation
// This TypeScript file mirrors the Rust config.rs constants

// Virtual world dimensions - the simulation space where particles exist
export const VIRTUAL_WORLD_WIDTH = 3240.0;
export const VIRTUAL_WORLD_HEIGHT = 3240.0;

// Canvas/render dimensions - the final output size
export const CANVAS_WIDTH = 1080.0;
export const CANVAS_HEIGHT = 1080.0;

// Particle rendering size - the diameter of particles in pixels
export const PARTICLE_SIZE = 9.0;
export const PARTICLE_SIZE_MIN = 8.0;
export const PARTICLE_SIZE_MAX = 32.0;

// Particle system configuration
export const DEFAULT_NUM_PARTICLES = 4800;
export const MAX_PARTICLES = 4800;
export const MIN_PARTICLES = 1200;

// FPS display configuration - no longer capped to allow 3-digit display
export const FPS_SAMPLE_COUNT = 10; // Number of samples for moving average
export const FPS_UPDATE_INTERVAL = 0.5; // Update interval in seconds
export const FPS_CONSOLE_INTERVAL = 3.0; // Console output interval in seconds

// Zoom configuration - maximum 12x zoom capability with direct canvas rendering
// The efficient direct-to-canvas pipeline allows for high zoom levels while maintaining quality
export const ZOOM_MIN = 1.0;
export const ZOOM_MAX = 12.0;
export const ZOOM_DEFAULT = 1.0;
export const ZOOM_STEP = 0.01;

// Rendering pipeline configuration
// We now render directly to canvas size (1080x1080) instead of virtual world size (3240x3240)
// This is 9x more efficient (1080²/3240² = 0.11) while maintaining visual quality through:
// - GPU viewport culling (only visible particles are rendered)
// - Proper particle scaling (particles maintain size across zoom levels)
// - Direct canvas-space rendering (no downsampling required)

// At max zoom (12x), each screen pixel represents this many world units
export const MAX_ZOOM_WORLD_UNITS_PER_PIXEL =
    VIRTUAL_WORLD_WIDTH / (CANVAS_WIDTH * ZOOM_MAX); // ~0.25

// Convenience constants derived from the main dimensions
export const VIRTUAL_WORLD_CENTER_X = VIRTUAL_WORLD_WIDTH / 2.0; // 1620.0
export const VIRTUAL_WORLD_CENTER_Y = VIRTUAL_WORLD_HEIGHT / 2.0; // 1620.0

// Scaling factor from virtual world to canvas
export const WORLD_TO_CANVAS_SCALE = CANVAS_WIDTH / VIRTUAL_WORLD_WIDTH; // 0.333

// Integer versions for GPU operations
export const VIRTUAL_WORLD_WIDTH_U32 = Math.floor(VIRTUAL_WORLD_WIDTH); // 3240
export const VIRTUAL_WORLD_HEIGHT_U32 = Math.floor(VIRTUAL_WORLD_HEIGHT); // 3240
export const CANVAS_WIDTH_U32 = Math.floor(CANVAS_WIDTH); // 1080
export const CANVAS_HEIGHT_U32 = Math.floor(CANVAS_HEIGHT); // 1080

// Configuration interface for easy experimentation
export interface WorldConfig {
    virtualWorldWidth: number;
    virtualWorldHeight: number;
    canvasWidth: number;
    canvasHeight: number;
    particleSize: number;
    defaultNumParticles: number;
    maxParticles: number;
    minParticles: number;
}

export const DEFAULT_WORLD_CONFIG: WorldConfig = {
    virtualWorldWidth: VIRTUAL_WORLD_WIDTH,
    virtualWorldHeight: VIRTUAL_WORLD_HEIGHT,
    canvasWidth: CANVAS_WIDTH,
    canvasHeight: CANVAS_HEIGHT,
    particleSize: PARTICLE_SIZE,
    defaultNumParticles: DEFAULT_NUM_PARTICLES,
    maxParticles: MAX_PARTICLES,
    minParticles: MIN_PARTICLES,
};

// Utility functions
export function getWorldCenter(
    config: WorldConfig = DEFAULT_WORLD_CONFIG,
): [number, number] {
    return [config.virtualWorldWidth / 2.0, config.virtualWorldHeight / 2.0];
}

export function getScaleFactor(
    config: WorldConfig = DEFAULT_WORLD_CONFIG,
): number {
    return config.canvasWidth / config.virtualWorldWidth;
}

export function getVirtualDimensions(
    config: WorldConfig = DEFAULT_WORLD_CONFIG,
): [number, number] {
    return [
        Math.floor(config.virtualWorldWidth),
        Math.floor(config.virtualWorldHeight),
    ];
}

export function getCanvasDimensions(
    config: WorldConfig = DEFAULT_WORLD_CONFIG,
): [number, number] {
    return [Math.floor(config.canvasWidth), Math.floor(config.canvasHeight)];
}

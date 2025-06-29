// Central configuration constants for the particle life simulation
// This TypeScript file mirrors the Rust config.rs constants

// Virtual world dimensions - the simulation space where particles exist
export const VIRTUAL_WORLD_WIDTH = 3240.0;
export const VIRTUAL_WORLD_HEIGHT = 3240.0;

// Canvas/render dimensions - the final output size
export const CANVAS_WIDTH = 1080.0;
export const CANVAS_HEIGHT = 1080.0;

// Particle rendering size - the diameter of particles in pixels
export const PARTICLE_SIZE = 16.0;
export const PARTICLE_SIZE_MIN = 8.0;
export const PARTICLE_SIZE_MAX = 32.0;

// Convenience constants derived from the main dimensions
export const VIRTUAL_WORLD_CENTER_X = VIRTUAL_WORLD_WIDTH / 2.0; // 1620.0
export const VIRTUAL_WORLD_CENTER_Y = VIRTUAL_WORLD_HEIGHT / 2.0; // 1620.0

// Scaling factor from virtual world to canvas
export const WORLD_TO_CANVAS_SCALE = CANVAS_WIDTH / VIRTUAL_WORLD_WIDTH; // 0.333...

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
}

export const DEFAULT_WORLD_CONFIG: WorldConfig = {
    virtualWorldWidth: VIRTUAL_WORLD_WIDTH,
    virtualWorldHeight: VIRTUAL_WORLD_HEIGHT,
    canvasWidth: CANVAS_WIDTH,
    canvasHeight: CANVAS_HEIGHT,
    particleSize: PARTICLE_SIZE,
};

// Utility functions
export function getWorldCenter(
    config: WorldConfig = DEFAULT_WORLD_CONFIG
): [number, number] {
    return [config.virtualWorldWidth / 2.0, config.virtualWorldHeight / 2.0];
}

export function getScaleFactor(
    config: WorldConfig = DEFAULT_WORLD_CONFIG
): number {
    return config.canvasWidth / config.virtualWorldWidth;
}

export function getVirtualDimensions(
    config: WorldConfig = DEFAULT_WORLD_CONFIG
): [number, number] {
    return [
        Math.floor(config.virtualWorldWidth),
        Math.floor(config.virtualWorldHeight),
    ];
}

export function getCanvasDimensions(
    config: WorldConfig = DEFAULT_WORLD_CONFIG
): [number, number] {
    return [Math.floor(config.canvasWidth), Math.floor(config.canvasHeight)];
}

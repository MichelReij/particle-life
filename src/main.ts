// --- WebGPU Particle Life Example ---
// This example demonstrates a particle simulation with interactions governed by rules,
// inspired by Particle Life.
// It uses a compute shader to update particle states and a render pipeline to draw them.

import computeWGSL from "./shaders/compute.wgsl?raw"; // Changed from .wgsl to .wgsl?raw
import vertWGSL from "./shaders/vert.wgsl?raw"; // Changed from .wgsl to .wgsl?raw
import fragWGSL from "./shaders/frag.wgsl?raw"; // Will need to be updated for particle rendering
import backgroundVertWGSL from "./shaders/background_vert.wgsl?raw"; // New background vertex shader
import backgroundFragWGSL from "./shaders/background_frag.wgsl?raw"; // New background fragment shader
import { generateParticleColors, logParticleColors } from "./hsluv-colors";
import { hsluvToRgb } from "hsluv-ts"; // Import HSLuv for background color generation
import vignetteFragWGSL from "./shaders/vignette_frag.wgsl?raw"; // Import for vignette shader
import fisheyeFragWGSL from "./shaders/fisheye_frag.wgsl?raw"; // Import for fisheye shader
import gridFragWGSL from "./shaders/grid_frag.wgsl?raw"; // Import for grid shader
import zoomFragWGSL from "./shaders/zoom_frag.wgsl?raw"; // Import for zoom shader

import {
    Particle,
    InteractionRule,
    ParticleRules,
    SimulationParams,
    SIM_PARAMS_SIZE_BYTES, // Import the constant
    BoundaryMode,
} from "./particle-life-types";

// --- Global Singleton & Canvas Setup (largely unchanged) ---
const GLOBAL_KEY = "__webgpu_particle_life_singleton__";
if ((window as any)[GLOBAL_KEY]) {
    const prev = (window as any)[GLOBAL_KEY];
    if (prev.cancelAnimation) prev.cancelAnimation();
    if (prev.device && typeof prev.device.destroy === "function") {
        try {
            prev.device.destroy();
        } catch (e) {
            console.warn("Error destroying previous device:", e);
        }
    }
    if (prev.canvas && prev.canvas.parentNode) {
        prev.canvas.parentNode.removeChild(prev.canvas);
    }
}

// @ts-ignore
if (
    (window as any).__webgpuDevice &&
    typeof (window as any).__webgpuDevice.destroy === "function"
) {
    try {
        (window as any).__webgpuDevice.destroy();
    } catch (e) {
        console.warn("Error destroying existing __webgpuDevice:", e);
    }
}
// @ts-ignore
window.__webgpuDevice = undefined;

const CANVAS_ID = "__webgpu_particle_life_canvas__";
const oldCanvas = document.getElementById(CANVAS_ID);
if (oldCanvas && oldCanvas.parentNode) {
    oldCanvas.parentNode.removeChild(oldCanvas);
}
const canvas = document.createElement("canvas");
canvas.id = CANVAS_ID;

// Add canvas to the canvasContainer div in the Dashboard
const canvasContainer = document.getElementById("canvasContainer");
if (canvasContainer) {
    canvasContainer.appendChild(canvas);
} else {
    // Fallback to body if canvasContainer not found
    document.body.appendChild(canvas);
    console.warn("canvasContainer not found, adding canvas to body");
}

canvas.width = 800;
canvas.height = 800;
canvas.style.width = "800px";
canvas.style.height = "800px";

// === Particle Life Configuration ===
const NUM_PARTICLES = 3200; // Number of particles - increased for stronger Lenia effects
const NUM_TYPES = 5; // Number of particle types - reduced for more coherent Lenia patterns
const PARTICLE_RENDER_SIZE = 12.0;
const PARTICLE_SIZE_BYTES = 24; // pos(2f) + vel(2f) + type(1u) + padding(1f for alignment if needed, or size for individual particle size)
// For PARTICLE_SIZE_BYTES = 24: pos(8) + vel(8) + type(4) + padding(4) to make it multiple of 16 for some platforms, or particle_size (f32)
const RULE_SIZE_BYTES = 16; // attraction(f32), min_radius(f32), max_radius(f32), padding(f32)
// SIM_PARAMS_SIZE_BYTES is now imported from particle-life-types.ts

let VIRTUAL_WORLD_BORDER = 0; // No border: virtual world and render world both 2400x2400px. Now a let.
let DRIFT_X_PER_SECOND = -10.0; // Pixels per second, negative for left drift. Now a let.
let FORCE_SCALE = 400.0;
let FRICTION = 0.1;
let R_SMOOTH = 5.0;
let INTER_TYPE_ATTRACTION_SCALE = 1.0; // Default: no change to attraction
let INTER_TYPE_RADIUS_SCALE = 1.0; // Default: no change to radii

// Environmental parameters for the new sliders
let temperature = 20; // Default temperature
let electricalActivity = 0.68; // Default electrical activity
let uvLight = 25; // Default UV light
let pressure = 1; // Default pressure

// localStorage functionality for persistent settings
const STORAGE_KEYS = {
    temperature: "particleLife_temperature",
    electricalActivity: "particleLife_electricalActivity",
    uvLight: "particleLife_uvLight",
    pressure: "particleLife_pressure",
    zoom: "particleLife_zoom",
    drift: "particleLife_drift",
    forceScale: "particleLife_forceScale",
    friction: "particleLife_friction",
    rSmooth: "particleLife_rSmooth",
    interTypeAttractionScale: "particleLife_interTypeAttractionScale",
    interTypeRadiusScale: "particleLife_interTypeRadiusScale",
    fisheyeStrength: "particleLife_fisheyeStrength",
};

function saveToLocalStorage(key: string, value: number): void {
    try {
        localStorage.setItem(key, value.toString());
    } catch (e) {
        console.warn("Failed to save to localStorage:", e);
    }
}

function loadFromLocalStorage(key: string, defaultValue: number): number {
    try {
        const stored = localStorage.getItem(key);
        if (stored !== null) {
            const parsed = parseFloat(stored);
            if (!isNaN(parsed)) {
                return parsed;
            }
        }
    } catch (e) {
        console.warn("Failed to load from localStorage:", e);
    }
    return defaultValue;
}

// Temperature mapping functions
function temperatureToDrift(temp: number): number {
    // Linear mapping: temp [3, 40] → drift [0, -80]
    // At temp = 3°C: drift = 0 px/s
    // At temp = 40°C: drift = -80 px/s
    return -((temp - 3) * 80) / 37;
}

function temperatureToFriction(temp: number): number {
    // Linear mapping: temp [3, 40] → friction [0.30, 0.01]
    // At temp = 3°C: friction = 0.30 (highest)
    // At temp = 40°C: friction = 0.01 (lowest)
    return 0.3 - ((temp - 3) * 0.29) / 37;
}

function updateDriftAndFrictionFromTemperature(temp: number): void {
    const newDrift = temperatureToDrift(temp);
    const newFriction = temperatureToFriction(temp);

    // Update drift using existing function (handles background color and GPU buffer)
    updateBackgroundColorAndDrift(newDrift);

    // Update friction parameter and GPU buffer
    simParams.friction = newFriction;
    if (device && simParamsBuffer) {
        device.queue.writeBuffer(
            simParamsBuffer,
            1 * 4, // Byte offset for friction (float at index 1)
            new Float32Array([simParams.friction])
        );
    }

    // Update drift slider and display
    const driftSlider = document.getElementById(
        "driftSlider"
    ) as HTMLInputElement;
    const driftValueDisplay = document.getElementById("driftValue");
    if (driftSlider && driftValueDisplay) {
        driftSlider.value = newDrift.toString();
        driftValueDisplay.textContent = newDrift.toFixed(2);
    }

    // Update friction slider and display
    const frictionSlider = document.getElementById(
        "frictionSlider"
    ) as HTMLInputElement;
    const frictionValueDisplay = document.getElementById("frictionValue");
    if (frictionSlider && frictionValueDisplay) {
        frictionSlider.value = newFriction.toString();
        frictionValueDisplay.textContent = newFriction.toFixed(2);
    }
}

// Pressure mapping functions
function pressureToRSmooth(pressure: number): number {
    // Linear mapping: pressure [0, 350] → rSmooth [20, 0.1]
    // At pressure = 0: rSmooth = 20 (highest)
    // At pressure = 350: rSmooth = 0.1 (lowest)
    return 20 - (pressure * 19.9) / 350;
}

function pressureToForceScale(pressure: number): number {
    // Linear mapping: pressure [0, 350] → forceScale [100, 800]
    // At pressure = 0: forceScale = 100 (lowest)
    // At pressure = 350: forceScale = 800 (highest)
    return 100 + (pressure * 700) / 350;
}

function pressureToInterTypeRadiusScale(pressure: number): number {
    // Linear mapping: pressure [0, 350] → interTypeRadiusScale [1.3, 0.50]
    // At pressure = 0: interTypeRadiusScale = 1.3 (highest)
    // At pressure = 350: interTypeRadiusScale = 0.50 (lowest)
    return 1.3 - (pressure * 0.8) / 350;
}

function pressureToInterTypeAttractionScale(pressure: number): number {
    // Linear mapping: pressure [0, 350] → interTypeAttractionScale [0.8, 2.0]
    // At pressure = 0: interTypeAttractionScale = 0.8 (lowest)
    // At pressure = 350: interTypeAttractionScale = 2.0 (highest)
    return 0.8 + (pressure * 1.2) / 350;
}

function updateParametersFromPressure(pressure: number): void {
    const newRSmooth = pressureToRSmooth(pressure);
    const newForceScale = pressureToForceScale(pressure);
    const newInterTypeRadiusScale = pressureToInterTypeRadiusScale(pressure);
    const newInterTypeAttractionScale =
        pressureToInterTypeAttractionScale(pressure);

    // Update R Smooth parameter and GPU buffer
    simParams.rSmooth = newRSmooth;
    if (device && simParamsBuffer) {
        device.queue.writeBuffer(
            simParamsBuffer,
            13 * 4, // Byte offset for rSmooth (float at index 13)
            new Float32Array([simParams.rSmooth])
        );
    }

    // Update Force Scale parameter and GPU buffer
    simParams.forceScale = newForceScale;
    if (device && simParamsBuffer) {
        device.queue.writeBuffer(
            simParamsBuffer,
            12 * 4, // Byte offset for forceScale (float at index 12)
            new Float32Array([simParams.forceScale])
        );
    }

    // Update Inter-Type Radius Scale parameter and GPU buffer
    simParams.interTypeRadiusScale = newInterTypeRadiusScale;
    if (device && simParamsBuffer) {
        device.queue.writeBuffer(
            simParamsBuffer,
            17 * 4, // Byte offset for interTypeRadiusScale (float at index 17)
            new Float32Array([simParams.interTypeRadiusScale])
        );
    }

    // Update Inter-Type Attraction Scale parameter and GPU buffer
    simParams.interTypeAttractionScale = newInterTypeAttractionScale;
    if (device && simParamsBuffer) {
        device.queue.writeBuffer(
            simParamsBuffer,
            16 * 4, // Byte offset for interTypeAttractionScale (float at index 16)
            new Float32Array([simParams.interTypeAttractionScale])
        );
    }

    // Update R Smooth slider and display
    const rSmoothSlider = document.getElementById(
        "rSmoothSlider"
    ) as HTMLInputElement;
    const rSmoothValueDisplay = document.getElementById("rSmoothValue");
    if (rSmoothSlider && rSmoothValueDisplay) {
        rSmoothSlider.value = newRSmooth.toString();
        rSmoothValueDisplay.textContent = newRSmooth.toFixed(2);
    }

    // Update Force Scale slider and display
    const forceScaleSlider = document.getElementById(
        "forceScaleSlider"
    ) as HTMLInputElement;
    const forceScaleValueDisplay = document.getElementById("forceScaleValue");
    if (forceScaleSlider && forceScaleValueDisplay) {
        forceScaleSlider.value = newForceScale.toString();
        forceScaleValueDisplay.textContent = newForceScale.toFixed(2);
    }

    // Update Inter-Type Radius Scale slider and display
    const interTypeRadiusScaleSlider = document.getElementById(
        "interTypeRadiusScaleSlider"
    ) as HTMLInputElement;
    const interTypeRadiusScaleValueDisplay = document.getElementById(
        "interTypeRadiusScaleValue"
    );
    if (interTypeRadiusScaleSlider && interTypeRadiusScaleValueDisplay) {
        interTypeRadiusScaleSlider.value = newInterTypeRadiusScale.toString();
        interTypeRadiusScaleValueDisplay.textContent =
            newInterTypeRadiusScale.toFixed(2);
    }

    // Update Inter-Type Attraction Scale slider and display
    const interTypeAttractionScaleSlider = document.getElementById(
        "interTypeAttractionScaleSlider"
    ) as HTMLInputElement;
    const interTypeAttractionScaleValueDisplay = document.getElementById(
        "interTypeAttractionScaleValue"
    );
    if (
        interTypeAttractionScaleSlider &&
        interTypeAttractionScaleValueDisplay
    ) {
        interTypeAttractionScaleSlider.value =
            newInterTypeAttractionScale.toString();
        interTypeAttractionScaleValueDisplay.textContent =
            newInterTypeAttractionScale.toFixed(2);
    }
}

let device: GPUDevice;
let presentationFormat: GPUTextureFormat;
let context: GPUCanvasContext;

// Buffers
let simParamsBuffer: GPUBuffer;
let rulesBuffer: GPUBuffer;
let particleBuffers: [GPUBuffer, GPUBuffer]; // Ping-pong buffers
let quadVertexBuffer: GPUBuffer; // Hoisted declaration
let particleColorsBuffer: GPUBuffer; // Buffer for precomputed HSLuv colors

// Pipelines and Bind Groups
let computePipeline: GPUComputePipeline;
let renderPipeline: GPURenderPipeline;
let backgroundRenderPipeline: GPURenderPipeline; // New pipeline for the background
let computeBindGroups: [GPUBindGroup, GPUBindGroup];
let renderBindGroup: GPUBindGroup;
let backgroundRenderBindGroup: GPUBindGroup;
let vignetteRenderPipeline: GPURenderPipeline; // For vignette
let vignetteRenderBindGroup: GPUBindGroup; // For vignette
let fisheyeRenderPipeline: GPURenderPipeline; // For fisheye distortion
let fisheyeRenderBindGroup: GPUBindGroup; // For fisheye distortion
let gridRenderPipeline: GPURenderPipeline; // For oscilloscope grid
let gridRenderBindGroup: GPUBindGroup; // For oscilloscope grid
let zoomRenderPipeline: GPURenderPipeline; // For zoom
let zoomRenderBindGroup: GPUBindGroup; // For zoom
let zoomUniformsBuffer: GPUBuffer; // For zoom level
let sceneTexture: GPUTexture; // For multi-pass rendering
let intermediateTexture: GPUTexture; // For fisheye post-processing
let sceneSampler: GPUSampler; // For multi-pass rendering

let currentParticleBufferIndex = 0;
let animationId: number | undefined;
let lastFrameTime = 0;
let currentTime = 0; // Tracks total elapsed time for animations
let currentZoomLevel = 1.5; // Current zoom level (1x to 3x)

// FPS calculation variables
let frameCount = 0;
let lastFPSTime = 0;
let fpsDisplayElement: HTMLElement | null;

// Simulation parameters object, matching the interface and shader struct
let simParams: SimulationParams = {
    deltaTime: 1 / 60, // Initial delta_time, will be updated each frame
    friction: FRICTION,
    numParticles: NUM_PARTICLES,
    numTypes: NUM_TYPES,
    virtualWorldWidth: 0, // Calculated in initWebGPU
    virtualWorldHeight: 0, // Calculated in initWebGPU
    canvasRenderWidth: 0, // Calculated in initWebGPU
    canvasRenderHeight: 0, // Calculated in initWebGPU
    virtualWorldOffsetX: 0, // Calculated in initWebGPU
    virtualWorldOffsetY: 0, // Calculated in initWebGPU
    boundaryMode: BoundaryMode.Wrap,
    particleRenderSize: PARTICLE_RENDER_SIZE,
    forceScale: FORCE_SCALE,
    rSmooth: R_SMOOTH,
    flatForce: false, // Default, will be converted to 0/1 for buffer
    driftXPerSecond: DRIFT_X_PER_SECOND,
    interTypeAttractionScale: INTER_TYPE_ATTRACTION_SCALE,
    interTypeRadiusScale: INTER_TYPE_RADIUS_SCALE,
    time: 0.0, // Current animation time, updated each frame
    fisheyeStrength: 3.0, // Default fisheye distortion strength
    backgroundColor: [0.0, 0.0, 0.0], // Initial: black, updated by updateBackgroundColorAndDrift

    // Lenia-inspired parameters
    leniaEnabled: true, // Start with Lenia enabled for immediate effect
    leniaGrowthMu: 0.18, // Slightly higher for more dynamic growth
    leniaGrowthSigma: 0.025, // Increased spread for more gradual transitions
    leniaKernelRadius: 75.0, // Larger kernel for stronger long-range effects
};

// --- Helper Functions ---
function createRandomRules(numTypes: number): InteractionRule[][] {
    const rules: InteractionRule[][] = [];
    for (let i = 0; i < numTypes; i++) {
        rules[i] = [];
        for (let j = 0; j < numTypes; j++) {
            rules[i][j] = {
                // Attraction: random between -0.1 and 0.3
                attraction: Math.random() * 0.4 - 0.1,
                // Min Radius: random between 10 and 30 (unchanged for now)
                minRadius: Math.random() * 20 + 10,
                // Max Radius: random between minRadius + 20 and minRadius + 80
                maxRadius: 0, // Will be set below
            };
            rules[i][j].maxRadius =
                rules[i][j].minRadius + (Math.random() * 60 + 20);

            // Self-interaction: stronger repulsive
            if (i === j) {
                rules[i][j].attraction = -Math.abs(Math.random() * 0.2 + 0.1); // -0.1 to -0.3
                rules[i][j].minRadius = 5;
                rules[i][j].maxRadius = 30; // Tighter band for self-repulsion
            }
        }
    }
    return rules;
}

function flattenRules(
    rules: InteractionRule[][],
    numTypes: number
): Float32Array {
    const flatRules = new Float32Array(
        numTypes * numTypes * (RULE_SIZE_BYTES / 4)
    );
    let offset = 0;
    for (let i = 0; i < numTypes; i++) {
        for (let j = 0; j < numTypes; j++) {
            flatRules[offset++] = rules[i][j].attraction;
            flatRules[offset++] = rules[i][j].minRadius;
            flatRules[offset++] = rules[i][j].maxRadius;
            flatRules[offset++] = 0; // Padding
        }
    }
    return flatRules;
}

function createInitialParticles(
    numParticles: number,
    numTypes: number,
    worldWidth: number,
    worldHeight: number
): ArrayBuffer {
    // Using ArrayBuffer which will be viewed as Float32Array and Uint32Array
    const particleData = new ArrayBuffer(numParticles * PARTICLE_SIZE_BYTES);
    const particleViewF32 = new Float32Array(particleData);
    const particleViewU32 = new Uint32Array(particleData);

    // Initial particle positions are within the VIRTUAL world dimensions
    const initialSpawnWidth = worldWidth; // virtualWorldWidth
    const initialSpawnHeight = worldHeight; // virtualWorldHeight

    for (let i = 0; i < numParticles; i++) {
        const bufferOffsetF32 = i * (PARTICLE_SIZE_BYTES / 4);
        const bufferOffsetU32 = bufferOffsetF32;

        // Position (vec2f) - within virtual world dimensions
        particleViewF32[bufferOffsetF32 + 0] =
            Math.random() * initialSpawnWidth;
        particleViewF32[bufferOffsetF32 + 1] =
            Math.random() * initialSpawnHeight;
        // Velocity (vec2f)
        particleViewF32[bufferOffsetF32 + 2] = (Math.random() - 0.5) * 2.0;
        particleViewF32[bufferOffsetF32 + 3] = (Math.random() - 0.5) * 2.0;
        // Type (u32) - Stored after the 4 floats of pos and vel
        const particleType = Math.floor(Math.random() * numTypes);
        particleViewU32[bufferOffsetU32 + 4] = particleType;

        // Size (f32) - Stored after type -- REVERTED
        // const sizeRange = typeSizeRanges[particleType];
        // particleViewF32[bufferOffsetF32 + 5] =
        //     Math.random() * (sizeRange.max - sizeRange.min) + sizeRange.min;
        // particleViewU32[bufferOffsetU32 + 5] is padding if PARTICLE_SIZE_BYTES is 24
    }
    return particleData;
}

// Function to update background color based on drift speed and update GPU buffer
function updateBackgroundColorAndDrift(newDriftXPerSecond: number): void {
    simParams.driftXPerSecond = newDriftXPerSecond;

    const normalizedAbsDrift = Math.min(
        1,
        Math.abs(newDriftXPerSecond) / 80.0 // Updated to match new drift range of ±80 px/s
    );

    // Use HSLuv for background color generation based on drift speed
    // Hue transitions from blue (240°) at no drift to red (0°) at max drift
    const hue = 215 - normalizedAbsDrift * 200; // 240° to 0° (blue 240 to red 10)
    const saturation = 44;
    const lightness = 66;

    // Convert HSLuv to RGB
    const [red, green, blue] = hsluvToRgb([hue, saturation, lightness]);

    simParams.backgroundColor = [red, green, blue];

    if (device && simParamsBuffer) {
        // Update driftXPerSecond (float at index 15)
        device.queue.writeBuffer(
            simParamsBuffer,
            15 * 4, // Byte offset for driftXPerSecond
            new Float32Array([simParams.driftXPerSecond])
        );
        // Update backgroundColor (vec3<f32> starting at float index 20)
        device.queue.writeBuffer(
            simParamsBuffer,
            20 * 4, // Byte offset for backgroundColor
            new Float32Array(simParams.backgroundColor)
        );
    }
}

async function initWebGPU() {
    if (!navigator.gpu) {
        throw new Error("WebGPU not supported on this browser.");
    }
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) {
        throw new Error("No appropriate GPUAdapter found.");
    }
    device = await adapter.requestDevice();
    // @ts-ignore
    window.__webgpuDevice = device; // Store for potential cleanup

    context = canvas.getContext("webgpu") as GPUCanvasContext;
    presentationFormat = navigator.gpu.getPreferredCanvasFormat();
    context.configure({
        device: device,
        format: presentationFormat,
        // alphaMode: "premultiplied", // Changed for multi-pass
        alphaMode: "opaque",
    });

    // Initialize fpsDisplayElement (ensure "fpsDisplay" ID exists in HTML)
    fpsDisplayElement = document.getElementById("fpsDisplay");

    // Update simParams with calculated dimensions
    // Canvas size for display (800x800px) but render at higher resolution (2400x2400px)
    simParams.canvasRenderWidth = 2400; // Render resolution, not canvas display size
    simParams.canvasRenderHeight = 2400; // Render resolution, not canvas display size
    simParams.virtualWorldWidth =
        simParams.canvasRenderWidth + 2 * VIRTUAL_WORLD_BORDER;
    simParams.virtualWorldHeight =
        simParams.canvasRenderHeight + 2 * VIRTUAL_WORLD_BORDER;
    simParams.virtualWorldOffsetX = VIRTUAL_WORLD_BORDER;
    simParams.virtualWorldOffsetY = VIRTUAL_WORLD_BORDER;
    // simParams.time is initialized to 0.0 and updated in frame()
    // simParams.driftXPerSecond is initialized from DRIFT_X_PER_SECOND

    // Create Simulation Parameters Buffer Data (96 bytes / 24 floats)
    // This ArrayBuffer will be used to initially populate simParamsBuffer.
    const simParamsData = new ArrayBuffer(SIM_PARAMS_SIZE_BYTES);
    const simParamsViewF32 = new Float32Array(simParamsData);
    const simParamsViewU32 = new Uint32Array(simParamsData);

    // Populate simParamsData from the simParams object
    // Order matches SimParams struct in WGSL
    simParamsViewF32[0] = simParams.deltaTime;
    simParamsViewF32[1] = simParams.friction;
    simParamsViewU32[2] = simParams.numParticles;
    simParamsViewU32[3] = simParams.numTypes;
    simParamsViewF32[4] = simParams.virtualWorldWidth;
    simParamsViewF32[5] = simParams.virtualWorldHeight;
    simParamsViewF32[6] = simParams.canvasRenderWidth;
    simParamsViewF32[7] = simParams.canvasRenderHeight;
    simParamsViewF32[8] = simParams.virtualWorldOffsetX;
    simParamsViewF32[9] = simParams.virtualWorldOffsetY;
    simParamsViewU32[10] = simParams.boundaryMode;
    simParamsViewF32[11] = simParams.particleRenderSize;
    simParamsViewF32[12] = simParams.forceScale;
    simParamsViewF32[13] = simParams.rSmooth;
    simParamsViewU32[14] = simParams.flatForce ? 1 : 0; // Boolean to 0/1
    simParamsViewF32[15] = simParams.driftXPerSecond;
    simParamsViewF32[16] = simParams.interTypeAttractionScale;
    simParamsViewF32[17] = simParams.interTypeRadiusScale;
    simParamsViewF32[18] = simParams.time; // Initial time
    simParamsViewF32[19] = simParams.fisheyeStrength; // Fisheye distortion strength
    simParamsViewF32[20] = simParams.backgroundColor[0]; // R
    simParamsViewF32[21] = simParams.backgroundColor[1]; // G
    simParamsViewF32[22] = simParams.backgroundColor[2]; // B

    // Lenia parameters (starting at index 23)
    simParamsViewU32[23] = simParams.leniaEnabled ? 1 : 0; // Boolean to 0/1
    simParamsViewF32[24] = simParams.leniaGrowthMu; // μ parameter
    simParamsViewF32[25] = simParams.leniaGrowthSigma; // σ parameter
    simParamsViewF32[26] = simParams.leniaKernelRadius; // Kernel radius
    simParamsViewF32[27] = 0.0; // _padding1

    simParamsBuffer = device.createBuffer({
        label: "Simulation Parameters Buffer",
        size: SIM_PARAMS_SIZE_BYTES, // Should be 112 bytes (28 floats)
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    device.queue.writeBuffer(simParamsBuffer, 0, simParamsData);

    // Now that simParamsBuffer is created and initially populated,
    // call updateBackgroundColorAndDrift to set the initial background color based on the initial drift.
    // This will also write the initial drift and backgroundColor to the GPU buffer.
    updateBackgroundColorAndDrift(simParams.driftXPerSecond);

    // Create Scene Texture and Sampler for multi-pass rendering
    sceneTexture = device.createTexture({
        size: [simParams.virtualWorldWidth, simParams.virtualWorldHeight],
        format: presentationFormat,
        usage:
            GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.RENDER_ATTACHMENT,
    });

    // Create Intermediate Texture for fisheye post-processing
    intermediateTexture = device.createTexture({
        size: [simParams.virtualWorldWidth, simParams.virtualWorldHeight],
        format: presentationFormat,
        usage:
            GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.RENDER_ATTACHMENT,
    });

    sceneSampler = device.createSampler({
        magFilter: "linear",
        minFilter: "linear",
    });

    // Create Interaction Rules
    const rulesData = createRandomRules(NUM_TYPES);
    const flatRulesData = flattenRules(rulesData, NUM_TYPES);
    rulesBuffer = device.createBuffer({
        label: "Interaction Rules Buffer",
        size: flatRulesData.byteLength,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        mappedAtCreation: true,
    });
    new Float32Array(rulesBuffer.getMappedRange()).set(flatRulesData);
    rulesBuffer.unmap();

    // Create Particle Colors Buffer for custom colors
    const particleColors = generateParticleColors(NUM_TYPES);
    particleColorsBuffer = device.createBuffer({
        label: "Particle Colors Buffer",
        size: particleColors.byteLength,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        mappedAtCreation: true,
    });
    new Float32Array(particleColorsBuffer.getMappedRange()).set(particleColors);
    particleColorsBuffer.unmap();

    // Debug: Log the custom colors
    logParticleColors(NUM_TYPES);

    // Create Particle Buffers
    const initialParticleData = createInitialParticles(
        NUM_PARTICLES,
        NUM_TYPES,
        simParams.virtualWorldWidth, // Pass virtual dimensions for initial spawn
        simParams.virtualWorldHeight
    );
    particleBuffers = [
        device.createBuffer({
            label: "Particle Buffer A",
            size: initialParticleData.byteLength,
            usage:
                GPUBufferUsage.STORAGE |
                GPUBufferUsage.VERTEX |
                GPUBufferUsage.COPY_DST,
            mappedAtCreation: true,
        }),
        device.createBuffer({
            label: "Particle Buffer B",
            size: initialParticleData.byteLength,
            usage:
                GPUBufferUsage.STORAGE |
                GPUBufferUsage.VERTEX |
                GPUBufferUsage.COPY_DST,
        }),
    ];
    new Uint8Array(particleBuffers[0].getMappedRange()).set(
        new Uint8Array(initialParticleData)
    );
    particleBuffers[0].unmap();
    // Copy initial data to buffer B as well, or ensure compute shader writes to B first if A is input.
    // For simplicity, the compute shader will read from one and write to the other.

    // --- Compute Pipeline ---
    const computeShaderModule = device.createShaderModule({
        code: computeWGSL,
    });
    computePipeline = device.createComputePipeline({
        label: "Particle Life Compute Pipeline",
        layout: "auto",
        compute: {
            module: computeShaderModule,
            entryPoint: "main",
        },
    });

    computeBindGroups = [
        device.createBindGroup({
            label: "Compute Bind Group A (In: A, Out: B)",
            layout: computePipeline.getBindGroupLayout(0),
            entries: [
                { binding: 0, resource: { buffer: particleBuffers[0] } }, // particles_in
                { binding: 1, resource: { buffer: rulesBuffer } },
                { binding: 2, resource: { buffer: simParamsBuffer } },
                { binding: 3, resource: { buffer: particleBuffers[1] } }, // particles_out
            ],
        }),
        device.createBindGroup({
            label: "Compute Bind Group B (In: B, Out: A)",
            layout: computePipeline.getBindGroupLayout(0),
            entries: [
                { binding: 0, resource: { buffer: particleBuffers[1] } }, // particles_in
                { binding: 1, resource: { buffer: rulesBuffer } },
                { binding: 2, resource: { buffer: simParamsBuffer } },
                { binding: 3, resource: { buffer: particleBuffers[0] } }, // particles_out
            ],
        }),
    ];

    // For now, let's assume particles are rendered as small quads or points.
    // We'll need a vertex buffer for a unit quad/point.
    // const unitQuad = new Float32Array([ // This was an example, not used
    //     // x, y, u, v (example for textured quad, adapt for simple colored quad)
    //     -0.5, -0.5, 0, 0, 0.5, -0.5, 1, 0, -0.5, 0.5, 0, 1, 0.5, 0.5, 1, 1,
    // ]);
    // A simpler quad for instancing, just positions
    const particleQuadVertices = new Float32Array([
        -1.0,
        -1.0, // bottom-left
        1.0,
        -1.0, // bottom-right
        -1.0,
        1.0, // top-left
        1.0,
        1.0, // top-right
    ]);

    quadVertexBuffer = device.createBuffer({
        label: "Particle Quad Vertex Buffer",
        size: particleQuadVertices.byteLength,
        usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
        mappedAtCreation: true,
    });
    new Float32Array(quadVertexBuffer.getMappedRange()).set(
        particleQuadVertices
    );
    quadVertexBuffer.unmap();

    const renderShaderModuleVert = device.createShaderModule({
        code: vertWGSL,
    });
    const renderShaderModuleFrag = device.createShaderModule({
        code: fragWGSL,
    });

    // --- Background Render Pipeline ---
    const backgroundShaderModuleVert = device.createShaderModule({
        code: backgroundVertWGSL,
    });
    const backgroundShaderModuleFrag = device.createShaderModule({
        code: backgroundFragWGSL,
    });

    // The background pipeline will also use sim_params
    // We need to ensure the layout for sim_params (group 0, binding 0) is defined before renderPipeline tries to use "auto"
    // or is compatible. Let's define a common bind group layout for sim_params.

    const simParamsBindGroupLayout = device.createBindGroupLayout({
        entries: [
            {
                binding: 0,
                visibility:
                    GPUShaderStage.VERTEX |
                    GPUShaderStage.FRAGMENT |
                    GPUShaderStage.COMPUTE,
                buffer: { type: "uniform" },
            },
        ],
    });

    const backgroundPipelineLayout = device.createPipelineLayout({
        bindGroupLayouts: [simParamsBindGroupLayout],
    });

    backgroundRenderPipeline = device.createRenderPipeline({
        label: "Background Render Pipeline",
        layout: backgroundPipelineLayout,
        vertex: {
            module: backgroundShaderModuleVert,
            entryPoint: "main",
        },
        fragment: {
            module: backgroundShaderModuleFrag,
            entryPoint: "main",
            targets: [{ format: presentationFormat }],
        },
        primitive: {
            topology: "triangle-list", // background_vert.wgsl outputs a triangle list (full screen quad)
        },
    });

    backgroundRenderBindGroup = device.createBindGroup({
        label: "Background Render Bind Group",
        layout: simParamsBindGroupLayout, // Use the common layout
        entries: [{ binding: 0, resource: { buffer: simParamsBuffer } }],
    });

    // --- Render Pipeline (Particles) ---
    // Create a new bind group layout for particle rendering that includes colors buffer
    const particleRenderBindGroupLayout = device.createBindGroupLayout({
        entries: [
            {
                binding: 0,
                visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
                buffer: { type: "uniform" },
            },
            {
                binding: 1,
                visibility: GPUShaderStage.VERTEX,
                buffer: { type: "read-only-storage" },
            },
        ],
    });

    renderPipeline = device.createRenderPipeline({
        label: "Particle Render Pipeline",
        layout: device.createPipelineLayout({
            bindGroupLayouts: [particleRenderBindGroupLayout],
        }), // Use the common layout
        vertex: {
            module: renderShaderModuleVert,
            entryPoint: "main",
            buffers: [
                {
                    // Per-instance particle data (position, type)
                    arrayStride: PARTICLE_SIZE_BYTES,
                    stepMode: "instance",
                    attributes: [
                        { shaderLocation: 0, offset: 0, format: "float32x2" }, // Particle position (offset 0)
                        { shaderLocation: 1, offset: 8, format: "float32x2" }, // Particle velocity (offset 8)
                        { shaderLocation: 2, offset: 16, format: "uint32" }, // Particle type (offset 16)
                    ],
                },
                {
                    // Per-vertex data for the quad
                    arrayStride: 2 * Float32Array.BYTES_PER_ELEMENT, // vec2f
                    stepMode: "vertex",
                    attributes: [
                        // Location 3 for quad_pos in vert.wgsl
                        { shaderLocation: 3, offset: 0, format: "float32x2" },
                    ],
                },
            ],
        },
        fragment: {
            module: renderShaderModuleFrag,
            entryPoint: "main",
            targets: [
                {
                    format: presentationFormat,
                    blend: {
                        color: {
                            srcFactor: "src-alpha",
                            dstFactor: "one-minus-src-alpha",
                            operation: "add",
                        },
                        alpha: {
                            srcFactor: "one",
                            dstFactor: "one-minus-src-alpha",
                            operation: "add",
                        },
                    },
                },
            ],
        },
        primitive: {
            topology: "triangle-strip",
            stripIndexFormat: undefined,
        },
    });

    renderBindGroup = device.createBindGroup({
        label: "Render BindGroup",
        layout: particleRenderBindGroupLayout,
        entries: [
            { binding: 0, resource: { buffer: simParamsBuffer } },
            { binding: 1, resource: { buffer: particleColorsBuffer } },
        ],
    });

    // --- Vignette Render Pipeline ---
    const vignetteShaderModuleFrag = device.createShaderModule({
        code: vignetteFragWGSL,
    });

    const vignetteBindGroupLayout = device.createBindGroupLayout({
        label: "Vignette BindGroupLayout",
        entries: [
            {
                // sim_params
                binding: 0,
                visibility: GPUShaderStage.FRAGMENT,
                buffer: { type: "uniform" },
            },
            {
                // scene_sampler
                binding: 1,
                visibility: GPUShaderStage.FRAGMENT,
                sampler: { type: "filtering" },
            },
            {
                // scene_texture
                binding: 2,
                visibility: GPUShaderStage.FRAGMENT,
                texture: { sampleType: "float" },
            },
        ],
    });

    const vignettePipelineLayout = device.createPipelineLayout({
        label: "Vignette Pipeline Layout",
        bindGroupLayouts: [vignetteBindGroupLayout],
    });

    vignetteRenderPipeline = device.createRenderPipeline({
        label: "Vignette Render Pipeline",
        layout: vignettePipelineLayout,
        vertex: {
            // Reuse the background's full-screen quad vertex shader
            module: backgroundShaderModuleVert,
            entryPoint: "main",
        },
        fragment: {
            module: vignetteShaderModuleFrag,
            entryPoint: "main",
            targets: [{ format: presentationFormat }], // Output to the canvas
        },
        primitive: {
            topology: "triangle-list",
        },
    });

    vignetteRenderBindGroup = device.createBindGroup({
        label: "Vignette Render Bind Group",
        layout: vignetteBindGroupLayout,
        entries: [
            { binding: 0, resource: { buffer: simParamsBuffer } },
            { binding: 1, resource: sceneSampler },
            { binding: 2, resource: intermediateTexture.createView() }, // Now reads from intermediate texture
        ],
    });

    // --- Fisheye Render Pipeline ---
    const fisheyeShaderModuleFrag = device.createShaderModule({
        code: fisheyeFragWGSL,
    });

    // Fisheye uses the same bind group layout as vignette (sim_params, sampler, texture)
    const fisheyeBindGroupLayout = device.createBindGroupLayout({
        label: "Fisheye BindGroupLayout",
        entries: [
            {
                // sim_params
                binding: 0,
                visibility: GPUShaderStage.FRAGMENT,
                buffer: { type: "uniform" },
            },
            {
                // scene_sampler
                binding: 1,
                visibility: GPUShaderStage.FRAGMENT,
                sampler: { type: "filtering" },
            },
            {
                // scene_texture
                binding: 2,
                visibility: GPUShaderStage.FRAGMENT,
                texture: { sampleType: "float" },
            },
        ],
    });

    const fisheyePipelineLayout = device.createPipelineLayout({
        label: "Fisheye Pipeline Layout",
        bindGroupLayouts: [fisheyeBindGroupLayout],
    });

    fisheyeRenderPipeline = device.createRenderPipeline({
        label: "Fisheye Render Pipeline",
        layout: fisheyePipelineLayout,
        vertex: {
            // Reuse the background's full-screen quad vertex shader
            module: backgroundShaderModuleVert,
            entryPoint: "main",
        },
        fragment: {
            module: fisheyeShaderModuleFrag,
            entryPoint: "main",
            targets: [{ format: presentationFormat }], // Output to intermediate texture
        },
        primitive: {
            topology: "triangle-list",
        },
    });

    fisheyeRenderBindGroup = device.createBindGroup({
        label: "Fisheye Render Bind Group",
        layout: fisheyeBindGroupLayout,
        entries: [
            { binding: 0, resource: { buffer: simParamsBuffer } },
            { binding: 1, resource: sceneSampler },
            { binding: 2, resource: sceneTexture.createView() },
        ],
    });

    // --- Oscilloscope Grid Render Pipeline ---
    const gridShaderModuleFrag = device.createShaderModule({
        code: gridFragWGSL,
    });

    // The grid pipeline will also use sim_params, so we can reuse simParamsBindGroupLayout
    const gridPipelineLayout = device.createPipelineLayout({
        bindGroupLayouts: [simParamsBindGroupLayout], // Reuse common layout for sim_params
    });

    gridRenderPipeline = device.createRenderPipeline({
        label: "Grid Render Pipeline",
        layout: gridPipelineLayout,
        vertex: {
            // Reuse the background's full-screen quad vertex shader
            module: backgroundShaderModuleVert,
            entryPoint: "main",
        },
        fragment: {
            module: gridShaderModuleFrag,
            entryPoint: "main",
            targets: [
                {
                    format: presentationFormat,
                    blend: {
                        // Enable alpha blending for the grid
                        color: {
                            srcFactor: "src-alpha",
                            dstFactor: "one-minus-src-alpha",
                            operation: "add",
                        },
                        alpha: {
                            srcFactor: "one", // Or "src-alpha" if grid alpha needs to blend
                            dstFactor: "one-minus-src-alpha",
                            operation: "add",
                        },
                    },
                },
            ],
        },
        primitive: {
            topology: "triangle-list",
        },
    });

    gridRenderBindGroup = device.createBindGroup({
        label: "Grid Render Bind Group",
        layout: simParamsBindGroupLayout, // Use the common layout for sim_params
        entries: [{ binding: 0, resource: { buffer: simParamsBuffer } }],
    });

    // --- Zoom Pipeline Setup ---
    // Create zoom uniforms buffer
    zoomUniformsBuffer = device.createBuffer({
        label: "Zoom Uniforms Buffer",
        size: 16, // 4 floats: zoom_level, center_x, center_y, padding
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });

    // Write initial zoom uniforms (zoom level and center position)
    device.queue.writeBuffer(
        zoomUniformsBuffer,
        0,
        new Float32Array([currentZoomLevel, zoomCenterX, zoomCenterY, 0.0]) // zoom_level, center_x, center_y, padding
    );

    const zoomShaderModuleFrag = device.createShaderModule({
        code: zoomFragWGSL,
    });

    const zoomBindGroupLayout = device.createBindGroupLayout({
        label: "Zoom BindGroupLayout",
        entries: [
            {
                binding: 0,
                visibility: GPUShaderStage.FRAGMENT,
                sampler: { type: "filtering" },
            },
            {
                binding: 1,
                visibility: GPUShaderStage.FRAGMENT,
                texture: { sampleType: "float" },
            },
            {
                binding: 2,
                visibility: GPUShaderStage.FRAGMENT,
                buffer: { type: "uniform" },
            },
        ],
    });

    const zoomPipelineLayout = device.createPipelineLayout({
        label: "Zoom Pipeline Layout",
        bindGroupLayouts: [zoomBindGroupLayout],
    });

    zoomRenderPipeline = device.createRenderPipeline({
        label: "Zoom Render Pipeline",
        layout: zoomPipelineLayout,
        vertex: {
            module: backgroundShaderModuleVert, // Reuse full-screen quad vertex shader
            entryPoint: "main",
        },
        fragment: {
            module: zoomShaderModuleFrag,
            entryPoint: "main",
            targets: [{ format: presentationFormat }],
        },
        primitive: {
            topology: "triangle-list",
        },
    });

    zoomRenderBindGroup = device.createBindGroup({
        label: "Zoom Render Bind Group",
        layout: zoomBindGroupLayout,
        entries: [
            { binding: 0, resource: sceneSampler },
            { binding: 1, resource: intermediateTexture.createView() }, // Sample from the final 2400x2400 rendered result
            { binding: 2, resource: { buffer: zoomUniformsBuffer } },
        ],
    });

    // Start the animation loop once all setup is complete
    animationId = requestAnimationFrame(frame);
}

// TODO: Implement or connect the drift slider event listener.
// The event listener should call:
//   updateBackgroundColorAndDrift(newDriftValue);
// Example:
const driftSlider = document.getElementById("driftSlider") as HTMLInputElement;
const driftValueDisplay = document.getElementById("driftValue");
if (driftSlider && driftValueDisplay) {
    // Load saved value or use default
    const savedDrift = loadFromLocalStorage(
        STORAGE_KEYS.drift,
        simParams.driftXPerSecond
    );
    simParams.driftXPerSecond = savedDrift;
    updateBackgroundColorAndDrift(savedDrift);
    driftSlider.value = savedDrift.toString();
    driftValueDisplay.textContent = savedDrift.toFixed(2);

    driftSlider.addEventListener("input", (event) => {
        const newDrift = parseFloat((event.target as HTMLInputElement).value);
        updateBackgroundColorAndDrift(newDrift);
        driftValueDisplay.textContent = newDrift.toFixed(2);
        saveToLocalStorage(STORAGE_KEYS.drift, newDrift);
    });
}

// Force Scale Slider
const forceScaleSlider = document.getElementById(
    "forceScaleSlider"
) as HTMLInputElement;
const forceScaleValueDisplay = document.getElementById("forceScaleValue");
if (forceScaleSlider && forceScaleValueDisplay) {
    // Load saved value or use default
    simParams.forceScale = loadFromLocalStorage(
        STORAGE_KEYS.forceScale,
        simParams.forceScale
    );
    forceScaleSlider.value = simParams.forceScale.toString();
    forceScaleValueDisplay.textContent = simParams.forceScale.toFixed(2);
    forceScaleSlider.addEventListener("input", (event) => {
        const newValue = parseFloat((event.target as HTMLInputElement).value);
        simParams.forceScale = newValue;
        forceScaleValueDisplay.textContent = newValue.toFixed(2);
        saveToLocalStorage(STORAGE_KEYS.forceScale, newValue);
        if (device && simParamsBuffer) {
            device.queue.writeBuffer(
                simParamsBuffer,
                12 * 4, // Byte offset for forceScale (float at index 12)
                new Float32Array([simParams.forceScale])
            );
        }
    });
}

// Friction Slider
const frictionSlider = document.getElementById(
    "frictionSlider"
) as HTMLInputElement;
const frictionValueDisplay = document.getElementById("frictionValue");
if (frictionSlider && frictionValueDisplay) {
    // Load saved value or use default
    simParams.friction = loadFromLocalStorage(
        STORAGE_KEYS.friction,
        simParams.friction
    );
    frictionSlider.value = simParams.friction.toString();
    frictionValueDisplay.textContent = simParams.friction.toFixed(2);
    frictionSlider.addEventListener("input", (event) => {
        const newValue = parseFloat((event.target as HTMLInputElement).value);
        simParams.friction = newValue;
        frictionValueDisplay.textContent = newValue.toFixed(2);
        saveToLocalStorage(STORAGE_KEYS.friction, newValue);
        if (device && simParamsBuffer) {
            device.queue.writeBuffer(
                simParamsBuffer,
                1 * 4, // Byte offset for friction (float at index 1)
                new Float32Array([simParams.friction])
            );
        }
    });
}

// R Smooth Slider
const rSmoothSlider = document.getElementById(
    "rSmoothSlider"
) as HTMLInputElement;
const rSmoothValueDisplay = document.getElementById("rSmoothValue");
if (rSmoothSlider && rSmoothValueDisplay) {
    // Load saved value or use default
    simParams.rSmooth = loadFromLocalStorage(
        STORAGE_KEYS.rSmooth,
        simParams.rSmooth
    );
    rSmoothSlider.value = simParams.rSmooth.toString();
    rSmoothValueDisplay.textContent = simParams.rSmooth.toFixed(2);
    rSmoothSlider.addEventListener("input", (event) => {
        const newValue = parseFloat((event.target as HTMLInputElement).value);
        simParams.rSmooth = newValue;
        rSmoothValueDisplay.textContent = newValue.toFixed(2);
        saveToLocalStorage(STORAGE_KEYS.rSmooth, newValue);
        if (device && simParamsBuffer) {
            device.queue.writeBuffer(
                simParamsBuffer,
                13 * 4, // Byte offset for rSmooth (float at index 13)
                new Float32Array([simParams.rSmooth])
            );
        }
    });
}

// Inter-Type Attraction Scale Slider
const interTypeAttractionScaleSlider = document.getElementById(
    "interTypeAttractionScaleSlider"
) as HTMLInputElement;
const interTypeAttractionScaleValueDisplay = document.getElementById(
    "interTypeAttractionScaleValue"
);
if (interTypeAttractionScaleSlider && interTypeAttractionScaleValueDisplay) {
    // Load saved value or use default
    simParams.interTypeAttractionScale = loadFromLocalStorage(
        STORAGE_KEYS.interTypeAttractionScale,
        simParams.interTypeAttractionScale
    );
    interTypeAttractionScaleSlider.value =
        simParams.interTypeAttractionScale.toString();
    interTypeAttractionScaleValueDisplay.textContent =
        simParams.interTypeAttractionScale.toFixed(2);
    interTypeAttractionScaleSlider.addEventListener("input", (event) => {
        const newValue = parseFloat((event.target as HTMLInputElement).value);
        simParams.interTypeAttractionScale = newValue;
        interTypeAttractionScaleValueDisplay.textContent = newValue.toFixed(2);
        saveToLocalStorage(STORAGE_KEYS.interTypeAttractionScale, newValue);
        if (device && simParamsBuffer) {
            device.queue.writeBuffer(
                simParamsBuffer,
                16 * 4, // Byte offset for interTypeAttractionScale (float at index 16)
                new Float32Array([simParams.interTypeAttractionScale])
            );
        }
    });
}

// Inter-Type Radius Scale Slider
const interTypeRadiusScaleSlider = document.getElementById(
    "interTypeRadiusScaleSlider"
) as HTMLInputElement;
const interTypeRadiusScaleValueDisplay = document.getElementById(
    "interTypeRadiusScaleValue"
);
if (interTypeRadiusScaleSlider && interTypeRadiusScaleValueDisplay) {
    // Load saved value or use default
    simParams.interTypeRadiusScale = loadFromLocalStorage(
        STORAGE_KEYS.interTypeRadiusScale,
        simParams.interTypeRadiusScale
    );
    interTypeRadiusScaleSlider.value =
        simParams.interTypeRadiusScale.toString();
    interTypeRadiusScaleValueDisplay.textContent =
        simParams.interTypeRadiusScale.toFixed(2);
    interTypeRadiusScaleSlider.addEventListener("input", (event) => {
        const newValue = parseFloat((event.target as HTMLInputElement).value);
        simParams.interTypeRadiusScale = newValue;
        interTypeRadiusScaleValueDisplay.textContent = newValue.toFixed(2);
        saveToLocalStorage(STORAGE_KEYS.interTypeRadiusScale, newValue);
        if (device && simParamsBuffer) {
            device.queue.writeBuffer(
                simParamsBuffer,
                17 * 4, // Byte offset for interTypeRadiusScale (float at index 17)
                new Float32Array([simParams.interTypeRadiusScale])
            );
        }
    });
}

// Fisheye Strength Slider
const fisheyeStrengthSlider = document.getElementById(
    "fisheyeStrengthSlider"
) as HTMLInputElement;
const fisheyeStrengthValueDisplay = document.getElementById(
    "fisheyeStrengthValue"
);
if (fisheyeStrengthSlider && fisheyeStrengthValueDisplay) {
    // Load saved value or use default
    simParams.fisheyeStrength = loadFromLocalStorage(
        STORAGE_KEYS.fisheyeStrength,
        simParams.fisheyeStrength
    );
    fisheyeStrengthSlider.value = simParams.fisheyeStrength.toString();
    fisheyeStrengthValueDisplay.textContent =
        simParams.fisheyeStrength.toFixed(2);
    fisheyeStrengthSlider.addEventListener("input", (event) => {
        const newValue = parseFloat((event.target as HTMLInputElement).value);
        simParams.fisheyeStrength = newValue;
        fisheyeStrengthValueDisplay.textContent = newValue.toFixed(2);
        saveToLocalStorage(STORAGE_KEYS.fisheyeStrength, newValue);
        if (device && simParamsBuffer) {
            device.queue.writeBuffer(
                simParamsBuffer,
                19 * 4, // Byte offset for fisheyeStrength (float at index 19)
                new Float32Array([simParams.fisheyeStrength])
            );
        }
    });
}

// Zoom slider setup
const zoomSlider = document.getElementById("zoomSlider") as HTMLInputElement;
const zoomValueDisplay = document.getElementById("zoomValue");
if (zoomSlider && zoomValueDisplay) {
    // Load saved value or use default
    currentZoomLevel = loadFromLocalStorage(
        STORAGE_KEYS.zoom,
        currentZoomLevel
    );
    zoomSlider.value = currentZoomLevel.toString();
    zoomValueDisplay.textContent = currentZoomLevel.toFixed(1);
    zoomSlider.addEventListener("input", (event) => {
        const newValue = parseFloat((event.target as HTMLInputElement).value);
        currentZoomLevel = newValue;
        zoomValueDisplay.textContent = newValue.toFixed(1);
        saveToLocalStorage(STORAGE_KEYS.zoom, newValue);

        // Constrain zoom center based on new zoom level
        constrainZoomCenter();

        // Update zoom center info display
        const zoomCenterInfo = document.getElementById("zoomCenterInfo");
        if (zoomCenterInfo) {
            const maxMovementRange = Math.max(
                0,
                111.24 * currentZoomLevel - 122.29
            );
            zoomCenterInfo.innerHTML = `Center: (${zoomCenterX.toFixed(
                0
            )}, ${zoomCenterY.toFixed(0)})<br>Range: ${maxMovementRange.toFixed(
                0
            )}`;
        }

        if (device && zoomUniformsBuffer) {
            device.queue.writeBuffer(
                zoomUniformsBuffer,
                0, // Write all zoom uniforms
                new Float32Array([
                    currentZoomLevel,
                    zoomCenterX,
                    zoomCenterY,
                    0.0,
                ])
            );
        }
    });
}

// Lenia Controls
const leniaEnabledCheckbox = document.getElementById(
    "leniaEnabledCheckbox"
) as HTMLInputElement;
const leniaEnabledStatus = document.getElementById("leniaEnabledStatus");
if (leniaEnabledCheckbox && leniaEnabledStatus) {
    leniaEnabledCheckbox.checked = simParams.leniaEnabled;
    leniaEnabledStatus.textContent = simParams.leniaEnabled ? "On" : "Off";
    leniaEnabledCheckbox.addEventListener("change", (event) => {
        const newValue = (event.target as HTMLInputElement).checked;
        simParams.leniaEnabled = newValue;
        leniaEnabledStatus.textContent = newValue ? "On" : "Off";
        if (device && simParamsBuffer) {
            device.queue.writeBuffer(
                simParamsBuffer,
                23 * 4, // Byte offset for leniaEnabled (u32 at index 23)
                new Uint32Array([newValue ? 1 : 0])
            );
        }
    });
}

const leniaGrowthMuSlider = document.getElementById(
    "leniaGrowthMuSlider"
) as HTMLInputElement;
const leniaGrowthMuValue = document.getElementById("leniaGrowthMuValue");
if (leniaGrowthMuSlider && leniaGrowthMuValue) {
    leniaGrowthMuSlider.value = simParams.leniaGrowthMu.toString();
    leniaGrowthMuValue.textContent = simParams.leniaGrowthMu.toFixed(3);
    leniaGrowthMuSlider.addEventListener("input", (event) => {
        const newValue = parseFloat((event.target as HTMLInputElement).value);
        simParams.leniaGrowthMu = newValue;
        leniaGrowthMuValue.textContent = newValue.toFixed(3);
        if (device && simParamsBuffer) {
            device.queue.writeBuffer(
                simParamsBuffer,
                24 * 4, // Byte offset for leniaGrowthMu (f32 at index 24)
                new Float32Array([simParams.leniaGrowthMu])
            );
        }
    });
}

const leniaGrowthSigmaSlider = document.getElementById(
    "leniaGrowthSigmaSlider"
) as HTMLInputElement;
const leniaGrowthSigmaValue = document.getElementById("leniaGrowthSigmaValue");
if (leniaGrowthSigmaSlider && leniaGrowthSigmaValue) {
    leniaGrowthSigmaSlider.value = simParams.leniaGrowthSigma.toString();
    leniaGrowthSigmaValue.textContent = simParams.leniaGrowthSigma.toFixed(3);
    leniaGrowthSigmaSlider.addEventListener("input", (event) => {
        const newValue = parseFloat((event.target as HTMLInputElement).value);
        simParams.leniaGrowthSigma = newValue;
        leniaGrowthSigmaValue.textContent = newValue.toFixed(3);
        if (device && simParamsBuffer) {
            device.queue.writeBuffer(
                simParamsBuffer,
                25 * 4, // Byte offset for leniaGrowthSigma (f32 at index 25)
                new Float32Array([simParams.leniaGrowthSigma])
            );
        }
    });
}

const leniaKernelRadiusSlider = document.getElementById(
    "leniaKernelRadiusSlider"
) as HTMLInputElement;
const leniaKernelRadiusValue = document.getElementById(
    "leniaKernelRadiusValue"
);
if (leniaKernelRadiusSlider && leniaKernelRadiusValue) {
    leniaKernelRadiusSlider.value = simParams.leniaKernelRadius.toString();
    leniaKernelRadiusValue.textContent = simParams.leniaKernelRadius.toFixed(1);
    leniaKernelRadiusSlider.addEventListener("input", (event) => {
        const newValue = parseFloat((event.target as HTMLInputElement).value);
        simParams.leniaKernelRadius = newValue;
        leniaKernelRadiusValue.textContent = newValue.toFixed(1);
        if (device && simParamsBuffer) {
            device.queue.writeBuffer(
                simParamsBuffer,
                26 * 4, // Byte offset for leniaKernelRadius (f32 at index 26)
                new Float32Array([simParams.leniaKernelRadius])
            );
        }
    });
}

// Environmental sliders (Temperature, Electrical Activity, UV Light, Pressure)
const tempSlider = document.getElementById("tempSlider") as HTMLInputElement;
const tempValueDisplay = document.getElementById("tempValue");
if (tempSlider && tempValueDisplay) {
    // Load saved value or use default
    temperature = loadFromLocalStorage(STORAGE_KEYS.temperature, temperature);
    tempSlider.value = temperature.toString();
    tempValueDisplay.textContent = temperature.toString();

    // Apply initial temperature-based parameters on page load
    updateDriftAndFrictionFromTemperature(temperature);

    tempSlider.addEventListener("input", (event) => {
        const newValue = parseFloat((event.target as HTMLInputElement).value);
        temperature = newValue;
        tempValueDisplay.textContent = newValue.toString();
        saveToLocalStorage(STORAGE_KEYS.temperature, newValue);

        // Update drift and friction parameters based on temperature
        updateDriftAndFrictionFromTemperature(newValue);
    });
}

const elecSlider = document.getElementById("elecSlider") as HTMLInputElement;
const elecValueDisplay = document.getElementById("elecValue");
if (elecSlider && elecValueDisplay) {
    // Load saved value or use default
    electricalActivity = loadFromLocalStorage(
        STORAGE_KEYS.electricalActivity,
        electricalActivity
    );
    elecSlider.value = electricalActivity.toString();
    elecValueDisplay.textContent = electricalActivity.toFixed(2);

    elecSlider.addEventListener("input", (event) => {
        const newValue = parseFloat((event.target as HTMLInputElement).value);
        electricalActivity = newValue;
        elecValueDisplay.textContent = newValue.toFixed(2);
        saveToLocalStorage(STORAGE_KEYS.electricalActivity, newValue);
    });
}

const uvSlider = document.getElementById("uvSlider") as HTMLInputElement;
const uvValueDisplay = document.getElementById("uvValue");
if (uvSlider && uvValueDisplay) {
    // Load saved value or use default
    uvLight = loadFromLocalStorage(STORAGE_KEYS.uvLight, uvLight);
    uvSlider.value = uvLight.toString();
    uvValueDisplay.textContent = uvLight.toString();

    uvSlider.addEventListener("input", (event) => {
        const newValue = parseFloat((event.target as HTMLInputElement).value);
        uvLight = newValue;
        uvValueDisplay.textContent = newValue.toString();
        saveToLocalStorage(STORAGE_KEYS.uvLight, newValue);
    });
}

const presSlider = document.getElementById("presSlider") as HTMLInputElement;
const presValueDisplay = document.getElementById("presValue");
if (presSlider && presValueDisplay) {
    // Load saved value or use default
    pressure = loadFromLocalStorage(STORAGE_KEYS.pressure, pressure);
    presSlider.value = pressure.toString();
    presValueDisplay.textContent = pressure.toString();

    // Apply initial pressure-based parameters on page load
    updateParametersFromPressure(pressure);

    presSlider.addEventListener("input", (event) => {
        const newValue = parseFloat((event.target as HTMLInputElement).value);
        pressure = newValue;
        presValueDisplay.textContent = newValue.toString();
        saveToLocalStorage(STORAGE_KEYS.pressure, newValue);

        // Update physics parameters based on pressure
        updateParametersFromPressure(newValue);
    });
}

// --- Main Animation Loop ---
function frame(timestamp?: number) {
    // timestamp from requestAnimationFrame is not used here.
    const now = performance.now();
    let dt = (now - lastFrameTime) / 1000; // deltaTime in seconds
    lastFrameTime = now;

    // Clamp deltaTime to avoid large jumps (e.g., if tab was inactive)
    // Also handle first frame where lastFrameTime is 0.
    if (frameCount === 0) {
        dt = 1 / 60; // Assume 60 FPS for the first frame's deltaTime
    }
    dt = Math.min(dt, 0.1); // Max deltaTime of 100ms

    simParams.deltaTime = dt;
    currentTime += dt; // Accumulate total time
    simParams.time = currentTime;

    // Update dynamic simParams in the GPU buffer
    // deltaTime (float at index 0)
    device.queue.writeBuffer(
        simParamsBuffer,
        0,
        new Float32Array([simParams.deltaTime])
    );
    // time (float at index 18)
    device.queue.writeBuffer(
        simParamsBuffer,
        18 * 4,
        new Float32Array([simParams.time])
    );

    const commandEncoder = device.createCommandEncoder({
        label: "Particle Life Frame Encoder",
    });

    // Compute Pass
    const computePass = commandEncoder.beginComputePass({
        label: "Particle Compute Pass",
    });
    computePass.setPipeline(computePipeline);
    computePass.setBindGroup(0, computeBindGroups[currentParticleBufferIndex]);
    // Dispatch based on number of particles. Workgroup size is 64 in compute.wgsl
    const numWorkgroups = Math.ceil(NUM_PARTICLES / 64);
    computePass.dispatchWorkgroups(numWorkgroups);
    computePass.end();

    // Render Pass (will be multi-pass now)
    // const textureView = context.getCurrentTexture().createView(); // This is for the final pass to canvas

    // --- Render Passes ---
    const sceneTextureView = sceneTexture.createView();

    // 1. Background Render Pass (to sceneTexture)
    const backgroundPassDescriptor: GPURenderPassDescriptor = {
        label: "Background Render Pass",
        colorAttachments: [
            {
                view: sceneTextureView, // Render to sceneTexture
                loadOp: "clear",
                // Clear with the dynamic background color
                clearValue: {
                    r: simParams.backgroundColor[0],
                    g: simParams.backgroundColor[1],
                    b: simParams.backgroundColor[2],
                    a: 1.0,
                },
                storeOp: "store",
            },
        ],
    };
    const backgroundPass = commandEncoder.beginRenderPass(
        backgroundPassDescriptor
    );
    // Set viewport to render only to the canvas portion of the virtual world texture
    backgroundPass.setViewport(
        simParams.virtualWorldOffsetX, // x offset (50px)
        simParams.virtualWorldOffsetY, // y offset (50px)
        simParams.canvasRenderWidth, // width (2400px)
        simParams.canvasRenderHeight, // height (2400px)
        0.0, // minDepth
        1.0 // maxDepth
    );
    backgroundPass.setPipeline(backgroundRenderPipeline);
    backgroundPass.setBindGroup(0, backgroundRenderBindGroup);
    backgroundPass.draw(6, 1, 0, 0);
    backgroundPass.end();

    // 2. Particle Render Pass (to sceneTexture, on top of background)
    const particleRenderPassDescriptor: GPURenderPassDescriptor = {
        label: "Particle Render Pass",
        colorAttachments: [
            {
                view: sceneTextureView, // Render to sceneTexture
                loadOp: "load", // Load the background drawn in the previous pass
                storeOp: "store",
            },
        ],
    };
    const particlePass = commandEncoder.beginRenderPass(
        particleRenderPassDescriptor
    );
    // Set viewport to render only to the canvas portion of the virtual world texture
    particlePass.setViewport(
        simParams.virtualWorldOffsetX, // x offset (50px)
        simParams.virtualWorldOffsetY, // y offset (50px)
        simParams.canvasRenderWidth, // width (2400px)
        simParams.canvasRenderHeight, // height (2400px)
        0.0, // minDepth
        1.0 // maxDepth
    );
    particlePass.setPipeline(renderPipeline);
    particlePass.setVertexBuffer(
        0,
        particleBuffers[currentParticleBufferIndex]
    );
    particlePass.setVertexBuffer(1, quadVertexBuffer);
    particlePass.setBindGroup(0, renderBindGroup);
    particlePass.draw(4, NUM_PARTICLES, 0, 0);
    particlePass.end();

    // 3. Grid Render Pass (to sceneTexture, on top of particles)
    const gridPassDescriptor: GPURenderPassDescriptor = {
        label: "Grid Pass to Scene Texture",
        colorAttachments: [
            {
                view: sceneTextureView, // Render to sceneTexture (not canvas)
                loadOp: "load", // Load the background and particles drawn in previous passes
                storeOp: "store",
            },
        ],
    };
    const gridPass = commandEncoder.beginRenderPass(gridPassDescriptor);
    // Set viewport to render only to the canvas portion of the virtual world texture
    gridPass.setViewport(
        simParams.virtualWorldOffsetX, // x offset (50px)
        simParams.virtualWorldOffsetY, // y offset (50px)
        simParams.canvasRenderWidth, // width (2400px)
        simParams.canvasRenderHeight, // height (2400px)
        0.0, // minDepth
        1.0 // maxDepth
    );
    gridPass.setPipeline(gridRenderPipeline);
    gridPass.setBindGroup(0, gridRenderBindGroup);
    gridPass.draw(6, 1, 0, 0); // Draw a full-screen quad
    gridPass.end();

    // 4. Fisheye Post-Processing Pass (from sceneTexture with grid to intermediateTexture)
    const intermediateTextureView = intermediateTexture.createView();
    const fisheyePassDescriptor: GPURenderPassDescriptor = {
        label: "Fisheye Pass to Intermediate Texture",
        colorAttachments: [
            {
                view: intermediateTextureView, // Render to intermediateTexture
                loadOp: "clear",
                clearValue: { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
                storeOp: "store",
            },
        ],
    };
    const fisheyePass = commandEncoder.beginRenderPass(fisheyePassDescriptor);
    fisheyePass.setPipeline(fisheyeRenderPipeline);
    fisheyePass.setBindGroup(0, fisheyeRenderBindGroup); // This uses sceneTexture (now with grid)
    fisheyePass.draw(6, 1, 0, 0); // Draw a full-screen quad
    fisheyePass.end();

    // 5. Zoom Post-Processing Pass (from intermediateTexture to canvas)
    const canvasTextureView = context.getCurrentTexture().createView();
    const zoomPassDescriptor: GPURenderPassDescriptor = {
        label: "Zoom Pass to Canvas",
        colorAttachments: [
            {
                view: canvasTextureView, // Render to the actual canvas (800x800px)
                loadOp: "clear", // Clear canvas before drawing zoomed scene
                clearValue: { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }, // Clear to black
                storeOp: "store",
            },
        ],
    };
    const zoomPass = commandEncoder.beginRenderPass(zoomPassDescriptor);
    zoomPass.setPipeline(zoomRenderPipeline);
    zoomPass.setBindGroup(0, zoomRenderBindGroup); // This uses intermediateTexture
    zoomPass.draw(6, 1, 0, 0); // Draw a full-screen quad
    zoomPass.end();

    device.queue.submit([commandEncoder.finish()]);

    // Ping-pong buffers
    currentParticleBufferIndex = 1 - currentParticleBufferIndex;

    // FPS calculation
    frameCount++;
    const fpsNow = performance.now();
    if (fpsNow - lastFPSTime > 1000) {
        // Update FPS display every second
        const fps = frameCount / ((fpsNow - lastFPSTime) / 1000);
        if (fpsDisplayElement) {
            fpsDisplayElement.textContent = fps.toFixed(1);
        }
        frameCount = 0;
        lastFPSTime = fpsNow;
    }

    animationId = requestAnimationFrame(frame); // Re-queue the next frame
}

// Initial setup
initWebGPU(); // Call the correct initialization function

// Add resize handling
window.addEventListener("resize", () => {
    if (device) {
        const fixedWidth = 2400;
        const fixedHeight = 2400;

        canvas.width = fixedWidth;
        canvas.height = fixedHeight;
        canvas.style.width = `${fixedWidth}px`;
        canvas.style.height = `${fixedHeight}px`;

        context.configure({
            device: device,
            format: presentationFormat,
            alphaMode: "premultiplied",
        });

        // Update relevant sim_params on resize
        const newCanvasRenderWidth = canvas.width;
        const newCanvasRenderHeight = canvas.height;
        const newVirtualWorldWidth =
            newCanvasRenderWidth + 2 * VIRTUAL_WORLD_BORDER;
        const newVirtualWorldHeight =
            newCanvasRenderHeight + 2 * VIRTUAL_WORLD_BORDER;
        const newVirtualWorldOffsetX = VIRTUAL_WORLD_BORDER;
        const newVirtualWorldOffsetY = VIRTUAL_WORLD_BORDER;

        // Offsets for writing to simParamsBuffer (in bytes)
        const virtualWorldWidthOffsetBytes = 4 * 4;
        const virtualWorldHeightOffsetBytes = 5 * 4;
        const canvasRenderWidthOffsetBytes = 6 * 4;
        const canvasRenderHeightOffsetBytes = 7 * 4;
        const virtualWorldOffsetXOffsetBytes = 8 * 4;
        const virtualWorldOffsetYOffsetBytes = 9 * 4;

        device.queue.writeBuffer(
            simParamsBuffer,
            virtualWorldWidthOffsetBytes,
            new Float32Array([newVirtualWorldWidth])
        );
        device.queue.writeBuffer(
            simParamsBuffer,
            virtualWorldHeightOffsetBytes,
            new Float32Array([newVirtualWorldHeight])
        );
        device.queue.writeBuffer(
            simParamsBuffer,
            canvasRenderWidthOffsetBytes,
            new Float32Array([newCanvasRenderWidth])
        );
        device.queue.writeBuffer(
            simParamsBuffer,
            canvasRenderHeightOffsetBytes,
            new Float32Array([newCanvasRenderHeight])
        );
        device.queue.writeBuffer(
            simParamsBuffer,
            virtualWorldOffsetXOffsetBytes,
            new Float32Array([newVirtualWorldOffsetX])
        );
        device.queue.writeBuffer(
            simParamsBuffer,
            virtualWorldOffsetYOffsetBytes,
            new Float32Array([newVirtualWorldOffsetY])
        );

        // Recreate textures with new virtual world dimensions
        sceneTexture.destroy();
        intermediateTexture.destroy();

        sceneTexture = device.createTexture({
            size: [newVirtualWorldWidth, newVirtualWorldHeight],
            format: presentationFormat,
            usage:
                GPUTextureUsage.TEXTURE_BINDING |
                GPUTextureUsage.RENDER_ATTACHMENT,
        });

        intermediateTexture = device.createTexture({
            size: [newVirtualWorldWidth, newVirtualWorldHeight],
            format: presentationFormat,
            usage:
                GPUTextureUsage.TEXTURE_BINDING |
                GPUTextureUsage.RENDER_ATTACHMENT,
        });

        // Update bind groups that reference the textures
        vignetteRenderBindGroup = device.createBindGroup({
            label: "Vignette Render Bind Group",
            layout: device.createBindGroupLayout({
                label: "Vignette BindGroupLayout",
                entries: [
                    {
                        binding: 0,
                        visibility: GPUShaderStage.FRAGMENT,
                        buffer: { type: "uniform" },
                    },
                    {
                        binding: 1,
                        visibility: GPUShaderStage.FRAGMENT,
                        sampler: { type: "filtering" },
                    },
                    {
                        binding: 2,
                        visibility: GPUShaderStage.FRAGMENT,
                        texture: { sampleType: "float" },
                    },
                ],
            }),
            entries: [
                { binding: 0, resource: { buffer: simParamsBuffer } },
                { binding: 1, resource: sceneSampler },
                { binding: 2, resource: intermediateTexture.createView() },
            ],
        });

        fisheyeRenderBindGroup = device.createBindGroup({
            label: "Fisheye Render Bind Group",
            layout: device.createBindGroupLayout({
                label: "Fisheye BindGroupLayout",
                entries: [
                    {
                        binding: 0,
                        visibility: GPUShaderStage.FRAGMENT,
                        buffer: { type: "uniform" },
                    },
                    {
                        binding: 1,
                        visibility: GPUShaderStage.FRAGMENT,
                        sampler: { type: "filtering" },
                    },
                    {
                        binding: 2,
                        visibility: GPUShaderStage.FRAGMENT,
                        texture: { sampleType: "float" },
                    },
                ],
            }),
            entries: [
                { binding: 0, resource: { buffer: simParamsBuffer } },
                { binding: 1, resource: sceneSampler },
                { binding: 2, resource: sceneTexture.createView() },
            ],
        });
    }
});

// === JoyStick Implementation ===

// JoyStick types (inline to avoid import issues)
interface JoyStickData {
    xPosition: number;
    yPosition: number;
    cardinalDirection: string;
    x: number;
    y: number;
}

// JoyStick variables
let joystick: any;
let joystickForceX = 0.0;
let joystickForceY = 0.0;
let joystickInfluence = 200.0; // Maximum force influence from joystick

// Zoom center variables for joystick navigation
let zoomCenterX = 1200.0; // Center X coordinate in 2400x2400 world (default: center)
let zoomCenterY = 1200.0; // Center Y coordinate in 2400x2400 world (default: center)

// Function to constrain zoom center based on zoom level
function constrainZoomCenter() {
    // Calculate maximum movement range based on zoom factor
    // f(x) ≈ 111.24·x - 122.29, where x = zoom factor
    const maxMovementRange = Math.max(0, 111.24 * currentZoomLevel - 122.29);

    // Calculate current distance from center (1200, 1200)
    const currentDistanceX = zoomCenterX - 1200.0;
    const currentDistanceY = zoomCenterY - 1200.0;
    const currentDistance = Math.sqrt(
        currentDistanceX * currentDistanceX +
            currentDistanceY * currentDistanceY
    );

    // If current position is beyond the allowed range, scale it back
    if (currentDistance > maxMovementRange && maxMovementRange > 0) {
        const scale = maxMovementRange / currentDistance;
        zoomCenterX = 1200.0 + currentDistanceX * scale;
        zoomCenterY = 1200.0 + currentDistanceY * scale;
    }

    // Clamp to world bounds as extra safety
    zoomCenterX = Math.max(0, Math.min(2400, zoomCenterX));
    zoomCenterY = Math.max(0, Math.min(2400, zoomCenterY));
}

async function initJoyStick() {
    console.log("Initializing JoyStick...");

    try {
        // Wait for the JoyStick library to be available globally
        if (typeof (window as any).JoyStick === "undefined") {
            console.log("Waiting for JoyStick library to load...");
            // Wait up to 5 seconds for the library to load
            for (let i = 0; i < 50; i++) {
                await new Promise((resolve) => setTimeout(resolve, 100));
                if (typeof (window as any).JoyStick !== "undefined") {
                    break;
                }
            }
        }

        const JoyStickConstructor = (window as any).JoyStick;

        if (!JoyStickConstructor) {
            throw new Error("JoyStick constructor not found after waiting");
        }

        console.log("JoyStick constructor found, creating instance...");

        // Initialize JoyStick with callback function
        joystick = new JoyStickConstructor(
            "joyDiv",
            {
                width: 150,
                height: 150,
                internalFillColor: "#E3C463",
                internalStrokeColor: "#B8A150",
                externalStrokeColor: "#E3C463",
                autoReturnToCenter: true,
            },
            function (stickData: JoyStickData) {
                // Calculate maximum movement range based on zoom factor
                // f(x) ≈ 111.24·x - 122.29, where x = zoom factor
                const maxMovementRange = Math.max(
                    0,
                    112 * currentZoomLevel - 150
                );

                // Convert joystick input (-100 to +100) to movement within the calculated range
                // Center is always (1200, 1200) and movement is limited by maxMovementRange
                const moveX = (stickData.x / 100.0) * maxMovementRange;
                const moveY = -(stickData.y / 100.0) * maxMovementRange; // Inverted Y-axis for intuitive navigation

                // Calculate new zoom center positions
                zoomCenterX = 1200.0 + moveX;
                zoomCenterY = 1200.0 + moveY;

                // Clamp values to stay within the 2400x2400 world (extra safety)
                // zoomCenterX = Math.max(0, Math.min(2400, zoomCenterX));
                // zoomCenterY = Math.max(0, Math.min(2400, zoomCenterY));

                // Update zoom uniforms buffer with new center position
                if (device && zoomUniformsBuffer) {
                    device.queue.writeBuffer(
                        zoomUniformsBuffer,
                        0,
                        new Float32Array([
                            currentZoomLevel,
                            zoomCenterX,
                            zoomCenterY,
                            0.0,
                        ])
                    );
                }

                // Update zoom center info display (separate from drift/force displays)
                const zoomCenterInfo =
                    document.getElementById("zoomCenterInfo");
                if (zoomCenterInfo) {
                    zoomCenterInfo.innerHTML = `Center: (${zoomCenterX.toFixed(
                        0
                    )}, ${zoomCenterY.toFixed(
                        0
                    )})<br>Range: ${maxMovementRange.toFixed(0)}`;
                }
            }
        );

        console.log("JoyStick initialized successfully");
    } catch (error) {
        console.error("Failed to initialize JoyStick:", error);

        // Show error message to user
        const joyDiv = document.getElementById("joyDiv");
        if (joyDiv) {
            joyDiv.innerHTML =
                '<div style="color: red; text-align: center; padding: 20px;">JoyStick failed to load</div>';
        }
    }
}

// Initialize JoyStick after DOM and libraries are loaded
setTimeout(() => {
    initJoyStick();

    // Initialize zoom center info display
    const zoomCenterInfo = document.getElementById("zoomCenterInfo");
    if (zoomCenterInfo) {
        const maxMovementRange = Math.max(
            0,
            111.24 * currentZoomLevel - 122.29
        );
        zoomCenterInfo.innerHTML = `Center: (${zoomCenterX.toFixed(
            0
        )}, ${zoomCenterY.toFixed(0)})<br>Range: ${maxMovementRange.toFixed(
            0
        )}`;
    }
}, 2000); // Increased delay to ensure script loading

// particle-lenia.ts - Core particle life simulation engine
// This module contains the WebGPU implementation of the particle-lenia system
// and is designed to be portable to other environments (e.g., Rust) without UI dependencies

import computeWGSL from "./shaders/compute.wgsl?raw";
import vertWGSL from "./shaders/vert.wgsl?raw";
import fragWGSL from "./shaders/frag.wgsl?raw";
import backgroundVertWGSL from "./shaders/background_vert.wgsl?raw";
import backgroundFragWGSL from "./shaders/background_frag.wgsl?raw";
import vignetteFragWGSL from "./shaders/vignette_frag.wgsl?raw";
import fisheyeFragWGSL from "./shaders/fisheye_frag.wgsl?raw";
import gridFragWGSL from "./shaders/grid_frag.wgsl?raw";
import zoomFragWGSL from "./shaders/zoom_frag.wgsl?raw";

import { hsluvToRgb } from "hsluv-ts";
import {
    Particle,
    InteractionRule,
    ParticleRules,
    SimulationParams,
    SIM_PARAMS_SIZE_BYTES,
    BoundaryMode,
} from "./particle-life-types";
import { generateParticleColors, logParticleColors } from "./hsluv-colors";

// === Configuration Constants ===
// Particle density constants for safe pressure-based scaling
export const MAX_PARTICLES = 6400; // Maximum particles (buffer allocation size)
export const MIN_PARTICLES = 1600; // Minimum particles (safety limit)
export const DEFAULT_PARTICLES = 3200; // Default active particle count
export const NUM_PARTICLES = MAX_PARTICLES; // Buffer allocation size (for backward compatibility)
export const NUM_TYPES = 5;
export const PARTICLE_RENDER_SIZE = 12.0;
export const PARTICLE_SIZE_BYTES = 24; // pos(8) + vel(8) + type(4) + size(4) = 24 bytes
export const RULE_SIZE_BYTES = 16; // attraction(f32), min_radius(f32), max_radius(f32), padding(f32)

// Size ranges for each particle type (multipliers of base size)
export const PARTICLE_TYPE_SIZE_RANGES = [
    { min: 1.4, max: 1.6 }, // Type 0: Blue   - large, dominant
    { min: 1.1, max: 1.3 }, // Type 1: Orange - medium-large
    { min: 0.6, max: 0.8 }, // Type 2: Red    - small, agile
    { min: 0.8, max: 1.0 }, // Type 3: Purple - smaller, compact
    { min: 0.9, max: 1.1 }, // Type 4: Green  - medium, balanced
];

// === Core Simulation State ===
export interface ParticleLeniaEngine {
    device: GPUDevice;
    presentationFormat: GPUTextureFormat;
    context: GPUCanvasContext;

    // Buffers
    simParamsBuffer: GPUBuffer;
    rulesBuffer: GPUBuffer;
    particleBuffers: [GPUBuffer, GPUBuffer];
    quadVertexBuffer: GPUBuffer;
    particleColorsBuffer: GPUBuffer;
    zoomUniformsBuffer: GPUBuffer;

    // Pipelines and Bind Groups
    computePipeline: GPUComputePipeline;
    renderPipeline: GPURenderPipeline;
    backgroundRenderPipeline: GPURenderPipeline;
    vignetteRenderPipeline: GPURenderPipeline;
    fisheyeRenderPipeline: GPURenderPipeline;
    gridRenderPipeline: GPURenderPipeline;
    zoomRenderPipeline: GPURenderPipeline;

    // Bind Groups
    computeBindGroups: [GPUBindGroup, GPUBindGroup];
    renderBindGroup: GPUBindGroup;
    backgroundRenderBindGroup: GPUBindGroup;
    vignetteRenderBindGroup: GPUBindGroup;
    fisheyeRenderBindGroup: GPUBindGroup;
    gridRenderBindGroup: GPUBindGroup;
    zoomRenderBindGroup: GPUBindGroup;

    // Textures and Sampler
    sceneTexture: GPUTexture;
    intermediateTexture: GPUTexture;
    sceneSampler: GPUSampler;

    // Simulation State
    simParams: SimulationParams;
    currentParticleBufferIndex: number;
    lastFrameTime: number;
    currentTime: number;
    currentZoomLevel: number;
    zoomCenterX: number;
    zoomCenterY: number;

    // Animation
    animationId?: number;
    isRunning: boolean;
}

// === Particle Transition System ===

interface ParticleTransition {
    startIndex: number;
    endIndex: number;
    startTime: number;
    duration: number; // in seconds
    type: "grow" | "shrink";
    targetSizes: Float32Array; // Target sizes for each particle
}

// Track active particle transitions
const activeTransitions: ParticleTransition[] = [];
const TRANSITION_DURATION = 1.5; // 1.5 seconds for smooth transitions

// Track the actual GPU particle count during transitions
let gpuParticleCount: number = 0;

// === Core Functions ===

export function createRandomRules(numTypes: number): InteractionRule[][] {
    const rules: InteractionRule[][] = [];
    for (let i = 0; i < numTypes; i++) {
        rules[i] = [];
        for (let j = 0; j < numTypes; j++) {
            rules[i][j] = {
                attraction: Math.random() * 0.4 - 0.1,
                minRadius: Math.random() * 20 + 10,
                maxRadius: 0,
            };
            rules[i][j].maxRadius =
                rules[i][j].minRadius + (Math.random() * 60 + 20);

            // Self-interaction: stronger repulsive
            if (i === j) {
                rules[i][j].attraction = Math.random() * -0.3 - 0.1; // -0.4 to -0.1
                rules[i][j].minRadius = Math.random() * 10 + 5; // 5 to 15
                rules[i][j].maxRadius =
                    rules[i][j].minRadius + (Math.random() * 20 + 15); // +15 to +35
            }
        }
    }
    return rules;
}

export function flattenRules(
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
            flatRules[offset++] = 0.0; // Padding
        }
    }
    return flatRules;
}

export function createInitialParticles(
    numParticles: number,
    numTypes: number,
    worldWidth: number,
    worldHeight: number
): ArrayBuffer {
    const particleData = new ArrayBuffer(numParticles * PARTICLE_SIZE_BYTES);
    const particleViewF32 = new Float32Array(particleData);
    const particleViewU32 = new Uint32Array(particleData);

    for (let i = 0; i < numParticles; i++) {
        const bufferOffsetF32 = i * (PARTICLE_SIZE_BYTES / 4);
        const bufferOffsetU32 = bufferOffsetF32;

        // Position (vec2f) - within virtual world dimensions
        particleViewF32[bufferOffsetF32 + 0] = Math.random() * worldWidth;
        particleViewF32[bufferOffsetF32 + 1] = Math.random() * worldHeight;

        // Velocity (vec2f)
        particleViewF32[bufferOffsetF32 + 2] = (Math.random() - 0.5) * 2.0;
        particleViewF32[bufferOffsetF32 + 3] = (Math.random() - 0.5) * 2.0;

        // Type (u32) - Stored after the 4 floats of pos and vel
        const particleType = Math.floor(Math.random() * numTypes);
        particleViewU32[bufferOffsetU32 + 4] = particleType;

        // Size (f32) - Stored after type, varies by particle type
        const sizeRange = PARTICLE_TYPE_SIZE_RANGES[particleType];
        const sizeMultiplier =
            Math.random() * (sizeRange.max - sizeRange.min) + sizeRange.min;
        particleViewF32[bufferOffsetF32 + 5] =
            PARTICLE_RENDER_SIZE * sizeMultiplier;
    }
    return particleData;
}

export function constrainZoomCenter(engine: ParticleLeniaEngine): void {
    // Calculate maximum movement range based on zoom factor
    const maxMovementRange = Math.max(
        0,
        111.24 * engine.currentZoomLevel - 122.29
    );

    // Calculate current distance from center (1200, 1200)
    const currentDistanceX = engine.zoomCenterX - 1200.0;
    const currentDistanceY = engine.zoomCenterY - 1200.0;
    const currentDistance = Math.sqrt(
        currentDistanceX * currentDistanceX +
            currentDistanceY * currentDistanceY
    );

    // If current position is beyond the allowed range, scale it back
    if (currentDistance > maxMovementRange && maxMovementRange > 0) {
        const scale = maxMovementRange / currentDistance;
        engine.zoomCenterX = 1200.0 + currentDistanceX * scale;
        engine.zoomCenterY = 1200.0 + currentDistanceY * scale;
    }

    // Clamp to world bounds as extra safety
    engine.zoomCenterX = Math.max(0, Math.min(2400, engine.zoomCenterX));
    engine.zoomCenterY = Math.max(0, Math.min(2400, engine.zoomCenterY));
}

export async function initializeParticleLeniaEngine(
    canvas: HTMLCanvasElement,
    virtualWorldBorder: number = 0
): Promise<ParticleLeniaEngine> {
    if (!navigator.gpu) {
        throw new Error("WebGPU not supported on this browser.");
    }

    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) {
        throw new Error("No appropriate GPUAdapter found.");
    }

    const device = await adapter.requestDevice();
    const context = canvas.getContext("webgpu") as GPUCanvasContext;
    const presentationFormat = navigator.gpu.getPreferredCanvasFormat();

    context.configure({
        device: device,
        format: presentationFormat,
        alphaMode: "opaque",
    });

    // Initialize simulation parameters
    const simParams: SimulationParams = {
        deltaTime: 1 / 60,
        friction: 0.1,
        numParticles: DEFAULT_PARTICLES, // Start with default, not max
        numTypes: NUM_TYPES,
        virtualWorldWidth: 2400 + 2 * virtualWorldBorder,
        virtualWorldHeight: 2400 + 2 * virtualWorldBorder,
        canvasRenderWidth: 2400,
        canvasRenderHeight: 2400,
        virtualWorldOffsetX: virtualWorldBorder,
        virtualWorldOffsetY: virtualWorldBorder,
        boundaryMode: BoundaryMode.Wrap,
        particleRenderSize: PARTICLE_RENDER_SIZE,
        forceScale: 400.0,
        rSmooth: 5.0,
        flatForce: false,
        driftXPerSecond: -10.0,
        interTypeAttractionScale: 1.0,
        interTypeRadiusScale: 1.0,
        time: 0.0,
        fisheyeStrength: 3.0,
        backgroundColor: [0.0, 0.0, 0.0],
        leniaEnabled: true,
        leniaGrowthMu: 0.18,
        leniaGrowthSigma: 0.025,
        leniaKernelRadius: 75.0,
    };

    // Create simulation parameters buffer
    const simParamsData = new ArrayBuffer(SIM_PARAMS_SIZE_BYTES);
    const simParamsViewF32 = new Float32Array(simParamsData);
    const simParamsViewU32 = new Uint32Array(simParamsData);

    // Populate simParamsData from the simParams object
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
    simParamsViewU32[14] = simParams.flatForce ? 1 : 0;
    simParamsViewF32[15] = simParams.driftXPerSecond;
    simParamsViewF32[16] = simParams.interTypeAttractionScale;
    simParamsViewF32[17] = simParams.interTypeRadiusScale;
    simParamsViewF32[18] = simParams.time;
    simParamsViewF32[19] = simParams.fisheyeStrength;
    simParamsViewF32[20] = simParams.backgroundColor[0];
    simParamsViewF32[21] = simParams.backgroundColor[1];
    simParamsViewF32[22] = simParams.backgroundColor[2];
    simParamsViewU32[23] = simParams.leniaEnabled ? 1 : 0;
    simParamsViewF32[24] = simParams.leniaGrowthMu;
    simParamsViewF32[25] = simParams.leniaGrowthSigma;
    simParamsViewF32[26] = simParams.leniaKernelRadius;
    simParamsViewF32[27] = 0.0; // padding

    const simParamsBuffer = device.createBuffer({
        label: "Simulation Parameters Buffer",
        size: SIM_PARAMS_SIZE_BYTES,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    device.queue.writeBuffer(simParamsBuffer, 0, simParamsData);

    // Create textures
    const sceneTexture = device.createTexture({
        size: [simParams.virtualWorldWidth, simParams.virtualWorldHeight],
        format: presentationFormat,
        usage:
            GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.RENDER_ATTACHMENT,
    });

    const intermediateTexture = device.createTexture({
        size: [simParams.virtualWorldWidth, simParams.virtualWorldHeight],
        format: presentationFormat,
        usage:
            GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.RENDER_ATTACHMENT,
    });

    const sceneSampler = device.createSampler({
        magFilter: "linear",
        minFilter: "linear",
    });

    // Create interaction rules
    const rulesData = createRandomRules(NUM_TYPES);
    const flatRulesData = flattenRules(rulesData, NUM_TYPES);
    const rulesBuffer = device.createBuffer({
        label: "Interaction Rules Buffer",
        size: flatRulesData.byteLength,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        mappedAtCreation: true,
    });
    new Float32Array(rulesBuffer.getMappedRange()).set(flatRulesData);
    rulesBuffer.unmap();

    // Create particle colors buffer
    const particleColors = generateParticleColors(NUM_TYPES);
    const particleColorsBuffer = device.createBuffer({
        label: "Particle Colors Buffer",
        size: particleColors.byteLength,
        usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
        mappedAtCreation: true,
    });
    new Float32Array(particleColorsBuffer.getMappedRange()).set(particleColors);
    particleColorsBuffer.unmap();

    // Log colors for debugging
    logParticleColors(NUM_TYPES);

    // Create particle buffers
    const initialParticleData = createInitialParticles(
        MAX_PARTICLES, // Allocate buffer for maximum particles
        NUM_TYPES,
        simParams.virtualWorldWidth,
        simParams.virtualWorldHeight
    );

    const particleBuffers: [GPUBuffer, GPUBuffer] = [
        device.createBuffer({
            label: "Particle Buffer A",
            size: initialParticleData.byteLength,
            usage:
                GPUBufferUsage.STORAGE |
                GPUBufferUsage.VERTEX |
                GPUBufferUsage.COPY_DST |
                GPUBufferUsage.COPY_SRC,
            mappedAtCreation: true,
        }),
        device.createBuffer({
            label: "Particle Buffer B",
            size: initialParticleData.byteLength,
            usage:
                GPUBufferUsage.STORAGE |
                GPUBufferUsage.VERTEX |
                GPUBufferUsage.COPY_DST |
                GPUBufferUsage.COPY_SRC,
        }),
    ];

    new Uint8Array(particleBuffers[0].getMappedRange()).set(
        new Uint8Array(initialParticleData)
    );
    particleBuffers[0].unmap();

    // Create quad vertex buffer for particle rendering
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

    const quadVertexBuffer = device.createBuffer({
        label: "Particle Quad Vertex Buffer",
        size: particleQuadVertices.byteLength,
        usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
        mappedAtCreation: true,
    });
    new Float32Array(quadVertexBuffer.getMappedRange()).set(
        particleQuadVertices
    );
    quadVertexBuffer.unmap();

    // Create zoom uniforms buffer
    const zoomUniformsBuffer = device.createBuffer({
        label: "Zoom Uniforms Buffer",
        size: 16, // 4 floats: zoom_level, center_x, center_y, padding
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });

    const initialZoomLevel = 1.5;
    const initialZoomCenterX = 1200.0;
    const initialZoomCenterY = 1200.0;

    device.queue.writeBuffer(
        zoomUniformsBuffer,
        0,
        new Float32Array([
            initialZoomLevel,
            initialZoomCenterX,
            initialZoomCenterY,
            0.0,
        ])
    );

    // Create shader modules
    const computeShaderModule = device.createShaderModule({
        code: computeWGSL,
    });
    const renderShaderModuleVert = device.createShaderModule({
        code: vertWGSL,
    });
    const renderShaderModuleFrag = device.createShaderModule({
        code: fragWGSL,
    });
    const backgroundShaderModuleVert = device.createShaderModule({
        code: backgroundVertWGSL,
    });
    const backgroundShaderModuleFrag = device.createShaderModule({
        code: backgroundFragWGSL,
    });
    const vignetteShaderModuleFrag = device.createShaderModule({
        code: vignetteFragWGSL,
    });
    const fisheyeShaderModuleFrag = device.createShaderModule({
        code: fisheyeFragWGSL,
    });
    const gridShaderModuleFrag = device.createShaderModule({
        code: gridFragWGSL,
    });
    const zoomShaderModuleFrag = device.createShaderModule({
        code: zoomFragWGSL,
    });

    // Create compute pipeline
    const computePipeline = device.createComputePipeline({
        label: "Particle Life Compute Pipeline",
        layout: "auto",
        compute: {
            module: computeShaderModule,
            entryPoint: "main",
        },
    });

    // Create bind group layouts
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

    // Create render pipelines
    const backgroundRenderPipeline = device.createRenderPipeline({
        label: "Background Render Pipeline",
        layout: device.createPipelineLayout({
            bindGroupLayouts: [simParamsBindGroupLayout],
        }),
        vertex: {
            module: backgroundShaderModuleVert,
            entryPoint: "main",
        },
        fragment: {
            module: backgroundShaderModuleFrag,
            entryPoint: "main",
            targets: [{ format: presentationFormat }],
        },
        primitive: { topology: "triangle-list" },
    });

    const renderPipeline = device.createRenderPipeline({
        label: "Particle Render Pipeline",
        layout: device.createPipelineLayout({
            bindGroupLayouts: [particleRenderBindGroupLayout],
        }),
        vertex: {
            module: renderShaderModuleVert,
            entryPoint: "main",
            buffers: [
                {
                    arrayStride: PARTICLE_SIZE_BYTES,
                    stepMode: "instance",
                    attributes: [
                        { shaderLocation: 0, offset: 0, format: "float32x2" },
                        { shaderLocation: 1, offset: 8, format: "float32x2" },
                        { shaderLocation: 2, offset: 16, format: "uint32" },
                        { shaderLocation: 3, offset: 20, format: "float32" },
                    ],
                },
                {
                    arrayStride: 2 * Float32Array.BYTES_PER_ELEMENT,
                    stepMode: "vertex",
                    attributes: [
                        { shaderLocation: 4, offset: 0, format: "float32x2" },
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
        primitive: { topology: "triangle-strip" },
    });

    // Create additional pipelines (vignette, fisheye, grid, zoom)
    const vignetteBindGroupLayout = device.createBindGroupLayout({
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
    });

    const vignetteRenderPipeline = device.createRenderPipeline({
        label: "Vignette Render Pipeline",
        layout: device.createPipelineLayout({
            bindGroupLayouts: [vignetteBindGroupLayout],
        }),
        vertex: { module: backgroundShaderModuleVert, entryPoint: "main" },
        fragment: {
            module: vignetteShaderModuleFrag,
            entryPoint: "main",
            targets: [{ format: presentationFormat }],
        },
        primitive: { topology: "triangle-list" },
    });

    const fisheyeBindGroupLayout = device.createBindGroupLayout({
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
    });

    const fisheyeRenderPipeline = device.createRenderPipeline({
        label: "Fisheye Render Pipeline",
        layout: device.createPipelineLayout({
            bindGroupLayouts: [fisheyeBindGroupLayout],
        }),
        vertex: { module: backgroundShaderModuleVert, entryPoint: "main" },
        fragment: {
            module: fisheyeShaderModuleFrag,
            entryPoint: "main",
            targets: [{ format: presentationFormat }],
        },
        primitive: { topology: "triangle-list" },
    });

    const gridRenderPipeline = device.createRenderPipeline({
        label: "Grid Render Pipeline",
        layout: device.createPipelineLayout({
            bindGroupLayouts: [simParamsBindGroupLayout],
        }),
        vertex: { module: backgroundShaderModuleVert, entryPoint: "main" },
        fragment: {
            module: gridShaderModuleFrag,
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
        primitive: { topology: "triangle-list" },
    });

    const zoomBindGroupLayout = device.createBindGroupLayout({
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

    const zoomRenderPipeline = device.createRenderPipeline({
        label: "Zoom Render Pipeline",
        layout: device.createPipelineLayout({
            bindGroupLayouts: [zoomBindGroupLayout],
        }),
        vertex: { module: backgroundShaderModuleVert, entryPoint: "main" },
        fragment: {
            module: zoomShaderModuleFrag,
            entryPoint: "main",
            targets: [{ format: presentationFormat }],
        },
        primitive: { topology: "triangle-list" },
    });

    // Create bind groups
    const computeBindGroups: [GPUBindGroup, GPUBindGroup] = [
        device.createBindGroup({
            label: "Compute Bind Group A",
            layout: computePipeline.getBindGroupLayout(0),
            entries: [
                { binding: 0, resource: { buffer: particleBuffers[0] } },
                { binding: 1, resource: { buffer: rulesBuffer } },
                { binding: 2, resource: { buffer: simParamsBuffer } },
                { binding: 3, resource: { buffer: particleBuffers[1] } },
            ],
        }),
        device.createBindGroup({
            label: "Compute Bind Group B",
            layout: computePipeline.getBindGroupLayout(0),
            entries: [
                { binding: 0, resource: { buffer: particleBuffers[1] } },
                { binding: 1, resource: { buffer: rulesBuffer } },
                { binding: 2, resource: { buffer: simParamsBuffer } },
                { binding: 3, resource: { buffer: particleBuffers[0] } },
            ],
        }),
    ];

    const renderBindGroup = device.createBindGroup({
        label: "Render BindGroup",
        layout: particleRenderBindGroupLayout,
        entries: [
            { binding: 0, resource: { buffer: simParamsBuffer } },
            { binding: 1, resource: { buffer: particleColorsBuffer } },
        ],
    });

    const backgroundRenderBindGroup = device.createBindGroup({
        label: "Background Render Bind Group",
        layout: simParamsBindGroupLayout,
        entries: [{ binding: 0, resource: { buffer: simParamsBuffer } }],
    });

    const vignetteRenderBindGroup = device.createBindGroup({
        label: "Vignette Render Bind Group",
        layout: vignetteBindGroupLayout,
        entries: [
            { binding: 0, resource: { buffer: simParamsBuffer } },
            { binding: 1, resource: sceneSampler },
            { binding: 2, resource: intermediateTexture.createView() },
        ],
    });

    const fisheyeRenderBindGroup = device.createBindGroup({
        label: "Fisheye Render Bind Group",
        layout: fisheyeBindGroupLayout,
        entries: [
            { binding: 0, resource: { buffer: simParamsBuffer } },
            { binding: 1, resource: sceneSampler },
            { binding: 2, resource: sceneTexture.createView() },
        ],
    });

    const gridRenderBindGroup = device.createBindGroup({
        label: "Grid Render Bind Group",
        layout: simParamsBindGroupLayout,
        entries: [{ binding: 0, resource: { buffer: simParamsBuffer } }],
    });

    const zoomRenderBindGroup = device.createBindGroup({
        label: "Zoom Render Bind Group",
        layout: zoomBindGroupLayout,
        entries: [
            { binding: 0, resource: sceneSampler },
            { binding: 1, resource: intermediateTexture.createView() },
            { binding: 2, resource: { buffer: zoomUniformsBuffer } },
        ],
    });

    // Return the complete engine instance
    const engine: ParticleLeniaEngine = {
        device,
        presentationFormat,
        context,
        simParamsBuffer,
        rulesBuffer,
        particleBuffers,
        quadVertexBuffer,
        particleColorsBuffer,
        zoomUniformsBuffer,
        computePipeline,
        renderPipeline,
        backgroundRenderPipeline,
        vignetteRenderPipeline,
        fisheyeRenderPipeline,
        gridRenderPipeline,
        zoomRenderPipeline,
        computeBindGroups,
        renderBindGroup,
        backgroundRenderBindGroup,
        vignetteRenderBindGroup,
        fisheyeRenderBindGroup,
        gridRenderBindGroup,
        zoomRenderBindGroup,
        sceneTexture,
        intermediateTexture,
        sceneSampler,
        simParams,
        currentParticleBufferIndex: 0,
        lastFrameTime: 0,
        currentTime: 0,
        currentZoomLevel: initialZoomLevel,
        zoomCenterX: initialZoomCenterX,
        zoomCenterY: initialZoomCenterY,
        isRunning: false,
    };

    // Initialize GPU particle count tracking
    gpuParticleCount = simParams.numParticles;

    return engine;
}

export function updateSimulationParameter<K extends keyof SimulationParams>(
    engine: ParticleLeniaEngine,
    parameterName: K,
    value: SimulationParams[K],
    bufferOffset: number
): void {
    // Type assertion is safe here because we know the types match
    (engine.simParams as any)[parameterName] = value;

    if (typeof value === "boolean") {
        engine.device.queue.writeBuffer(
            engine.simParamsBuffer,
            bufferOffset,
            new Uint32Array([value ? 1 : 0])
        );
    } else if (typeof value === "number") {
        engine.device.queue.writeBuffer(
            engine.simParamsBuffer,
            bufferOffset,
            new Float32Array([value])
        );
    } else if (Array.isArray(value)) {
        engine.device.queue.writeBuffer(
            engine.simParamsBuffer,
            bufferOffset,
            new Float32Array(value)
        );
    }
}

// Buffer offset mapping for simulation parameters
// This maps parameter names to their byte offsets in the GPU buffer
const PARAMETER_OFFSETS: Record<keyof SimulationParams, number> = {
    deltaTime: 0 * 4, // simParamsViewF32[0]
    friction: 1 * 4, // simParamsViewF32[1]
    numParticles: 2 * 4, // simParamsViewU32[2]
    numTypes: 3 * 4, // simParamsViewU32[3]
    virtualWorldWidth: 4 * 4, // simParamsViewF32[4]
    virtualWorldHeight: 5 * 4, // simParamsViewF32[5]
    canvasRenderWidth: 6 * 4, // simParamsViewF32[6]
    canvasRenderHeight: 7 * 4, // simParamsViewF32[7]
    virtualWorldOffsetX: 8 * 4, // simParamsViewF32[8]
    virtualWorldOffsetY: 9 * 4, // simParamsViewF32[9]
    boundaryMode: 10 * 4, // simParamsViewU32[10]
    particleRenderSize: 11 * 4, // simParamsViewF32[11]
    forceScale: 12 * 4, // simParamsViewF32[12]
    rSmooth: 13 * 4, // simParamsViewF32[13]
    flatForce: 14 * 4, // simParamsViewU32[14]
    driftXPerSecond: 15 * 4, // simParamsViewF32[15]
    interTypeAttractionScale: 16 * 4, // simParamsViewF32[16]
    interTypeRadiusScale: 17 * 4, // simParamsViewF32[17]
    time: 18 * 4, // simParamsViewF32[18]
    fisheyeStrength: 19 * 4, // simParamsViewF32[19]
    backgroundColor: 20 * 4, // simParamsViewF32[20] (3 floats: RGB)
    leniaEnabled: 23 * 4, // simParamsViewU32[23]
    leniaGrowthMu: 24 * 4, // simParamsViewF32[24]
    leniaGrowthSigma: 25 * 4, // simParamsViewF32[25]
    leniaKernelRadius: 26 * 4, // simParamsViewF32[26]
};

// Convenient wrapper function that automatically calculates buffer offset
export function updateSimulationParameterAuto<K extends keyof SimulationParams>(
    engine: ParticleLeniaEngine,
    parameterName: K,
    value: SimulationParams[K]
): void {
    const bufferOffset = PARAMETER_OFFSETS[parameterName];
    if (bufferOffset === undefined) {
        console.warn(`Unknown parameter: ${parameterName}`);
        return;
    }

    // Type assertion is safe here because we know the types match
    (engine.simParams as any)[parameterName] = value;

    if (typeof value === "boolean") {
        engine.device.queue.writeBuffer(
            engine.simParamsBuffer,
            bufferOffset,
            new Uint32Array([value ? 1 : 0])
        );
    } else if (typeof value === "number") {
        engine.device.queue.writeBuffer(
            engine.simParamsBuffer,
            bufferOffset,
            new Float32Array([value])
        );
    } else if (Array.isArray(value)) {
        engine.device.queue.writeBuffer(
            engine.simParamsBuffer,
            bufferOffset,
            new Float32Array(value)
        );
    }
}

export function updateZoom(
    engine: ParticleLeniaEngine,
    zoomLevel: number,
    centerX?: number,
    centerY?: number
): void {
    engine.currentZoomLevel = zoomLevel;

    if (centerX !== undefined) engine.zoomCenterX = centerX;
    if (centerY !== undefined) engine.zoomCenterY = centerY;

    constrainZoomCenter(engine);

    engine.device.queue.writeBuffer(
        engine.zoomUniformsBuffer,
        0,
        new Float32Array([
            engine.currentZoomLevel,
            engine.zoomCenterX,
            engine.zoomCenterY,
            0.0,
        ])
    );
}

export function renderFrame(engine: ParticleLeniaEngine): void {
    const now = performance.now();
    let dt = (now - engine.lastFrameTime) / 1000;
    engine.lastFrameTime = now;

    // Clamp deltaTime to avoid large jumps
    if (dt === 0 || dt > 0.1) {
        dt = 1 / 60; // Assume 60 FPS
    }

    engine.simParams.deltaTime = dt;
    engine.currentTime += dt;
    engine.simParams.time = engine.currentTime;

    // Update dynamic simParams in the GPU buffer
    engine.device.queue.writeBuffer(
        engine.simParamsBuffer,
        0,
        new Float32Array([engine.simParams.deltaTime])
    );
    engine.device.queue.writeBuffer(
        engine.simParamsBuffer,
        18 * 4,
        new Float32Array([engine.simParams.time])
    );

    const commandEncoder = engine.device.createCommandEncoder({
        label: "Particle Life Frame Encoder",
    });

    // Compute Pass - MUST run before updateParticleTransitions
    const computePass = commandEncoder.beginComputePass({
        label: "Particle Compute Pass",
    });
    computePass.setPipeline(engine.computePipeline);
    computePass.setBindGroup(
        0,
        engine.computeBindGroups[engine.currentParticleBufferIndex]
    );
    // Use gpuParticleCount for compute - this maintains higher count during shrink transitions
    const numWorkgroups = Math.ceil(gpuParticleCount / 64);
    computePass.dispatchWorkgroups(numWorkgroups);
    computePass.end();

    // Update particle transitions AFTER compute pass to prevent compute shader from overwriting our size changes
    // CRITICAL: The compute shader copies the entire particle struct (including size) from input to output,
    // so any size modifications we make before the compute pass will be lost. We must update sizes after
    // the compute shader has finished processing to ensure our transitions are actually visible.
    updateParticleTransitions(engine);

    // Render Passes
    const sceneTextureView = engine.sceneTexture.createView();

    // 1. Background Render Pass
    const backgroundPassDescriptor: GPURenderPassDescriptor = {
        label: "Background Render Pass",
        colorAttachments: [
            {
                view: sceneTextureView,
                loadOp: "clear",
                clearValue: {
                    r: engine.simParams.backgroundColor[0],
                    g: engine.simParams.backgroundColor[1],
                    b: engine.simParams.backgroundColor[2],
                    a: 1.0,
                },
                storeOp: "store",
            },
        ],
    };
    const backgroundPass = commandEncoder.beginRenderPass(
        backgroundPassDescriptor
    );
    backgroundPass.setViewport(
        engine.simParams.virtualWorldOffsetX,
        engine.simParams.virtualWorldOffsetY,
        engine.simParams.canvasRenderWidth,
        engine.simParams.canvasRenderHeight,
        0.0,
        1.0
    );
    backgroundPass.setPipeline(engine.backgroundRenderPipeline);
    backgroundPass.setBindGroup(0, engine.backgroundRenderBindGroup);
    backgroundPass.draw(6, 1, 0, 0);
    backgroundPass.end();

    // 2. Particle Render Pass
    const particleRenderPassDescriptor: GPURenderPassDescriptor = {
        label: "Particle Render Pass",
        colorAttachments: [
            {
                view: sceneTextureView,
                loadOp: "load",
                storeOp: "store",
            },
        ],
    };
    const particlePass = commandEncoder.beginRenderPass(
        particleRenderPassDescriptor
    );
    particlePass.setViewport(
        engine.simParams.virtualWorldOffsetX,
        engine.simParams.virtualWorldOffsetY,
        engine.simParams.canvasRenderWidth,
        engine.simParams.canvasRenderHeight,
        0.0,
        1.0
    );
    particlePass.setPipeline(engine.renderPipeline);
    particlePass.setVertexBuffer(
        0,
        engine.particleBuffers[engine.currentParticleBufferIndex]
    );
    particlePass.setVertexBuffer(1, engine.quadVertexBuffer);
    particlePass.setBindGroup(0, engine.renderBindGroup);
    particlePass.draw(4, gpuParticleCount, 0, 0);
    particlePass.end();

    // 3. Grid Render Pass
    const gridPassDescriptor: GPURenderPassDescriptor = {
        label: "Grid Pass to Scene Texture",
        colorAttachments: [
            {
                view: sceneTextureView,
                loadOp: "load",
                storeOp: "store",
            },
        ],
    };
    const gridPass = commandEncoder.beginRenderPass(gridPassDescriptor);
    gridPass.setViewport(
        engine.simParams.virtualWorldOffsetX,
        engine.simParams.virtualWorldOffsetY,
        engine.simParams.canvasRenderWidth,
        engine.simParams.canvasRenderHeight,
        0.0,
        1.0
    );
    gridPass.setPipeline(engine.gridRenderPipeline);
    gridPass.setBindGroup(0, engine.gridRenderBindGroup);
    gridPass.draw(6, 1, 0, 0);
    gridPass.end();

    // 4. Fisheye Post-Processing Pass
    const intermediateTextureView = engine.intermediateTexture.createView();
    const fisheyePassDescriptor: GPURenderPassDescriptor = {
        label: "Fisheye Pass to Intermediate Texture",
        colorAttachments: [
            {
                view: intermediateTextureView,
                loadOp: "clear",
                clearValue: { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
                storeOp: "store",
            },
        ],
    };
    const fisheyePass = commandEncoder.beginRenderPass(fisheyePassDescriptor);
    fisheyePass.setPipeline(engine.fisheyeRenderPipeline);
    fisheyePass.setBindGroup(0, engine.fisheyeRenderBindGroup);
    fisheyePass.draw(6, 1, 0, 0);
    fisheyePass.end();

    // 5. Zoom Post-Processing Pass
    const canvasTextureView = engine.context.getCurrentTexture().createView();
    const zoomPassDescriptor: GPURenderPassDescriptor = {
        label: "Zoom Pass to Canvas",
        colorAttachments: [
            {
                view: canvasTextureView,
                loadOp: "clear",
                clearValue: { r: 0.0, g: 0.0, b: 0.0, a: 1.0 },
                storeOp: "store",
            },
        ],
    };
    const zoomPass = commandEncoder.beginRenderPass(zoomPassDescriptor);
    zoomPass.setPipeline(engine.zoomRenderPipeline);
    zoomPass.setBindGroup(0, engine.zoomRenderBindGroup);
    zoomPass.draw(6, 1, 0, 0);
    zoomPass.end();

    engine.device.queue.submit([commandEncoder.finish()]);

    // Ping-pong buffers
    engine.currentParticleBufferIndex = 1 - engine.currentParticleBufferIndex;
}

/**
 * Updates active particle transitions (grow/shrink animations)
 * Handles smooth size changes over time and cleans up completed transitions
 */
function updateParticleTransitions(engine: ParticleLeniaEngine): void {
    if (activeTransitions.length === 0) return;

    const currentTime = engine.currentTime;
    const completedTransitions: number[] = [];

    for (let i = 0; i < activeTransitions.length; i++) {
        const transition = activeTransitions[i];
        const elapsed = currentTime - transition.startTime;
        const progress = Math.min(elapsed / transition.duration, 1.0);

        // Calculate size updates for all particles in this transition
        const particleCount = transition.endIndex - transition.startIndex;
        const sizeUpdates = new Float32Array(particleCount);

        for (let j = 0; j < particleCount; j++) {
            if (transition.type === "grow") {
                // Grow from 0.001 to target size
                const targetSize = transition.targetSizes[j];
                sizeUpdates[j] = 0.001 + (targetSize - 0.001) * progress;
            } else {
                // Shrink from target size to 0
                const startingSize = transition.targetSizes[j];
                sizeUpdates[j] = startingSize * (1.0 - progress);
            }
        }

        // Update particle sizes in both buffers
        for (let bufferIndex = 0; bufferIndex < 2; bufferIndex++) {
            for (let j = 0; j < particleCount; j++) {
                const particleIndex = transition.startIndex + j;
                const sizeOffset = particleIndex * PARTICLE_SIZE_BYTES + 20; // Size is at offset 20 (pos + vel + type)

                engine.device.queue.writeBuffer(
                    engine.particleBuffers[bufferIndex],
                    sizeOffset,
                    new Float32Array([sizeUpdates[j]])
                );
            }
        }

        // Check if transition is complete
        if (progress >= 1.0) {
            console.log(
                `✅ Completed ${
                    transition.type
                } transition for ${particleCount} particles (indices ${
                    transition.startIndex
                } to ${transition.endIndex - 1})`
            );

            // Handle completion based on transition type
            if (transition.type === "shrink") {
                console.log(
                    `🍂 Shrink transition complete - scheduling off-screen repositioning and GPU count update`
                );

                // Reposition particles off-screen as safety measure
                repositionParticlesOffScreen(engine, transition);

                // Update GPU particle count to the new (lower) count
                // This is when we finally reduce the GPU particle count after the visual transition
                gpuParticleCount = engine.simParams.numParticles;

                console.log(
                    `📉 Updated GPU particle count from ${transition.endIndex} to ${gpuParticleCount} after shrink completion`
                );
            }

            completedTransitions.push(i);
        }
    }

    // Remove completed transitions (in reverse order to avoid index issues)
    for (let i = completedTransitions.length - 1; i >= 0; i--) {
        activeTransitions.splice(completedTransitions[i], 1);
    }

    if (completedTransitions.length > 0) {
        console.log(
            `🧹 Cleaned up ${completedTransitions.length} completed transitions, ${activeTransitions.length} remain active`
        );
    }
}

export function updateBackgroundColorAndDrift(
    engine: ParticleLeniaEngine,
    newDriftXPerSecond: number
): void {
    // Update the simulation parameter
    updateSimulationParameterAuto(
        engine,
        "driftXPerSecond",
        newDriftXPerSecond
    );

    // Calculate background color based on drift speed using HSLuv
    const normalizedAbsDrift = Math.min(
        1,
        Math.abs(newDriftXPerSecond) / 80.0 // Match drift range of ±80 px/s
    );

    // Hue transitions from blue (240°) at no drift to red (0°) at max drift
    const hue = 215 - normalizedAbsDrift * 200; // 240° to 0° (blue to red)
    const saturation = 33;
    const lightness = 66;

    // Convert HSLuv to RGB
    const [red, green, blue] = hsluvToRgb([hue, saturation, lightness]);

    // Update background color parameter
    updateSimulationParameterAuto(engine, "backgroundColor", [
        red,
        green,
        blue,
    ]);
}

export function startAnimation(engine: ParticleLeniaEngine): void {
    if (engine.isRunning) return;

    engine.isRunning = true;
    engine.lastFrameTime = performance.now();

    function frame() {
        if (!engine.isRunning) return;

        renderFrame(engine);
        engine.animationId = requestAnimationFrame(frame);
    }

    frame();
}

export function stopAnimation(engine: ParticleLeniaEngine): void {
    engine.isRunning = false;
    if (engine.animationId) {
        cancelAnimationFrame(engine.animationId);
        engine.animationId = undefined;
    }
}

export function destroyEngine(engine: ParticleLeniaEngine): void {
    stopAnimation(engine);

    // Clean up GPU resources
    engine.simParamsBuffer.destroy();
    engine.rulesBuffer.destroy();
    engine.particleBuffers[0].destroy();
    engine.particleBuffers[1].destroy();
    engine.quadVertexBuffer.destroy();
    engine.particleColorsBuffer.destroy();
    engine.zoomUniformsBuffer.destroy();
    engine.sceneTexture.destroy();
    engine.intermediateTexture.destroy();

    // Destroy device if possible
    if (typeof engine.device.destroy === "function") {
        engine.device.destroy();
    }
}

/**
 * Initializes newly activated particles when particle count is increased
 * Ensures particles beyond the previous count have proper positions, velocities, and types
 */
function initializeNewParticles(
    engine: ParticleLeniaEngine,
    startIndex: number,
    endIndex: number
): void {
    try {
        const particleCount = endIndex - startIndex;
        if (particleCount <= 0) return;

        console.log(
            `🚀 Initializing ${particleCount} new particles (indices ${startIndex} to ${
                endIndex - 1
            })`
        );

        // Create properly initialized particle data for the new particles
        const newParticleData = new ArrayBuffer(
            particleCount * PARTICLE_SIZE_BYTES
        );
        const particleViewF32 = new Float32Array(newParticleData);
        const particleViewU32 = new Uint32Array(newParticleData);

        for (let i = 0; i < particleCount; i++) {
            const bufferOffsetF32 = i * (PARTICLE_SIZE_BYTES / 4);
            const bufferOffsetU32 = bufferOffsetF32;

            // Position (vec2f) - spawn randomly within virtual world
            particleViewF32[bufferOffsetF32 + 0] =
                Math.random() * engine.simParams.virtualWorldWidth;
            particleViewF32[bufferOffsetF32 + 1] =
                Math.random() * engine.simParams.virtualWorldHeight;

            // Velocity (vec2f) - small random initial velocity
            particleViewF32[bufferOffsetF32 + 2] = (Math.random() - 0.5) * 2.0;
            particleViewF32[bufferOffsetF32 + 3] = (Math.random() - 0.5) * 2.0;

            // Type (u32) - random type for diversity
            const particleType = Math.floor(Math.random() * NUM_TYPES);
            particleViewU32[bufferOffsetU32 + 4] = particleType;

            // Size (f32) - varies by particle type
            const sizeRange = PARTICLE_TYPE_SIZE_RANGES[particleType];
            const sizeMultiplier =
                Math.random() * (sizeRange.max - sizeRange.min) + sizeRange.min;
            particleViewF32[bufferOffsetF32 + 5] =
                PARTICLE_RENDER_SIZE * sizeMultiplier;
        }

        // Update both particle buffers with the new particle data
        const startByteOffset = startIndex * PARTICLE_SIZE_BYTES;

        // Update buffer A
        engine.device.queue.writeBuffer(
            engine.particleBuffers[0],
            startByteOffset,
            newParticleData
        );

        // Update buffer B
        engine.device.queue.writeBuffer(
            engine.particleBuffers[1],
            startByteOffset,
            newParticleData
        );

        console.log(
            `✅ Successfully initialized ${particleCount} new particles`
        );
    } catch (error) {
        console.error("💥 Error initializing new particles:", error);
    }
}

/**
 * Initializes newly activated particles with smooth grow-in transition
 */
function initializeNewParticlesWithTransition(
    engine: ParticleLeniaEngine,
    startIndex: number,
    endIndex: number
): void {
    try {
        const particleCount = endIndex - startIndex;
        if (particleCount <= 0) return;

        console.log(
            `🌱 Starting grow transition for ${particleCount} new particles (indices ${startIndex} to ${
                endIndex - 1
            })`
        );

        // Initialize particles with normal properties but very small sizes
        const newParticleData = new ArrayBuffer(
            particleCount * PARTICLE_SIZE_BYTES
        );
        const particleViewF32 = new Float32Array(newParticleData);
        const particleViewU32 = new Uint32Array(newParticleData);
        const targetSizes = new Float32Array(particleCount);

        for (let i = 0; i < particleCount; i++) {
            const bufferOffsetF32 = i * (PARTICLE_SIZE_BYTES / 4);
            const bufferOffsetU32 = bufferOffsetF32;

            // Position (vec2f) - spawn randomly within virtual world
            particleViewF32[bufferOffsetF32 + 0] =
                Math.random() * engine.simParams.virtualWorldWidth;
            particleViewF32[bufferOffsetF32 + 1] =
                Math.random() * engine.simParams.virtualWorldHeight;

            // Velocity (vec2f) - small random initial velocity
            particleViewF32[bufferOffsetF32 + 2] = (Math.random() - 0.5) * 2.0;
            particleViewF32[bufferOffsetF32 + 3] = (Math.random() - 0.5) * 2.0;

            // Type (u32) - random type for diversity
            const particleType = Math.floor(Math.random() * NUM_TYPES);
            particleViewU32[bufferOffsetU32 + 4] = particleType;

            // Calculate target size for this particle type
            const sizeRange = PARTICLE_TYPE_SIZE_RANGES[particleType];
            const sizeMultiplier =
                Math.random() * (sizeRange.max - sizeRange.min) + sizeRange.min;
            const targetSize = PARTICLE_RENDER_SIZE * sizeMultiplier;
            targetSizes[i] = targetSize;

            // Start with truly invisible size (will grow over time)
            particleViewF32[bufferOffsetF32 + 5] = 0.001; // Tiny initial size
        }

        // Update both particle buffers with the new particle data
        const startByteOffset = startIndex * PARTICLE_SIZE_BYTES;

        engine.device.queue.writeBuffer(
            engine.particleBuffers[0],
            startByteOffset,
            newParticleData
        );
        engine.device.queue.writeBuffer(
            engine.particleBuffers[1],
            startByteOffset,
            newParticleData
        );

        // Add transition to active list
        activeTransitions.push({
            startIndex,
            endIndex,
            startTime: engine.currentTime,
            duration: TRANSITION_DURATION,
            type: "grow",
            targetSizes,
        });

        console.log(
            `✅ Started grow transition for ${particleCount} new particles`
        );
    } catch (error) {
        console.error("💥 Error starting particle grow transition:", error);
    }
}

/**
 * Reads actual particle sizes from GPU buffer for accurate shrink transitions
 */
async function readParticleSizesFromGPU(
    engine: ParticleLeniaEngine,
    startIndex: number,
    endIndex: number,
    targetSizes: Float32Array
): Promise<void> {
    try {
        const particleCount = endIndex - startIndex;

        // Create a staging buffer to read data from GPU
        const stagingBuffer = engine.device.createBuffer({
            label: "Particle Size Read Staging Buffer",
            size: particleCount * PARTICLE_SIZE_BYTES,
            usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
        });

        // Copy particle data from current buffer to staging buffer
        const commandEncoder = engine.device.createCommandEncoder({
            label: "Read Particle Sizes Encoder",
        });

        const sourceBuffer =
            engine.particleBuffers[engine.currentParticleBufferIndex];
        const sourceOffset = startIndex * PARTICLE_SIZE_BYTES;

        commandEncoder.copyBufferToBuffer(
            sourceBuffer,
            sourceOffset,
            stagingBuffer,
            0,
            particleCount * PARTICLE_SIZE_BYTES
        );

        engine.device.queue.submit([commandEncoder.finish()]);

        // Wait for GPU operations to complete and map the buffer
        await stagingBuffer.mapAsync(GPUMapMode.READ);
        const mappedData = new Float32Array(stagingBuffer.getMappedRange());

        // Extract particle sizes (size is at offset 5 in Float32Array view)
        for (let i = 0; i < particleCount; i++) {
            const particleOffset = i * (PARTICLE_SIZE_BYTES / 4); // Convert to Float32Array index
            const sizeIndex = particleOffset + 5; // Size is at position 5 (after pos, vel, type)
            targetSizes[i] = mappedData[sizeIndex];

            console.log(
                `📏 Particle ${startIndex + i}: actual size = ${targetSizes[
                    i
                ].toFixed(2)}px`
            );
        }

        // Clean up
        stagingBuffer.unmap();
        stagingBuffer.destroy();

        console.log(
            `✅ Successfully read ${particleCount} particle sizes from GPU buffer`
        );
    } catch (error) {
        console.error("💥 Error reading particle sizes from GPU:", error);

        // Fallback to approximation if GPU read fails
        const particleCount = endIndex - startIndex;
        for (let i = 0; i < particleCount; i++) {
            targetSizes[i] = PARTICLE_RENDER_SIZE;
        }
        console.log(
            `⚠️ Fallback: Using approximated particle sizes (${PARTICLE_RENDER_SIZE}px)`
        );
    }
}

/**
 * Starts shrink transition for particles being deactivated
 */
async function startParticleShrinkTransition(
    engine: ParticleLeniaEngine,
    newCount: number,
    oldCount: number
): Promise<void> {
    try {
        const particleCount = oldCount - newCount;
        if (particleCount <= 0) return;

        console.log(
            `🍂 Starting shrink transition for ${particleCount} particles (indices ${newCount} to ${
                oldCount - 1
            })`
        );

        // Read actual particle sizes from GPU buffer to use as starting sizes
        const targetSizes = new Float32Array(particleCount);

        // Read current particle sizes from the GPU buffer
        await readParticleSizesFromGPU(engine, newCount, oldCount, targetSizes);

        // Add transition to active list
        activeTransitions.push({
            startIndex: newCount,
            endIndex: oldCount,
            startTime: engine.currentTime,
            duration: TRANSITION_DURATION,
            type: "shrink",
            targetSizes, // This now contains actual starting sizes from GPU
        });

        console.log(
            `✅ Started shrink transition for ${particleCount} particles at time ${engine.currentTime.toFixed(
                3
            )}s`
        );
    } catch (error) {
        console.error("💥 Error starting particle shrink transition:", error);
    }
}

/**
 * Repositions particles off-screen after shrink transition is complete
 * This is done as a safety measure to ensure particles are truly inactive
 */
function repositionParticlesOffScreen(
    engine: ParticleLeniaEngine,
    transition: ParticleTransition
): void {
    const particleCount = transition.endIndex - transition.startIndex;

    console.log(
        `🔄 Repositioning ${particleCount} particles off-screen after shrink transition completion`
    );

    // Move particles off-screen as a safety measure
    for (let bufferIndex = 0; bufferIndex < 2; bufferIndex++) {
        for (let i = 0; i < particleCount; i++) {
            const particlePositionOffset =
                (transition.startIndex + i) * PARTICLE_SIZE_BYTES + 0; // Position at offset 0
            engine.device.queue.writeBuffer(
                engine.particleBuffers[bufferIndex],
                particlePositionOffset,
                new Float32Array([-10000.0, -10000.0]) // Move far off-screen
            );
        }
    }
}

// === Pressure-Based Particle Count Management ===

/**
 * Maps pressure value to particle count with safety validation
 * Ensures conservative scaling and proper workgroup alignment
 */
export function pressureToParticleCount(pressure: number): number {
    // Validate input
    if (typeof pressure !== "number" || isNaN(pressure) || pressure < 0) {
        console.warn(`⚠️ Invalid pressure value: ${pressure}, using default`);
        return DEFAULT_PARTICLES;
    }

    // Clamp pressure to safe range
    const clampedPressure = Math.max(0, Math.min(350, pressure));

    // Linear mapping from pressure to particle count
    // Pressure 0 → MIN_PARTICLES (1600)
    // Pressure 350 → MAX_PARTICLES (6400)
    const normalizedPressure = clampedPressure / 350.0;
    const particleRange = MAX_PARTICLES - MIN_PARTICLES;
    let targetCount = MIN_PARTICLES + normalizedPressure * particleRange;

    // Round to nearest multiple of 64 for optimal GPU workgroup dispatch
    targetCount = Math.round(targetCount / 64) * 64;

    // Final safety clamp
    targetCount = Math.max(MIN_PARTICLES, Math.min(MAX_PARTICLES, targetCount));

    console.log(`🔢 Pressure ${clampedPressure} → ${targetCount} particles`);
    return targetCount;
}

/**
 * Validates particle count change for safety
 * Prevents sudden large changes that could cause instability
 */
function validateParticleCountChange(
    currentCount: number,
    newCount: number
): number {
    const maxChangePercent = 0.5; // Maximum 50% change per operation
    const maxChange = currentCount * maxChangePercent;

    if (newCount > currentCount) {
        // Increasing particles - limit growth rate
        const actualIncrease = Math.min(newCount - currentCount, maxChange);
        const validatedCount = currentCount + actualIncrease;

        if (validatedCount < newCount) {
            console.log(
                `⚠️ Rate limited particle increase: ${currentCount} → ${validatedCount} (target: ${newCount})`
            );
        }

        return validatedCount;
    } else if (newCount < currentCount) {
        // Decreasing particles - limit reduction rate
        const actualDecrease = Math.min(currentCount - newCount, maxChange);
        const validatedCount = currentCount - actualDecrease;

        if (validatedCount > newCount) {
            console.log(
                `⚠️ Rate limited particle decrease: ${currentCount} → ${validatedCount} (target: ${newCount})`
            );
        }

        return validatedCount;
    }

    return newCount; // No change needed
}

/**
 * Safely updates the active particle count with transition effects
 * Handles both increases and decreases with proper GPU buffer management
 */
export function updateParticleCount(
    engine: ParticleLeniaEngine,
    newCount: number
): boolean {
    try {
        // Validate input
        if (!engine || typeof newCount !== "number" || isNaN(newCount)) {
            console.error("💥 Invalid parameters for updateParticleCount");
            return false;
        }

        // Ensure newCount is within safe bounds
        const clampedCount = Math.max(
            MIN_PARTICLES,
            Math.min(MAX_PARTICLES, newCount)
        );
        const currentCount = engine.simParams.numParticles;

        // Skip if no change needed
        if (clampedCount === currentCount) {
            return true;
        }

        // Apply rate limiting for safety
        const validatedCount = validateParticleCountChange(
            currentCount,
            clampedCount
        );

        console.log(
            `🔄 Updating particle count: ${currentCount} → ${validatedCount}`
        );

        if (validatedCount > currentCount) {
            // Increasing particles - update GPU buffer immediately and initialize new ones
            engine.simParams.numParticles = validatedCount;
            gpuParticleCount = validatedCount; // Track GPU count
            engine.device.queue.writeBuffer(
                engine.simParamsBuffer,
                PARAMETER_OFFSETS.numParticles, // 2 * 4 = 8 bytes offset
                new Uint32Array([validatedCount])
            );
            initializeNewParticlesWithTransition(
                engine,
                currentCount,
                validatedCount
            );
        } else if (validatedCount < currentCount) {
            // Decreasing particles - start shrink transition but KEEP original count in GPU for now
            engine.simParams.numParticles = validatedCount; // Update engine params
            // Keep original count in GPU so particles remain visible during shrink
            gpuParticleCount = currentCount; // Explicitly maintain higher count until transition completes
            console.log(
                `🍂 Starting particle decrease: engine=${validatedCount}, gpu=${gpuParticleCount} (keeping ${currentCount} visible during shrink)`
            );
            // Start async shrink transition with proper error handling
            startParticleShrinkTransition(
                engine,
                validatedCount,
                currentCount
            ).catch((error) => {
                console.error("💥 Error starting shrink transition:", error);
            });
        }

        console.log(
            `✅ Particle count updated successfully: ${validatedCount}`
        );
        return true;
    } catch (error) {
        console.error("💥 Error updating particle count:", error);

        // Emergency fallback to default count
        try {
            engine.simParams.numParticles = DEFAULT_PARTICLES;
            engine.device.queue.writeBuffer(
                engine.simParamsBuffer,
                PARAMETER_OFFSETS.numParticles,
                new Uint32Array([DEFAULT_PARTICLES])
            );
            console.log(
                `🚨 Emergency fallback to ${DEFAULT_PARTICLES} particles`
            );
        } catch (fallbackError) {
            console.error("💥💥 Critical error in fallback:", fallbackError);
        }

        return false;
    }
}

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
export const NUM_PARTICLES = 3200;
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
        numParticles: NUM_PARTICLES,
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
        NUM_PARTICLES,
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

    // Compute Pass
    const computePass = commandEncoder.beginComputePass({
        label: "Particle Compute Pass",
    });
    computePass.setPipeline(engine.computePipeline);
    computePass.setBindGroup(
        0,
        engine.computeBindGroups[engine.currentParticleBufferIndex]
    );
    const numWorkgroups = Math.ceil(NUM_PARTICLES / 64);
    computePass.dispatchWorkgroups(numWorkgroups);
    computePass.end();

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
    particlePass.draw(4, NUM_PARTICLES, 0, 0);
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
    const saturation = 66;
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

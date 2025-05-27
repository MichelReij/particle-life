// --- WebGPU Particle Life Example ---
// This example demonstrates a particle simulation with interactions governed by rules,
// inspired by Particle Life.
// It uses a compute shader to update particle states and a render pipeline to draw them.

import computeWGSL from "./shaders/compute.wgsl";
// Placeholder for new shaders, old ones will be replaced or updated.
import vertWGSL from "./shaders/vert.wgsl"; // Will need to be updated for particle rendering
import fragWGSL from "./shaders/frag.wgsl"; // Will need to be updated for particle rendering
import {
    Particle,
    InteractionRule,
    ParticleRules,
    SimulationParams,
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
document.body.appendChild(canvas);

canvas.width = 800;
canvas.height = 800;
canvas.style.width = "800px";
canvas.style.height = "800px";

// === Particle Life Configuration ===
const NUM_PARTICLES = 1024; // Number of particles
const NUM_TYPES = 11; // Number of particle types
const PARTICLE_RENDER_SIZE = 3.0;
const PARTICLE_SIZE_BYTES = 24;
const RULE_SIZE_BYTES = 16;
const SIM_PARAMS_SIZE_BYTES = 68; // Updated from 64 to 68 bytes

const VIRTUAL_WORLD_BORDER = 100; // 100px border on each side

let device: GPUDevice;
let presentationFormat: GPUTextureFormat;
let context: GPUCanvasContext;

// Buffers
let simParamsBuffer: GPUBuffer;
let rulesBuffer: GPUBuffer;
let particleBuffers: [GPUBuffer, GPUBuffer]; // Ping-pong buffers
let quadVertexBuffer: GPUBuffer; // For rendering particles as quads

// Pipelines and Bind Groups
let computePipeline: GPUComputePipeline;
let renderPipeline: GPURenderPipeline;
let computeBindGroups: [GPUBindGroup, GPUBindGroup];
let renderBindGroup: GPUBindGroup; // Might need more for particle rendering

let currentParticleBufferIndex = 0;
let animationId: number | undefined;

// FPS calculation variables
let frameCount = 0;
let lastFPSTime = 0;
let fpsDisplayElement: HTMLElement | null;

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
        alphaMode: "premultiplied", // Changed from 'opaque'
    });

    // Create Simulation Parameters
    const simParamsData = new ArrayBuffer(SIM_PARAMS_SIZE_BYTES);
    const simParamsViewF32 = new Float32Array(simParamsData);
    const simParamsViewU32 = new Uint32Array(simParamsData);

    const canvasRenderWidth = canvas.width;
    const canvasRenderHeight = canvas.height;
    const virtualWorldWidth = canvasRenderWidth + 2 * VIRTUAL_WORLD_BORDER;
    const virtualWorldHeight = canvasRenderHeight + 2 * VIRTUAL_WORLD_BORDER;
    const virtualWorldOffsetX = VIRTUAL_WORLD_BORDER;
    const virtualWorldOffsetY = VIRTUAL_WORLD_BORDER;

    // Order matches SimParams struct in WGSL (17 fields total for 68 bytes)
    simParamsViewF32[0] = 0.001; // delta_time (will be updated dynamically)
    simParamsViewF32[1] = 0.15; // friction
    simParamsViewU32[2] = NUM_PARTICLES;
    simParamsViewU32[3] = NUM_TYPES;
    simParamsViewF32[4] = virtualWorldWidth; // virtual_world_width
    simParamsViewF32[5] = virtualWorldHeight; // virtual_world_height
    simParamsViewF32[6] = canvasRenderWidth; // canvas_render_width
    simParamsViewF32[7] = canvasRenderHeight; // canvas_render_height
    simParamsViewF32[8] = virtualWorldOffsetX; // virtual_world_offset_x
    simParamsViewF32[9] = virtualWorldOffsetY; // virtual_world_offset_y
    simParamsViewU32[10] = 0; // boundary_mode (0 for disappear, 1 for wrap)
    simParamsViewF32[11] = PARTICLE_RENDER_SIZE; // particle_render_size
    simParamsViewF32[12] = 250.0; // force_scale
    simParamsViewF32[13] = 5.0; // r_smooth
    simParamsViewU32[14] = 0; // flat_force (0 for false, 1 for true)
    simParamsViewF32[15] = 0.0; // _padding0 (can be left as 0)
    simParamsViewF32[16] = 0.0; // _padding_final (can be left as 0)

    simParamsBuffer = device.createBuffer({
        label: "Simulation Parameters Buffer",
        size: SIM_PARAMS_SIZE_BYTES,
        usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
    });
    device.queue.writeBuffer(simParamsBuffer, 0, simParamsData);

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

    // Create Particle Buffers
    const initialParticleData = createInitialParticles(
        NUM_PARTICLES,
        NUM_TYPES,
        virtualWorldWidth, // Pass virtual dimensions for initial spawn
        virtualWorldHeight
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

    // --- Render Pipeline (Basic Placeholder for Particles) ---
    // This will need significant updates to render particles correctly.
    // For now, let's assume particles are rendered as small quads or points.
    // We'll need a vertex buffer for a unit quad/point.
    const unitQuad = new Float32Array([
        // x, y, u, v (example for textured quad, adapt for simple colored quad)
        -0.5, -0.5, 0, 0, 0.5, -0.5, 1, 0, -0.5, 0.5, 0, 1, 0.5, 0.5, 1, 1,
    ]);
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

    renderPipeline = device.createRenderPipeline({
        label: "Particle Render Pipeline",
        layout: "auto",
        vertex: {
            module: renderShaderModuleVert,
            entryPoint: "main", // Ensure your new vert.wgsl has a 'main' entry point
            buffers: [
                {
                    // Per-instance particle data (position, type)
                    arrayStride: PARTICLE_SIZE_BYTES, // Stride for each particle
                    stepMode: "instance",
                    attributes: [
                        { shaderLocation: 0, offset: 0, format: "float32x2" }, // Particle position (offset 0)
                        { shaderLocation: 1, offset: 8, format: "float32x2" }, // Particle velocity (offset 8)
                        { shaderLocation: 2, offset: 16, format: "uint32" }, // Particle type (offset 16)
                        // { shaderLocation: 3, offset: 20, format: "float32" },     // Particle size (offset 20) -- REVERTED
                    ],
                },
                {
                    // Per-vertex data for the quad
                    arrayStride: 2 * Float32Array.BYTES_PER_ELEMENT, // vec2f
                    stepMode: "vertex",
                    attributes: [
                        { shaderLocation: 3, offset: 0, format: "float32x2" }, // Quad vertex positions - REVERTED from 4
                    ],
                },
            ],
        },
        fragment: {
            module: renderShaderModuleFrag,
            entryPoint: "main", // Ensure your new frag.wgsl has a 'main' entry point
            targets: [
                {
                    format: presentationFormat,
                    // Basic blending for potential particle overlap
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
            topology: "triangle-strip", // Each quad is 2 triangles
            stripIndexFormat: undefined, // or 'uint16'/'uint32' if using an index buffer for the quad
        },
    });

    // Bind group for render pipeline (e.g., for sim_params if needed by shaders)
    // For now, render bind group might not be strictly needed if vert shader gets all from instance/vertex buffers
    // But if sim_params (like canvas size for normalization) is needed:
    renderBindGroup = device.createBindGroup({
        label: "Render Bind Group",
        layout: renderPipeline.getBindGroupLayout(0), // Assuming group 0 for uniforms
        entries: [
            { binding: 0, resource: { buffer: simParamsBuffer } },
            // Add particle type colors buffer here if needed
        ],
    });
}

let lastFrameTime = 0;
// const minFrameTime = 16; // Target ~60 FPS for simulation updates - we can remove this if we dynamically update delta_time

function frame(timestamp: number) {
    if (!device) {
        animationId = requestAnimationFrame(frame);
        return;
    }

    // Calculate deltaTime
    const deltaTime = (timestamp - lastFrameTime) / 1000; // Convert to seconds
    lastFrameTime = timestamp;

    // FPS calculation
    frameCount++;
    if (timestamp - lastFPSTime >= 1000) {
        // Update FPS every second
        if (fpsDisplayElement) {
            const fps = frameCount;
            fpsDisplayElement.textContent = `FPS: ${fps}`;
        }
        frameCount = 0;
        lastFPSTime = timestamp;
    }

    // Update delta_time in simParamsBuffer
    // The first element (offset 0) in simParamsViewF32 is delta_time
    device.queue.writeBuffer(simParamsBuffer, 0, new Float32Array([deltaTime]));

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

    // Render Pass
    const textureView = context.getCurrentTexture().createView();
    const renderPassDescriptor: GPURenderPassDescriptor = {
        label: "Particle Render Pass",
        colorAttachments: [
            {
                view: textureView,
                loadOp: "clear",
                clearValue: { r: 0.0, g: 0.0, b: 0.05, a: 1.0 }, // Dark background
                storeOp: "store",
            },
        ],
    };
    const renderPass = commandEncoder.beginRenderPass(renderPassDescriptor);
    renderPass.setPipeline(renderPipeline);
    renderPass.setVertexBuffer(0, particleBuffers[currentParticleBufferIndex]); // Particle instance data
    renderPass.setVertexBuffer(1, quadVertexBuffer); // Quad vertex data
    renderPass.setBindGroup(0, renderBindGroup); // Uniforms like sim_params
    // Draw NUM_PARTICLES instances, each instance is a quad (4 vertices)
    renderPass.draw(4, NUM_PARTICLES, 0, 0);
    renderPass.end();

    device.queue.submit([commandEncoder.finish()]);

    // Ping-pong buffers
    currentParticleBufferIndex = 1 - currentParticleBufferIndex;

    animationId = requestAnimationFrame(frame);
}

async function main() {
    try {
        await initWebGPU();
        fpsDisplayElement = document.getElementById("fpsDisplay"); // Get the FPS display element
        lastFPSTime = performance.now(); // Initialize lastFPSTime for FPS calculation
        lastFrameTime = performance.now(); // Initialize lastFrameTime for deltaTime calculation
        (window as any)[GLOBAL_KEY] = {
            // Store for cleanup
            device: device,
            canvas: canvas,
            cancelAnimation: () => {
                if (animationId) cancelAnimationFrame(animationId);
            },
        };
        animationId = requestAnimationFrame(frame);
    } catch (e) {
        console.error("Failed to initialize Particle Life:", e);
        const errorDiv = document.createElement("div");
        errorDiv.innerHTML = `<h2>Error initializing WebGPU Particle Life</h2><p>${
            (e as Error).message
        }</p><p>Please ensure your browser supports WebGPU and it's enabled. Check the console for more details.</p>`;
        document.body.appendChild(errorDiv);
    }
}

main();

// Add resize handling
window.addEventListener("resize", () => {
    if (device) {
        canvas.width = Math.min(window.innerWidth, 2048); // Cap max size
        canvas.height = Math.min(window.innerHeight, 2048);
        canvas.style.width = `${canvas.width}px`;
        canvas.style.height = `${canvas.height}px`;

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
    }
});

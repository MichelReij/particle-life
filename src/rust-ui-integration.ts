// rust-ui-integration.ts - Clean TypeScript integration layer for Rust WASM engine
// This file connects all existing HTML UI controls directly to the Rust API
// Eliminates TypeScript simulation logic, Rust handles everything

import { ParticleLifeEngine } from "../pkg/particle_life_wasm.js";

// Global state
let rustEngine: ParticleLifeEngine | null = null;
let canvas: HTMLCanvasElement | null = null;
let animationId: number | null = null;
let isRunning = false;

// UI control references
interface UIControls {
    // Main dashboard controls
    zoomSlider: HTMLInputElement;
    tempSlider: HTMLInputElement;
    uvSlider: HTMLInputElement;
    presSlider: HTMLInputElement;
    elecSlider: HTMLInputElement;

    // Physics controls
    driftSlider: HTMLInputElement;
    forceScaleSlider: HTMLInputElement;
    frictionSlider: HTMLInputElement;
    rSmoothSlider: HTMLInputElement;
    interTypeAttractionScaleSlider: HTMLInputElement;
    interTypeRadiusScaleSlider: HTMLInputElement;
    fisheyeStrengthSlider: HTMLInputElement;

    // Lenia controls
    leniaEnabledCheckbox: HTMLInputElement;
    leniaGrowthMuSlider: HTMLInputElement;
    leniaGrowthSigmaSlider: HTMLInputElement;
    leniaKernelRadiusSlider: HTMLInputElement;

    // Display elements
    zoomValue: HTMLSpanElement;
    tempValue: HTMLSpanElement;
    uvValue: HTMLSpanElement;
    presValue: HTMLSpanElement;
    elecValue: HTMLSpanElement;
    particleCount: HTMLSpanElement;
    driftValue: HTMLSpanElement;
    forceScaleValue: HTMLSpanElement;
    frictionValue: HTMLSpanElement;
    rSmoothValue: HTMLSpanElement;
    interTypeAttractionScaleValue: HTMLSpanElement;
    interTypeRadiusScaleValue: HTMLSpanElement;
    fisheyeStrengthValue: HTMLSpanElement;
    leniaEnabledStatus: HTMLSpanElement;
    leniaGrowthMuValue: HTMLSpanElement;
    leniaGrowthSigmaValue: HTMLSpanElement;
    leniaKernelRadiusValue: HTMLSpanElement;
    fpsDisplay: HTMLElement;
    zoomCenterInfo: HTMLElement;
}

let uiControls: UIControls | null = null;

// FPS tracking
let lastFrameTime = 0;
let frameCount = 0;
let fpsUpdateTime = 0;

/**
 * Update debug status display
 */
function updateDebugStatus(message: string): void {
    const statusElement = document.getElementById("rustEngineInfo");
    if (statusElement) {
        statusElement.textContent = message;
    }
}

/**
 * Initialize the Rust engine and connect all UI controls
 */
export async function initializeRustIntegration(): Promise<boolean> {
    try {
        console.log("🚀 Initializing Rust WASM engine with UI integration...");
        updateDebugStatus("Creating Rust engine...");

        // With bundler target, WASM initialization is automatic
        // Create Rust engine instance
        rustEngine = new ParticleLifeEngine();
        console.log("✅ Rust engine created");
        updateDebugStatus("Engine created, setting up canvas...");

        // Set up canvas
        setupCanvas();
        updateDebugStatus("Canvas ready, connecting UI...");

        // Connect all UI controls
        connectUIControls();
        updateDebugStatus("UI connected, initializing values...");

        // Initialize with default values
        initializeDefaultValues();
        updateDebugStatus("Values initialized, starting animation...");

        // Start animation loop
        startAnimationLoop();
        updateDebugStatus("✅ Running!");

        console.log("🎉 Rust integration initialized successfully!");
        return true;
    } catch (error) {
        console.error("❌ Failed to initialize Rust integration:", error);
        updateDebugStatus(`❌ Error: ${error}`);
        return false;
    }
}

/**
 * Set up canvas and prepare for rendering
 */
function setupCanvas(): void {
    const canvasContainer = document.getElementById("canvasContainer");
    if (!canvasContainer) {
        throw new Error("Canvas container not found");
    }

    // Create canvas element
    canvas = document.createElement("canvas");
    canvas.id = "particleCanvas";
    canvas.width = 800;
    canvas.height = 800;
    canvas.style.width = "800px";
    canvas.style.height = "800px";
    canvas.style.border = "1px solid #333";
    canvas.style.background = "#000";

    // Clear container and add canvas
    canvasContainer.innerHTML = "";
    canvasContainer.appendChild(canvas);

    console.log("✅ Canvas set up successfully");
}

/**
 * Connect all HTML UI controls to Rust engine methods
 */
function connectUIControls(): void {
    // Get all UI control references
    uiControls = {
        // Main dashboard controls
        zoomSlider: getElement("zoomSlider") as HTMLInputElement,
        tempSlider: getElement("tempSlider") as HTMLInputElement,
        uvSlider: getElement("uvSlider") as HTMLInputElement,
        presSlider: getElement("presSlider") as HTMLInputElement,
        elecSlider: getElement("elecSlider") as HTMLInputElement,

        // Physics controls
        driftSlider: getElement("driftSlider") as HTMLInputElement,
        forceScaleSlider: getElement("forceScaleSlider") as HTMLInputElement,
        frictionSlider: getElement("frictionSlider") as HTMLInputElement,
        rSmoothSlider: getElement("rSmoothSlider") as HTMLInputElement,
        interTypeAttractionScaleSlider: getElement(
            "interTypeAttractionScaleSlider"
        ) as HTMLInputElement,
        interTypeRadiusScaleSlider: getElement(
            "interTypeRadiusScaleSlider"
        ) as HTMLInputElement,
        fisheyeStrengthSlider: getElement(
            "fisheyeStrengthSlider"
        ) as HTMLInputElement,

        // Lenia controls
        leniaEnabledCheckbox: getElement(
            "leniaEnabledCheckbox"
        ) as HTMLInputElement,
        leniaGrowthMuSlider: getElement(
            "leniaGrowthMuSlider"
        ) as HTMLInputElement,
        leniaGrowthSigmaSlider: getElement(
            "leniaGrowthSigmaSlider"
        ) as HTMLInputElement,
        leniaKernelRadiusSlider: getElement(
            "leniaKernelRadiusSlider"
        ) as HTMLInputElement,

        // Display elements
        zoomValue: getElement("zoomValue") as HTMLSpanElement,
        tempValue: getElement("tempValue") as HTMLSpanElement,
        uvValue: getElement("uvValue") as HTMLSpanElement,
        presValue: getElement("presValue") as HTMLSpanElement,
        elecValue: getElement("elecValue") as HTMLSpanElement,
        particleCount: getElement("particleCount") as HTMLSpanElement,
        driftValue: getElement("driftValue") as HTMLSpanElement,
        forceScaleValue: getElement("forceScaleValue") as HTMLSpanElement,
        frictionValue: getElement("frictionValue") as HTMLSpanElement,
        rSmoothValue: getElement("rSmoothValue") as HTMLSpanElement,
        interTypeAttractionScaleValue: getElement(
            "interTypeAttractionScaleValue"
        ) as HTMLSpanElement,
        interTypeRadiusScaleValue: getElement(
            "interTypeRadiusScaleValue"
        ) as HTMLSpanElement,
        fisheyeStrengthValue: getElement(
            "fisheyeStrengthValue"
        ) as HTMLSpanElement,
        leniaEnabledStatus: getElement("leniaEnabledStatus") as HTMLSpanElement,
        leniaGrowthMuValue: getElement("leniaGrowthMuValue") as HTMLSpanElement,
        leniaGrowthSigmaValue: getElement(
            "leniaGrowthSigmaValue"
        ) as HTMLSpanElement,
        leniaKernelRadiusValue: getElement(
            "leniaKernelRadiusValue"
        ) as HTMLSpanElement,
        fpsDisplay: getElement("fpsDisplay"),
        zoomCenterInfo: getElement("zoomCenterInfo"),
    };

    // Connect event listeners - these directly call Rust API methods
    setupMainDashboardControls();
    setupPhysicsControls();
    setupLeniaControls();

    console.log("✅ All UI controls connected to Rust engine");
}

/**
 * Set up main dashboard controls (zoom, temp, UV, pressure, electrical)
 */
function setupMainDashboardControls(): void {
    if (!uiControls || !rustEngine) return;

    // Zoom control - maps to world scale/viewport
    uiControls.zoomSlider.addEventListener("input", (e) => {
        const value = parseFloat((e.target as HTMLInputElement).value);
        uiControls!.zoomValue.textContent = value.toFixed(2);
        // Zoom affects the virtual world size
        rustEngine!.update_parameter("virtual_world_width", 2400 / value);
        rustEngine!.update_parameter("virtual_world_height", 2400 / value);
        console.log(`🔍 Zoom: ${value}`);
    });

    // Temperature control - maps to some thermal parameter
    uiControls.tempSlider.addEventListener("input", (e) => {
        const value = parseInt((e.target as HTMLInputElement).value);
        uiControls!.tempValue.textContent = `${value}°C`;
        // Temperature could affect friction or force scale
        const thermalFactor = value / 20.0; // Normalize around 20°C
        rustEngine!.update_parameter("force_scale", 400 * thermalFactor);
        console.log(
            `🌡️ Temperature: ${value}°C → force_scale: ${400 * thermalFactor}`
        );
    });

    // UV light control - could affect lightning or energy
    uiControls.uvSlider.addEventListener("input", (e) => {
        const value = parseInt((e.target as HTMLInputElement).value);
        uiControls!.uvValue.textContent = value.toString();
        // UV affects lightning frequency
        rustEngine!.update_parameter("lightning_frequency", value / 10.0);
        console.log(
            `☀️ UV Light: ${value} → lightning_frequency: ${value / 10.0}`
        );
    });

    // Pressure control (affects particle count)
    uiControls.presSlider.addEventListener("input", (e) => {
        const value = parseInt((e.target as HTMLInputElement).value);
        uiControls!.presValue.textContent = value.toString();

        // Convert pressure to particle count using Rust engine method
        const particleCount = rustEngine!.pressure_to_particle_count(value);
        const success = rustEngine!.set_particle_count(particleCount);

        if (success) {
            uiControls!.particleCount.textContent = particleCount.toString();
            console.log(`🌪️ Pressure: ${value} → ${particleCount} particles`);
        } else {
            console.warn(`⚠️ Failed to set particle count to ${particleCount}`);
        }
    });

    // Electrical activity control - maps to lightning intensity
    uiControls.elecSlider.addEventListener("input", (e) => {
        const value = parseFloat((e.target as HTMLInputElement).value);
        uiControls!.elecValue.textContent = value.toFixed(2);
        rustEngine!.update_parameter("lightning_intensity", value);
        console.log(`⚡ Electrical Activity: ${value}`);
    });
}

/**
 * Set up physics controls (drift, force, friction, etc.)
 */
function setupPhysicsControls(): void {
    if (!uiControls || !rustEngine) return;

    // Drift speed control
    uiControls.driftSlider.addEventListener("input", (e) => {
        const value = parseInt((e.target as HTMLInputElement).value);
        uiControls!.driftValue.textContent = value.toString();
        rustEngine!.update_parameter("drift_x_per_second", value);
        console.log(`🌊 Drift Speed: ${value} px/s`);
    });

    // Force scale control
    uiControls.forceScaleSlider.addEventListener("input", (e) => {
        const value = parseInt((e.target as HTMLInputElement).value);
        uiControls!.forceScaleValue.textContent = value.toString();
        rustEngine!.update_parameter("force_scale", value);
        console.log(`💪 Force Scale: ${value}`);
    });

    // Friction control
    uiControls.frictionSlider.addEventListener("input", (e) => {
        const value = parseFloat((e.target as HTMLInputElement).value);
        uiControls!.frictionValue.textContent = value.toFixed(2);
        rustEngine!.update_parameter("friction", value);
        console.log(`🛑 Friction: ${value}`);
    });

    // R Smooth control
    uiControls.rSmoothSlider.addEventListener("input", (e) => {
        const value = parseFloat((e.target as HTMLInputElement).value);
        uiControls!.rSmoothValue.textContent = value.toFixed(1);
        rustEngine!.update_parameter("r_smooth", value);
        console.log(`📐 R Smooth: ${value}`);
    });

    // Inter-type attraction scale control
    uiControls.interTypeAttractionScaleSlider.addEventListener("input", (e) => {
        const value = parseFloat((e.target as HTMLInputElement).value);
        uiControls!.interTypeAttractionScaleValue.textContent =
            value.toFixed(1);
        rustEngine!.update_parameter("inter_type_attraction_scale", value);
        console.log(`🧲 Inter-Type Attraction Scale: ${value}`);
    });

    // Inter-type radius scale control
    uiControls.interTypeRadiusScaleSlider.addEventListener("input", (e) => {
        const value = parseFloat((e.target as HTMLInputElement).value);
        uiControls!.interTypeRadiusScaleValue.textContent = value.toFixed(2);
        rustEngine!.update_parameter("inter_type_radius_scale", value);
        console.log(`📏 Inter-Type Radius Scale: ${value}`);
    });

    // Fisheye strength control
    uiControls.fisheyeStrengthSlider.addEventListener("input", (e) => {
        const value = parseFloat((e.target as HTMLInputElement).value);
        uiControls!.fisheyeStrengthValue.textContent = value.toFixed(1);
        rustEngine!.update_parameter("fisheye_strength", value);
        console.log(`🐠 Fisheye Strength: ${value}`);
    });
}

/**
 * Set up Lenia-specific controls
 */
function setupLeniaControls(): void {
    if (!uiControls || !rustEngine) return;

    // Lenia enabled checkbox
    uiControls.leniaEnabledCheckbox.addEventListener("change", (e) => {
        const enabled = (e.target as HTMLInputElement).checked;
        uiControls!.leniaEnabledStatus.textContent = enabled ? "On" : "Off";
        rustEngine!.update_parameter("lenia_enabled", enabled ? 1 : 0);
        console.log(`🧬 Lenia: ${enabled ? "Enabled" : "Disabled"}`);
    });

    // Lenia growth μ control
    uiControls.leniaGrowthMuSlider.addEventListener("input", (e) => {
        const value = parseFloat((e.target as HTMLInputElement).value);
        uiControls!.leniaGrowthMuValue.textContent = value.toFixed(2);
        rustEngine!.update_parameter("lenia_growth_mu", value);
        console.log(`μ Lenia Growth μ: ${value}`);
    });

    // Lenia growth σ control
    uiControls.leniaGrowthSigmaSlider.addEventListener("input", (e) => {
        const value = parseFloat((e.target as HTMLInputElement).value);
        uiControls!.leniaGrowthSigmaValue.textContent = value.toFixed(3);
        rustEngine!.update_parameter("lenia_growth_sigma", value);
        console.log(`σ Lenia Growth σ: ${value}`);
    });

    // Lenia kernel radius control
    uiControls.leniaKernelRadiusSlider.addEventListener("input", (e) => {
        const value = parseFloat((e.target as HTMLInputElement).value);
        uiControls!.leniaKernelRadiusValue.textContent = value.toFixed(1);
        rustEngine!.update_parameter("lenia_kernel_radius", value);
        console.log(`🔵 Lenia Kernel Radius: ${value}`);
    });
}

/**
 * Initialize UI controls with their default values
 */
function initializeDefaultValues(): void {
    if (!uiControls || !rustEngine) return;

    // Set Rust engine to match HTML default values using update_parameter
    rustEngine.update_parameter(
        "virtual_world_width",
        2400 / parseFloat(uiControls.zoomSlider.value)
    );
    rustEngine.update_parameter(
        "virtual_world_height",
        2400 / parseFloat(uiControls.zoomSlider.value)
    );

    // Temperature affects force scale
    const temp = parseInt(uiControls.tempSlider.value);
    const thermalFactor = temp / 20.0;
    rustEngine.update_parameter("force_scale", 400 * thermalFactor);

    // UV affects lightning
    rustEngine.update_parameter(
        "lightning_frequency",
        parseInt(uiControls.uvSlider.value) / 10.0
    );

    // Set initial particle count from pressure using Rust method
    const pressure = parseInt(uiControls.presSlider.value);
    const particleCount = rustEngine.pressure_to_particle_count(pressure);
    rustEngine.set_particle_count(particleCount);
    uiControls.particleCount.textContent = particleCount.toString();

    // Electrical activity maps to lightning intensity
    rustEngine.update_parameter(
        "lightning_intensity",
        parseFloat(uiControls.elecSlider.value)
    );

    // Physics parameters
    rustEngine.update_parameter(
        "drift_x_per_second",
        parseInt(uiControls.driftSlider.value)
    );
    rustEngine.update_parameter(
        "force_scale",
        parseInt(uiControls.forceScaleSlider.value)
    );
    rustEngine.update_parameter(
        "friction",
        parseFloat(uiControls.frictionSlider.value)
    );
    rustEngine.update_parameter(
        "r_smooth",
        parseFloat(uiControls.rSmoothSlider.value)
    );
    rustEngine.update_parameter(
        "inter_type_attraction_scale",
        parseFloat(uiControls.interTypeAttractionScaleSlider.value)
    );
    rustEngine.update_parameter(
        "inter_type_radius_scale",
        parseFloat(uiControls.interTypeRadiusScaleSlider.value)
    );
    rustEngine.update_parameter(
        "fisheye_strength",
        parseFloat(uiControls.fisheyeStrengthSlider.value)
    );

    // Lenia controls
    rustEngine.update_parameter(
        "lenia_enabled",
        uiControls.leniaEnabledCheckbox.checked ? 1 : 0
    );
    rustEngine.update_parameter(
        "lenia_growth_mu",
        parseFloat(uiControls.leniaGrowthMuSlider.value)
    );
    rustEngine.update_parameter(
        "lenia_growth_sigma",
        parseFloat(uiControls.leniaGrowthSigmaSlider.value)
    );
    rustEngine.update_parameter(
        "lenia_kernel_radius",
        parseFloat(uiControls.leniaKernelRadiusSlider.value)
    );

    console.log("✅ Default values initialized in Rust engine");
}

/**
 * Main animation loop - calls Rust engine update and handles rendering
 */
function animationLoop(currentTime: number): void {
    if (!isRunning || !rustEngine || !canvas) return;

    // Calculate delta time in seconds
    const deltaTime = (currentTime - lastFrameTime) / 1000.0;
    lastFrameTime = currentTime;

    // Update FPS counter and debug info
    frameCount++;
    if (currentTime - fpsUpdateTime >= 1000) {
        const fps = Math.round(
            frameCount / ((currentTime - fpsUpdateTime) / 1000)
        );
        if (uiControls?.fpsDisplay) {
            uiControls.fpsDisplay.textContent = `FPS: ${fps}`;
        }

        // Update debug info
        const debugInfo = document.getElementById("rustEngineInfo");
        if (debugInfo && rustEngine) {
            const particleCount = rustEngine.get_particle_count();
            debugInfo.innerHTML = `
                Particles: ${particleCount}<br>
                FPS: ${fps}<br>
                Debug: ${rustEngine.get_debug_info()}
            `;
        }

        frameCount = 0;
        fpsUpdateTime = currentTime;
    }

    // Update simulation in Rust (pass delta time)
    rustEngine.update_frame(deltaTime);

    // Get particle buffer from Rust and convert to Float32Array
    const particleBufferUint8 = rustEngine.get_particle_buffer();
    const particleBuffer = new Float32Array(particleBufferUint8.buffer);

    // Render particles to canvas
    renderParticles(particleBuffer);

    // Continue animation loop
    animationId = requestAnimationFrame(animationLoop);
}

/**
 * Start the animation loop
 */
function startAnimationLoop(): void {
    if (isRunning) return;

    isRunning = true;
    lastFrameTime = performance.now();
    fpsUpdateTime = lastFrameTime;
    frameCount = 0;

    animationId = requestAnimationFrame(animationLoop);
    console.log("🎬 Animation loop started");
}

/**
 * Stop the animation loop
 */
export function stopAnimationLoop(): void {
    isRunning = false;
    if (animationId) {
        cancelAnimationFrame(animationId);
        animationId = null;
    }
    console.log("⏹️ Animation loop stopped");
}

/**
 * Render particles to canvas using WebGPU or Canvas 2D fallback
 */
function renderParticles(particleBuffer: Float32Array): void {
    if (!canvas) return;

    // For now, use Canvas 2D as a simple renderer
    // TODO: Replace with WebGPU rendering for better performance
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    // Clear canvas
    ctx.fillStyle = "rgba(0, 0, 0, 0.1)";
    ctx.fillRect(0, 0, canvas.width, canvas.height);

    // Render particles
    const particleCount = particleBuffer.length / 8; // 8 floats per particle (x, y, vx, vy, type, r, g, b)

    for (let i = 0; i < particleCount; i++) {
        const offset = i * 8;
        const x = particleBuffer[offset];
        const y = particleBuffer[offset + 1];
        const r = Math.floor(particleBuffer[offset + 5] * 255);
        const g = Math.floor(particleBuffer[offset + 6] * 255);
        const b = Math.floor(particleBuffer[offset + 7] * 255);

        ctx.fillStyle = `rgb(${r}, ${g}, ${b})`;
        ctx.beginPath();
        ctx.arc(x, y, 2, 0, 2 * Math.PI);
        ctx.fill();
    }
}

/**
 * Utility function to get DOM element with error handling
 */
function getElement(id: string): HTMLElement {
    const element = document.getElementById(id);
    if (!element) {
        throw new Error(`Element with id '${id}' not found`);
    }
    return element;
}

/**
 * Clean up resources
 */
export function cleanup(): void {
    stopAnimationLoop();
    rustEngine = null;
    uiControls = null;
    canvas = null;
    console.log("🧹 Rust integration cleaned up");
}

// Auto-initialize when DOM is loaded
document.addEventListener("DOMContentLoaded", () => {
    initializeRustIntegration().then((success) => {
        if (success) {
            console.log("🎉 Rust UI integration ready!");
        } else {
            console.error("❌ Failed to initialize Rust UI integration");
        }
    });
});

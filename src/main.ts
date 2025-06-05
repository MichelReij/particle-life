// main.ts - Application Orchestrator
// This file coordinates the particle simulation engine and UI controls

import {
    ParticleLeniaEngine,
    initializeParticleLeniaEngine,
    renderFrame,
    startAnimation,
    updateSimulationParameterAuto,
    updateBackgroundColorAndDrift,
    updateZoom,
    pressureToParticleCount,
    updateParticleCount,
} from "./particle-lenia";

import { SimulationParams } from "./particle-life-types";

import {
    setupCanvas,
    cleanup,
    initializeUI,
    setParameterUpdateCallbacks,
    updateFPS,
} from "./ui";

// === Global State ===
let engine: ParticleLeniaEngine | null = null;
let canvas: HTMLCanvasElement;

// === Initialization ===
async function initializeApplication(): Promise<void> {
    console.log("Initializing Particle Life application...");

    try {
        // Clean up any previous instances
        cleanup();

        // Set up canvas
        canvas = setupCanvas();

        // Initialize the particle simulation engine
        engine = await initializeParticleLeniaEngine(canvas);

        // Set up UI parameter update callbacks
        setParameterUpdateCallbacks({
            updateDriftAndBackground: (value: number) => {
                if (engine) {
                    updateBackgroundColorAndDrift(engine, value);
                }
            },
            updateSimulationParameter: (paramName: string, value: number) => {
                if (engine) {
                    updateSimulationParameterAuto(
                        engine,
                        paramName as keyof SimulationParams,
                        value
                    );
                }
            },
            updateZoom: (level: number, centerX?: number, centerY?: number) => {
                if (engine) {
                    updateZoom(engine, level, centerX, centerY);
                }
            },
            updateParticleCount: (pressure: number) => {
                if (engine) {
                    const targetParticleCount =
                        pressureToParticleCount(pressure);
                    const currentParticleCount = engine.simParams.numParticles;

                    console.log(
                        `🎯 updateParticleCount callback: pressure=${pressure} → target=${targetParticleCount}, current=${currentParticleCount}`
                    );

                    if (targetParticleCount > currentParticleCount) {
                        console.log(
                            `📈 GROW: ${currentParticleCount} → ${targetParticleCount}`
                        );
                    } else if (targetParticleCount < currentParticleCount) {
                        console.log(
                            `📉 SHRINK: ${currentParticleCount} → ${targetParticleCount}`
                        );
                    } else {
                        console.log(`➡️ NO CHANGE: ${currentParticleCount}`);
                    }

                    const success = updateParticleCount(
                        engine,
                        targetParticleCount
                    );
                    if (!success) {
                        console.error(
                            "💥 Failed to update particle count for pressure:",
                            pressure
                        );
                    }
                }
            },
        });

        // Initialize UI with current simulation parameters
        initializeUI(engine.simParams, engine.currentZoomLevel);

        // Start the animation loop
        startAnimation(engine);

        console.log("Application initialized successfully");
    } catch (error) {
        console.error("Failed to initialize application:", error);
        throw error;
    }
}

// === Main Entry Point ===
async function main(): Promise<void> {
    try {
        await initializeApplication();
    } catch (error) {
        console.error("Application startup failed:", error);

        // Show error message to user
        const canvasContainer = document.getElementById("canvasContainer");
        if (canvasContainer) {
            canvasContainer.innerHTML = `
                <div style="color: red; text-align: center; padding: 20px; border: 1px solid red; border-radius: 5px;">
                    <h3>Failed to Initialize WebGPU</h3>
                    <p>Your browser may not support WebGPU, or there was an error initializing the graphics context.</p>
                    <p>Error: ${
                        error instanceof Error ? error.message : String(error)
                    }</p>
                </div>
            `;
        }
    }
}

// === Resize Handling ===
window.addEventListener("resize", () => {
    if (engine && canvas) {
        // Update canvas size
        canvas.width = 800;
        canvas.height = 800;
        canvas.style.width = "800px";
        canvas.style.height = "800px";

        // Reconfigure WebGPU context
        engine.context.configure({
            device: engine.device,
            format: engine.presentationFormat,
            alphaMode: "premultiplied",
        });

        // Update simulation parameters for new canvas size
        // The engine handles this internally
    }
});

// === Cleanup on Page Unload ===
window.addEventListener("beforeunload", () => {
    cleanup();
});

// === Start Application ===
main();

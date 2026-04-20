console.log("🚀 main.ts loading with proper WASM integration...");

// Import WASM module using the standard wasm-bindgen approach
import init, { ParticleLifeEngine } from "./pkg/particle_life_wasm";
import {
    setParameterUpdateCallbacks,
    initializeUI,
    initColorPanel,
    updateJoystickPan,
} from "./ui";
import { SimulationParams } from "./particle-life-types";
import {
    CANVAS_WIDTH,
    CANVAS_HEIGHT,
    VIRTUAL_WORLD_WIDTH,
    VIRTUAL_WORLD_HEIGHT,
    PARTICLE_SIZE,
} from "./config";

class App {
    private engine: ParticleLifeEngine | null = null;
    private canvas: HTMLCanvasElement | null = null;
    private animationId: number | null = null;
    private frameCount = 0;
    private lastTime = 0;
    private fpsFrameCount = 0;
    private fpsLastTime = 0;
    private currentFPS = 0;
    private pendingScreenshot = false;
    private mediaRecorder: MediaRecorder | null = null;
    private recordedChunks: Blob[] = [];
    private isRecording = false;
    private isAudioOn = false;
    // Guard against wasm-bindgen re-entrancy: async check_super_lightning holds a
    // &mut self borrow for the GPU readback; all other &mut self calls must be
    // deferred until it completes.
    private engineBusy = false;

    async init() {
        console.log("🔧 App.init() called");

        // Add status message to DOM
        this.updateStatus("Initializing application...");

        try {
            // Get canvas first
            this.canvas = document.getElementById(
                "canvas",
            ) as HTMLCanvasElement;
            if (!this.canvas) {
                console.error("❌ Canvas element not found");
                this.updateStatus("Error: Canvas not found");
                return;
            }

            // Set canvas size from config
            this.canvas.width = CANVAS_WIDTH;
            this.canvas.height = CANVAS_HEIGHT;

            console.log(
                "🎨 Canvas configured:",
                this.canvas.width,
                "x",
                this.canvas.height,
            );
            this.updateStatus("Canvas initialized...");

            // Initialize WASM module
            console.log("🦀 Initializing WASM module...");
            this.updateStatus("Loading WASM module...");

            // Resolve the WASM path relative to the current page URL so it works
            // regardless of subdirectory deployment.
            // Cache-bust zodat de browser altijd de nieuwste WASM laadt
            const wasmUrl = new URL(`pkg/particle_life_wasm_bg.wasm?v=${Date.now()}`, window.location.href).toString();
            let initSuccess = false;
            try {
                console.log(`🔍 Loading WASM from: ${wasmUrl}`);
                await init({ module_or_path: wasmUrl });
                console.log(`✅ WASM initialized successfully`);
                initSuccess = true;
            } catch (error) {
                console.warn(`⚠️ WASM init failed:`, error);
            }

            if (!initSuccess) {
                throw new Error("Failed to initialize WASM with any path");
            }

            this.updateStatus("WASM module loaded successfully!");

            // Create Rust engine
            console.log("🚀 Creating ParticleLifeEngine...");
            this.updateStatus("Creating Rust engine...");
            this.engine = new ParticleLifeEngine();
            console.log("✅ ParticleLifeEngine created successfully");
            console.log("Engine details:", this.engine.get_debug_info());
            this.updateStatus("Rust engine created successfully!");

            // Set up WebGPU (if available)
            this.updateStatus("Initializing WebGPU...");
            try {
                await this.engine.initialize_webgpu(this.canvas);
                console.log("✅ WebGPU initialized");
                this.updateStatus("WebGPU initialized successfully!");
            } catch (error) {
                console.warn(
                    "⚠️ WebGPU initialization failed, using fallback:",
                    error,
                );
                this.updateStatus("WebGPU failed, using CPU fallback");
            }

            // Wire up UI controls
            this.wireUpControls();
            this.updateStatus("UI controls wired up");

            // Start the real Rust simulation
            this.updateStatus("Starting particle simulation...");
            this.startRustSimulation();
            // Remove status message after simulation starts successfully
            setTimeout(() => {
                const statusElement = document.getElementById("status-message");
                if (statusElement) {
                    statusElement.style.display = "none";
                }
            }, 2000); // Hide after 2 seconds
        } catch (error) {
            console.error("💥 Failed to initialize WASM:", error);
            if (error instanceof Error) {
                console.error("Error details:", error.message, error.stack);
                this.updateStatus(`Error: ${error.message}`);
            } else {
                this.updateStatus("Unknown error occurred");
            }
            this.fallbackAnimation();
        }
    }

    private updateStatus(message: string) {
        // Try to find a status element, create one if it doesn't exist
        let statusElement = document.getElementById("status-message");
        if (!statusElement) {
            statusElement = document.createElement("div");
            statusElement.id = "status-message";
            statusElement.style.cssText = `
                position: fixed;
                top: 10px;
                right: 10px;
                background: rgba(0, 0, 0, 0.8);
                color: #00ff00;
                padding: 10px;
                border-radius: 5px;
                font-family: monospace;
                font-size: 12px;
                z-index: 1000;
                max-width: 300px;
            `;
            document.body.appendChild(statusElement);
        }
        statusElement.textContent = message;
        console.log("📍 Status:", message);
    }

    private wireUpControls() {
        if (!this.engine) return;

        console.log("🔌 Wiring up UI controls...");

        // Set up UI parameter update callbacks
        setParameterUpdateCallbacks({
            updateDriftAndBackground: (value: number) => {
                // Update drift parameter in the engine
                if (this.engine && !this.engineBusy) {
                    this.engine.update_parameter("driftXPerSecond", value);
                }
            },
            updateBackgroundColor: (r: number, g: number, b: number) => {
                // Update background color parameters in the engine
                if (this.engine && !this.engineBusy) {
                    this.engine.update_background_color(r, g, b);
                }
            },
            updateBackgroundColorFromTemperature: (temp: number) => {
                // Update all temperature-related parameters using comprehensive method
                if (this.engine && !this.engineBusy) {
                    this.engine.set_temperature(temp);
                }
            },
            updateSimulationParameter: (paramName: string, value: number) => {
                // Update any simulation parameter
                if (this.engine && !this.engineBusy) {
                    this.engine.update_parameter(paramName, value);
                }
            },
            updateBooleanParameter: (paramName: string, value: boolean) => {
                // Update any boolean simulation parameter
                if (this.engine && !this.engineBusy) {
                    this.engine.update_boolean_parameter(paramName, value);
                }
            },
            getParameter: (paramName: string) => {
                // Get any simulation parameter value
                if (!this.engine || this.engineBusy) return 0;

                switch (paramName) {
                    case "driftXPerSecond":
                        return this.engine.get_drift_x_per_second();
                    case "friction":
                        return this.engine.get_friction();
                    case "forceScale":
                        return this.engine.get_force_scale();
                    case "rSmooth":
                        return this.engine.get_r_smooth();
                    case "interTypeAttractionScale":
                        return this.engine.get_inter_type_attraction_scale();
                    case "interTypeRadiusScale":
                        return this.engine.get_inter_type_radius_scale();
                    case "fisheyeStrength":
                        return this.engine.get_fisheye_strength();
                    case "leniaGrowthMu":
                        return this.engine.get_lenia_growth_mu();
                    case "leniaGrowthSigma":
                        return this.engine.get_lenia_growth_sigma();
                    case "leniaKernelRadius":
                        return this.engine.get_lenia_kernel_radius();
                    case "lightningFrequency":
                        return this.engine.get_lightning_frequency();
                    case "lightningIntensity":
                        return this.engine.get_lightning_intensity();
                    case "lightningDuration":
                        return this.engine.get_lightning_duration();
                    case "particleRenderSize":
                        return this.engine.get_particle_render_size();
                    default:
                        console.warn(`Unknown parameter: ${paramName}`);
                        return 0;
                }
            },
            getBooleanParameter: (paramName: string) => {
                // Get any boolean simulation parameter value
                if (!this.engine || this.engineBusy) return false;

                switch (paramName) {
                    case "flatForce":
                        return this.engine.get_flat_force();
                    case "leniaEnabled":
                        return this.engine.get_lenia_enabled();
                    default:
                        console.warn(`Unknown boolean parameter: ${paramName}`);
                        return false;
                }
            },
            updateZoom: (level: number, centerX?: number, centerY?: number) => {
                if (this.engine && !this.engineBusy) {
                    this.engine.set_zoom(level, centerX, centerY);
                }
            },
            updateParticleCount: (pressure: number) => {
                // Update particle count based on pressure
                if (this.engine && !this.engineBusy) {
                    this.engine.set_particle_count_from_pressure(pressure);
                }
            },
            // Comprehensive parameter methods that handle all effects internally in Rust
            setTemperature: (temp: number) => {
                if (this.engine && !this.engineBusy) {
                    this.engine.set_temperature(temp);
                    // Update rule-evolution speed based on temperature
                    // 3°C → 1800s (30 min), 160°C → 180s (3 min), linear
                    const duration = 1800 - (1620 * (temp - 3)) / 157;
                    this.engine.set_rules_lerp_duration(Math.round(duration));
                    this.currentTemperature = temp;
                }
            },
            setPressure: (pressure: number) => {
                if (this.engine && !this.engineBusy) {
                    this.engine.set_pressure(pressure);
                    // Also update particle count
                    this.engine.set_particle_count_from_pressure(pressure);
                }
            },
            setPH: (ph: number) => {
                if (this.engine && !this.engineBusy) {
                    this.engine.set_ph(ph);
                }
            },
            setElectricalActivity: (electrical: number) => {
                if (this.engine && !this.engineBusy) {
                    this.engine.set_electrical_activity(electrical);
                }
            },
            setParticleOpacity: (opacity: number) => {
                if (this.engine && !this.engineBusy) {
                    this.engine.set_particle_opacity(opacity);
                }
            },
            setTypeColor: (
                typeIdx: number,
                r: number,
                g: number,
                b: number,
            ) => {
                if (this.engine && !this.engineBusy) {
                    this.engine.set_type_color(typeIdx, r, g, b);
                }
            },
            getTypeColorsRgb: (): Float32Array | null => {
                if (this.engine) {
                    return this.engine.get_type_colors_rgb() as unknown as Float32Array;
                }
                return null;
            },
            setZoom: (level: number, centerX?: number, centerY?: number) => {
                if (this.engine && !this.engineBusy) {
                    this.engine.set_zoom(level, centerX, centerY);
                }
            },
            getZoom: () => {
                return this.engine ? this.engine.get_zoom() : null;
            },
            // Rule regeneration
            regenerateRules: () => {
                if (this.engine && !this.engineBusy) {
                    this.engine.regenerate_interaction_rules();
                    console.log("🎲 Interaction rules regenerated via UI");
                } else if (!this.engine) {
                    console.warn("Engine not available for rule regeneration");
                }
            },
        });

        // Initialize the full UI system with default parameters
        const defaultSimParams: SimulationParams = {
            deltaTime: 0.016,
            friction: 0.8,
            numParticles: 1000,
            numTypes: 4,
            virtualWorldWidth: VIRTUAL_WORLD_WIDTH,
            virtualWorldHeight: VIRTUAL_WORLD_HEIGHT,
            canvasRenderWidth: CANVAS_WIDTH,
            canvasRenderHeight: CANVAS_HEIGHT,
            virtualWorldOffsetX: 0,
            virtualWorldOffsetY: 0,
            boundaryMode: 1,
            particleRenderSize: PARTICLE_SIZE,
            forceScale: 1.0,
            rSmooth: 0.5,
            flatForce: false,
            driftXPerSecond: 0,
            interTypeAttractionScale: 1.0,
            interTypeRadiusScale: 1.0,
            time: 0,
            fisheyeStrength: 0,
            backgroundColor: [0.05, 0.05, 0.1],
            leniaEnabled: false,
            leniaGrowthMu: 0.15,
            leniaGrowthSigma: 0.017,
            leniaKernelRadius: 15.0,
            lightningFrequency: 5.0,
            lightningIntensity: 1.0,
            lightningDuration: 0.1,
        };

        const defaultZoomLevel = 1.0;
        console.log("🎯 Initializing UI with default parameters...");
        initializeUI(defaultSimParams, defaultZoomLevel);
        initColorPanel();

        // Audio toggle button
        document
            .getElementById("audio-btn")
            ?.addEventListener("click", () => this.toggleAudio());

        // Screenshot button
        document
            .getElementById("screenshot-btn")
            ?.addEventListener("click", () => this.triggerScreenshot());

        // Video record button
        document
            .getElementById("record-btn")
            ?.addEventListener("click", () => this.toggleRecording());

        // Keyboard shortcuts: S = screenshot, V = toggle video recording, A = audio toggle
        document.addEventListener("keydown", (e: KeyboardEvent) => {
            if (
                document.activeElement instanceof HTMLInputElement ||
                document.activeElement instanceof HTMLTextAreaElement
            )
                return;
            if (e.key === "a" || e.key === "A") this.toggleAudio();
            if (e.key === "s" || e.key === "S") this.triggerScreenshot();
            if (e.key === "v" || e.key === "V") this.toggleRecording();
        });

        console.log("✅ UI controls wired up");
    }

    private startRustSimulation() {
        if (!this.engine || !this.canvas) {
            console.error(
                "❌ Cannot start simulation: engine or canvas missing",
            );
            return;
        }

        console.log("🎮 Starting Rust particle simulation...");

        this.lastTime = performance.now();
        this.fpsLastTime = this.lastTime;

        const animate = (currentTime: number) => {
            const deltaTime = (currentTime - this.lastTime) / 1000.0; // Convert to seconds
            this.lastTime = currentTime;

            // Calculate FPS
            this.fpsFrameCount++;
            if (currentTime - this.fpsLastTime >= 1000) {
                // Update FPS every second
                this.currentFPS = Math.round(
                    (this.fpsFrameCount * 1000) /
                        (currentTime - this.fpsLastTime),
                );
                this.fpsFrameCount = 0;
                this.fpsLastTime = currentTime;

                // Update FPS display
                this.updateFPSDisplay();

                // Check lightning status (once per second to avoid spam)
                this.checkLightningStatus();
            }

            if (this.engine && !this.engineBusy) {
                try {
                    // Apply relative joystick panning before the simulation step
                    updateJoystickPan(deltaTime);

                    // Update the Rust simulation
                    this.engine.update_frame(deltaTime);

                    // Render using WebGPU (Rust handles this automatically)
                    this.engine.render();

                    // Capture screenshot right after render (same RAF frame)
                    if (this.pendingScreenshot) {
                        this.pendingScreenshot = false;
                        this.captureScreenshot();
                    }
                } catch (error) {
                    console.error("💥 Error in Rust simulation:", error);
                    // Don't stop the animation, just log the error
                }
            }

            this.frameCount++;
            // Removed 60-frame checks that could cause hiccups

            this.animationId = requestAnimationFrame(animate);
        };

        this.animationId = requestAnimationFrame(animate);
        console.log("✅ Rust simulation animation loop started");
    }

    private toggleAudio() {
        if (!this.engine) return;
        this.isAudioOn = !this.isAudioOn;
        this.engine.set_audio_paused(!this.isAudioOn);

        const btn = document.getElementById("audio-btn");
        const icon = btn?.querySelector(".material-symbols-outlined");
        if (icon) icon.textContent = this.isAudioOn ? "volume_up" : "volume_off";
        btn?.classList.toggle("audio-on", this.isAudioOn);
    }

    private triggerScreenshot() {
        this.pendingScreenshot = true;
    }

    private toggleRecording() {
        if (this.isRecording) {
            this.stopRecording();
        } else {
            this.startRecording();
        }
    }

    private startRecording() {
        if (!this.canvas) return;

        const stream = this.canvas.captureStream(60);

        // Voeg audiotrack toe als sonificatie actief is
        if (this.isAudioOn && this.engine) {
            const audioStream = this.engine.get_audio_stream();
            if (audioStream) {
                audioStream.getAudioTracks().forEach(track => stream.addTrack(track));
            }
        }

        const mimeType = MediaRecorder.isTypeSupported("video/webm;codecs=vp9,opus")
            ? "video/webm;codecs=vp9,opus"
            : MediaRecorder.isTypeSupported("video/webm;codecs=vp9")
            ? "video/webm;codecs=vp9"
            : "video/webm";

        this.recordedChunks = [];
        this.mediaRecorder = new MediaRecorder(stream, { mimeType });

        this.mediaRecorder.ondataavailable = (e) => {
            if (e.data.size > 0) this.recordedChunks.push(e.data);
        };

        this.mediaRecorder.onstop = () => {
            const blob = new Blob(this.recordedChunks, { type: mimeType });
            const now = new Date();
            const ts = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}_${String(now.getHours()).padStart(2, "0")}-${String(now.getMinutes()).padStart(2, "0")}-${String(now.getSeconds()).padStart(2, "0")}`;
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a");
            a.href = url;
            a.download = `particle-life-${ts}.webm`;
            a.click();
            URL.revokeObjectURL(url);
            this.recordedChunks = [];
        };

        this.mediaRecorder.start(100); // collect data every 100ms
        this.isRecording = true;

        document.getElementById("record-btn")?.classList.add("recording");
        document.getElementById("rec-indicator")?.classList.add("visible");
        console.log("🎥 Video recording started");
    }

    private stopRecording() {
        if (!this.mediaRecorder || !this.isRecording) return;
        this.mediaRecorder.stop();
        this.isRecording = false;

        document.getElementById("record-btn")?.classList.remove("recording");
        document.getElementById("rec-indicator")?.classList.remove("visible");
        console.log("⏹ Video recording stopped, preparing download...");
    }

    private captureScreenshot() {
        if (!this.canvas) return;

        // Flash feedback
        const flash = document.getElementById("screenshot-flash");
        if (flash) {
            flash.classList.add("flash");
            requestAnimationFrame(() => {
                flash.classList.remove("flash");
            });
        }

        const now = new Date();
        const ts = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}_${String(now.getHours()).padStart(2, "0")}-${String(now.getMinutes()).padStart(2, "0")}-${String(now.getSeconds()).padStart(2, "0")}`;

        this.canvas.toBlob((blob) => {
            if (!blob) {
                console.warn("Screenshot failed: canvas.toBlob returned null");
                return;
            }
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a");
            a.href = url;
            a.download = `particle-life-${ts}.png`;
            a.click();
            URL.revokeObjectURL(url);
        }, "image/png");
    }

    private updateFPSDisplay() {
        // Use the existing FPS display element
        const fpsElement = document.getElementById("fps-display");
        if (fpsElement) {
            // Show only the integer FPS number
            fpsElement.textContent = this.currentFPS.toString();
        }
    }

    private fallbackAnimation() {
        if (!this.canvas) return;

        this.updateStatus("FALLBACK ANIMATION - WASM failed to load");
        console.log("✅ Fallback animation started");

        const ctx = this.canvas.getContext("2d");
        if (!ctx) return;

        let frame = 0;

        const animate = () => {
            ctx.fillStyle = "#2a1a1a";
            ctx.fillRect(0, 0, this.canvas!.width, this.canvas!.height);

            // Draw "FALLBACK" text
            ctx.fillStyle = "#ff0000";
            ctx.font = "20px Arial";
            ctx.fillText("FALLBACK ANIMATION", 10, 30);

            // Simple spiral pattern
            for (let i = 0; i < 50; i++) {
                const angle = frame * 0.02 + i * 0.4;
                const radius = i * 8 + Math.sin(frame * 0.05) * 50;
                const x = 400 + Math.cos(angle) * radius;
                const y = 400 + Math.sin(angle) * radius;
                const hue = (frame + i * 20) % 360;

                ctx.fillStyle = `hsl(${hue}, 60%, 50%)`;
                ctx.beginPath();
                ctx.arc(x, y, 3, 0, 2 * Math.PI);
                ctx.fill();
            }

            frame++;
            requestAnimationFrame(animate);
        };

        animate();
    }

    public destroy() {
        if (this.animationId) {
            cancelAnimationFrame(this.animationId);
            this.animationId = null;
        }

        if (this.engine) {
            this.engine.free();
            this.engine = null;
            console.log("🧹 Rust engine freed");
        }
    }

    // Getter for debugging access
    get simulation() {
        return this.engine;
    }

    private lastElectricalActivity = 0;
    private lastLightningCheck = 0;
    private currentTemperature = 20; // default matches HTML slider

    private async checkLightningStatus() {
        if (!this.engine || this.engineBusy) return;
        try {
            const currentTime = performance.now();
            const currentElectricalActivity =
                this.engine.get_electrical_activity();

            // Track electrical activity changes
            if (
                Math.abs(
                    currentElectricalActivity - this.lastElectricalActivity,
                ) > 0.1
            ) {
                this.lastElectricalActivity = currentElectricalActivity;
            }

            // Check for super-lightning events (async GPU readback).
            // Lock the engine for the duration to prevent wasm re-entrancy panics:
            // check_super_lightning is async &mut self and holds the borrow during await.
            if (currentTime - this.lastLightningCheck > 1000) {
                this.lastLightningCheck = currentTime;
                this.engineBusy = true;
                try {
                    const isSuper = await this.engine.check_super_lightning();
                    if (isSuper) {
                        console.log(
                            "⚡ Super-lightning detected! Rules snapped (handled in Rust).",
                        );
                    }
                } finally {
                    this.engineBusy = false;
                }
            }
        } catch (error) {
            this.engineBusy = false; // ensure lock is released on unexpected error
        }
    }
}

// Initialize the app when DOM is loaded
console.log("📋 Setting up DOMContentLoaded listener...");
document.addEventListener("DOMContentLoaded", async () => {
    console.log("🌟 DOM loaded, creating app...");
    const app = new App();
    await app.init();

    // Expose the simulation to the global window object for debugging
    (window as any).app = app;
    (window as any).simulation = app.simulation;
    console.log("🔧 Exposed app and simulation to window object for debugging");

    window.addEventListener("beforeunload", () => {
        app.destroy();
    });
});

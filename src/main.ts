console.log("🚀 main.ts loading with proper WASM integration...");

// Import WASM module using the standard wasm-bindgen approach
import init, { ParticleLifeEngine } from "./pkg/particle_life_wasm.js";

class App {
    private engine: ParticleLifeEngine | null = null;
    private canvas: HTMLCanvasElement | null = null;
    private animationId: number | null = null;
    private frameCount = 0;
    private lastTime = 0;
    private fpsFrameCount = 0;
    private fpsLastTime = 0;
    private currentFPS = 0;

    async init() {
        console.log("🔧 App.init() called");

        // Add status message to DOM
        this.updateStatus("Initializing application...");

        try {
            // Get canvas first
            this.canvas = document.getElementById(
                "canvas"
            ) as HTMLCanvasElement;
            if (!this.canvas) {
                console.error("❌ Canvas element not found");
                this.updateStatus("Error: Canvas not found");
                return;
            }

            console.log(
                "🎨 Canvas found:",
                this.canvas.width,
                "x",
                this.canvas.height
            );
            this.canvas.style.borderRadius = "10px";
            this.canvas.style.border = "2px solid #ff8800";
            this.updateStatus("Canvas initialized...");

            // Initialize WASM module
            console.log("🦀 Initializing WASM module...");
            this.updateStatus("Loading WASM module...");

            // Try different path approaches
            const wasmPaths = [
                "/pkg/particle_life_wasm_bg.wasm", // Absolute path from domain root
                "./pkg/particle_life_wasm_bg.wasm", // Relative to current page
                "pkg/particle_life_wasm_bg.wasm", // Relative without ./
            ];

            let initSuccess = false;
            for (const wasmPath of wasmPaths) {
                try {
                    console.log(`🔍 Trying WASM path: ${wasmPath}`);
                    await init(wasmPath);
                    console.log(
                        `✅ WASM initialized successfully with path: ${wasmPath}`
                    );
                    initSuccess = true;
                    break;
                } catch (error) {
                    console.warn(`⚠️ Failed with path ${wasmPath}:`, error);
                }
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
                    error
                );
                this.updateStatus("WebGPU failed, using CPU fallback");
            }

            // Wire up UI controls
            this.wireUpControls();
            this.updateStatus("UI controls wired up");

            // Start the real Rust simulation
            this.updateStatus("Starting particle simulation...");
            this.startRustSimulation();
            this.updateStatus("Particle simulation running!");
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

        // Particle count slider
        const particleCountSlider = document.getElementById(
            "particleCountSlider"
        ) as HTMLInputElement;
        const particleCountValue = document.getElementById(
            "particleCountValue"
        ) as HTMLSpanElement;

        if (particleCountSlider && particleCountValue) {
            const updateParticleCount = () => {
                const count = parseInt(particleCountSlider.value);
                particleCountValue.textContent = count.toString();
                if (this.engine) {
                    this.engine.set_particle_count(count);
                    console.log(`� Particle count set to: ${count}`);
                }
            };

            particleCountSlider.addEventListener("input", updateParticleCount);
            updateParticleCount(); // Set initial value
        }

        // Other controls can be wired up here as needed
        console.log("✅ UI controls wired up");
    }

    private startRustSimulation() {
        if (!this.engine || !this.canvas) {
            console.error(
                "❌ Cannot start simulation: engine or canvas missing"
            );
            return;
        }

        console.log("🎮 Starting Rust particle simulation...");

        const ctx = this.canvas.getContext("2d");
        if (!ctx) {
            console.error("❌ Could not get 2D context");
            return;
        }

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
                        (currentTime - this.fpsLastTime)
                );
                this.fpsFrameCount = 0;
                this.fpsLastTime = currentTime;

                // Update FPS display
                this.updateFPSDisplay();
            }

            if (this.engine) {
                try {
                    // Update the Rust simulation
                    this.engine.update_frame(deltaTime);

                    // Clear canvas
                    ctx.fillStyle = "#0a0a0a";
                    ctx.fillRect(0, 0, this.canvas!.width, this.canvas!.height);

                    // Render using Rust
                    this.engine.render();

                    // Optional: render to canvas if the method is available
                    try {
                        this.engine.render_to_canvas("canvas");
                    } catch (e) {
                        // Fallback: render test graphics
                        this.engine.render_test_graphics();
                    }
                } catch (error) {
                    console.error("💥 Error in Rust simulation:", error);
                    // Don't stop the animation, just log the error
                }
            }

            this.frameCount++;
            if (this.frameCount % 60 === 0) {
                const particleCount = this.engine?.get_particle_count() || 0;
                console.log(
                    `🔄 Frame ${this.frameCount}, delta: ${deltaTime.toFixed(
                        4
                    )}s, particles: ${particleCount}, FPS: ${this.currentFPS}`
                );
                this.updateStatus(
                    `Frame ${this.frameCount}, particles: ${particleCount}, FPS: ${this.currentFPS}`
                );
            }

            this.animationId = requestAnimationFrame(animate);
        };

        this.animationId = requestAnimationFrame(animate);
        console.log("✅ Rust simulation animation loop started");
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
}

// Initialize the app when DOM is loaded
console.log("📋 Setting up DOMContentLoaded listener...");
document.addEventListener("DOMContentLoaded", async () => {
    console.log("🌟 DOM loaded, creating app...");
    const app = new App();
    await app.init();

    window.addEventListener("beforeunload", () => {
        app.destroy();
    });
});

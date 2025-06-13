/**
 * Clean Architecture Demo - TypeScript UI Layer
 *
 * This demonstrates the ideal architecture where:
 * - TypeScript ONLY handles UI controls and DOM manipulation
 * - Rust handles ALL simulation logic and (future) WebGPU rendering
 * - Clear separation enables easy native deployment (ESP32 → Rust → Native rendering)
 */

import init, { ParticleLifeEngine } from "./pkg/particle_life_wasm.js";

class ParticleLifeUI {
    private engine: ParticleLifeEngine | null = null;
    private canvas: HTMLCanvasElement;
    private ctx: CanvasRenderingContext2D;
    private animationId: number = 0;
    private lastFrameTime: number = 0;
    private frameCount: number = 0;
    private fpsStartTime: number = 0;

    // UI Elements
    private particleCountSlider: HTMLInputElement;
    private particleCountValue: HTMLSpanElement;
    private frictionSlider: HTMLInputElement;
    private frictionValue: HTMLSpanElement;
    private forceScaleSlider: HTMLInputElement;
    private forceScaleValue: HTMLSpanElement;
    private backgroundColorPicker: HTMLInputElement;
    private randomizeRulesBtn: HTMLButtonElement;
    private resetSimulationBtn: HTMLButtonElement;

    // Stats elements
    private fpsDisplay: HTMLDivElement;
    private particleInfoDisplay: HTMLDivElement;
    private engineInfoDisplay: HTMLDivElement;

    constructor() {
        this.canvas = document.getElementById("canvas") as HTMLCanvasElement;
        this.ctx = this.canvas.getContext("2d")!;

        // Initialize UI elements
        this.particleCountSlider = document.getElementById(
            "particleCount"
        ) as HTMLInputElement;
        this.particleCountValue = document.getElementById(
            "particleCountValue"
        ) as HTMLSpanElement;
        this.frictionSlider = document.getElementById(
            "friction"
        ) as HTMLInputElement;
        this.frictionValue = document.getElementById(
            "frictionValue"
        ) as HTMLSpanElement;
        this.forceScaleSlider = document.getElementById(
            "forceScale"
        ) as HTMLInputElement;
        this.forceScaleValue = document.getElementById(
            "forceScaleValue"
        ) as HTMLSpanElement;
        this.backgroundColorPicker = document.getElementById(
            "backgroundColor"
        ) as HTMLInputElement;
        this.randomizeRulesBtn = document.getElementById(
            "randomizeRules"
        ) as HTMLButtonElement;
        this.resetSimulationBtn = document.getElementById(
            "resetSimulation"
        ) as HTMLButtonElement;

        this.fpsDisplay = document.getElementById("fps") as HTMLDivElement;
        this.particleInfoDisplay = document.getElementById(
            "particleInfo"
        ) as HTMLDivElement;
        this.engineInfoDisplay = document.getElementById(
            "engineInfo"
        ) as HTMLDivElement;

        this.setupEventListeners();
        this.resizeCanvas();

        window.addEventListener("resize", () => this.resizeCanvas());
    }

    async init() {
        console.log("🚀 Initializing Particle Life Clean Architecture Demo");

        // Initialize the WASM module
        await init();

        // Create the Rust simulation engine
        this.engine = new ParticleLifeEngine();

        console.log("✅ Rust engine initialized");
        console.log("📊 Engine info:", this.engine.get_debug_info());

        // Start the render loop
        this.startRenderLoop();

        console.log(
            "🎬 Render loop started - TypeScript handles UI, Rust handles simulation"
        );
    }

    private setupEventListeners() {
        // Particle count slider
        this.particleCountSlider.addEventListener("input", () => {
            const count = parseInt(this.particleCountSlider.value);
            this.particleCountValue.textContent = count.toString();
            this.engine?.set_particle_count(count);
        });

        // Friction slider
        this.frictionSlider.addEventListener("input", () => {
            const friction = parseFloat(this.frictionSlider.value);
            this.frictionValue.textContent = friction.toFixed(2);
            this.engine?.update_parameter("friction", friction);
        });

        // Force scale slider
        this.forceScaleSlider.addEventListener("input", () => {
            const forceScale = parseFloat(this.forceScaleSlider.value);
            this.forceScaleValue.textContent = forceScale.toFixed(1);
            this.engine?.update_parameter("force_scale", forceScale);
        });

        // Background color picker
        this.backgroundColorPicker.addEventListener("input", () => {
            const color = this.backgroundColorPicker.value;
            const r = parseInt(color.slice(1, 3), 16) / 255.0;
            const g = parseInt(color.slice(3, 5), 16) / 255.0;
            const b = parseInt(color.slice(5, 7), 16) / 255.0;

            this.engine?.update_parameter("background_color_r", r);
            this.engine?.update_parameter("background_color_g", g);
            this.engine?.update_parameter("background_color_b", b);
        });

        // Randomize rules button
        this.randomizeRulesBtn.addEventListener("click", () => {
            console.log("🎲 Randomizing interaction rules (handled by Rust)");
            this.engine?.regenerate_rules();
        });

        // Reset simulation button
        this.resetSimulationBtn.addEventListener("click", () => {
            console.log("🔄 Resetting simulation (handled by Rust)");
            // Reset to default particle count
            const defaultCount = 1000;
            this.particleCountSlider.value = defaultCount.toString();
            this.particleCountValue.textContent = defaultCount.toString();
            this.engine?.set_particle_count(defaultCount);
            this.engine?.regenerate_rules();
        });
    }

    private resizeCanvas() {
        this.canvas.width = window.innerWidth;
        this.canvas.height = window.innerHeight;

        // Update canvas size in Rust engine
        if (this.engine) {
            this.engine.update_parameter(
                "canvas_render_width",
                this.canvas.width
            );
            this.engine.update_parameter(
                "canvas_render_height",
                this.canvas.height
            );
        }
    }

    private startRenderLoop() {
        this.fpsStartTime = performance.now();

        const renderFrame = (currentTime: number) => {
            const deltaTime = (currentTime - this.lastFrameTime) / 1000; // Convert to seconds
            this.lastFrameTime = currentTime;

            if (this.engine) {
                // Update the simulation in Rust
                this.engine.update_frame(deltaTime);

                // Render the frame (currently using Canvas2D for demo, will be WebGPU in Rust later)
                this.renderFrameCanvas2D();

                // Update UI stats
                this.updateStats();
            }

            this.frameCount++;
            this.animationId = requestAnimationFrame(renderFrame);
        };

        this.animationId = requestAnimationFrame(renderFrame);
    }

    private renderFrameCanvas2D() {
        if (!this.engine) return;

        // Clear canvas with background color
        this.ctx.fillStyle = this.backgroundColorPicker.value;
        this.ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);

        // Get particle data from Rust engine
        const particleBuffer = this.engine.get_particle_buffer();
        const particleCount = this.engine.get_particle_count();
        const particleSizeBytes = this.engine.get_particle_size_bytes();

        // Simple particle rendering for demo
        // (In final version, this will all be handled by Rust WebGPU renderer)
        this.ctx.fillStyle = "#00ff88";

        for (let i = 0; i < particleCount; i++) {
            const offset = i * particleSizeBytes;
            const dataView = new DataView(particleBuffer.buffer, offset);

            // Read particle data (pos.x, pos.y, vel.x, vel.y, type, size)
            const x = dataView.getFloat32(0, true);
            const y = dataView.getFloat32(4, true);
            const size = dataView.getFloat32(20, true);

            // Draw particle as simple circle
            this.ctx.beginPath();
            this.ctx.arc(x, y, size, 0, Math.PI * 2);
            this.ctx.fill();
        }
    }

    private updateStats() {
        if (!this.engine) return;

        // Calculate FPS
        const currentTime = performance.now();
        const elapsed = currentTime - this.fpsStartTime;
        if (elapsed >= 1000) {
            // Update every second
            const fps = Math.round((this.frameCount * 1000) / elapsed);
            this.fpsDisplay.textContent = `FPS: ${fps}`;
            this.frameCount = 0;
            this.fpsStartTime = currentTime;
        }

        // Update particle info
        const particleCount = this.engine.get_particle_count();
        const maxParticles = this.engine.get_max_particles();
        this.particleInfoDisplay.textContent = `Particles: ${particleCount}/${maxParticles}`;

        // Update engine info
        const debugInfo = this.engine.get_debug_info();
        this.engineInfoDisplay.textContent = `Engine: ${debugInfo}`;
    }

    destroy() {
        if (this.animationId) {
            cancelAnimationFrame(this.animationId);
        }

        // Clean up Rust engine (WASM will handle memory)
        this.engine = null;

        console.log("🧹 UI cleaned up");
    }
}

// Initialize the application
console.log("🎯 Starting Clean Architecture Demo");
console.log("📋 Architecture:");
console.log("   🦀 Rust: All simulation logic, future WebGPU rendering");
console.log("   🎯 TypeScript: UI controls only, thin presentation layer");
console.log("   🎯 Future: ESP32 input → Rust engine → Native rendering");

const app = new ParticleLifeUI();
app.init().catch(console.error);

// Handle page unload
window.addEventListener("beforeunload", () => {
    app.destroy();
});

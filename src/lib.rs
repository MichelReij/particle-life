use rand::prelude::*;
use rand::rngs::SmallRng;
use wasm_bindgen::prelude::*;

mod buffer_utils;
mod interaction_rules;
mod lightning_system;
mod particle_system;
mod particle_transitions;
mod shaders;
mod simulation_params;
mod spatial_grid;
mod webgpu_renderer_modern;

pub use buffer_utils::*;
pub use interaction_rules::*;
pub use lightning_system::*;
pub use particle_system::*;
pub use particle_transitions::*;
pub use shaders::*;
pub use simulation_params::*;
pub use spatial_grid::*;
pub use webgpu_renderer_modern::WebGpuRenderer;
pub use simulation_params::*;
pub use spatial_grid::*;
// pub use webgpu_renderer_modern::WebGpuRenderer;  // Disabled temporarily

// Initialize panic hook for better error messages in browser console
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
}

// Macro for logging from Rust to browser console
macro_rules! console_log {
    ($($t:tt)*) => (crate::log(&format_args!($($t)*).to_string()))
}

pub(crate) use console_log;

// Main simulation engine that orchestrates everything
#[wasm_bindgen]
pub struct ParticleLifeEngine {
    particle_system: ParticleSystem,
    simulation_params: SimulationParams,
    interaction_rules: InteractionRules,
    particle_transitions: ParticleTransitions,
    lightning_system: LightningSystem,
    rng: SmallRng,
    current_time: f32,
    last_frame_time: f32,
    frame_count: u32,
    renderer: Option<WebGpuRenderer>,
}

#[wasm_bindgen]
impl ParticleLifeEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_log!("🦀 Initializing Rust ParticleLifeEngine");

        let mut rng = SmallRng::from_entropy();

        let simulation_params = SimulationParams::new();
        let interaction_rules = InteractionRules::new_random(&mut rng);
        let particle_system = ParticleSystem::new(&simulation_params, &interaction_rules, &mut rng);
        let particle_transitions = ParticleTransitions::new();
        let lightning_system = LightningSystem::new();

        Self {
            particle_system,
            simulation_params,
            interaction_rules,
            particle_transitions,
            lightning_system,
            rng,
            current_time: 0.0,
            last_frame_time: 0.0,
            frame_count: 0,
            renderer: None,
        }
    }

    // Get the current simulation parameters as a buffer for GPU
    #[wasm_bindgen]
    pub fn get_simulation_params_buffer(&self) -> Vec<u8> {
        self.simulation_params.to_buffer()
    }

    // Get the interaction rules as a buffer for GPU
    #[wasm_bindgen]
    pub fn get_interaction_rules_buffer(&self) -> Vec<u8> {
        self.interaction_rules.to_buffer()
    }

    // Get the particle data as a buffer for GPU
    #[wasm_bindgen]
    pub fn get_particle_buffer(&self) -> Vec<u8> {
        self.particle_system.to_buffer()
    }

    // Get particle colors buffer
    #[wasm_bindgen]
    pub fn get_particle_colors_buffer(&self) -> Vec<u8> {
        self.particle_system.get_colors_buffer()
    }

    // Update simulation parameters
    #[wasm_bindgen]
    pub fn update_simulation_params(&mut self, params_json: &str) -> Result<(), JsValue> {
        let params: SimulationParams =
            serde_wasm_bindgen::from_value(js_sys::JSON::parse(params_json)?)?;
        self.simulation_params = params;
        Ok(())
    }

    // Update a specific parameter by name
    #[wasm_bindgen]
    pub fn update_parameter(&mut self, name: &str, value: f32) -> bool {
        self.simulation_params.update_parameter(name, value)
    }

    // Frame update - called every frame from JavaScript
    #[wasm_bindgen]
    pub fn update_frame(&mut self, delta_time: f32) {
        let start_time = if self.frame_count % 60 == 0 {
            Some(js_sys::Date::now())
        } else {
            None
        };

        self.current_time += delta_time;
        self.simulation_params.set_time(self.current_time);
        self.simulation_params.set_delta_time(delta_time);

        // Update particle physics simulation
        self.particle_system
            .update_physics(&self.simulation_params, &self.interaction_rules);

        // Update particle transitions
        self.particle_transitions
            .update(self.current_time, &mut self.particle_system);

        // Update lightning system
        self.lightning_system
            .update(self.current_time, &self.simulation_params);

        self.frame_count += 1;

        // Log performance every 60 frames
        if let Some(start) = start_time {
            let end_time = js_sys::Date::now();
            let frame_time = end_time - start;
            console_log!(
                "⚡ Rust frame time: {:.2}ms (delta_time: {:.3}s)",
                frame_time,
                delta_time
            );
        }
    }

    // Particle count management
    #[wasm_bindgen]
    pub fn get_particle_count(&self) -> u32 {
        self.particle_system.get_active_count()
    }

    #[wasm_bindgen]
    pub fn set_particle_count(&mut self, count: u32) -> bool {
        if count > self.particle_system.get_max_particles() {
            return false;
        }

        let current_count = self.particle_system.get_active_count();

        if count > current_count {
            // Growing - initialize new particles
            self.particle_system.grow_particles(count, &mut self.rng);
            self.particle_transitions.start_grow_transition(
                current_count,
                count,
                self.current_time,
            );
        } else if count < current_count {
            // Shrinking - start shrink transition
            self.particle_transitions.start_shrink_transition(
                count,
                current_count,
                self.current_time,
            );
        }

        true
    }

    // Pressure-based particle count mapping
    #[wasm_bindgen]
    pub fn pressure_to_particle_count(&self, pressure: f32) -> u32 {
        let clamped_pressure = pressure.max(0.0).min(350.0);
        let normalized = clamped_pressure / 350.0;
        let range = (self.particle_system.get_max_particles()
            - self.particle_system.get_min_particles()) as f32;
        let target = self.particle_system.get_min_particles() as f32 + normalized * range;

        // Round to nearest multiple of 64 for optimal GPU workgroup dispatch
        ((target / 64.0).round() * 64.0) as u32
    }

    // Lightning system access
    #[wasm_bindgen]
    pub fn get_lightning_segments_buffer(&self) -> Vec<u8> {
        self.lightning_system.get_segments_buffer()
    }

    #[wasm_bindgen]
    pub fn get_lightning_bolts_buffer(&self) -> Vec<u8> {
        self.lightning_system.get_bolts_buffer()
    }

    // Generate new random interaction rules
    #[wasm_bindgen]
    pub fn regenerate_rules(&mut self) {
        self.interaction_rules = InteractionRules::new_random(&mut self.rng);
        console_log!("🔄 Generated new interaction rules");
    }

    // Get various constants needed by TypeScript
    #[wasm_bindgen]
    pub fn get_max_particles(&self) -> u32 {
        self.particle_system.get_max_particles()
    }

    #[wasm_bindgen]
    pub fn get_min_particles(&self) -> u32 {
        self.particle_system.get_min_particles()
    }

    #[wasm_bindgen]
    pub fn get_num_types(&self) -> u32 {
        self.particle_system.get_num_types()
    }

    #[wasm_bindgen]
    pub fn get_particle_size_bytes(&self) -> u32 {
        24 // pos(8) + vel(8) + type(4) + size(4)
    }

    #[wasm_bindgen]
    pub fn get_sim_params_size_bytes(&self) -> u32 {
        self.simulation_params.get_buffer_size()
    }

    // Debug information
    #[wasm_bindgen]
    pub fn get_debug_info(&self) -> String {
        format!(
            "Frame: {}, Time: {:.2}s, Particles: {}/{}, Active Transitions: {}",
            self.frame_count,
            self.current_time,
            self.particle_system.get_active_count(),
            self.particle_system.get_max_particles(),
            self.particle_transitions.get_active_count()
        )
    }

    // Simple canvas rendering for debugging/fallback
    #[wasm_bindgen]
    pub fn render_to_canvas(&self, canvas_id: &str) -> Result<(), JsValue> {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let canvas = document
            .get_element_by_id(canvas_id)
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()?;

        let context = canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()?;

        let canvas_width = canvas.width() as f32;
        let canvas_height = canvas.height() as f32;

        // Clear canvas with background color
        let bg_r = self.simulation_params.background_color_r;
        let bg_g = self.simulation_params.background_color_g;
        let bg_b = self.simulation_params.background_color_b;

        let bg_color = format!(
            "rgb({},{},{})",
            (bg_r * 255.0) as u8,
            (bg_g * 255.0) as u8,
            (bg_b * 255.0) as u8
        );
        context.set_fill_style_str(&bg_color);
        context.fill_rect(0.0, 0.0, canvas_width as f64, canvas_height as f64);

        // Calculate the center view window (800x800px) in the 2400x2400px world
        let world_width = self.simulation_params.virtual_world_width;
        let world_height = self.simulation_params.virtual_world_height;

        // View the center of the world (no scaling, 1:1 pixel mapping)
        let view_center_x = world_width / 2.0;
        let view_center_y = world_height / 2.0;
        let view_left = view_center_x - canvas_width / 2.0;
        let view_top = view_center_y - canvas_height / 2.0;
        let view_right = view_center_x + canvas_width / 2.0;
        let view_bottom = view_center_y + canvas_height / 2.0;

        // Debug logging for view window
        if self.frame_count % 60 == 0 {
            console_log!(
                "🎨 View window - World: {}x{}, View: ({:.1},{:.1}) to ({:.1},{:.1})",
                world_width,
                world_height,
                view_left,
                view_top,
                view_right,
                view_bottom
            );
        }

        // Always debug first few frames
        if self.frame_count < 5 {
            console_log!(
                "🔧 Frame {} - World: {}x{}, View center: ({:.1},{:.1}), View bounds: ({:.1},{:.1}) to ({:.1},{:.1})",
                self.frame_count,
                world_width,
                world_height,
                view_center_x,
                view_center_y,
                view_left,
                view_top,
                view_right,
                view_bottom
            );
        }

        let mut particles_rendered = 0;
        for i in 0..self.particle_system.get_active_count() {
            if let Some(particle) = self.particle_system.get_particle(i as usize) {
                // Check if particle is within the view window
                if particle.position[0] < view_left
                    || particle.position[0] > view_right
                    || particle.position[1] < view_top
                    || particle.position[1] > view_bottom
                {
                    continue; // Skip particles outside the view window
                }

                // Convert world coordinates to canvas coordinates
                let canvas_x = particle.position[0] - view_left;
                let canvas_y = particle.position[1] - view_top;

                // Use particle size directly
                let canvas_radius = if self.frame_count < 100 {
                    5.0 // Large particles for debugging
                } else {
                    particle.size
                };

                // Get particle color based on type
                let hue = (particle.particle_type as f32 / self.simulation_params.num_types as f32)
                    * 360.0;
                let color = format!("hsl({}, 70%, 60%)", hue);
                context.set_fill_style_str(&color);

                context.begin_path();
                context.arc(
                    canvas_x as f64,
                    canvas_y as f64,
                    canvas_radius as f64,
                    0.0,
                    2.0 * std::f64::consts::PI,
                )?;
                context.fill();

                particles_rendered += 1;
            }
        }

        // Debug logging for particle rendering count
        if self.frame_count % 60 == 0 || self.frame_count < 10 {
            console_log!(
                "🎨 Frame {} - Rendered {} particles out of {} active",
                self.frame_count,
                particles_rendered,
                self.particle_system.get_active_count()
            );
        }

        Ok(())
    }

    // Initialize WebGPU renderer
    #[wasm_bindgen]
    pub async fn initialize_webgpu(
        &mut self,
        canvas: web_sys::HtmlCanvasElement,
    ) -> Result<(), JsValue> {
        console_log!("� Attempting to initialize WebGPU renderer with WGPU 25...");

        match WebGpuRenderer::new(&canvas).await {
            Ok(renderer) => {
                console_log!("✅ WebGPU renderer initialized successfully!");
                self.renderer = Some(renderer);
                Ok(())
            }
            Err(e) => {
                console_log!("⚠️ WebGPU initialization failed: {:?}", e);
                console_log!("🔄 Falling back to Canvas 2D rendering");
                Err(e)
            }
        }
    }

    // Render using WebGPU (preferred) or fallback to Canvas 2D
    #[wasm_bindgen]
    pub fn render(&mut self) -> Result<(), JsValue> {
        if let Some(ref mut renderer) = self.renderer {
            // Use WebGPU renderer
            renderer.render(&self.particle_system, &self.simulation_params)?;
        } else {
            // Fallback to Canvas 2D
            self.render_to_canvas("canvas")?;

            // Also render a simple test circle to ensure canvas is working
            if self.frame_count == 1 {
                self.render_test_graphics()?;
            }
        }

        Ok(())
    }

    // Test graphics to verify canvas is working
    #[wasm_bindgen]
    pub fn render_test_graphics(&self) -> Result<(), JsValue> {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let canvas = document
            .get_element_by_id("canvas")
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()?;

        let context = canvas
            .get_context("2d")?
            .unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()?;

        // Draw a bright red test circle in the center
        context.set_fill_style_str("#ff0000");
        context.begin_path();
        context.arc(400.0, 400.0, 50.0, 0.0, 2.0 * std::f64::consts::PI)?;
        context.fill();

        console_log!("🎯 Test graphics rendered - red circle at center");

        Ok(())
    }
}

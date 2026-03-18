#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use rand::rngs::SmallRng;
#[cfg(target_arch = "wasm32")]
use rand::{Rng, SeedableRng};

mod buffer_utils;
pub mod config;
mod interaction_rules;
mod particle_system;
mod shader_constants;
mod simulation_params;
mod spatial_grid;
mod webgpu_renderer;

// ESP32 communication only for native builds
#[cfg(not(target_arch = "wasm32"))]
mod esp32_communication;

// Audio system only for native builds
#[cfg(not(target_arch = "wasm32"))]
mod audio;

pub use buffer_utils::*;
pub use config::*;
pub use interaction_rules::*;
pub use particle_system::*;
pub use shader_constants::*;
pub use simulation_params::*;
pub use spatial_grid::*;
pub use webgpu_renderer::*;

#[cfg(not(target_arch = "wasm32"))]
pub use esp32_communication::*;

#[cfg(not(target_arch = "wasm32"))]
pub use audio::*;

// Hook for better error messages in browser console
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}

// Platform-specific logging implementations
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn log(s: &str) {
    println!("{}", s);
}

// Cross-platform logging macro that works for both web and native
#[macro_export]
macro_rules! console_log {
    ($($t:tt)*) => ($crate::log(&format_args!($($t)*).to_string()))
}

// Main simulation engine that orchestrates everything
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub struct ParticleLifeEngine {
    particle_system: ParticleSystem,
    simulation_params: SimulationParams,
    interaction_rules: InteractionRules,
    rng: SmallRng,
    current_time: f32,
    frame_count: u32,
    renderer: Option<WebGpuRenderer>,
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
impl ParticleLifeEngine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        // Log WASM module build info for version verification - auto-generated at build time
        console_log!(
            "🦀 Rust ParticleLifeEngine v{} - BUILD_ID: {}",
            env!("CARGO_PKG_VERSION"),
            env!("BUILD_ID")
        );

        // Calculate and display WASM module age using current timestamp as reference
        let current_time = js_sys::Date::now(); // Current time in milliseconds
                                                // Build timestamp - auto-generated at build time
        let build_timestamp = env!("BUILD_TIMESTAMP").parse::<f64>().unwrap() * 1000.0; // Convert to milliseconds
        let age_ms = current_time - build_timestamp;
        let age_seconds = (age_ms / 1000.0) as i32;
        let age_minutes = age_seconds / 60;
        let age_hours = age_minutes / 60;
        let remaining_minutes = age_minutes % 60;
        let remaining_seconds = age_seconds % 60;

        if age_hours > 0 {
            console_log!("⏰ WASM Age: {}h {}m ago", age_hours, remaining_minutes);
        } else if age_minutes > 0 {
            console_log!("⏰ WASM Age: {}m {}s ago", age_minutes, remaining_seconds);
        } else {
            console_log!("⏰ WASM Age: {}s ago", age_seconds);
        }

        let mut rng = SmallRng::from_entropy();

        let simulation_params = SimulationParams::new();
        let interaction_rules = InteractionRules::new_random(&mut rng);
        let particle_system = ParticleSystem::new(&simulation_params, &interaction_rules, &mut rng);

        Self {
            particle_system,
            simulation_params,
            interaction_rules,
            rng,
            current_time: 0.0,
            frame_count: 0,
            renderer: None,
        }
    }

    // Get the current simulation parameters as a buffer for GPU
    #[wasm_bindgen]
    pub fn get_simulation_params_buffer(&self) -> Vec<u8> {
        // Use actual particle count from particle system and current zoom level for drift adjustment
        let actual_particle_count = self.particle_system.get_active_count() as u32;
        self.simulation_params
            .to_buffer_with_particle_count_and_zoom(
                actual_particle_count,
                self.simulation_params.current_zoom_level,
            )
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

    // Update a specific parameter by name - routes through individual setters for validation
    #[wasm_bindgen]
    pub fn update_parameter(&mut self, name: &str, value: f32) -> bool {
        match name {
            "friction" => {
                self.set_friction(value);
                true
            }
            "forceScale" => {
                self.set_force_scale(value);
                true
            }
            "rSmooth" => {
                self.set_r_smooth(value);
                true
            }
            "driftXPerSecond" => {
                self.set_drift_x_per_second(value);
                true
            }
            "interTypeAttractionScale" => {
                self.set_inter_type_attraction_scale(value);
                true
            }
            "interTypeRadiusScale" => {
                self.set_inter_type_radius_scale(value);
                true
            }
            "fisheyeStrength" => {
                self.set_fisheye_strength(value);
                true
            }
            "leniaGrowthMu" => {
                self.set_lenia_growth_mu(value);
                true
            }
            "leniaGrowthSigma" => {
                self.set_lenia_growth_sigma(value);
                true
            }
            "leniaKernelRadius" => {
                self.set_lenia_kernel_radius(value);
                true
            }
            "lightningFrequency" => {
                self.set_lightning_frequency(value);
                true
            }
            "lightningIntensity" => {
                self.set_lightning_intensity(value);
                true
            }
            "lightningDuration" => {
                self.set_lightning_duration(value);
                true
            }
            "particleRenderSize" => {
                self.set_particle_render_size(value);
                true
            }
            _ => {
                console_log!("⚠️ Unknown parameter: {}", name);
                false
            }
        }
    }

    // Update a specific boolean parameter by name
    #[wasm_bindgen]
    pub fn update_boolean_parameter(&mut self, name: &str, value: bool) -> bool {
        match name {
            "flatForce" => {
                self.set_flat_force(value);
                true
            }
            "leniaEnabled" => {
                self.set_lenia_enabled(value);
                true
            }
            _ => {
                console_log!("⚠️ Unknown boolean parameter: {}", name);
                false
            }
        }
    }

    // Frame update - called every frame from JavaScript
    #[wasm_bindgen]
    pub fn update_frame(&mut self, delta_time: f32) {
        self.current_time += delta_time;
        self.simulation_params.set_time(self.current_time);
        self.simulation_params.set_delta_time(delta_time);

        // Debug deltaTime and particle count correlation every 300 frames (5 seconds)
        if self.frame_count % 300 == 0 {
            let active_particles = self.particle_system.get_active_count();
            console_log!(
                "� Frame {}: particles={}, fps={:.1}",
                self.frame_count,
                active_particles,
                if delta_time > 0.0 {
                    1.0 / delta_time
                } else {
                    0.0
                }
            );
        }

        // PHYSICS IS HANDLED BY GPU COMPUTE SHADER - no CPU physics needed
        // The WebGPU renderer will handle all physics calculations via compute shaders

        // Particle transitions are now handled entirely by GPU compute shader

        // Lightning generation is now handled by the GPU compute shader
        // (Rust lightning_system.update() removed to avoid conflicts)

        self.frame_count += 1;

        // Removed verbose frame time logging
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

        if count != current_count {
            // Start GPU-based transition
            console_log!(
                "🕒 Starting transition at time {:.3}s: {} -> {} particles",
                self.current_time,
                current_count,
                count
            );
            self.simulation_params.start_particle_transition(
                current_count,
                count,
                self.current_time,
            );

            // For grow transitions, update active count immediately and initialize new particles
            // For shrink transitions, defer until transition completes
            if count > current_count {
                self.particle_system.set_active_count(count);
                self.simulation_params.set_num_particles(count);

                // Initialize the new particles in the grow range
                self.initialize_particles_for_grow_transition(current_count, count);

                // Update only transition fields without overwriting GPU physics data
                if let Some(ref mut renderer) = self.renderer {
                    renderer.update_particle_transitions(&self.particle_system);
                }

                // console_log!(
                //     "🌱 GROW: {} -> {} particles, active_count now: {}, num_particles now: {}",
                //     current_count,
                //     count,
                //     self.particle_system.get_active_count(),
                //     self.simulation_params.num_particles
                // );
            } else {
                // Initialize particles for shrink transition
                self.initialize_particles_for_shrink_transition(count, current_count);

                // Update only transition fields without overwriting GPU physics data
                if let Some(ref mut renderer) = self.renderer {
                    renderer.update_particle_transitions(&self.particle_system);
                }
            }
            // For shrink: keep old count during transition

            // console_log!(
            //     "🔄 Starting GPU transition: {} -> {} particles (deferred: {})",
            //     current_count,
            //     count,
            //     if count < current_count {
            //         "true"
            //     } else {
            //         "false"
            //     }
            // );
        } else {
            // No change needed
            self.simulation_params.set_num_particles(count);
        }

        true
    }

    // Initialize particles for grow transition
    fn initialize_particles_for_grow_transition(&mut self, start_index: u32, end_index: u32) {
        for i in start_index..end_index {
            // let particle_type = (i % self.particle_system.get_num_types()) as u32;

            if let Some(particle) = self.particle_system.get_particle_mut(i as usize) {
                // Initialize position, velocity, type, and target size
                particle.position = [
                    self.rng
                        .gen_range(0.0..self.simulation_params.virtual_world_width),
                    self.rng
                        .gen_range(0.0..self.simulation_params.virtual_world_height),
                ];
                particle.velocity = [self.rng.gen_range(-2.0..2.0), self.rng.gen_range(-2.0..2.0)];

                // For GPU transitions, start the growth with a very small size (will grow via GPU)
                particle.size = 0.1; // Start tiny, GPU will handle the growth

                // Set transition fields for grow transition
                particle.transition_start = self.current_time;
                particle.transition_type = 0; // 0 = grow
                particle.is_active = false; // GPU will set this to true at transition start

                // console_log!(
                //     "🌱 Initialized particle {} with target_size={:.2}, starting size=0.1, transition_start={:.3}, is_active=true",
                //     i,
                //     particle.target_size,
                //     self.current_time
                // );
            }
        }
    }

    // Initialize particles for shrink transition
    fn initialize_particles_for_shrink_transition(&mut self, start_index: u32, end_index: u32) {
        for i in start_index..end_index {
            if let Some(particle) = self.particle_system.get_particle_mut(i as usize) {
                // Set transition fields for shrink transition
                particle.transition_start = self.current_time;
                particle.transition_type = 1; // 1 = shrink
                                              // Keep is_active = true during transition, will be set to false when transition completes

                // console_log!(
                //     "🍂 Set shrink transition for particle {} at time {:.3}, is_active remains true during transition",
                //     i,
                //     self.current_time
                // );
            }
        }
    }

    // Temperature-based background color mapping - now moved to SimulationParams as static function

    // Set temperature and update all temperature-related simulation parameters
    #[wasm_bindgen]
    pub fn set_temperature(&mut self, temp: f32) {
        self.simulation_params.apply_temperature(temp);
        console_log!(
            "🌡️ Temperature set to {:.1}°C → applied to simulation parameters",
            temp.max(3.0).min(40.0)
        );
    }

    // Set pressure and update all pressure-related simulation parameters
    #[wasm_bindgen]
    pub fn set_pressure(&mut self, pressure: f32) {
        self.simulation_params.apply_pressure(pressure);
        console_log!(
            "🔧 Pressure set to {:.1} → applied to simulation parameters",
            pressure.max(0.0).min(350.0)
        );
    }

    // Set pH and update all pH-related simulation parameters
    #[wasm_bindgen]
    pub fn set_ph(&mut self, ph: f32) {
        self.simulation_params.apply_ph(ph);
        console_log!(
            "🧪 pH set to {:.1} → applied to simulation parameters (optimum ~10)",
            ph.max(0.0).min(14.0)
        );
    }

    // Set particle opacity (0.0 = invisible, 1.0 = fully opaque)
    #[wasm_bindgen]
    pub fn set_particle_opacity(&mut self, opacity: f32) {
        self.particle_system.particle_opacity = opacity.clamp(0.0, 1.0);
    }

    // Set color for a specific particle type (sRGB 0-1 range)
    #[wasm_bindgen]
    pub fn set_type_color(&mut self, type_idx: usize, r: f32, g: f32, b: f32) {
        if type_idx < 5 {
            self.particle_system.type_colors[type_idx] =
                [r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)];
        }
    }

    // Set electrical activity and update all electrical-related simulation parameters
    #[wasm_bindgen]
    pub fn set_electrical_activity(&mut self, electrical_activity: f32) {
        self.simulation_params
            .apply_electrical_activity(electrical_activity);
        console_log!(
            "⚡ Electrical activity set to {:.2} → applied to simulation parameters",
            electrical_activity.max(0.0).min(3.0)
        );
    }

    // Lightning system access - now handled by GPU compute shader
    #[wasm_bindgen]
    pub fn get_lightning_segments_buffer(&self) -> Vec<u8> {
        // Return empty buffer - lightning is now generated by GPU compute shader
        Vec::new()
    }

    #[wasm_bindgen]
    pub fn get_lightning_bolts_buffer(&self) -> Vec<u8> {
        // Return empty buffer - lightning is now generated by GPU compute shader
        Vec::new()
    }

    // Generate new random interaction rules
    #[wasm_bindgen]
    pub fn regenerate_rules(&mut self) {
        self.interaction_rules = InteractionRules::new_random(&mut self.rng);
        console_log!("🔄 Generated new interaction rules");
    }

    // Regenerate interaction rules for physics testing
    #[wasm_bindgen]
    pub fn regenerate_interaction_rules(&mut self) {
        self.interaction_rules = InteractionRules::new_random(&mut self.rng);
        console_log!("🎲 Generated new interaction rules");
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
        48 // pos(8) + vel(8) + type(4) + size(4) + target_size(4) + transition_start(4) + transition_type(4) + is_active(4) + padding(8) = 48 bytes (16-byte aligned)
    }

    #[wasm_bindgen]
    pub fn get_sim_params_size_bytes(&self) -> u32 {
        self.simulation_params.get_buffer_size()
    }

    // Debug information
    #[wasm_bindgen]
    pub fn get_debug_info(&self) -> String {
        format!(
            "Frame: {}, Time: {:.2}s, Particles: {}/{}, Current: {}",
            self.frame_count,
            self.current_time,
            self.particle_system.get_active_count(),
            self.particle_system.get_max_particles(),
            self.get_particle_count()
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

        // Calculate the center view window (canvas size) in the virtual world
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
        // Check if GPU transition is complete and finalize it
        if self
            .simulation_params
            .is_transition_complete(self.current_time)
            && self.simulation_params.transition_active
        {
            let elapsed = self.current_time - self.simulation_params.transition_start_time;
            console_log!(
                "🕒 Transition completing: elapsed={:.3}s, duration={:.3}s, start_time={:.3}s, current_time={:.3}s",
                elapsed,
                self.simulation_params.transition_duration,
                self.simulation_params.transition_start_time,
                self.current_time
            );

            let target_count = self.simulation_params.transition_end_count;

            if self.simulation_params.transition_is_grow {
                console_log!(
                    "🌱 Completed grow transition - GPU handled all size and activation changes"
                );
            } else {
                // For shrink transitions, update the CPU-side active count to match GPU state
                self.particle_system.set_active_count(target_count);
                self.simulation_params.set_num_particles(target_count);

                console_log!(
                    "🍂 Completed shrink transition - CPU active count now matches GPU: {}",
                    target_count
                );
            }

            self.simulation_params.stop_particle_transition();
        }

        if let Some(ref mut renderer) = self.renderer {
            // Lightning data is now generated by GPU compute shader, pass empty buffers
            let lightning_segments_data = Vec::new();
            let lightning_bolts_data = Vec::new();

            // Use WebGPU renderer - lightning will be generated by compute shader
            renderer.render(
                &self.particle_system,
                &self.simulation_params,
                &self.interaction_rules,
                &lightning_segments_data,
                &lightning_bolts_data,
            )?;
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

    // Update background color (separate R, G, B components)
    #[wasm_bindgen]
    pub fn update_background_color(&mut self, r: f32, g: f32, b: f32) {
        self.simulation_params.background_color_r = r;
        self.simulation_params.background_color_g = g;
        self.simulation_params.background_color_b = b;
        console_log!(
            "🎨 Background color updated: R={:.3}, G={:.3}, B={:.3}",
            r,
            g,
            b
        );
    }

    // Set particle count from pressure (using pressure-to-particle mapping)
    #[wasm_bindgen]
    pub fn set_particle_count_from_pressure(&mut self, pressure: f32) -> bool {
        let particle_count = self.simulation_params.pressure_to_particle_count(
            pressure,
            self.particle_system.get_max_particles(),
            self.particle_system.get_min_particles(),
        );
        console_log!("📊 Pressure {} → {} particles", pressure, particle_count);
        self.set_particle_count(particle_count)
    }

    // Set zoom level and update viewport parameters
    #[wasm_bindgen]
    pub fn set_zoom(&mut self, zoom_level: f32, center_x: Option<f32>, center_y: Option<f32>) {
        self.simulation_params
            .apply_zoom(zoom_level, center_x, center_y);
        console_log!(
            "🔍 Zoom set to {:.2}x → applied to viewport parameters",
            zoom_level.max(ZOOM_MIN).min(ZOOM_MAX)
        );
    }

    // === COMPREHENSIVE PARAMETER GETTERS ===
    // These getters allow the UI to read all current simulation parameter values

    #[wasm_bindgen]
    pub fn get_drift_x_per_second(&self) -> f32 {
        self.simulation_params.drift_x_per_second
    }

    #[wasm_bindgen]
    pub fn get_friction(&self) -> f32 {
        self.simulation_params.friction
    }

    #[wasm_bindgen]
    pub fn get_force_scale(&self) -> f32 {
        self.simulation_params.force_scale
    }

    #[wasm_bindgen]
    pub fn get_r_smooth(&self) -> f32 {
        self.simulation_params.r_smooth
    }

    #[wasm_bindgen]
    pub fn get_inter_type_attraction_scale(&self) -> f32 {
        self.simulation_params.inter_type_attraction_scale
    }

    #[wasm_bindgen]
    pub fn get_inter_type_radius_scale(&self) -> f32 {
        self.simulation_params.inter_type_radius_scale
    }

    #[wasm_bindgen]
    pub fn get_fisheye_strength(&self) -> f32 {
        self.simulation_params.fisheye_strength
    }

    #[wasm_bindgen]
    pub fn get_lenia_enabled(&self) -> bool {
        self.simulation_params.lenia_enabled
    }

    #[wasm_bindgen]
    pub fn get_lenia_growth_mu(&self) -> f32 {
        self.simulation_params.lenia_growth_mu
    }

    #[wasm_bindgen]
    pub fn get_lenia_growth_sigma(&self) -> f32 {
        self.simulation_params.lenia_growth_sigma
    }

    #[wasm_bindgen]
    pub fn get_lenia_kernel_radius(&self) -> f32 {
        self.simulation_params.lenia_kernel_radius
    }

    #[wasm_bindgen]
    pub fn get_lightning_frequency(&self) -> f32 {
        self.simulation_params.lightning_frequency
    }

    #[wasm_bindgen]
    pub fn get_lightning_intensity(&self) -> f32 {
        self.simulation_params.lightning_intensity
    }

    #[wasm_bindgen]
    pub fn get_lightning_duration(&self) -> f32 {
        self.simulation_params.lightning_duration
    }

    // Add missing getter methods that are used in main.ts
    #[wasm_bindgen]
    pub fn get_particle_render_size(&self) -> f32 {
        self.simulation_params.particle_render_size
    }

    #[wasm_bindgen]
    pub fn get_flat_force(&self) -> bool {
        self.simulation_params.flat_force
    }

    // === COMPREHENSIVE PARAMETER SETTERS ===
    // These setters allow the UI to update individual parameters directly

    #[wasm_bindgen]
    pub fn set_drift_x_per_second(&mut self, value: f32) {
        self.simulation_params.drift_x_per_second = value;
        console_log!("🔧 Individual parameter: drift_x_per_second = {:.2}", value);
    }

    #[wasm_bindgen]
    pub fn set_friction(&mut self, value: f32) {
        self.simulation_params.friction = value.max(0.01).min(1.0);
        console_log!(
            "🔧 Individual parameter: friction = {:.3}",
            self.simulation_params.friction
        );
    }

    #[wasm_bindgen]
    pub fn set_force_scale(&mut self, value: f32) {
        self.simulation_params.force_scale = value.max(100.0).min(500.0);
        console_log!(
            "🔧 Individual parameter: force_scale = {:.1}",
            self.simulation_params.force_scale
        );
    }

    #[wasm_bindgen]
    pub fn set_r_smooth(&mut self, value: f32) {
        self.simulation_params.r_smooth = value.max(0.1).min(20.0);
        console_log!(
            "🔧 Individual parameter: r_smooth = {:.2}",
            self.simulation_params.r_smooth
        );
    }

    #[wasm_bindgen]
    pub fn set_inter_type_attraction_scale(&mut self, value: f32) {
        self.simulation_params.inter_type_attraction_scale = value.max(0.0).min(3.0);
        console_log!(
            "🔧 Individual parameter: inter_type_attraction_scale = {:.2}",
            self.simulation_params.inter_type_attraction_scale
        );
    }

    #[wasm_bindgen]
    pub fn set_inter_type_radius_scale(&mut self, value: f32) {
        self.simulation_params.inter_type_radius_scale = value.max(0.1).min(2.0);
        console_log!(
            "🔧 Individual parameter: inter_type_radius_scale = {:.2}",
            self.simulation_params.inter_type_radius_scale
        );
    }

    #[wasm_bindgen]
    pub fn set_fisheye_strength(&mut self, value: f32) {
        self.simulation_params.fisheye_strength = value.max(0.0).min(3.0);
        console_log!(
            "🔧 Individual parameter: fisheye_strength = {:.2}",
            self.simulation_params.fisheye_strength
        );
    }

    #[wasm_bindgen]
    pub fn set_lenia_enabled(&mut self, enabled: bool) {
        self.simulation_params.lenia_enabled = enabled;
        console_log!("🔧 Individual parameter: lenia_enabled = {}", enabled);
    }

    #[wasm_bindgen]
    pub fn set_lenia_growth_mu(&mut self, value: f32) {
        self.simulation_params.lenia_growth_mu = value.max(0.05).min(0.3);
        console_log!(
            "🔧 Individual parameter: lenia_growth_mu = {:.3}",
            self.simulation_params.lenia_growth_mu
        );
    }

    #[wasm_bindgen]
    pub fn set_lenia_growth_sigma(&mut self, value: f32) {
        self.simulation_params.lenia_growth_sigma = value.max(0.005).min(0.05);
        console_log!(
            "🔧 Individual parameter: lenia_growth_sigma = {:.4}",
            self.simulation_params.lenia_growth_sigma
        );
    }

    #[wasm_bindgen]
    pub fn set_lenia_kernel_radius(&mut self, value: f32) {
        self.simulation_params.lenia_kernel_radius = value.max(20.0).min(100.0);
        console_log!(
            "🔧 Individual parameter: lenia_kernel_radius = {:.1}",
            self.simulation_params.lenia_kernel_radius
        );
    }

    #[wasm_bindgen]
    pub fn set_lightning_frequency(&mut self, value: f32) {
        self.simulation_params.lightning_frequency = value.max(0.0).min(2.0);
        console_log!(
            "🔧 Individual parameter: lightning_frequency = {:.2}",
            self.simulation_params.lightning_frequency
        );
    }

    #[wasm_bindgen]
    pub fn set_lightning_intensity(&mut self, value: f32) {
        self.simulation_params.lightning_intensity = value.max(0.0).min(2.0);
        console_log!(
            "🔧 Individual parameter: lightning_intensity = {:.2}",
            self.simulation_params.lightning_intensity
        );
    }

    #[wasm_bindgen]
    pub fn set_lightning_duration(&mut self, value: f32) {
        self.simulation_params.lightning_duration = value.max(0.1).min(2.0);
        console_log!(
            "🔧 Individual parameter: lightning_duration = {:.2}",
            self.simulation_params.lightning_duration
        );
    }

    #[wasm_bindgen]
    pub fn set_flat_force(&mut self, enabled: bool) {
        self.simulation_params.flat_force = enabled;
        console_log!("🔧 Individual parameter: flat_force = {}", enabled);
    }

    #[wasm_bindgen]
    pub fn set_particle_render_size(&mut self, value: f32) {
        self.simulation_params.particle_render_size =
            value.max(PARTICLE_SIZE_MIN).min(PARTICLE_SIZE_MAX);

        // Update all existing particle sizes
        self.particle_system
            .update_particle_sizes(self.simulation_params.particle_render_size, &mut self.rng);

        // Update GPU buffers with the new particle sizes
        if let Some(renderer) = &mut self.renderer {
            renderer.update_particle_sizes(&self.particle_system);
        }

        console_log!(
            "🔧 Individual parameter: particle_render_size = {:.1}",
            self.simulation_params.particle_render_size
        );
    }

    /// Check if lightning is currently visible (simple status check)
    #[wasm_bindgen]
    pub fn is_lightning_visible(&self) -> bool {
        // Return true if there's electrical activity that could generate lightning
        // This is a simplified check that doesn't require GPU buffer reads
        self.simulation_params.lightning_frequency > 0.0
            && self.simulation_params.inter_type_attraction_scale > 0.1
    }

    /// Get current electrical activity level for lightning generation
    #[wasm_bindgen]
    pub fn get_electrical_activity(&self) -> f32 {
        self.simulation_params.inter_type_attraction_scale
    }
}

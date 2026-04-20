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

// Audio system only for native builds (oud mp3-systeem, voor referentie)
#[cfg(not(target_arch = "wasm32"))]
mod audio;

// Gedeelde DSP-primitieven (SawOscillator, BiquadLPF, NoiseGen, Stem)
pub mod dsp;

// Sonificatie: parameter → StemState mapping — gedeeld tussen native en WASM
pub mod sonification;

// Audio engine: supersaw synthesizer via rodio (native only)
#[cfg(not(target_arch = "wasm32"))]
pub mod audio_engine;

// Audio engine: supersaw synthesizer via Web Audio API (WASM only)
#[cfg(target_arch = "wasm32")]
pub mod audio_engine_wasm;

// GPU stats reader: particle-type statistieken voor sonificatie (native only)
#[cfg(not(target_arch = "wasm32"))]
pub mod stats_reader;

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
    // Continuous rule evolution (shared logic, lives in interaction_rules.rs)
    rule_evolution: RuleEvolution,
    last_seen_flash_id: u32,
    // Sonificatie
    audio_engine: Option<audio_engine_wasm::WasmAudioEngine>,
    sonification_state: sonification::SonificationState,
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

        let rule_evolution = RuleEvolution::new(interaction_rules.clone(), &mut rng);

        Self {
            particle_system,
            simulation_params,
            interaction_rules,
            rng,
            current_time: 0.0,
            frame_count: 0,
            renderer: None,
            rule_evolution,
            last_seen_flash_id: 0,
            audio_engine: None, // Lazy: aangemaakt bij eerste set_audio_paused(false)
            sonification_state: sonification::SonificationState::default(),
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
                "🎯 Frame {}: particles={}, fps={:.1}",
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
        // === Continuous rule evolution (lerp) ===
        self.interaction_rules = self.rule_evolution.tick(delta_time, &mut self.rng).clone();

        // === Sonificatie ===
        self.sonification_state = sonification::compute_sonification(
            &self.simulation_params,
            None, // GPU-stats niet beschikbaar in WASM (voor nu)
            None,
            &self.sonification_state,
        );
        if let Some(ref engine) = self.audio_engine {
            engine.update(self.sonification_state.clone());
        }

        self.frame_count += 1;
    }

    #[wasm_bindgen]
    pub fn set_audio_paused(&mut self, paused: bool) {
        if !paused && self.audio_engine.is_none() {
            // Lazy init: aanmaken bij eerste klik (user-gesture context vereist)
            match audio_engine_wasm::WasmAudioEngine::new() {
                Ok(engine) => { self.audio_engine = Some(engine); }
                Err(e) => { crate::console_log!("⚠️ Audio engine fout: {:?}", e); return; }
            }
        }
        if let Some(ref mut engine) = self.audio_engine {
            engine.set_paused(paused);
        }
    }

    #[wasm_bindgen]
    pub fn set_audio_volume(&self, v: f32) {
        if let Some(ref engine) = self.audio_engine {
            engine.set_master_volume(v);
        }
    }

    #[wasm_bindgen]
    pub fn is_audio_active(&self) -> bool {
        self.audio_engine.is_some()
    }

    /// Geeft de audio MediaStream terug voor gebruik in MediaRecorder.
    /// Returnt None als de audio engine niet beschikbaar is.
    #[wasm_bindgen]
    pub fn get_audio_stream(&self) -> Option<web_sys::MediaStream> {
        self.audio_engine.as_ref().map(|e| e.get_media_stream())
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

            if count > current_count {
                self.particle_system.set_active_count(count);
                self.simulation_params.set_num_particles(count);
                self.initialize_particles_for_grow_transition(current_count, count);
                if let Some(ref mut renderer) = self.renderer {
                    renderer.update_particle_transitions(&self.particle_system);
                }
            } else {
                self.initialize_particles_for_shrink_transition(count, current_count);
                if let Some(ref mut renderer) = self.renderer {
                    renderer.update_particle_transitions(&self.particle_system);
                }
            }
        } else {
            self.simulation_params.set_num_particles(count);
        }

        true
    }

    fn initialize_particles_for_grow_transition(&mut self, start_index: u32, end_index: u32) {
        for i in start_index..end_index {
            if let Some(particle) = self.particle_system.get_particle_mut(i as usize) {
                particle.position = [
                    self.rng.gen_range(0.0..self.simulation_params.virtual_world_width),
                    self.rng.gen_range(0.0..self.simulation_params.virtual_world_height),
                ];
                particle.velocity = [self.rng.gen_range(-2.0..2.0), self.rng.gen_range(-2.0..2.0)];
                particle.size = 0.1;
                particle.transition_start = self.current_time;
                particle.transition_type = 0;
                particle.is_active = false;
            }
        }
    }

    fn initialize_particles_for_shrink_transition(&mut self, start_index: u32, end_index: u32) {
        for i in start_index..end_index {
            if let Some(particle) = self.particle_system.get_particle_mut(i as usize) {
                particle.transition_start = self.current_time;
                particle.transition_type = 1;
            }
        }
    }

    #[wasm_bindgen]
    pub fn set_temperature(&mut self, temp: f32) {
        self.simulation_params.apply_temperature(temp);
    }

    #[wasm_bindgen]
    pub fn set_pressure(&mut self, pressure: f32) {
        self.simulation_params.apply_pressure(pressure);
    }

    #[wasm_bindgen]
    pub fn set_ph(&mut self, ph: f32) {
        self.simulation_params.apply_ph(ph);
    }

    #[wasm_bindgen]
    pub fn set_particle_opacity(&mut self, opacity: f32) {
        self.particle_system.particle_opacity = opacity.clamp(0.0, 1.0);
    }

    #[wasm_bindgen]
    pub fn set_type_color(&mut self, type_idx: usize, r: f32, g: f32, b: f32) {
        if type_idx < 7 {
            self.particle_system.type_colors[type_idx] =
                [r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)];
        }
    }

    #[wasm_bindgen]
    pub fn get_type_colors_rgb(&self) -> Vec<f32> {
        self.particle_system
            .type_colors
            .iter()
            .flat_map(|c| c.iter().copied())
            .collect()
    }

    #[wasm_bindgen]
    pub fn set_electrical_activity(&mut self, electrical_activity: f32) {
        self.simulation_params.apply_electrical_activity(electrical_activity);
    }

    #[wasm_bindgen]
    pub fn get_lightning_segments_buffer(&self) -> Vec<u8> { Vec::new() }

    #[wasm_bindgen]
    pub fn get_lightning_bolts_buffer(&self) -> Vec<u8> { Vec::new() }

    #[wasm_bindgen]
    pub fn regenerate_rules(&mut self) {
        self.interaction_rules = InteractionRules::new_random(&mut self.rng);
    }

    #[wasm_bindgen]
    pub fn regenerate_interaction_rules(&mut self) {
        self.interaction_rules = InteractionRules::new_random(&mut self.rng);
    }

    #[wasm_bindgen]
    pub fn set_rules_lerp_duration(&mut self, seconds: f32) {
        self.rule_evolution.set_duration(seconds);
    }

    #[wasm_bindgen]
    pub fn snap_to_new_rules(&mut self) {
        self.rule_evolution.snap_to_new(&mut self.rng);
        console_log!("⚡ Super-lightning: rules snapped immediately, new lerp started");
    }

    #[wasm_bindgen]
    pub fn get_rules_lerp_progress(&self) -> f32 {
        self.rule_evolution.progress()
    }

    #[wasm_bindgen]
    pub fn get_max_particles(&self) -> u32 { self.particle_system.get_max_particles() }

    #[wasm_bindgen]
    pub fn get_min_particles(&self) -> u32 { self.particle_system.get_min_particles() }

    #[wasm_bindgen]
    pub fn get_num_types(&self) -> u32 { self.particle_system.get_num_types() }

    #[wasm_bindgen]
    pub fn get_particle_size_bytes(&self) -> u32 { 48 }

    #[wasm_bindgen]
    pub fn get_sim_params_size_bytes(&self) -> u32 { self.simulation_params.get_buffer_size() }

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

        let bg_color = format!(
            "rgb({},{},{})",
            (self.simulation_params.background_color_r * 255.0) as u8,
            (self.simulation_params.background_color_g * 255.0) as u8,
            (self.simulation_params.background_color_b * 255.0) as u8
        );
        context.set_fill_style_str(&bg_color);
        context.fill_rect(0.0, 0.0, canvas_width as f64, canvas_height as f64);

        let world_width = self.simulation_params.virtual_world_width;
        let world_height = self.simulation_params.virtual_world_height;
        let view_left = world_width / 2.0 - canvas_width / 2.0;
        let view_top  = world_height / 2.0 - canvas_height / 2.0;

        for i in 0..self.particle_system.get_active_count() {
            if let Some(particle) = self.particle_system.get_particle(i as usize) {
                let cx = particle.position[0] - view_left;
                let cy = particle.position[1] - view_top;
                if cx < 0.0 || cx > canvas_width || cy < 0.0 || cy > canvas_height { continue; }
                let hue = (particle.particle_type as f32 / self.simulation_params.num_types as f32) * 360.0;
                context.set_fill_style_str(&format!("hsl({}, 70%, 60%)", hue));
                context.begin_path();
                context.arc(cx as f64, cy as f64, particle.size as f64, 0.0, 2.0 * std::f64::consts::PI)?;
                context.fill();
            }
        }
        Ok(())
    }

    #[wasm_bindgen]
    pub async fn initialize_webgpu(&mut self, canvas: web_sys::HtmlCanvasElement) -> Result<(), JsValue> {
        match WebGpuRenderer::new(&canvas).await {
            Ok(renderer) => { self.renderer = Some(renderer); Ok(()) }
            Err(e) => Err(e)
        }
    }

    #[wasm_bindgen]
    pub async fn check_super_lightning(&mut self) -> u32 {
        let mut renderer = match self.renderer.take() { Some(r) => r, None => return 0 };
        let result = match renderer.update_lightning_cache().await {
            Ok(()) => {
                if let Some(bolt) = renderer.get_cached_lightning_bolt() {
                    if bolt.flash_id > self.last_seen_flash_id {
                        self.last_seen_flash_id = bolt.flash_id;
                        if bolt.is_super() { 1 } else { 0 }
                    } else { 0 }
                } else { 0 }
            }
            Err(_) => 0,
        };
        self.renderer = Some(renderer);
        if result == 1 {
            self.rule_evolution.snap_to_new(&mut self.rng);
            console_log!("⚡ Super-lightning: rules snapped immediately, new lerp started");
        }
        result
    }

    #[wasm_bindgen]
    pub fn render(&mut self) -> Result<(), JsValue> {
        if self.simulation_params.is_transition_complete(self.current_time)
            && self.simulation_params.transition_active
        {
            let target_count = self.simulation_params.transition_end_count;
            if !self.simulation_params.transition_is_grow {
                self.particle_system.set_active_count(target_count);
                self.simulation_params.set_num_particles(target_count);
            }
            self.simulation_params.stop_particle_transition();
        }

        if let Some(ref mut renderer) = self.renderer {
            renderer.render(
                &self.particle_system,
                &self.simulation_params,
                &self.interaction_rules,
                &Vec::new(),
                &Vec::new(),
            )?;
        } else {
            self.render_to_canvas("canvas")?;
        }
        Ok(())
    }

    #[wasm_bindgen]
    pub fn render_test_graphics(&self) -> Result<(), JsValue> {
        let window = web_sys::window().unwrap();
        let document = window.document().unwrap();
        let canvas = document.get_element_by_id("canvas").unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()?;
        let context = canvas.get_context("2d")?.unwrap()
            .dyn_into::<web_sys::CanvasRenderingContext2d>()?;
        context.set_fill_style_str("#ff0000");
        context.begin_path();
        context.arc(400.0, 400.0, 50.0, 0.0, 2.0 * std::f64::consts::PI)?;
        context.fill();
        Ok(())
    }

    #[wasm_bindgen]
    pub fn update_background_color(&mut self, r: f32, g: f32, b: f32) {
        self.simulation_params.background_color_r = r;
        self.simulation_params.background_color_g = g;
        self.simulation_params.background_color_b = b;
    }

    #[wasm_bindgen]
    pub fn set_particle_count_from_pressure(&mut self, pressure: f32) -> bool {
        let count = self.simulation_params.pressure_to_particle_count(
            pressure,
            self.particle_system.get_max_particles(),
            self.particle_system.get_min_particles(),
        );
        self.set_particle_count(count)
    }

    #[wasm_bindgen]
    pub fn set_zoom(&mut self, slider_value: f32, center_x: Option<f32>, center_y: Option<f32>) {
        let zoom = SimulationParams::slider_to_zoom(slider_value);
        self.simulation_params.apply_zoom(zoom, center_x, center_y);
    }

    #[wasm_bindgen]
    pub fn get_zoom(&self) -> f32 {
        self.simulation_params.current_zoom_level
    }

    #[wasm_bindgen] pub fn get_drift_x_per_second(&self) -> f32 { self.simulation_params.drift_x_per_second }
    #[wasm_bindgen] pub fn get_friction(&self) -> f32 { self.simulation_params.friction }
    #[wasm_bindgen] pub fn get_force_scale(&self) -> f32 { self.simulation_params.force_scale }
    #[wasm_bindgen] pub fn get_r_smooth(&self) -> f32 { self.simulation_params.r_smooth }
    #[wasm_bindgen] pub fn get_inter_type_attraction_scale(&self) -> f32 { self.simulation_params.inter_type_attraction_scale }
    #[wasm_bindgen] pub fn get_inter_type_radius_scale(&self) -> f32 { self.simulation_params.inter_type_radius_scale }
    #[wasm_bindgen] pub fn get_fisheye_strength(&self) -> f32 { self.simulation_params.fisheye_strength }
    #[wasm_bindgen] pub fn get_lenia_enabled(&self) -> bool { self.simulation_params.lenia_enabled }
    #[wasm_bindgen] pub fn get_lenia_growth_mu(&self) -> f32 { self.simulation_params.lenia_growth_mu }
    #[wasm_bindgen] pub fn get_lenia_growth_sigma(&self) -> f32 { self.simulation_params.lenia_growth_sigma }
    #[wasm_bindgen] pub fn get_lenia_kernel_radius(&self) -> f32 { self.simulation_params.lenia_kernel_radius }
    #[wasm_bindgen] pub fn get_lightning_frequency(&self) -> f32 { self.simulation_params.lightning_frequency }
    #[wasm_bindgen] pub fn get_lightning_intensity(&self) -> f32 { self.simulation_params.lightning_intensity }
    #[wasm_bindgen] pub fn get_lightning_duration(&self) -> f32 { self.simulation_params.lightning_duration }
    #[wasm_bindgen] pub fn get_particle_render_size(&self) -> f32 { self.simulation_params.particle_render_size }
    #[wasm_bindgen] pub fn get_flat_force(&self) -> bool { self.simulation_params.flat_force }

    #[wasm_bindgen] pub fn set_drift_x_per_second(&mut self, v: f32) { self.simulation_params.drift_x_per_second = v; }
    #[wasm_bindgen] pub fn set_friction(&mut self, v: f32) { self.simulation_params.friction = v.max(0.01).min(1.0); }
    #[wasm_bindgen] pub fn set_force_scale(&mut self, v: f32) { self.simulation_params.force_scale = v.max(100.0).min(500.0); }
    #[wasm_bindgen] pub fn set_r_smooth(&mut self, v: f32) { self.simulation_params.r_smooth = v.max(0.1).min(20.0); }
    #[wasm_bindgen] pub fn set_inter_type_attraction_scale(&mut self, v: f32) { self.simulation_params.inter_type_attraction_scale = v.max(-3.0).min(3.0); }
    #[wasm_bindgen] pub fn set_inter_type_radius_scale(&mut self, v: f32) { self.simulation_params.inter_type_radius_scale = v.max(0.1).min(2.0); }
    #[wasm_bindgen] pub fn set_fisheye_strength(&mut self, v: f32) { self.simulation_params.fisheye_strength = v.max(0.0).min(3.0); }
    #[wasm_bindgen] pub fn set_lenia_enabled(&mut self, v: bool) { self.simulation_params.lenia_enabled = v; }
    #[wasm_bindgen] pub fn set_lenia_growth_mu(&mut self, v: f32) { self.simulation_params.lenia_growth_mu = v.max(0.05).min(0.3); }
    #[wasm_bindgen] pub fn set_lenia_growth_sigma(&mut self, v: f32) { self.simulation_params.lenia_growth_sigma = v.max(0.005).min(0.05); }
    #[wasm_bindgen] pub fn set_lenia_kernel_radius(&mut self, v: f32) { self.simulation_params.lenia_kernel_radius = v.max(20.0).min(100.0); }
    #[wasm_bindgen] pub fn set_lightning_frequency(&mut self, v: f32) { self.simulation_params.lightning_frequency = v.max(0.0).min(2.0); }
    #[wasm_bindgen] pub fn set_lightning_intensity(&mut self, v: f32) { self.simulation_params.lightning_intensity = v.max(0.0).min(2.0); }
    #[wasm_bindgen] pub fn set_lightning_duration(&mut self, v: f32) { self.simulation_params.lightning_duration = v.max(0.1).min(2.0); }
    #[wasm_bindgen] pub fn set_flat_force(&mut self, v: bool) { self.simulation_params.flat_force = v; }

    #[wasm_bindgen]
    pub fn set_particle_render_size(&mut self, value: f32) {
        self.simulation_params.particle_render_size = value.max(PARTICLE_SIZE_MIN).min(PARTICLE_SIZE_MAX);
        self.particle_system.update_particle_sizes(self.simulation_params.particle_render_size, &mut self.rng);
        if let Some(renderer) = &mut self.renderer {
            renderer.update_particle_sizes(&self.particle_system);
        }
    }

    #[wasm_bindgen]
    pub fn is_lightning_visible(&self) -> bool {
        self.simulation_params.lightning_frequency > 0.0
            && self.simulation_params.inter_type_attraction_scale > 0.1
    }

    #[wasm_bindgen]
    pub fn get_electrical_activity(&self) -> f32 {
        self.simulation_params.inter_type_attraction_scale
    }
}

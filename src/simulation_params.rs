use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// Import console_log macro
use crate::console_log;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationParams {
    pub delta_time: f32,
    pub friction: f32,
    pub num_particles: u32,
    pub num_types: u32,
    pub virtual_world_width: f32,
    pub virtual_world_height: f32,
    pub canvas_render_width: f32,
    pub canvas_render_height: f32,
    pub virtual_world_offset_x: f32,
    pub virtual_world_offset_y: f32,
    // Viewport size for zoom (the area of virtual world being viewed)
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub boundary_mode: u32,
    pub particle_render_size: f32,
    pub force_scale: f32,
    pub r_smooth: f32,
    pub flat_force: bool,
    pub drift_x_per_second: f32,
    pub inter_type_attraction_scale: f32,
    pub inter_type_radius_scale: f32,
    pub time: f32,
    pub fisheye_strength: f32,
    // Note: Arrays can't be directly used with wasm-bindgen, so we'll use getters/setters
    pub background_color_r: f32,
    pub background_color_g: f32,
    pub background_color_b: f32,
    pub lenia_enabled: bool,
    pub lenia_growth_mu: f32,
    pub lenia_growth_sigma: f32,
    pub lenia_kernel_radius: f32,
    pub lightning_frequency: f32,
    pub lightning_intensity: f32,
    pub lightning_duration: f32,
    // Particle transition parameters for GPU-based size transitions
    pub transition_active: bool,
    pub transition_start_time: f32,
    pub transition_duration: f32,
    pub transition_start_count: u32,
    pub transition_end_count: u32,
    pub transition_is_grow: bool, // true for grow, false for shrink

    // Spatial grid optimization parameters
    pub spatial_grid_enabled: bool,
    pub spatial_grid_cell_size: f32,
    pub spatial_grid_width: u32,
    pub spatial_grid_height: u32,
}

// Native version (same fields, no wasm_bindgen annotations)
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationParams {
    pub delta_time: f32,
    pub friction: f32,
    pub num_particles: u32,
    pub num_types: u32,
    pub virtual_world_width: f32,
    pub virtual_world_height: f32,
    pub canvas_render_width: f32,
    pub canvas_render_height: f32,
    pub virtual_world_offset_x: f32,
    pub virtual_world_offset_y: f32,
    // Viewport size for zoom (the area of virtual world being viewed)
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub boundary_mode: u32,
    pub particle_render_size: f32,
    pub force_scale: f32,
    pub r_smooth: f32,
    pub flat_force: bool,
    pub drift_x_per_second: f32,
    pub inter_type_attraction_scale: f32,
    pub inter_type_radius_scale: f32,
    pub time: f32,
    pub fisheye_strength: f32,
    pub background_color_r: f32,
    pub background_color_g: f32,
    pub background_color_b: f32,
    pub lenia_enabled: bool,
    pub lenia_growth_mu: f32,
    pub lenia_growth_sigma: f32,
    pub lenia_kernel_radius: f32,
    pub lightning_frequency: f32,
    pub lightning_intensity: f32,
    pub lightning_duration: f32,
    // Particle transition parameters for GPU-based size transitions
    pub transition_active: bool,
    pub transition_start_time: f32,
    pub transition_duration: f32,
    pub transition_start_count: u32,
    pub transition_end_count: u32,
    pub transition_is_grow: bool, // true for grow, false for shrink

    // Spatial grid optimization parameters
    pub spatial_grid_enabled: bool,
    pub spatial_grid_cell_size: f32,
    pub spatial_grid_width: u32,
    pub spatial_grid_height: u32,
}

impl SimulationParams {
    pub fn new() -> Self {
        Self {
            delta_time: 1.0 / 60.0,
            friction: 0.1,
            num_particles: 6400, // Initialize with all particles active to prevent GPU buffer hiccups
            num_types: 5,
            virtual_world_width: 2400.0,
            virtual_world_height: 2400.0,
            canvas_render_width: 800.0,
            canvas_render_height: 800.0,
            // Start at top-left corner initially (where particles are visible)
            virtual_world_offset_x: 0.0,
            virtual_world_offset_y: 0.0,
            // Default viewport is the entire virtual world (zoom level 1.0)
            viewport_width: 2400.0,
            viewport_height: 2400.0,
            boundary_mode: 1,           // Wrap mode
            particle_render_size: 12.0, // Increased from 12.0 to account for 3x scaling down (2400->800)
            force_scale: 400.0,
            r_smooth: 10.0, // Increased from 5.0 to make repulsion forces more visible
            flat_force: false,
            drift_x_per_second: 0.0, // Start with no drift to isolate particle interactions
            inter_type_attraction_scale: 1.0,
            inter_type_radius_scale: 1.0,
            time: 0.0,
            fisheye_strength: 3.0,
            background_color_r: 0.0,
            background_color_g: 0.0,
            background_color_b: 0.0,
            lenia_enabled: true, // Disabled by default to test basic particle interactions
            lenia_growth_mu: 0.15,
            lenia_growth_sigma: 0.02,
            lenia_kernel_radius: 60.0,
            lightning_frequency: 0.7,
            lightning_intensity: 1.0,
            lightning_duration: 0.6,
            transition_active: false,
            transition_start_time: 0.0,
            transition_duration: 1.0,
            transition_start_count: 0,
            transition_end_count: 0,
            transition_is_grow: true,

            // Spatial grid optimization - enabled by default with reasonable cell size
            spatial_grid_enabled: true,
            spatial_grid_cell_size: 120.0, // 120 units per cell (2400/120 = 20x20 grid)
            spatial_grid_width: 20,        // 20 cells horizontally
            spatial_grid_height: 20,       // 20 cells vertically
        }
    }

    pub fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    pub fn set_delta_time(&mut self, delta_time: f32) {
        self.delta_time = delta_time;
    }

    pub fn set_num_particles(&mut self, count: u32) {
        self.num_particles = count;
    }

    // Start a GPU-based particle transition
    pub fn start_particle_transition(&mut self, from_count: u32, to_count: u32, current_time: f32) {
        self.transition_active = true;
        self.transition_start_time = current_time;
        self.transition_duration = 1.5; // 1.5 second transitions
        self.transition_start_count = from_count;
        self.transition_end_count = to_count;
        self.transition_is_grow = to_count > from_count;
    }

    // Stop any active transition
    pub fn stop_particle_transition(&mut self) {
        self.transition_active = false;
    }

    // Check if transition is complete
    pub fn is_transition_complete(&self, current_time: f32) -> bool {
        if !self.transition_active {
            return true;
        }
        (current_time - self.transition_start_time) >= self.transition_duration
    }

    pub fn update_parameter(&mut self, name: &str, value: f32) -> bool {
        match name {
            "friction" => self.friction = value,
            "forceScale" => self.force_scale = value,
            "rSmooth" => self.r_smooth = value,
            "driftXPerSecond" => self.drift_x_per_second = value,
            "interTypeAttractionScale" => self.inter_type_attraction_scale = value,
            "interTypeRadiusScale" => self.inter_type_radius_scale = value,
            "fisheyeStrength" => self.fisheye_strength = value,
            "leniaGrowthMu" => self.lenia_growth_mu = value,
            "leniaGrowthSigma" => self.lenia_growth_sigma = value,
            "leniaKernelRadius" => self.lenia_kernel_radius = value,
            "lightningFrequency" => self.lightning_frequency = value,
            "lightningIntensity" => self.lightning_intensity = value,
            "lightningDuration" => self.lightning_duration = value,
            "spatialGridCellSize" => {
                self.spatial_grid_cell_size = value;
                // Recalculate grid dimensions when cell size changes
                self.spatial_grid_width = (self.virtual_world_width / value).ceil() as u32;
                self.spatial_grid_height = (self.virtual_world_height / value).ceil() as u32;
            }
            _ => return false,
        }
        true
    }

    pub fn set_background_color(&mut self, r: f32, g: f32, b: f32) {
        self.background_color_r = r;
        self.background_color_g = g;
        self.background_color_b = b;
    }

    pub fn get_background_color(&self) -> [f32; 3] {
        [
            self.background_color_r,
            self.background_color_g,
            self.background_color_b,
        ]
    }

    pub fn set_boolean_parameter(&mut self, name: &str, value: bool) -> bool {
        match name {
            "flatForce" => self.flat_force = value,
            "leniaEnabled" => self.lenia_enabled = value,
            "spatialGridEnabled" => self.spatial_grid_enabled = value,
            _ => return false,
        }
        true
    }

    // Convert to buffer format for GPU upload
    pub fn to_buffer(&self) -> Vec<u8> {
        self.to_buffer_with_particle_count(self.num_particles)
    }

    // Convert to buffer format for GPU upload with specific particle count
    pub fn to_buffer_with_particle_count(&self, actual_particle_count: u32) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(160); // 40 * 4 bytes = 160 bytes (added 4 spatial grid fields)

        // Match the exact layout from WGSL SimParams struct
        buffer.extend_from_slice(&self.delta_time.to_le_bytes()); // 0
        buffer.extend_from_slice(&self.friction.to_le_bytes()); // 1
        buffer.extend_from_slice(&actual_particle_count.to_le_bytes()); // 2 - Use actual count!
        buffer.extend_from_slice(&self.num_types.to_le_bytes()); // 3
        buffer.extend_from_slice(&self.virtual_world_width.to_le_bytes()); // 4
        buffer.extend_from_slice(&self.virtual_world_height.to_le_bytes()); // 5
        buffer.extend_from_slice(&self.canvas_render_width.to_le_bytes()); // 6
        buffer.extend_from_slice(&self.canvas_render_height.to_le_bytes()); // 7
        buffer.extend_from_slice(&self.virtual_world_offset_x.to_le_bytes()); // 8
        buffer.extend_from_slice(&self.virtual_world_offset_y.to_le_bytes()); // 9
                                                                              // NOTE: Removed viewport_width and viewport_height as they don't exist in WGSL
        buffer.extend_from_slice(&self.boundary_mode.to_le_bytes()); // 10
        buffer.extend_from_slice(&self.particle_render_size.to_le_bytes()); // 11
        buffer.extend_from_slice(&self.force_scale.to_le_bytes()); // 12
        buffer.extend_from_slice(&self.r_smooth.to_le_bytes()); // 13
        buffer.extend_from_slice(&(if self.flat_force { 1u32 } else { 0u32 }).to_le_bytes()); // 14
        buffer.extend_from_slice(&self.drift_x_per_second.to_le_bytes()); // 15
        buffer.extend_from_slice(&self.inter_type_attraction_scale.to_le_bytes()); // 16
        buffer.extend_from_slice(&self.inter_type_radius_scale.to_le_bytes()); // 17
        buffer.extend_from_slice(&self.time.to_le_bytes()); // 18
        buffer.extend_from_slice(&self.fisheye_strength.to_le_bytes()); // 19
        buffer.extend_from_slice(&self.background_color_r.to_le_bytes()); // 20
        buffer.extend_from_slice(&self.background_color_g.to_le_bytes()); // 21
        buffer.extend_from_slice(&self.background_color_b.to_le_bytes()); // 22
        buffer.extend_from_slice(&(if self.lenia_enabled { 1u32 } else { 0u32 }).to_le_bytes()); // 23
        buffer.extend_from_slice(&self.lenia_growth_mu.to_le_bytes()); // 24
        buffer.extend_from_slice(&self.lenia_growth_sigma.to_le_bytes()); // 25
        buffer.extend_from_slice(&self.lenia_kernel_radius.to_le_bytes()); // 26
        buffer.extend_from_slice(&self.lightning_frequency.to_le_bytes()); // 27
        buffer.extend_from_slice(&self.lightning_intensity.to_le_bytes()); // 28
        buffer.extend_from_slice(&self.lightning_duration.to_le_bytes()); // 29
                                                                          // Particle transition parameters
        buffer.extend_from_slice(&(if self.transition_active { 1u32 } else { 0u32 }).to_le_bytes()); // 30
        buffer.extend_from_slice(&self.transition_start_time.to_le_bytes()); // 31
        buffer.extend_from_slice(&self.transition_duration.to_le_bytes()); // 32
        buffer.extend_from_slice(&self.transition_start_count.to_le_bytes()); // 33
        buffer.extend_from_slice(&self.transition_end_count.to_le_bytes()); // 34
        buffer
            .extend_from_slice(&(if self.transition_is_grow { 1u32 } else { 0u32 }).to_le_bytes()); // 35

        // Spatial grid optimization parameters
        buffer.extend_from_slice(
            &(if self.spatial_grid_enabled {
                1u32
            } else {
                0u32
            })
            .to_le_bytes(),
        ); // 36
        buffer.extend_from_slice(&self.spatial_grid_cell_size.to_le_bytes()); // 37
        buffer.extend_from_slice(&self.spatial_grid_width.to_le_bytes()); // 38
        buffer.extend_from_slice(&self.spatial_grid_height.to_le_bytes()); // 39

        buffer
    }

    pub fn get_buffer_size(&self) -> u32 {
        160 // 40 * 4 bytes (added 4 spatial grid fields)
    }

    // === CENTRAL CONVERSION FUNCTIONS ===
    // These functions handle the complex mappings from user-friendly parameters
    // to internal simulation parameters, used by both WASM and native

    // Set temperature and update all temperature-related simulation parameters
    pub fn apply_temperature(&mut self, temp: f32) {
        // Clamp temperature to valid range (3°C to 40°C)
        let clamped_temp = temp.max(3.0).min(40.0);

        // 1. Update drift speed: temp [3, 40] → drift [0, -80]
        let drift = -((clamped_temp - 3.0) * 80.0) / 37.0;
        self.drift_x_per_second = drift;

        // 2. Update friction: exponential mapping temp [3, 40] → friction [0.98, 0.05]
        let normalized_temp = (clamped_temp - 3.0) / 37.0;
        let friction = 0.98 * (-3.0 * normalized_temp).exp();
        self.friction = friction;

        // 3. Update background color using HSLuv: temp [3, 40] → hue [215°, 15°]
        let (r, g, b) = Self::temperature_to_background_color(clamped_temp);
        self.background_color_r = r;
        self.background_color_g = g;
        self.background_color_b = b;
    }

    // Set pressure and update all pressure-related simulation parameters
    pub fn apply_pressure(&mut self, pressure: f32) {
        // Clamp pressure to valid range (0 to 350)
        let clamped_pressure = pressure.max(0.0).min(350.0);

        // 1. Update force scale: pressure [0, 350] → force_scale [100, 800]
        let force_scale = 100.0 + (clamped_pressure * 700.0) / 350.0;
        self.force_scale = force_scale;

        // 2. Update rSmooth: exponential mapping pressure [0, 350] → rSmooth [20, 0.1]
        let normalized_pressure = clamped_pressure / 350.0;
        let r_smooth = 20.0 * (-5.3 * normalized_pressure).exp();
        self.r_smooth = r_smooth;
    }

    // Set UV light and update all UV-related simulation parameters
    pub fn apply_uv_light(&mut self, uv: f32) {
        // Clamp UV to valid range (0 to 50)
        let clamped_uv = uv.max(0.0).min(50.0);

        // Update inter-type radius scale: UV [0, 50] → interTypeRadiusScale [0.1, 2.0]
        let inter_type_radius_scale = 0.1 + (clamped_uv / 50.0) * (2.0 - 0.1);
        self.inter_type_radius_scale = inter_type_radius_scale;
    }

    // Set electrical activity and update all electrical-related simulation parameters
    pub fn apply_electrical_activity(&mut self, electrical_activity: f32) {
        // Clamp electrical activity to valid range (0 to 3)
        let clamped_electrical = electrical_activity.max(0.0).min(3.0);

        // Update inter-type attraction scale: cubic mapping [0, 3] → interTypeAttractionScale [0, 3]
        let normalized_electrical = clamped_electrical / 3.0;
        let cubic_value = normalized_electrical * normalized_electrical * normalized_electrical;
        let inter_type_attraction_scale = cubic_value * 3.0;
        self.inter_type_attraction_scale = inter_type_attraction_scale;

        // Update lightning parameters based on electrical activity
        // Lightning frequency: 0 at min activity, 1.0 at max activity
        self.lightning_frequency = normalized_electrical;

        // Lightning intensity: 0.5 at min activity, 2.0 at max activity
        self.lightning_intensity = 0.5 + (normalized_electrical * 1.5);

        // Lightning duration: 0.3 at min activity, 0.8 at max activity
        self.lightning_duration = 0.3 + (normalized_electrical * 0.5);
    }

    // Set zoom level and update viewport parameters
    pub fn apply_zoom(&mut self, zoom_level: f32, center_x: Option<f32>, center_y: Option<f32>) {
        // Clamp zoom level to valid range (1.45 to 6.0)
        let clamped_zoom = zoom_level.max(1.45).min(6.0);

        // Calculate viewport size: at zoom 1.0 = full world (2400x2400), at zoom 2.0 = half world (1200x1200), etc.
        let viewport_width = 2400.0 / clamped_zoom;
        let viewport_height = 2400.0 / clamped_zoom;

        // Center the viewport around (1200, 1200) by default
        let center_x = center_x.unwrap_or(1200.0);
        let center_y = center_y.unwrap_or(1200.0);

        // Calculate offset to center the viewport
        let offset_x = center_x - (viewport_width / 2.0);
        let offset_y = center_y - (viewport_height / 2.0);

        // Clamp offsets to ensure viewport stays within virtual world bounds [0, 2400]
        let max_offset_x = 2400.0 - viewport_width;
        let max_offset_y = 2400.0 - viewport_height;

        let clamped_offset_x = offset_x.max(0.0).min(max_offset_x);
        let clamped_offset_y = offset_y.max(0.0).min(max_offset_y);

        // Update viewport offset AND size
        self.virtual_world_offset_x = clamped_offset_x;
        self.virtual_world_offset_y = clamped_offset_y;
        self.viewport_width = viewport_width;
        self.viewport_height = viewport_height;
    }

    // Temperature-based background color mapping using HSLuv (static helper)
    fn temperature_to_background_color(temp: f32) -> (f32, f32, f32) {
        // Temperature mapping: 3°C to 40°C → Hue 215° to 15°
        // Clamp temperature to valid range
        let clamped_temp = temp.max(3.0).min(40.0);

        // Normalize temperature: 0.0 at 3°C, 1.0 at 40°C
        let normalized_temp = (clamped_temp - 3.0) / (40.0 - 3.0);

        // Map to hue range: 215° (cold/blue) to 15° (hot/red)
        let hue = 215.0 - normalized_temp * 200.0; // 215° to 15°

        // Uniform saturation and lightness values for both platforms
        let (saturation, lightness) = (33.0, 66.0);

        // Convert HSLuv to RGB
        let (r, g, b) = hsluv::hsluv_to_rgb(hue as f64, saturation as f64, lightness as f64);

        // Debug info for color mapping consistency
        #[cfg(debug_assertions)]
        crate::console_log!(
            "🎨 Temperature {:.1}°C → HSLuv({:.1}°, {:.1}%, {:.1}%) → RGB({:.3}, {:.3}, {:.3}) [{}]",
            temp, hue, saturation, lightness, r, g, b,
            if cfg!(target_arch = "wasm32") { "WASM" } else { "Native" }
        );

        (r as f32, g as f32, b as f32)
    }

    // Pressure-based particle count mapping
    pub fn pressure_to_particle_count(
        &self,
        pressure: f32,
        max_particles: u32,
        min_particles: u32,
    ) -> u32 {
        let clamped_pressure = pressure.max(0.0).min(350.0);
        let normalized = clamped_pressure / 350.0;
        let range = (max_particles - min_particles) as f32;
        let target = min_particles as f32 + normalized * range;

        // Round to nearest multiple of 64 for optimal GPU workgroup dispatch
        ((target / 64.0).round() * 64.0) as u32
    }
}

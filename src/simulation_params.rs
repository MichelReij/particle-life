use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

// Import console_log macro
use crate::console_log;

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

        // Debug log key parameters every 60 frames
        static mut DEBUG_FRAME_COUNT: u32 = 0;
        unsafe {
            DEBUG_FRAME_COUNT += 1;
            if DEBUG_FRAME_COUNT % 60 == 0 {
                console_log!(
                    "🎛️  GPU Buffer: particles={} (actual={}), force_scale={}, r_smooth={}, drift={}, bg_color=({:.2},{:.2},{:.2})",
                    actual_particle_count,
                    self.num_particles,
                    self.force_scale,
                    self.r_smooth,
                    self.drift_x_per_second,
                    self.background_color_r,
                    self.background_color_g,
                    self.background_color_b
                );
            }
        }

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
}

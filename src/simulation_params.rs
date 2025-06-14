use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

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
}

impl SimulationParams {
    pub fn new() -> Self {
        Self {
            delta_time: 1.0 / 60.0,
            friction: 0.1,
            num_particles: 3200,
            num_types: 5,
            virtual_world_width: 2400.0,
            virtual_world_height: 2400.0,
            canvas_render_width: 800.0,
            canvas_render_height: 800.0,
            virtual_world_offset_x: 0.0,
            virtual_world_offset_y: 0.0,
            boundary_mode: 1, // Wrap mode
            particle_render_size: 12.0, // Increased from 12.0 to account for 3x scaling down (2400->800)
            force_scale: 400.0,
            r_smooth: 5.0,
            flat_force: false,
            drift_x_per_second: -10.0,
            inter_type_attraction_scale: 1.0,
            inter_type_radius_scale: 1.0,
            time: 0.0,
            fisheye_strength: 3.0,
            background_color_r: 0.0,
            background_color_g: 0.0,
            background_color_b: 0.0,
            lenia_enabled: true,
            lenia_growth_mu: 0.18,
            lenia_growth_sigma: 0.025,
            lenia_kernel_radius: 75.0,
            lightning_frequency: 0.7,
            lightning_intensity: 1.0,
            lightning_duration: 0.6,
        }
    }

    pub fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    pub fn set_delta_time(&mut self, delta_time: f32) {
        self.delta_time = delta_time;
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
            _ => return false,
        }
        true
    }

    // Convert to buffer format for GPU upload
    pub fn to_buffer(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(128); // 32 * 4 bytes = 128 bytes

        // Match the exact layout from TypeScript
        buffer.extend_from_slice(&self.delta_time.to_le_bytes()); // 0
        buffer.extend_from_slice(&self.friction.to_le_bytes()); // 1
        buffer.extend_from_slice(&self.num_particles.to_le_bytes()); // 2
        buffer.extend_from_slice(&self.num_types.to_le_bytes()); // 3
        buffer.extend_from_slice(&self.virtual_world_width.to_le_bytes()); // 4
        buffer.extend_from_slice(&self.virtual_world_height.to_le_bytes()); // 5
        buffer.extend_from_slice(&self.canvas_render_width.to_le_bytes()); // 6
        buffer.extend_from_slice(&self.canvas_render_height.to_le_bytes()); // 7
        buffer.extend_from_slice(&self.virtual_world_offset_x.to_le_bytes()); // 8
        buffer.extend_from_slice(&self.virtual_world_offset_y.to_le_bytes()); // 9
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
        buffer.extend_from_slice(&0.0f32.to_le_bytes()); // 30 - padding
        buffer.extend_from_slice(&0.0f32.to_le_bytes()); // 31 - padding

        buffer
    }

    pub fn get_buffer_size(&self) -> u32 {
        128 // 32 * 4 bytes
    }
}

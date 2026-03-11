use crate::config::*;
use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use web_sys;

// Central SimulationParams struct definition - used by both WASM and native
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
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

    // Viewport/zoom parameters for rendering optimization
    pub viewport_center_x: f32,
    pub viewport_center_y: f32,
    pub viewport_radius: f32,

    // Current zoom level - used for zoom-adjusted drift calculation
    pub current_zoom_level: f32,

    // Environmental control levels (stored for cross-dependency computation)
    pub ph: f32,                        // pH scale 0-14, optimum for life ~10
    pub pressure_level: f32,            // bar (0-1000), ~0-10000m depth
    pub electrical_activity_level: f32, // 0-3

    // Cached base friction from temperature (before pressure modifier is applied)
    pub base_friction: f32,
}

impl SimulationParams {
    pub fn new() -> Self {
        Self {
            delta_time: 1.0 / 60.0,
            friction: 0.1,
            num_particles: DEFAULT_NUM_PARTICLES, // Initialize with all particles active to prevent GPU buffer hiccups
            num_types: 5,
            virtual_world_width: VIRTUAL_WORLD_WIDTH,
            virtual_world_height: VIRTUAL_WORLD_HEIGHT,
            canvas_render_width: CANVAS_WIDTH,
            canvas_render_height: CANVAS_HEIGHT,
            // Start at top-left corner initially (where particles are visible)
            virtual_world_offset_x: 0.0,
            virtual_world_offset_y: 0.0,
            // Default viewport is the entire virtual world (zoom level 1.0)
            viewport_width: VIRTUAL_WORLD_WIDTH,
            viewport_height: VIRTUAL_WORLD_HEIGHT,
            boundary_mode: 2, // Respawn mode: particles respawn on opposite axis when out of bounds (0=wrap, 1=hybrid, 2=respawn)
            particle_render_size: PARTICLE_SIZE, // Use centralized particle size configuration
            force_scale: 400.0,
            r_smooth: 10.0, // Increased from 5.0 to make repulsion forces more visible
            flat_force: false,
            drift_x_per_second: 0.0, // Start with no drift to isolate particle interactions
            inter_type_attraction_scale: 1.0,
            inter_type_radius_scale: 1.0,
            time: 0.0,
            fisheye_strength: 1.5, // Fixed fisheye strength for consistent global effect
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

            // Spatial grid optimization - enabled by default with optimized cell size
            spatial_grid_enabled: true,
            spatial_grid_cell_size: 80.0, // 80 units per cell (VIRTUAL_WORLD_WIDTH/80 = 54x54 grid)
            spatial_grid_width: (VIRTUAL_WORLD_WIDTH / 80.0) as u32, // 54 cells horizontally
            spatial_grid_height: (VIRTUAL_WORLD_HEIGHT / 80.0) as u32, // 54 cells vertically

            // Viewport/zoom parameters for rendering optimization
            viewport_center_x: VIRTUAL_WORLD_CENTER_X, // Default to world center
            viewport_center_y: VIRTUAL_WORLD_CENTER_Y, // Default to world center
            viewport_radius: VIRTUAL_WORLD_WIDTH / 2.0, // Default radius for circular viewport

            // Default zoom level
            current_zoom_level: 1.0,

            // Environmental control levels (start at non-optimal to encourage exploration)
            ph: 7.0,             // Neutral pH — optimum is 10
            pressure_level: 0.0, // Surface — need to find depth
            electrical_activity_level: 0.0,

            base_friction: 0.1,
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
            // "fisheyeStrength" => self.fisheye_strength = value, // Now fixed at 1.5
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
        self.to_buffer_with_particle_count_and_zoom(self.num_particles, 1.0) // Default zoom level
    }

    // Convert to buffer format for GPU upload with specific particle count
    pub fn to_buffer_with_particle_count(&self, actual_particle_count: u32) -> Vec<u8> {
        self.to_buffer_with_particle_count_and_zoom(actual_particle_count, 1.0) // Default zoom level
    }

    // Convert to buffer format for GPU upload with specific particle count and zoom level
    // The zoom level is used to adjust drift speed: higher zoom = slower drift
    pub fn to_buffer_with_particle_count_and_zoom(
        &self,
        actual_particle_count: u32,
        zoom_level: f32,
    ) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(192); // 48 * 4 bytes = 192 bytes (45 fields + 3 padding = 48 fields total)

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
        buffer.extend_from_slice(&self.boundary_mode.to_le_bytes()); // 10
        buffer.extend_from_slice(&self.particle_render_size.to_le_bytes()); // 11
        buffer.extend_from_slice(&self.force_scale.to_le_bytes()); // 12
        buffer.extend_from_slice(&self.r_smooth.to_le_bytes()); // 13
        buffer.extend_from_slice(&(if self.flat_force { 1u32 } else { 0u32 }).to_le_bytes()); // 14

        // Apply zoom-adjusted drift: divide drift by zoom level for better user experience when zoomed in
        let zoom_adjusted_drift = self.drift_x_per_second / zoom_level.max(1.0); // Ensure we don't divide by 0
        buffer.extend_from_slice(&zoom_adjusted_drift.to_le_bytes()); // 15
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

        // Viewport/zoom parameters for rendering optimization
        buffer.extend_from_slice(&self.viewport_center_x.to_le_bytes()); // 40
        buffer.extend_from_slice(&self.viewport_center_y.to_le_bytes()); // 41
        buffer.extend_from_slice(&self.viewport_width.to_le_bytes()); // 42
        buffer.extend_from_slice(&self.viewport_height.to_le_bytes()); // 43
        buffer.extend_from_slice(&self.viewport_radius.to_le_bytes()); // 44

        // Padding to ensure 16-byte alignment (3 × f32 = 12 bytes)
        buffer.extend_from_slice(&0.0f32.to_le_bytes()); // 45 - padding
        buffer.extend_from_slice(&0.0f32.to_le_bytes()); // 46 - padding
        buffer.extend_from_slice(&0.0f32.to_le_bytes()); // 47 - padding

        buffer
    }

    pub fn get_buffer_size(&self) -> u32 {
        192 // 48 * 4 bytes = 192 bytes (45 fields + 3 padding = 48 fields total)
    }

    // === CENTRAL CONVERSION FUNCTIONS ===
    // These functions handle the complex mappings from user-friendly parameters
    // to internal simulation parameters, used by both WASM and native

    // Set temperature and update all temperature-related simulation parameters
    pub fn apply_temperature(&mut self, temp: f32) {
        // Clamp temperature to valid range (3°C to 160°C)
        let clamped_temp = temp.max(3.0).min(160.0);

        // 1. Update drift speed: temp [3, 160] → drift [0, -120]
        let drift = -((clamped_temp - 3.0) * 120.0) / 157.0;
        self.drift_x_per_second = drift;

        // 2. Update friction: stays high until ~80°C, then drops sharply toward max.
        // Uses normalized_temp² so the curve hangs high and only plunges near the top.
        // 80°C → ~0.52  |  120°C → ~0.14  |  160°C → ~0.010
        // At 160°C + extreme pressure (modifier 0.5) → 0.005 (very chaotic)
        let normalized_temp = (clamped_temp - 3.0) / 157.0;
        let t = normalized_temp * normalized_temp;
        let friction = 0.98 * (-4.6 * t).exp();
        self.base_friction = friction;
        // Apply pressure modifier so extreme depth also disrupts particle order
        self.friction = friction * self.pressure_friction_modifier();

        // 3. Update background color using HSL: temp [3, 160] → hue [200°, -10°]
        let (r, g, b) = Self::temperature_to_background_color(clamped_temp);
        self.background_color_r = r;
        self.background_color_g = g;
        self.background_color_b = b;
    }

    // Set pressure and update all pressure-related simulation parameters
    pub fn apply_pressure(&mut self, pressure: f32) {
        // Clamp pressure to valid range (0 to 1000 bar = ~10,000m depth)
        let clamped_pressure = pressure.max(0.0).min(1000.0);
        self.pressure_level = clamped_pressure;

        // 1. Update force scale: pressure [0, 1000] → force_scale [100, 500]
        let force_scale = 100.0 + (clamped_pressure * 400.0) / 1000.0;
        self.force_scale = force_scale;

        // 2. Update rSmooth: exponential mapping pressure [0, 1000] → rSmooth [20, 0.1]
        let normalized_pressure = clamped_pressure / 1000.0;
        let r_smooth = 20.0 * (-5.3 * normalized_pressure).exp();
        self.r_smooth = r_smooth;

        // Pressure also modifies friction — extreme depths drive friction toward chaos
        self.friction = self.base_friction * self.pressure_friction_modifier();

        self.recompute_cross_dependencies();
    }

    // Set pH and update all pH-related simulation parameters
    // pH optimum for life (hydrothermal vent theory): ~10 (basic)
    // Response curve: asymmetric Gaussian centred on 10, σ=3
    pub fn apply_ph(&mut self, ph: f32) {
        self.ph = ph.max(0.0).min(14.0);
        self.recompute_cross_dependencies();
    }

    // Set electrical activity and update all electrical-related simulation parameters
    pub fn apply_electrical_activity(&mut self, electrical_activity: f32) {
        // Clamp electrical activity to valid range (0 to 3)
        let clamped_electrical = electrical_activity.max(0.0).min(3.0);
        self.electrical_activity_level = clamped_electrical;

        // Update inter-type attraction scale: cubic mapping [0, 3] → interTypeAttractionScale [0, 1.5]
        let normalized_electrical = clamped_electrical / 3.0;
        let cubic_value = normalized_electrical * normalized_electrical * normalized_electrical;
        let inter_type_attraction_scale = cubic_value * 1.5;
        self.inter_type_attraction_scale = inter_type_attraction_scale;

        // Update lightning parameters based on electrical activity
        // Lightning frequency: 0 at min activity, 1.0 at max activity
        self.lightning_frequency = normalized_electrical;

        // Lightning intensity: 0.5 at min activity, 2.0 at max activity
        self.lightning_intensity = 0.5 + (normalized_electrical * 1.5);

        // Lightning duration: 0.3 at min activity, 0.8 at max activity
        self.lightning_duration = 0.3 + (normalized_electrical * 0.5);

        // Cross-dependency: electrical noise reduces interaction radius
        self.recompute_cross_dependencies();
    }

    // Pressure friction modifier: Gaussian centred at 350 bar (σ=150).
    // Returns 1.0 at optimal pressure, down to ~0.5 at surface (0 bar) or extreme depth (1000 bar).
    // Low friction → chaos; this makes pressure a second driver of disorder.
    fn pressure_friction_modifier(&self) -> f32 {
        let pressure_quality =
            (-(self.pressure_level - 350.0).powi(2) / (2.0 * 150.0_f32.powi(2))).exp();
        // modifier ∈ [0.5, 1.0]: optimal pressure preserves friction, extremes halve it
        0.5 + 0.5 * pressure_quality
    }

    // Recompute parameters that depend on multiple sliders simultaneously.
    // Called by apply_ph, apply_pressure, and apply_electrical_activity.
    //
    // Cross-dependencies (what makes finding the optimum hard):
    //   inter_type_radius_scale  ← pH (positive) × electrical penalty (negative)
    //   lenia_growth_mu          ← pressure (Gaussian peak ~350 bar) × pH quality
    //   lenia_kernel_radius      ← pH quality only
    //   lenia_growth_sigma       ← inverse pH quality (tight at pH 10, loose at extremes)
    fn recompute_cross_dependencies(&mut self) {
        // pH quality: TIGHT Gaussian, σ≈2.83 (variance=8), centred on pH 10
        // pH 10 → q = 1.0  |  pH 7 → q ≈ 0.32  |  pH 14 → q ≈ 0.14  |  pH 0 → q ≈ 0.000003
        // Neutral pH (7) is mediocre, extreme pH values are genuinely chaotic
        let ph_quality = (-(self.ph - 10.0).powi(2) / 8.0).exp();

        // Electrical noise penalty on long-range interaction radius
        // Max penalty at electrical=3: radius is reduced by 65%
        let elec_normalized = self.electrical_activity_level / 3.0;
        let elec_radius_penalty = 1.0 - 0.65 * elec_normalized;

        // inter_type_radius_scale: pH drives it up, electrical noise drives it down
        // At all-max (pH=14, elec=3): (0.1 + 0.14*1.9) * 0.35 ≈ 0.13  →  near-zero structure
        // At optimal (pH=10, elec=0): (0.1 + 1.9) * 1.0 = 2.0  →  rich cross-type interaction
        self.inter_type_radius_scale = (0.1 + ph_quality * 1.9) * elec_radius_penalty;

        // Pressure quality: TIGHT Gaussian, σ=150 bar, centred on 350 bar (~3500 m depth)
        // 0 bar → q ≈ 0.07  |  350 bar → q = 1.0  |  1000 bar → q ≈ 0.0001
        let pressure_quality =
            (-(self.pressure_level - 350.0).powi(2) / (2.0 * 150.0_f32.powi(2))).exp();

        // lenia_growth_mu: requires BOTH right pressure (~350 bar) AND right pH (~10) for maximum
        self.lenia_growth_mu = (0.05 + pressure_quality * 0.10) * (0.5 + 0.5 * ph_quality);

        // lenia_kernel_radius: purely from pH — at optimal pH, Lenia interactions reach far
        self.lenia_kernel_radius = 30.0 + ph_quality * 70.0;

        // lenia_growth_sigma: chaos when EITHER pH OR pressure is off-optimal
        // Uses max() so that getting one wrong is enough to disrupt Lenia growth
        let ph_chaos = 1.0 - ph_quality;
        let pressure_chaos = 1.0 - pressure_quality;
        let combined_chaos = ph_chaos.max(pressure_chaos);
        // Range: 0.02 (fully optimal) → 0.16 (fully chaotic)
        self.lenia_growth_sigma = 0.02 + combined_chaos * 0.14;
    }

    // Set zoom level and update viewport parameters
    pub fn apply_zoom(&mut self, zoom_level: f32, center_x: Option<f32>, center_y: Option<f32>) {
        // Clamp zoom level to valid range (1.0 to ZOOM_MAX)
        let clamped_zoom = zoom_level.max(ZOOM_MIN).min(ZOOM_MAX);

        // Store the current zoom level for drift adjustment
        self.current_zoom_level = clamped_zoom;

        // Calculate viewport size: at zoom 1.0 = full world, at zoom 2.0 = half world, etc.
        let viewport_width = VIRTUAL_WORLD_WIDTH / clamped_zoom;
        let viewport_height = VIRTUAL_WORLD_HEIGHT / clamped_zoom;

        // Center the viewport around world center by default
        let center_x = center_x.unwrap_or(VIRTUAL_WORLD_CENTER_X);
        let center_y = center_y.unwrap_or(VIRTUAL_WORLD_CENTER_Y);

        // Calculate offset to center the viewport
        let offset_x = center_x - (viewport_width / 2.0);
        let offset_y = center_y - (viewport_height / 2.0);

        // Clamp offsets to ensure viewport stays within virtual world bounds
        let max_offset_x = VIRTUAL_WORLD_WIDTH - viewport_width;
        let max_offset_y = VIRTUAL_WORLD_HEIGHT - viewport_height;

        let clamped_offset_x = offset_x.max(0.0).min(max_offset_x);
        let clamped_offset_y = offset_y.max(0.0).min(max_offset_y);

        // Update viewport offset AND size
        self.virtual_world_offset_x = clamped_offset_x;
        self.virtual_world_offset_y = clamped_offset_y;
        self.viewport_width = viewport_width;
        self.viewport_height = viewport_height;

        // IMPORTANT: Update viewport center coordinates that are sent to GPU!
        // Recalculate the actual center from the clamped offset
        self.viewport_center_x = clamped_offset_x + (viewport_width / 2.0);
        self.viewport_center_y = clamped_offset_y + (viewport_height / 2.0);
    }

    // Temperature-based background color mapping using standard HSL (static helper)
    fn temperature_to_background_color(temp: f32) -> (f32, f32, f32) {
        // Hue: 220 (blauw) → 0 (rood)
        // Bereikt max blauw al bij 80°C, max rood bij 140°C
        // Saturation en lightness zijn constant
        const S: f32 = 22.0;
        const L: f32 = 66.0;
        let hue_temp = temp.max(80.0).min(140.0);
        let normalized_hue = (hue_temp - 80.0) / (140.0 - 80.0);
        let hue = 220.0 + normalized_hue * (0.0 - 220.0); // 220 → 0
        crate::buffer_utils::hsl_to_rgb(hue, S, L)
    }

    // Pressure-based particle count mapping
    pub fn pressure_to_particle_count(
        &self,
        pressure: f32,
        max_particles: u32,
        min_particles: u32,
    ) -> u32 {
        let clamped_pressure = pressure.max(0.0).min(1000.0);
        let normalized = clamped_pressure / 1000.0;
        let range = (max_particles - min_particles) as f32;
        let target = min_particles as f32 + normalized * range;

        // Round to nearest multiple of 64 for optimal GPU workgroup dispatch
        ((target / 64.0).round() * 64.0) as u32
    }

    // === ESP32 SENSOR INTEGRATION ===
    // Apply ESP32 sensor data to all simulation parameters
    #[cfg(not(target_arch = "wasm32"))]
    pub fn apply_esp32_sensor_data(
        &mut self,
        sensor_data: &crate::esp32_communication::ESP32SensorData,
        delta_time: f32,
    ) {
        // Apply zoom
        let zoom_level = sensor_data.to_zoom_level();

        // Apply relative pan (velocity-based)
        let (new_center_x, new_center_y) = if sensor_data.joy_click || sensor_data.joystick_button {
            // Button pressed: snap viewport back to world center
            (VIRTUAL_WORLD_CENTER_X, VIRTUAL_WORLD_CENTER_Y)
        } else {
            let (vel_x, vel_y) = sensor_data.to_pan_velocity();
            let viewport_width = VIRTUAL_WORLD_WIDTH / zoom_level;
            let viewport_height = VIRTUAL_WORLD_HEIGHT / zoom_level;
            // 50 % of visible viewport per second at max deflection
            let speed_factor = 0.5_f32;
            (
                self.viewport_center_x + vel_x * viewport_width * speed_factor * delta_time,
                self.viewport_center_y + vel_y * viewport_height * speed_factor * delta_time,
            )
        };

        self.apply_zoom(zoom_level, Some(new_center_x), Some(new_center_y));

        // Apply temperature
        let temperature = sensor_data.to_temperature_celsius();
        self.apply_temperature(temperature);

        // Apply pressure
        let pressure = sensor_data.to_pressure();
        self.apply_pressure(pressure);

        // Apply pH
        let ph = sensor_data.to_ph();
        self.apply_ph(ph);

        // Apply electrical activity
        let electrical = sensor_data.to_electrical_activity();
        self.apply_electrical_activity(electrical);

        // Note: sleep parameter is handled separately by the application
    }
}

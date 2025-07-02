/// Central configuration constants for the particle life simulation
/// This module centralizes all size and dimension constants to make experimentation easier

/// Virtual world dimensions - the simulation space where particles exist
pub const VIRTUAL_WORLD_WIDTH: f32 = 3240.0;
pub const VIRTUAL_WORLD_HEIGHT: f32 = 3240.0;

/// Canvas/render dimensions - the final output size
pub const CANVAS_WIDTH: f32 = 1080.0;
pub const CANVAS_HEIGHT: f32 = 1080.0;

/// Particle rendering size - the diameter of particles in pixels
pub const PARTICLE_SIZE: f32 = 10.0;
pub const PARTICLE_SIZE_MIN: f32 = 8.0;
pub const PARTICLE_SIZE_MAX: f32 = 32.0;

/// Particle system configuration
pub const DEFAULT_NUM_PARTICLES: u32 = 4800;
pub const MAX_PARTICLES: u32 = 4800;
pub const MIN_PARTICLES: u32 = 1200;

/// FPS display configuration - no longer capped to allow 3-digit display
pub const FPS_SAMPLE_COUNT: usize = 10; // Number of samples for moving average
pub const FPS_UPDATE_INTERVAL: f32 = 0.5; // Update interval in seconds
pub const FPS_CONSOLE_INTERVAL: f32 = 3.0; // Console output interval in seconds

/// Zoom configuration - maximum 12x zoom capability with direct canvas rendering
/// The efficient direct-to-canvas pipeline allows for high zoom levels while maintaining quality
pub const ZOOM_MIN: f32 = 1.0;
pub const ZOOM_MAX: f32 = 12.0;
pub const ZOOM_DEFAULT: f32 = 1.0;
pub const ZOOM_STEP: f32 = 0.01;

/// At max zoom (12x), each screen pixel represents this many world units
pub const MAX_ZOOM_WORLD_UNITS_PER_PIXEL: f32 = VIRTUAL_WORLD_WIDTH / (CANVAS_WIDTH * ZOOM_MAX); // ~0.25

/// Fisheye configuration - fixed strength for consistent global fisheye effect
pub const FISHEYE_STRENGTH: f32 = 1.5;
pub const FISHEYE_BUFFER_SCALE: f32 = 1.3; // Buffer enlargement factor (can be tweaked iteratively)

/// Convenience constants derived from the main dimensions
pub const VIRTUAL_WORLD_CENTER_X: f32 = VIRTUAL_WORLD_WIDTH / 2.0; // 1620.0
pub const VIRTUAL_WORLD_CENTER_Y: f32 = VIRTUAL_WORLD_HEIGHT / 2.0; // 1620.0

/// Scaling factor from virtual world to canvas
pub const WORLD_TO_CANVAS_SCALE: f32 = CANVAS_WIDTH / VIRTUAL_WORLD_WIDTH; // 0.333

/// Integer versions for WebGPU texture creation
pub const VIRTUAL_WORLD_WIDTH_U32: u32 = VIRTUAL_WORLD_WIDTH as u32; // 3240
pub const VIRTUAL_WORLD_HEIGHT_U32: u32 = VIRTUAL_WORLD_HEIGHT as u32; // 3240
pub const CANVAS_WIDTH_U32: u32 = CANVAS_WIDTH as u32; // 1080
pub const CANVAS_HEIGHT_U32: u32 = CANVAS_HEIGHT as u32; // 1080

/// Fisheye buffer dimensions - larger rendering area for global fisheye effect
/// We render to this larger buffer and then crop the center for the final output
pub const FISHEYE_BUFFER_WIDTH: f32 = CANVAS_WIDTH * FISHEYE_BUFFER_SCALE; // 1404.0
pub const FISHEYE_BUFFER_HEIGHT: f32 = CANVAS_HEIGHT * FISHEYE_BUFFER_SCALE; // 1404.0
pub const FISHEYE_BUFFER_WIDTH_U32: u32 = FISHEYE_BUFFER_WIDTH as u32; // 1404
pub const FISHEYE_BUFFER_HEIGHT_U32: u32 = FISHEYE_BUFFER_HEIGHT as u32; // 1404

/// Calculate the offset needed to center the canvas crop within the fisheye buffer
pub const FISHEYE_CROP_OFFSET_X: f32 = (FISHEYE_BUFFER_WIDTH - CANVAS_WIDTH) / 2.0; // 162.0
pub const FISHEYE_CROP_OFFSET_Y: f32 = (FISHEYE_BUFFER_HEIGHT - CANVAS_HEIGHT) / 2.0; // 162.0

/// Calculate the size of the fisheye buffer based on fisheye strength
/// For true global fisheye, we need a larger buffer that gets distorted and cropped
pub fn calculate_fisheye_buffer_size(fisheye_strength: f32) -> (u32, u32) {
    if fisheye_strength <= 0.0 {
        // No fisheye, use canvas size
        (CANVAS_WIDTH_U32, CANVAS_HEIGHT_U32)
    } else {
        // Buffer needs to be larger than canvas by fisheye_strength factor
        // For fisheye strength 1.3, buffer should be 1404x1404
        let scale_factor = 1.0 + fisheye_strength;
        let buffer_width = (CANVAS_WIDTH * scale_factor).ceil() as u32;
        let buffer_height = (CANVAS_HEIGHT * scale_factor).ceil() as u32;
        (buffer_width, buffer_height)
    }
}

/// Configuration for easy experimentation
/// Change these values to experiment with different world/canvas sizes
pub struct WorldConfig {
    pub virtual_world_width: f32,
    pub virtual_world_height: f32,
    pub canvas_width: f32,
    pub canvas_height: f32,
    pub particle_size: f32,
    pub default_num_particles: u32,
    pub max_particles: u32,
    pub min_particles: u32,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            virtual_world_width: VIRTUAL_WORLD_WIDTH,
            virtual_world_height: VIRTUAL_WORLD_HEIGHT,
            canvas_width: CANVAS_WIDTH,
            canvas_height: CANVAS_HEIGHT,
            particle_size: PARTICLE_SIZE,
            default_num_particles: DEFAULT_NUM_PARTICLES,
            max_particles: MAX_PARTICLES,
            min_particles: MIN_PARTICLES,
        }
    }
}

impl WorldConfig {
    /// Create a custom world configuration
    pub fn new(
        virtual_width: f32,
        virtual_height: f32,
        canvas_width: f32,
        canvas_height: f32,
        particle_size: f32,
    ) -> Self {
        Self {
            virtual_world_width: virtual_width,
            virtual_world_height: virtual_height,
            canvas_width,
            canvas_height,
            particle_size,
            default_num_particles: DEFAULT_NUM_PARTICLES,
            max_particles: MAX_PARTICLES,
            min_particles: MIN_PARTICLES,
        }
    }

    /// Get the center coordinates of the virtual world
    pub fn center(&self) -> (f32, f32) {
        (
            self.virtual_world_width / 2.0,
            self.virtual_world_height / 2.0,
        )
    }

    /// Get the scaling factor from virtual world to canvas
    pub fn scale_factor(&self) -> f32 {
        self.canvas_width / self.virtual_world_width
    }

    /// Get virtual world dimensions as u32 for texture creation
    pub fn virtual_dimensions_u32(&self) -> (u32, u32) {
        (
            self.virtual_world_width as u32,
            self.virtual_world_height as u32,
        )
    }

    /// Get canvas dimensions as u32 for texture creation
    pub fn canvas_dimensions_u32(&self) -> (u32, u32) {
        (self.canvas_width as u32, self.canvas_height as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = WorldConfig::default();
        assert_eq!(config.virtual_world_width, 3240.0);
        assert_eq!(config.virtual_world_height, 3240.0);
        assert_eq!(config.canvas_width, 1080.0);
        assert_eq!(config.canvas_height, 1080.0);
        assert_eq!(config.particle_size, 20.0);
        assert_eq!(config.default_num_particles, 4800);
        assert_eq!(config.max_particles, 4800);
        assert_eq!(config.min_particles, 1200);

        let (center_x, center_y) = config.center();
        assert_eq!(center_x, 1620.0);
        assert_eq!(center_y, 1620.0);

        let scale = config.scale_factor();
        assert!((scale - 0.333333).abs() < 0.001);
    }

    #[test]
    fn test_custom_config() {
        let config = WorldConfig::new(3000.0, 3000.0, 1000.0, 1000.0, 15.0);
        assert_eq!(config.virtual_world_width, 3000.0);
        assert_eq!(config.canvas_width, 1000.0);
        assert_eq!(config.particle_size, 15.0);
        assert_eq!(config.default_num_particles, 4800); // Should use default values
        assert_eq!(config.max_particles, 4800);
        assert_eq!(config.min_particles, 1200);

        let (center_x, center_y) = config.center();
        assert_eq!(center_x, 1500.0);
        assert_eq!(center_y, 1500.0);
    }
}

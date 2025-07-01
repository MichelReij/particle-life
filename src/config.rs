/// Central configuration constants for the particle life simulation
/// This module centralizes all size and dimension constants to make experimentation easier

/// Virtual world dimensions - the simulation space where particles exist
pub const VIRTUAL_WORLD_WIDTH: f32 = 3240.0;
pub const VIRTUAL_WORLD_HEIGHT: f32 = 3240.0;

/// Canvas/render dimensions - the final output size
pub const CANVAS_WIDTH: f32 = 1080.0;
pub const CANVAS_HEIGHT: f32 = 1080.0;

/// Particle rendering size - the diameter of particles in pixels
pub const PARTICLE_SIZE: f32 = 20.0;
pub const PARTICLE_SIZE_MIN: f32 = 8.0;
pub const PARTICLE_SIZE_MAX: f32 = 32.0;

/// Particle system configuration
pub const DEFAULT_NUM_PARTICLES: u32 = 4800;
pub const MAX_PARTICLES: u32 = 4800;
pub const MIN_PARTICLES: u32 = 1200;

/// FPS display configuration
pub const FPS_SAMPLE_COUNT: usize = 10; // Number of samples for moving average
pub const FPS_UPDATE_INTERVAL: f32 = 0.5; // Update interval in seconds
pub const FPS_CONSOLE_INTERVAL: f32 = 3.0; // Console output interval in seconds
pub const FPS_DISPLAY_MAX: f32 = 99.0; // Maximum FPS to display (cap at 99 for formatting)

/// Cap FPS value for consistent display formatting
pub fn cap_fps_for_display(fps: f32) -> f32 {
    fps.min(FPS_DISPLAY_MAX)
}

/// Convenience constants derived from the main dimensions
pub const VIRTUAL_WORLD_CENTER_X: f32 = VIRTUAL_WORLD_WIDTH / 2.0; // 1620.0
pub const VIRTUAL_WORLD_CENTER_Y: f32 = VIRTUAL_WORLD_HEIGHT / 2.0; // 1620.0

/// Scaling factor from virtual world to canvas
pub const WORLD_TO_CANVAS_SCALE: f32 = CANVAS_WIDTH / VIRTUAL_WORLD_WIDTH; // 0.333...

/// Integer versions for WebGPU texture creation
pub const VIRTUAL_WORLD_WIDTH_U32: u32 = VIRTUAL_WORLD_WIDTH as u32; // 3240
pub const VIRTUAL_WORLD_HEIGHT_U32: u32 = VIRTUAL_WORLD_HEIGHT as u32; // 3240
pub const CANVAS_WIDTH_U32: u32 = CANVAS_WIDTH as u32; // 1080
pub const CANVAS_HEIGHT_U32: u32 = CANVAS_HEIGHT as u32; // 1080

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
        assert_eq!(config.particle_size, 24.0);
        assert_eq!(config.default_num_particles, 3200);
        assert_eq!(config.max_particles, 6400);
        assert_eq!(config.min_particles, 1600);

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
        assert_eq!(config.default_num_particles, 3200); // Should use default values
        assert_eq!(config.max_particles, 6400);
        assert_eq!(config.min_particles, 1600);

        let (center_x, center_y) = config.center();
        assert_eq!(center_x, 1500.0);
        assert_eq!(center_y, 1500.0);
    }
}

/// Abstract renderer trait for platform-agnostic GPU rendering
pub trait Renderer {
    type Error;

    /// Initialize the renderer with the given surface and configuration
    async fn new(surface: wgpu::Surface<'static>, size: (u32, u32)) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// Resize the renderer surface
    fn resize(&mut self, new_size: (u32, u32));

    /// Update simulation parameters
    fn update_sim_params(&mut self, params: &crate::core::SimulationParams);

    /// Update particle data
    fn update_particles(&mut self, particles: &crate::core::ParticleSystem);

    /// Update interaction rules
    fn update_interaction_rules(&mut self, rules: &crate::core::InteractionRules);

    /// Render a frame
    fn render(&mut self, delta_time: f32) -> Result<(), Self::Error>;

    /// Get current frame time for performance monitoring
    fn get_frame_time(&self) -> f32;
}

/// Platform-agnostic renderer error types
#[derive(Debug)]
pub enum RendererError {
    SurfaceError(wgpu::SurfaceError),
    DeviceError(wgpu::DeviceError),
    RequestDeviceError(wgpu::RequestDeviceError),
    CreateSurfaceError(wgpu::CreateSurfaceError),
    Other(String),
}

impl std::fmt::Display for RendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RendererError::SurfaceError(e) => write!(f, "Surface error: {}", e),
            RendererError::DeviceError(e) => write!(f, "Device error: {}", e),
            RendererError::RequestDeviceError(e) => write!(f, "Request device error: {}", e),
            RendererError::CreateSurfaceError(e) => write!(f, "Create surface error: {}", e),
            RendererError::Other(e) => write!(f, "Renderer error: {}", e),
        }
    }
}

impl std::error::Error for RendererError {}

impl From<wgpu::SurfaceError> for RendererError {
    fn from(e: wgpu::SurfaceError) -> Self {
        RendererError::SurfaceError(e)
    }
}

impl From<wgpu::DeviceError> for RendererError {
    fn from(e: wgpu::DeviceError) -> Self {
        RendererError::DeviceError(e)
    }
}

impl From<wgpu::RequestDeviceError> for RendererError {
    fn from(e: wgpu::RequestDeviceError) -> Self {
        RendererError::RequestDeviceError(e)
    }
}

impl From<wgpu::CreateSurfaceError> for RendererError {
    fn from(e: wgpu::CreateSurfaceError) -> Self {
        RendererError::CreateSurfaceError(e)
    }
}

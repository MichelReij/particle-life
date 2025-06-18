/// All WGSL shaders embedded as strings for the particle life simulation
/// This allows for a clean architecture where Rust handles ALL rendering logic

// Core particle simulation shaders
pub const VERTEX_SHADER: &str = include_str!("shaders/vert.wgsl");
pub const FRAGMENT_SHADER: &str = include_str!("shaders/frag.wgsl");
pub const COMPUTE_SHADER: &str = include_str!("shaders/compute.wgsl");

// Background and post-processing shaders
pub const BACKGROUND_VERTEX_SHADER: &str = include_str!("shaders/background_vert.wgsl");
pub const BACKGROUND_FRAGMENT_SHADER: &str = include_str!("shaders/background_frag.wgsl");
pub const GRID_FRAGMENT_SHADER: &str = include_str!("shaders/grid_frag.wgsl");

// Visual effects shaders
pub const FISHEYE_FRAGMENT_SHADER: &str = include_str!("shaders/fisheye_frag.wgsl");
pub const ZOOM_FRAGMENT_SHADER: &str = include_str!("shaders/zoom_frag.wgsl");
pub const VIGNETTE_FRAGMENT_SHADER: &str = include_str!("shaders/vignette_frag.wgsl");

// Lightning system shaders
pub const LIGHTNING_VERTEX_SHADER: &str = include_str!("shaders/lightning_vert.wgsl");
pub const LIGHTNING_FRAGMENT_SHADER: &str = include_str!("shaders/lightning_frag_buffer.wgsl");
pub const LIGHTNING_FRAGMENT_BUFFER: &str = include_str!("shaders/lightning_frag_buffer.wgsl");
pub const LIGHTNING_COMPUTE_SHADER: &str = include_str!("shaders/lightning_compute.wgsl");

/// Shader type enumeration for easy management
#[derive(Debug, Clone, Copy)]
pub enum ShaderType {
    // Core particle rendering
    ParticleVertex,
    ParticleFragment,
    ParticleCompute,

    // Background rendering
    BackgroundVertex,
    BackgroundFragment,
    GridFragment,

    // Post-processing effects
    FisheyeFragment,
    ZoomFragment,
    VignetteFragment,

    // Lightning system
    LightningVertex,
    LightningFragment,
    LightningFragmentBuffer,
    LightningCompute,
}

impl ShaderType {
    /// Get the WGSL source code for this shader type
    pub fn source(&self) -> &'static str {
        match self {
            ShaderType::ParticleVertex => VERTEX_SHADER,
            ShaderType::ParticleFragment => FRAGMENT_SHADER,
            ShaderType::ParticleCompute => COMPUTE_SHADER,

            ShaderType::BackgroundVertex => BACKGROUND_VERTEX_SHADER,
            ShaderType::BackgroundFragment => BACKGROUND_FRAGMENT_SHADER,
            ShaderType::GridFragment => GRID_FRAGMENT_SHADER,

            ShaderType::FisheyeFragment => FISHEYE_FRAGMENT_SHADER,
            ShaderType::ZoomFragment => ZOOM_FRAGMENT_SHADER,
            ShaderType::VignetteFragment => VIGNETTE_FRAGMENT_SHADER,

            ShaderType::LightningVertex => LIGHTNING_VERTEX_SHADER,
            ShaderType::LightningFragment => LIGHTNING_FRAGMENT_SHADER,
            ShaderType::LightningFragmentBuffer => LIGHTNING_FRAGMENT_BUFFER,
            ShaderType::LightningCompute => LIGHTNING_COMPUTE_SHADER,
        }
    }

    /// Get a descriptive label for this shader type
    pub fn label(&self) -> &'static str {
        match self {
            ShaderType::ParticleVertex => "Particle Vertex Shader",
            ShaderType::ParticleFragment => "Particle Fragment Shader",
            ShaderType::ParticleCompute => "Particle Compute Shader",

            ShaderType::BackgroundVertex => "Background Vertex Shader",
            ShaderType::BackgroundFragment => "Background Fragment Shader",
            ShaderType::GridFragment => "Grid Fragment Shader",

            ShaderType::FisheyeFragment => "Fisheye Fragment Shader",
            ShaderType::ZoomFragment => "Zoom Fragment Shader",
            ShaderType::VignetteFragment => "Vignette Fragment Shader",

            ShaderType::LightningVertex => "Lightning Vertex Shader",
            ShaderType::LightningFragment => "Lightning Fragment Shader",
            ShaderType::LightningFragmentBuffer => "Lightning Fragment Buffer Shader",
            ShaderType::LightningCompute => "Lightning Compute Shader",
        }
    }
}

struct SimulationParams {
    deltaTime: f32,
    friction: f32,
    numParticles: u32,
    numTypes: u32,
    virtualWorldWidth: f32,
    virtualWorldHeight: f32,
    canvasRenderWidth: f32,
    canvasRenderHeight: f32,
    virtualWorldOffsetX: f32,
    virtualWorldOffsetY: f32,
    boundaryMode: u32,
    particleRenderSize: f32,
    forceScale: f32,
    rSmooth: f32,
    flatForce: u32,
    driftXPerSecond: f32,
    interTypeAttractionScale: f32,
    interTypeRadiusScale: f32,
    time: f32,
    // Time in seconds for animation
    fisheyeStrength: f32,
    // Fisheye distortion strength
    backgroundColor: vec3<f32>,
    // RGB background color

    // Lenia-inspired parameters
    leniaEnabled: u32,
    // Boolean as u32: enable Lenia-style interactions
    leniaGrowthMu: f32,
    // Lenia growth function center (μ)
    leniaGrowthSigma: f32,
    // Lenia growth function spread (σ)
    leniaKernelRadius: f32,
    // Lenia kernel radius in pixels
}

;

@group(0) @binding(0)
var<uniform> sim_params: SimulationParams;

@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    // Create a full-screen quad
    var pos = array<vec2<f32>, 6>(vec2<f32>(- 1.0, - 1.0), vec2<f32>(1.0, - 1.0), vec2<f32>(- 1.0, 1.0), vec2<f32>(- 1.0, 1.0), vec2<f32>(1.0, - 1.0), vec2<f32>(1.0, 1.0));
    return vec4<f32>(pos[vertex_index], 0.0, 1.0);
}

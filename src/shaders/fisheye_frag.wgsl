struct SimParams {
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
    viewportWidth: f32,
    viewportHeight: f32,
    boundaryMode: u32,
    particleRenderSize: f32,
    forceScale: f32,
    rSmooth: f32,
    flatForce: u32,
    driftXPerSecond: f32,
    interTypeAttractionScale: f32,
    interTypeRadiusScale: f32,
    time: f32,
    fisheyeStrength: f32,
    backgroundColorR: f32,
    backgroundColorG: f32,
    backgroundColorB: f32,
    _padding1: f32,

    // Lenia-inspired parameters
    leniaEnabled: u32,
    leniaGrowthMu: f32,
    leniaGrowthSigma: f32,
    leniaKernelRadius: f32,

    // Lightning parameters
    lightningFrequency: f32,
    lightningIntensity: f32,
    lightningDuration: f32,
    _padding2: f32,
}

;

@group(0) @binding(0)
var<uniform> sim_params: SimParams;
@group(0) @binding(1)
var scene_sampler: sampler;
@group(0) @binding(2)
var scene_texture: texture_2d<f32>;

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    // frag_coord.xy is in intermediate texture coordinates (0 to 2400)
    // Convert to UV coordinates (0 to 1)
    let uv = frag_coord.xy / vec2<f32>(2400.0, 2400.0);

    // Center the UV coordinates around (0, 0) for distortion calculation
    let centered_uv = uv - 0.5;

    // Calculate the distance from center
    let original_dist = length(centered_uv);

    // Apply barrel/fisheye distortion using the formula: r' = r * (1 + k * r^2)
    let fisheye_strength = sim_params.fisheyeStrength;
    let r_squared = original_dist * original_dist;
    let distortion_factor = 1.0 + fisheye_strength * r_squared;

    // Calculate the distorted UV coordinates
    var distorted_uv: vec2<f32>;
    if (original_dist > 0.0) {
        // Apply the distortion and center back to [0,1] space
        distorted_uv = (centered_uv * distortion_factor) + 0.5;
    } else {
        distorted_uv = uv; // No distortion at the center
    }

    // Sample the scene texture (2400x2400) using the distorted UV coordinates
    let scene_color = textureSample(scene_texture, scene_sampler, distorted_uv);

    // Create a boundary mask to handle areas outside [0,1]
    let boundary_mask = step(0.0, distorted_uv.x) * step(distorted_uv.x, 1.0) *
                        step(0.0, distorted_uv.y) * step(distorted_uv.y, 1.0);

    // Optional vignetting effect
    let vignette_strength = 0.15;
    let vignette_factor = 1.0 - smoothstep(0.5, 0.8, original_dist) * vignette_strength;

    // Apply masking effects
    let final_mask = boundary_mask * vignette_factor;

    return vec4<f32>(scene_color.rgb * final_mask, scene_color.a);
}

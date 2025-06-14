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
    // frag_coord.xy is in virtual world texture coordinates (0 to virtualWorldWidth/Height)
    // Calculate where this fragment is relative to the canvas portion
    let canvas_pixel_x = frag_coord.x - sim_params.virtualWorldOffsetX;
    let canvas_pixel_y = frag_coord.y - sim_params.virtualWorldOffsetY;

    // Convert to canvas UV coordinates (0 to 1 within canvas area)
    let canvas_uv = vec2<f32>(canvas_pixel_x / sim_params.canvasRenderWidth, canvas_pixel_y / sim_params.canvasRenderHeight);

    // Center the UV coordinates around (0, 0) for distortion calculation
    let centered_uv = canvas_uv - 0.5;

    // Calculate the distance from center
    let original_dist = length(centered_uv);

    // Fisheye distortion parameters
    let fisheye_strength = sim_params.fisheyeStrength;

    // Apply barrel/fisheye distortion using the formula: r' = r * (1 + k * r^2)
    let r_squared = original_dist * original_dist;
    let distortion_factor = 1.0 + fisheye_strength * r_squared;

    // Calculate the distorted UV coordinates (still in canvas space)
    var distorted_canvas_uv: vec2<f32>;
    if (original_dist > 0.0) {
        // Apply the distortion and center back to [0,1] space
        distorted_canvas_uv = (centered_uv * distortion_factor) + 0.5;
    }
    else {
        distorted_canvas_uv = canvas_uv;
        // No distortion at the center
    }

    // Convert distorted canvas UV to virtual world texture coordinates for sampling
    let virtual_texture_x = distorted_canvas_uv.x * sim_params.canvasRenderWidth + sim_params.virtualWorldOffsetX;
    let virtual_texture_y = distorted_canvas_uv.y * sim_params.canvasRenderHeight + sim_params.virtualWorldOffsetY;
    let virtual_texture_uv = vec2<f32>(virtual_texture_x / sim_params.virtualWorldWidth, virtual_texture_y / sim_params.virtualWorldHeight);

    // Sample the scene texture using the virtual world texture coordinates (always executed)
    let scene_color = textureSample(scene_texture, scene_sampler, virtual_texture_uv);

    // Check if we're outside the canvas area and create boundary conditions
    let is_outside_canvas = canvas_pixel_x < 0.0 || canvas_pixel_x >= sim_params.canvasRenderWidth || canvas_pixel_y < 0.0 || canvas_pixel_y >= sim_params.canvasRenderHeight;

    // Create a smooth boundary mask to fade to black at edges
    let boundary_mask = smoothstep(0.0, 0.05, distorted_canvas_uv.x) * smoothstep(0.0, 0.05, distorted_canvas_uv.y) * smoothstep(0.0, 0.05, 1.0 - distorted_canvas_uv.x) * smoothstep(0.0, 0.05, 1.0 - distorted_canvas_uv.y);

    // Optional vignetting effect to complement the fisheye
    let vignette_strength = 0.15;
    let vignette_factor = 1.0 - smoothstep(0.5, 0.8, original_dist) * vignette_strength;

    // Apply all masking effects, including outside canvas check
    let final_mask = select(boundary_mask * vignette_factor, 0.0, is_outside_canvas);

    return vec4<f32>(scene_color.rgb * final_mask, scene_color.a);
}

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
    backgroundColor: vec3<f32>,
    _padding1: f32,
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
    // frag_coord.xy is in canvas coordinates (0 to canvasRenderWidth/Height)
    // Convert to UV coordinates for sampling the intermediate texture
    let canvas_uv = frag_coord.xy / vec2<f32>(sim_params.canvasRenderWidth, sim_params.canvasRenderHeight);

    // Convert canvas UV to intermediate texture coordinates
    // The intermediate texture is virtual world sized, and canvas content is at offset
    let intermediate_texture_x = canvas_uv.x * sim_params.canvasRenderWidth + sim_params.virtualWorldOffsetX;
    let intermediate_texture_y = canvas_uv.y * sim_params.canvasRenderHeight + sim_params.virtualWorldOffsetY;
    let intermediate_texture_uv = vec2<f32>(intermediate_texture_x / sim_params.virtualWorldWidth, intermediate_texture_y / sim_params.virtualWorldHeight);

    let scene_color = textureSample(scene_texture, scene_sampler, intermediate_texture_uv);

    // Calculate the center of the canvas for vignette effect
    let center = vec2<f32>(sim_params.canvasRenderWidth / 2.0, sim_params.canvasRenderHeight / 2.0);

    // Calculate the distance of the current fragment from the center
    let dist = distance(frag_coord.xy, center);

    // Make vignette radius relative to canvas size (50% of canvas width)
    let vignette_radius = sim_params.canvasRenderWidth * 0.5;
    // Calculate vignette alpha: 0.0 at center, smoothly to 0.5 at vignette_radius (50% transparent)
    let vignette_alpha_factor = smoothstep(0.0, vignette_radius, dist);
    let vignette_target_opacity = 0.5;
    let current_vignette_alpha = vignette_alpha_factor * vignette_target_opacity;

    // The vignette color is black
    let vignette_rgb = vec3<f32>(0.0, 0.0, 0.0);

    // Blend the scene color with the vignette color
    // final_rgb = scene_color.rgb * (1.0 - current_vignette_alpha) + vignette_rgb * current_vignette_alpha
    // Since vignette_rgb is black (0,0,0), this simplifies to:
    let final_rgb = scene_color.rgb * (1.0 - current_vignette_alpha);

    // Output the final color, assuming the canvas is opaque
    return vec4<f32>(final_rgb, 1.0);
}

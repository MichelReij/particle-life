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
    _padding0: f32,
    // Padding to align backgroundColor
    backgroundColor: vec3<f32>,
    _padding1: f32,
    // Padding to make total size 96 bytes (24 * 4)
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
    // Calculate UV coordinates for sampling the scene texture
    let uv = frag_coord.xy / vec2<f32>(sim_params.canvasRenderWidth, sim_params.canvasRenderHeight);
    let scene_color = textureSample(scene_texture, scene_sampler, uv);

    // Calculate the center of the canvas
    let center = vec2<f32>(sim_params.canvasRenderWidth / 2.0, sim_params.canvasRenderHeight / 2.0);

    // Calculate the distance of the current fragment from the center
    let dist = distance(frag_coord.xy, center);

    let vignette_radius = 400.0;
    // Calculate vignette alpha: 0.0 at center, smoothly to 0.2 at vignette_radius (temporarily changed from 0.3)
    // smoothstep(edge0, edge1, x) results in 0 if x < edge0, 1 if x > edge1, and smooth transition between.
    let vignette_alpha_factor = smoothstep(0.0, vignette_radius, dist);
    let vignette_target_opacity = 0.3;
    // Temporarily changed from 0.3 to 0.2
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

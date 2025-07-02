struct SimParams {
    delta_time: f32,
    friction: f32,
    num_particles: u32,
    num_types: u32,

    virtual_world_width: f32,
    virtual_world_height: f32,
    canvas_render_width: f32,
    canvas_render_height: f32,
    virtual_world_offset_x: f32,
    virtual_world_offset_y: f32,
    boundary_mode: u32,
    particle_render_size: f32,
    force_scale: f32,
    r_smooth: f32,
    flat_force: u32,
    drift_x_per_second: f32,
    inter_type_attraction_scale: f32,
    inter_type_radius_scale: f32,
    time: f32,
    fisheye_strength: f32,
    background_color_r: f32,
    background_color_g: f32,
    background_color_b: f32,

    // Lenia-inspired parameters
    lenia_enabled: u32,
    lenia_growth_mu: f32,
    lenia_growth_sigma: f32,
    lenia_kernel_radius: f32,

    // Lightning parameters
    lightning_frequency: f32,
    lightning_intensity: f32,
    lightning_duration: f32,

    // Particle transition parameters for GPU-based size transitions
    transition_active: u32,
    transition_start_time: f32,
    transition_duration: f32,
    transition_start_count: u32,
    transition_end_count: u32,
    transition_is_grow: u32,

    // Spatial grid optimization parameters
    spatial_grid_enabled: u32,
    spatial_grid_cell_size: f32,
    spatial_grid_width: u32,
    spatial_grid_height: u32,
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
    // frag_coord.xy is now in canvas coordinates (0 to canvas_width/height)
    // Convert to UV coordinates (0 to 1)
    let uv = frag_coord.xy / vec2<f32>(sim_params.canvas_render_width, sim_params.canvas_render_height);

    // Center the UV coordinates around (0, 0) for distortion calculation
    let centered_uv = uv - 0.5;

    // Calculate the distance from center
    let original_dist = length(centered_uv);

    // Apply barrel/fisheye distortion using the formula: r' = r * (1 + k * r^2)
    let fisheye_strength = sim_params.fisheye_strength;
    let r_squared = original_dist * original_dist;
    let distortion_factor = 1.0 + fisheye_strength * r_squared;

    // Calculate the distorted UV coordinates
    var distorted_uv: vec2<f32>;
    if (original_dist > 0.0) {
        // Apply the distortion and center back to [0,1] space
        distorted_uv = (centered_uv * distortion_factor) + 0.5;
    }
    else {
        distorted_uv = uv;
        // No distortion at the center
    }

    // Sample the scene texture (now canvas_width x canvas_height) using the distorted UV coordinates
    let scene_color = textureSample(scene_texture, scene_sampler, distorted_uv);

    // Create a boundary mask to handle areas outside [0,1]
    let boundary_mask = step(0.0, distorted_uv.x) * step(distorted_uv.x, 1.0) * step(0.0, distorted_uv.y) * step(distorted_uv.y, 1.0);

    // Optional vignetting effect
    let vignette_strength = 0.15;
    let vignette_factor = 1.0 - smoothstep(0.5, 0.8, original_dist) * vignette_strength;

    // Apply masking effects
    let final_mask = boundary_mask * vignette_factor;

    return vec4<f32>(scene_color.rgb * final_mask, scene_color.a);
}

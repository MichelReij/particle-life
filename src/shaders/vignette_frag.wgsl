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

    // Viewport/zoom parameters for rendering optimization
    viewport_center_x: f32,
    viewport_center_y: f32,
    viewport_width: f32,
    viewport_height: f32,
    viewport_radius: f32,

    // Padding to ensure 16-byte alignment (3 × f32 = 12 bytes)
    _viewport_padding1: f32,
    _viewport_padding2: f32,
    _viewport_padding3: f32,
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
    // frag_coord.xy is in canvas coordinates (0 to canvas_render_width/height)
    // Convert to UV coordinates for sampling the intermediate texture
    let canvas_uv = frag_coord.xy / vec2<f32>(sim_params.canvas_render_width, sim_params.canvas_render_height);

    // Convert canvas UV to intermediate texture coordinates
    // The intermediate texture is virtual world sized, and canvas content is at offset
    let intermediate_texture_x = canvas_uv.x * sim_params.canvas_render_width + sim_params.virtual_world_offset_x;
    let intermediate_texture_y = canvas_uv.y * sim_params.canvas_render_height + sim_params.virtual_world_offset_y;
    let intermediate_texture_uv = vec2<f32>(intermediate_texture_x / sim_params.virtual_world_width, intermediate_texture_y / sim_params.virtual_world_height);

    let scene_color = textureSample(scene_texture, scene_sampler, intermediate_texture_uv);

    // Calculate the center of the canvas for vignette effect
    let center = vec2<f32>(sim_params.canvas_render_width / 2.0, sim_params.canvas_render_height / 2.0);

    // Calculate the distance of the current fragment from the center
    let dist = distance(frag_coord.xy, center);

    // Make vignette radius relative to canvas size (50% of canvas width)
    let vignette_radius = sim_params.canvas_render_width * 0.5;
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

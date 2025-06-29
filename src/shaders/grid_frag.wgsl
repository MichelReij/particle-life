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
// No texture input needed for a static grid overlay, unless we were blending with previous pass manually.
// However, this pass will be blended by the pipeline settings on top of the canvas.

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let regular_line_thickness: f32 = 3.0;
    // Iets dikker voor betere zichtbaarheid bij alle zoom niveaus
    let center_line_thickness: f32 = 5.0;
    // Aangepast om proportie te behouden

    // Dynamic spacing: always show exactly 21 lines (10 + center + 10)
    // This means 20 intervals, so spacing = world_size / 20
    let spacing_x: f32 = sim_params.virtual_world_width / 20.0;
    let spacing_y: f32 = sim_params.virtual_world_height / 20.0;

    let line_color_rgb = vec3<f32>(1.0, 1.0, 1.0);
    // Jouw huidige waarde (wit)
    let line_alpha: f32 = 0.1;
    // Jouw huidige waarde

    // Grid should always be centered at the center of the virtual world
    let center_x = sim_params.virtual_world_width / 2.0;
    // Center of virtual world
    let center_y = sim_params.virtual_world_height / 2.0;
    // Center of virtual world

    // Calculate grid lines that are aligned with the virtual world center
    let offset_from_center_x = frag_coord.x - center_x;
    let offset_from_center_y = frag_coord.y - center_y;

    // Calculate distance to nearest grid line relative to center using dynamic spacing
    let mod_x = offset_from_center_x - spacing_x * floor(offset_from_center_x / spacing_x);
    let dist_to_regular_vertical_line = min(abs(mod_x), spacing_x - abs(mod_x));

    let mod_y = offset_from_center_y - spacing_y * floor(offset_from_center_y / spacing_y);
    let dist_to_regular_horizontal_line = min(abs(mod_y), spacing_y - abs(mod_y));

    var final_alpha: f32 = 0.0;

    // Use smoothstep for consistent anti-aliased lines at all zoom levels
    let falloff = 0.5;
    // Kleinere falloff voor scherpere lijnen met anti-aliasing

    // Calculate intensities using smoothstep for anti-aliasing
    let vertical_intensity = 1.0 - smoothstep(regular_line_thickness * 0.5 - falloff, regular_line_thickness * 0.5 + falloff, dist_to_regular_vertical_line);
    let horizontal_intensity = 1.0 - smoothstep(regular_line_thickness * 0.5 - falloff, regular_line_thickness * 0.5 + falloff, dist_to_regular_horizontal_line);

    // Center lines with thicker appearance
    let center_vertical_intensity = 1.0 - smoothstep(center_line_thickness * 0.5 - falloff, center_line_thickness * 0.5 + falloff, abs(frag_coord.x - center_x));
    let center_horizontal_intensity = 1.0 - smoothstep(center_line_thickness * 0.5 - falloff, center_line_thickness * 0.5 + falloff, abs(frag_coord.y - center_y));

    // Combine all line intensities - center lines override regular lines
    let regular_grid_intensity = max(vertical_intensity, horizontal_intensity);
    let center_lines_intensity = max(center_vertical_intensity, center_horizontal_intensity);

    // Center lines take priority, otherwise use regular grid
    final_alpha = max(regular_grid_intensity, center_lines_intensity) * line_alpha;

    return vec4<f32>(line_color_rgb, final_alpha);
}

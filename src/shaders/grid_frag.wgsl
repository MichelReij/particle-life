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

    // Padding to ensure 16-byte alignment
    _viewport_padding1: f32,
    _viewport_padding2: f32,
}

;

@group(0) @binding(0)
var<uniform> sim_params: SimParams;
// No texture input needed for a static grid overlay, unless we were blending with previous pass manually.
// However, this pass will be blended by the pipeline settings on top of the canvas.

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    // Convert screen coordinates to world coordinates using viewport
    let viewport_left = sim_params.viewport_center_x - sim_params.viewport_width * 0.5;
    let viewport_top = sim_params.viewport_center_y - sim_params.viewport_height * 0.5;

    // Convert fragment coord (0-1080) to world coordinate
    let world_x = viewport_left + (frag_coord.x / sim_params.canvas_render_width) * sim_params.viewport_width;
    let world_y = viewport_top + (frag_coord.y / sim_params.canvas_render_height) * sim_params.viewport_height;

    // Grid spacing in world coordinates (always show ~21 lines across the virtual world)
    let world_grid_spacing_x = sim_params.virtual_world_width / 20.0;
    let world_grid_spacing_y = sim_params.virtual_world_height / 20.0;

    // Calculate zoom level (how much we're zoomed in)
    let zoom_level = sim_params.virtual_world_width / sim_params.viewport_width;

    // Line thickness in world coordinates, scaled by zoom for consistent visual appearance
    let world_line_thickness = (3.0 / zoom_level);
    // Regular lines get thinner when zoomed in
    let world_center_line_thickness = (5.0 / zoom_level);
    // Center lines get thinner when zoomed in

    // World center coordinates
    let world_center_x = sim_params.virtual_world_width * 0.5;
    let world_center_y = sim_params.virtual_world_height * 0.5;

    // Calculate distance to grid lines in world coordinates
    let offset_from_center_x = world_x - world_center_x;
    let offset_from_center_y = world_y - world_center_y;

    // Distance to nearest vertical grid line
    let mod_x = offset_from_center_x - world_grid_spacing_x * floor(offset_from_center_x / world_grid_spacing_x);
    let dist_to_vertical_line = min(abs(mod_x), world_grid_spacing_x - abs(mod_x));

    // Distance to nearest horizontal grid line
    let mod_y = offset_from_center_y - world_grid_spacing_y * floor(offset_from_center_y / world_grid_spacing_y);
    let dist_to_horizontal_line = min(abs(mod_y), world_grid_spacing_y - abs(mod_y));

    // Distance to center lines
    let dist_to_center_vertical = abs(world_x - world_center_x);
    let dist_to_center_horizontal = abs(world_y - world_center_y);

    let line_color_rgb = vec3<f32>(1.0, 1.0, 1.0);
    let line_alpha: f32 = 0.1;

    // Anti-aliasing falloff in world coordinates
    let falloff = 0.5 / zoom_level;
    // Falloff scales with zoom to maintain sharpness

    // Calculate line intensities using smoothstep for anti-aliasing
    let vertical_intensity = 1.0 - smoothstep(world_line_thickness * 0.5 - falloff, world_line_thickness * 0.5 + falloff, dist_to_vertical_line);
    let horizontal_intensity = 1.0 - smoothstep(world_line_thickness * 0.5 - falloff, world_line_thickness * 0.5 + falloff, dist_to_horizontal_line);

    // Center lines with thicker appearance
    let center_vertical_intensity = 1.0 - smoothstep(world_center_line_thickness * 0.5 - falloff, world_center_line_thickness * 0.5 + falloff, dist_to_center_vertical);
    let center_horizontal_intensity = 1.0 - smoothstep(world_center_line_thickness * 0.5 - falloff, world_center_line_thickness * 0.5 + falloff, dist_to_center_horizontal);

    // Combine all line intensities - center lines override regular lines
    let regular_grid_intensity = max(vertical_intensity, horizontal_intensity);
    let center_lines_intensity = max(center_vertical_intensity, center_horizontal_intensity);

    // Center lines take priority, otherwise use regular grid
    let final_alpha = max(regular_grid_intensity, center_lines_intensity) * line_alpha;

    return vec4<f32>(line_color_rgb, final_alpha);
}

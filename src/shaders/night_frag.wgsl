// Night overlay fragment shader — draws a semi-transparent black quad.
// night_alpha is 0 during day, up to 0.5 at full night (WLP only).

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
    lenia_enabled: u32,
    lenia_growth_mu: f32,
    lenia_growth_sigma: f32,
    lenia_kernel_radius: f32,
    lightning_frequency: f32,
    lightning_intensity: f32,
    lightning_duration: f32,
    transition_active: u32,
    transition_start_time: f32,
    transition_duration: f32,
    transition_start_count: u32,
    transition_end_count: u32,
    transition_is_grow: u32,
    spatial_grid_enabled: u32,
    spatial_grid_cell_size: f32,
    spatial_grid_width: u32,
    spatial_grid_height: u32,
    viewport_center_x: f32,
    viewport_center_y: f32,
    viewport_width: f32,
    viewport_height: f32,
    viewport_radius: f32,
    night_alpha: f32,
    _viewport_padding2: f32,
    _viewport_padding3: f32,
}

@group(0) @binding(0)
var<uniform> sim_params: SimParams;

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, sim_params.night_alpha);
}

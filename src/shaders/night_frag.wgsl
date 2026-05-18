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
    wlp_start_time: f32,
    _viewport_padding3: f32,
}

@group(0) @binding(0)
var<uniform> sim_params: SimParams;

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    // night_alpha == 0 means HTV mode or full day — skip entirely
    if sim_params.night_alpha <= 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // A night band sweeps right→left over 20s (5s fade-in + 10s night + 5s fade-out).
    // The band has two parallel fuzzy edges of equal width:
    //   tx_eve  = avondschemering (leading/right edge, day→night)
    //   tx_morn = ochtendschemer  (trailing/left edge, night→day)
    //
    // core_width = 1.0 means the full-darkness core equals the canvas width.
    // band_width = core_width + 2*fuzz so that at peak night both schemering zones
    // sit exactly outside the canvas edges (and the viewer sees only uniform darkness).
    let cycle      = 60.0;
    let day        = 40.0;
    let non_day    = 40.0;  // 28 / 0.7 ≈ 40s
    let fuzz       = 0.45;  // half-width of each schemering zone (1.5× previous 0.30)
    let core_width = 1.5;   // 1.5× canvas width
    let band_width = core_width + 2.0 * fuzz;

    let t        = (sim_params.time - sim_params.wlp_start_time) % (day + non_day);
    let t_non    = max(t - day, 0.0);

    // Start: tx_morn = 1+fuzz (morning edge just off-screen right, entire band off-screen)
    // End:   tx_eve  = -fuzz  (evening edge just off-screen left,  entire band off-screen)
    // Total travel = 1 + 2*fuzz + band_width
    let travel   = 1.0 + 2.0 * fuzz + band_width;
    let tx_eve   = (1.0 + fuzz + band_width) - (t_non / non_day) * travel;
    let tx_morn  = tx_eve - band_width;

    let px       = frag_coord.x / sim_params.canvas_render_width;

    // avond: 1 left of tx_eve (night side), 0 right (day side)
    // — smoothstep requires edge0 < edge1, so use 1-smoothstep with correct order
    let eve_side  = 1.0 - smoothstep(tx_eve  - fuzz, tx_eve  + fuzz, px);
    // ochtend: 1 right of tx_morn (night side), 0 left (day side)
    let morn_side = smoothstep(tx_morn - fuzz, tx_morn + fuzz, px);

    let night = eve_side * morn_side;

    return vec4<f32>(0.0, 0.0, 0.0, night * 0.8);
}

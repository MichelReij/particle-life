// Night overlay fragment shader — draws a semi-transparent black quad.
// night_alpha is 0 during day, up to 0.8 at full night (WLP only).

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
    // night_alpha is 0 in HTV mode or full day — skip entirely.
    if sim_params.night_alpha <= 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Synchronise with simulation_params::update_night_alpha() cycle (120s):
    //   0–80s  = day  (night_alpha == 0, already returned above)
    //  80–90s  = fade-in   (band sweeps in from the right)
    //  90–110s = full night
    // 110–120s = fade-out  (band sweeps out to the left)
    let cycle   = 120.0;
    let day     = 80.0;
    let non_day = 40.0;

    let t     = (sim_params.time - sim_params.wlp_start_time) % cycle;
    let t_non = max(t - day, 0.0); // 0–40s within the non-day window

    // Band geometry: core wider than the canvas so at peak night the
    // entire screen is covered uniformly.
    let fuzz       = 0.30;
    let core_width = 1.4;
    let band_width = core_width + 2.0 * fuzz;

    // Band travels from fully off-screen right to fully off-screen left.
    let travel  = 1.0 + 2.0 * fuzz + band_width;
    let tx_eve  = (1.0 + fuzz + band_width) - (t_non / non_day) * travel;
    let tx_morn = tx_eve - band_width;

    let px = frag_coord.x / sim_params.canvas_render_width;

    let eve_side  = 1.0 - smoothstep(tx_eve  - fuzz, tx_eve  + fuzz, px);
    let morn_side = smoothstep(tx_morn - fuzz, tx_morn + fuzz, px);
    let night     = eve_side * morn_side;

    // night_alpha already encodes the fade-in/out envelope — use it to
    // scale the band so darkness matches the simulation's own timing.
    var alpha = night * sim_params.night_alpha;

    // Ordered (Bayer) dither: an 8-bit framebuffer only has 256 alpha steps,
    // and this overlay's smoothstep gradients are wide and slow-moving —
    // without dithering the quantization shows up as visible banded rings.
    // Adding a per-pixel offset before quantization breaks the bands into
    // fine-grained noise the eye perceives as smooth. A 4x4 pattern at ±1
    // LSB amplitude (the textbook-correct minimum) still left faint banding
    // visible — likely because this overlay is alpha-BLENDED over a
    // brightish background, so even a single alpha LSB step can translate
    // into a perceptible luminance step. Moved to an 8x8 pattern (less
    // repetitive, so any residual structure reads as texture rather than
    // rings) at roughly double the amplitude (~±2 LSB).
    let bayer = array<f32, 64>(
        0.0 / 64.0, 32.0 / 64.0,  8.0 / 64.0, 40.0 / 64.0,  2.0 / 64.0, 34.0 / 64.0, 10.0 / 64.0, 42.0 / 64.0,
        48.0 / 64.0, 16.0 / 64.0, 56.0 / 64.0, 24.0 / 64.0, 50.0 / 64.0, 18.0 / 64.0, 58.0 / 64.0, 26.0 / 64.0,
        12.0 / 64.0, 44.0 / 64.0,  4.0 / 64.0, 36.0 / 64.0, 14.0 / 64.0, 46.0 / 64.0,  6.0 / 64.0, 38.0 / 64.0,
        60.0 / 64.0, 28.0 / 64.0, 52.0 / 64.0, 20.0 / 64.0, 62.0 / 64.0, 30.0 / 64.0, 54.0 / 64.0, 22.0 / 64.0,
        3.0 / 64.0, 35.0 / 64.0, 11.0 / 64.0, 43.0 / 64.0,  1.0 / 64.0, 33.0 / 64.0,  9.0 / 64.0, 41.0 / 64.0,
        51.0 / 64.0, 19.0 / 64.0, 59.0 / 64.0, 27.0 / 64.0, 49.0 / 64.0, 17.0 / 64.0, 57.0 / 64.0, 25.0 / 64.0,
        15.0 / 64.0, 47.0 / 64.0,  7.0 / 64.0, 39.0 / 64.0, 13.0 / 64.0, 45.0 / 64.0,  5.0 / 64.0, 37.0 / 64.0,
        63.0 / 64.0, 31.0 / 64.0, 55.0 / 64.0, 23.0 / 64.0, 61.0 / 64.0, 29.0 / 64.0, 53.0 / 64.0, 21.0 / 64.0
    );
    let px_coord = vec2<u32>(frag_coord.xy) % vec2<u32>(8u, 8u);
    let dither = (bayer[px_coord.y * 8u + px_coord.x] - 0.5) * (2.0 / 255.0);
    alpha = clamp(alpha + dither, 0.0, 1.0);

    return vec4<f32>(0.0, 0.0, 0.0, alpha);
}

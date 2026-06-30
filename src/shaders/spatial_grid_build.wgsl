// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// GPU uniform-grid bucket sort. Bucket alle actieve deeltjes per cel zodat de
// hoofdfysica-pass (compute.wgsl) buren kan opzoeken via een handvol cellen i.p.v.
// over alle deeltjes te loopen (was O(n²), wordt ~O(n)).
//
// Drie entry points, elke frame ná elkaar gedispatcht vóór de fysica-pass:
//   1. count       — tel actieve deeltjes per cel (atomicAdd)
//   2. prefix_sum  — één thread: exclusive prefix sum → cel-startindex + vul-cursor
//   3. scatter     — schrijf elk deeltje-id op zijn plek in de gesorteerde array
//
// grid_cell_count wordt vóór "count" door de CPU-kant gecleard (encoder.clear_buffer).
// grid_cell_start en grid_fill_cursor worden door "prefix_sum" elke frame volledig
// herschreven, dus hoeven niet apart geclear te worden.

struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    ptype: u32,
    size: f32,
    target_size: f32,
    transition_start: f32,
    transition_type: u32,
    is_active: u32,
    spawn_time: f32,
    _padding2: f32,
}

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

@group(0) @binding(0) var<uniform> sim_params: SimParams;
@group(0) @binding(1) var<storage, read> particles_in: array<Particle>;
@group(0) @binding(2) var<storage, read_write> grid_cell_count: array<atomic<u32>>;
@group(0) @binding(3) var<storage, read_write> grid_cell_start: array<u32>;
@group(0) @binding(4) var<storage, read_write> grid_fill_cursor: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> grid_particle_indices: array<u32>;

fn cell_index_for(pos: vec2<f32>) -> u32 {
    let gw = i32(sim_params.spatial_grid_width);
    let gh = i32(sim_params.spatial_grid_height);
    let cs = sim_params.spatial_grid_cell_size;
    let cx = clamp(i32(floor(pos.x / cs)), 0, gw - 1);
    let cy = clamp(i32(floor(pos.y / cs)), 0, gh - 1);
    return u32(cy) * sim_params.spatial_grid_width + u32(cx);
}

@compute @workgroup_size(64)
fn count(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if (idx >= sim_params.num_particles) {
        return;
    }
    let p = particles_in[idx];
    if (p.is_active == 0u) {
        return;
    }
    atomicAdd(&grid_cell_count[cell_index_for(p.pos)], 1u);
}

@compute @workgroup_size(1)
fn prefix_sum() {
    let total_cells = sim_params.spatial_grid_width * sim_params.spatial_grid_height;
    var running: u32 = 0u;
    for (var c: u32 = 0u; c < total_cells; c = c + 1u) {
        let cell_count = atomicLoad(&grid_cell_count[c]);
        grid_cell_start[c] = running;
        atomicStore(&grid_fill_cursor[c], running);
        running = running + cell_count;
    }
}

@compute @workgroup_size(64)
fn scatter(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if (idx >= sim_params.num_particles) {
        return;
    }
    let p = particles_in[idx];
    if (p.is_active == 0u) {
        return;
    }
    let write_pos = atomicAdd(&grid_fill_cursor[cell_index_for(p.pos)], 1u);
    grid_particle_indices[write_pos] = idx;
}

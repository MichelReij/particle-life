// stats_compute.wgsl
// Berekent per-frame statistieken voor sonificatie.
// Dispatch: (8, 1, 1) workgroups van 64 threads.
//
//   Workgroups 0-6 : per-type stats
//   Workgroup  7   : globale stats (alle types samen)
//
// Output stats_out[i]:
//   i = 0..6 : vec4(viewport_count, energy, order, centroid_x)
//   i = 7    : vec4(total_viewport_count, cluster_count, avg_cluster_size, 0)
//
// viewport_count : absolute aantal particles van dit type in de viewport
// energy         : gemiddelde snelheidsgrootte (world-units/s)
// order          : clusteringmaat [0,1]; 1 = volledig geclusterd, 0 = uniform verspreid
// centroid_x     : gewogen X-positie in viewport [0,1] (voor stereo panning)
// cluster_count  : aantal niet-lege cellen in 8×8 viewport-grid (benadering #clusters)
// avg_cluster_size: total_viewport_count / cluster_count

struct Particle {
    pos:              vec2<f32>,
    vel:              vec2<f32>,
    ptype:            u32,
    size:             f32,
    target_size:      f32,
    transition_start: f32,
    transition_type:  u32,
    is_active:        u32,
    _padding1:        f32,
    _padding2:        f32,
}

struct SimParams {
    delta_time:             f32,
    friction:               f32,
    num_particles:          u32,
    num_types:              u32,
    virtual_world_width:    f32,
    virtual_world_height:   f32,
    canvas_render_width:    f32,
    canvas_render_height:   f32,
    virtual_world_offset_x: f32,
    virtual_world_offset_y: f32,
    boundary_mode:          u32,
    particle_render_size:   f32,
    force_scale:            f32,
    r_smooth:               f32,
    flat_force:             u32,
    drift_x_per_second:     f32,
    inter_type_attraction_scale: f32,
    inter_type_radius_scale:     f32,
    time:                   f32,
    fisheye_strength:       f32,
    background_color_r:     f32,
    background_color_g:     f32,
    background_color_b:     f32,
    lenia_enabled:          u32,
    lenia_growth_mu:        f32,
    lenia_growth_sigma:     f32,
    lenia_kernel_radius:    f32,
    lightning_frequency:    f32,
    lightning_intensity:    f32,
    lightning_duration:     f32,
    transition_active:      u32,
    transition_start_time:  f32,
    transition_duration:    f32,
    transition_start_count: u32,
    transition_end_count:   u32,
    transition_is_grow:     u32,
    spatial_grid_enabled:   u32,
    spatial_grid_cell_size: f32,
    spatial_grid_width:     u32,
    spatial_grid_height:    u32,
    viewport_center_x:      f32,
    viewport_center_y:      f32,
    viewport_width:         f32,
    viewport_height:        f32,
    viewport_radius:        f32,
    _pad1:                  f32,
    _pad2:                  f32,
    _pad3:                  f32,
}

@group(0) @binding(0) var<storage, read>       particles:  array<Particle>;
@group(0) @binding(1) var<uniform>             sim_params: SimParams;
@group(0) @binding(2) var<storage, read_write> stats_out:  array<vec4<f32>, 8>;

// Per-type scratch (workgroups 0-6): 6 × 64 × 4 = 1536 bytes
var<workgroup> scratch_count: array<f32, 64>;
var<workgroup> scratch_speed: array<f32, 64>;
var<workgroup> scratch_x:     array<f32, 64>;
var<workgroup> scratch_y:     array<f32, 64>;
var<workgroup> scratch_x2:    array<f32, 64>;
var<workgroup> scratch_y2:    array<f32, 64>;

// Globale scratch (workgroup 7)
var<workgroup> cell_counts:   array<atomic<u32>, 64>; // 8×8 viewport-grid
var<workgroup> scratch_total: array<f32, 64>;

@compute @workgroup_size(64)
fn main(
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id)        wg_id:    vec3<u32>,
) {
    let lid      = local_id.x;
    let type_idx = wg_id.x;

    let vp_left = sim_params.viewport_center_x - sim_params.viewport_width  * 0.5;
    let vp_top  = sim_params.viewport_center_y - sim_params.viewport_height * 0.5;
    let vp_w    = max(sim_params.viewport_width,  1.0);
    let vp_h    = max(sim_params.viewport_height, 1.0);

    if type_idx < 7u {
        // --- Per-type stats ---
        // Elke thread verwerkt particles op indices lid, lid+64, lid+128, ...
        // zodat alle particles precies één keer worden meegenomen.

        var lcount: f32 = 0.0;
        var lspeed: f32 = 0.0;
        var lx:     f32 = 0.0;
        var ly:     f32 = 0.0;
        var lx2:    f32 = 0.0;
        var ly2:    f32 = 0.0;

        var idx: u32 = lid;
        loop {
            if idx >= sim_params.num_particles { break; }
            let p = particles[idx];
            if p.is_active == 1u && p.ptype == type_idx {
                let nx = (p.pos.x - vp_left) / vp_w;
                let ny = (p.pos.y - vp_top)  / vp_h;
                if nx >= 0.0 && nx <= 1.0 && ny >= 0.0 && ny <= 1.0 {
                    lcount += 1.0;
                    lspeed += length(p.vel);
                    lx     += nx;
                    ly     += ny;
                    lx2    += nx * nx;
                    ly2    += ny * ny;
                }
            }
            idx += 64u;
        }

        scratch_count[lid] = lcount;
        scratch_speed[lid] = lspeed;
        scratch_x[lid]     = lx;
        scratch_y[lid]     = ly;
        scratch_x2[lid]    = lx2;
        scratch_y2[lid]    = ly2;
        workgroupBarrier();

        // Parallel reductie (halveer telkens: 32→16→8→4→2→1)
        var stride: u32 = 32u;
        loop {
            if lid < stride {
                scratch_count[lid] += scratch_count[lid + stride];
                scratch_speed[lid] += scratch_speed[lid + stride];
                scratch_x[lid]     += scratch_x[lid + stride];
                scratch_y[lid]     += scratch_y[lid + stride];
                scratch_x2[lid]    += scratch_x2[lid + stride];
                scratch_y2[lid]    += scratch_y2[lid + stride];
            }
            workgroupBarrier();
            if stride == 1u { break; }
            stride = stride >> 1u;
        }

        if lid == 0u {
            let total = scratch_count[0];
            var cx:     f32 = 0.5;
            var energy: f32 = 0.0;
            var order:  f32 = 0.5;

            if total > 0.0 {
                cx          = scratch_x[0] / total;
                let cy      = scratch_y[0] / total;
                energy      = scratch_speed[0] / total;

                // Variantie via E[x²] - E[x]² (één pass)
                let var_x   = max(scratch_x2[0] / total - cx * cx, 0.0);
                let var_y   = max(scratch_y2[0] / total - cy * cy, 0.0);
                let stddev  = sqrt(var_x + var_y);
                // Uniform over [0,1]² → stddev = sqrt(1/6) ≈ 0.408
                // order=1: volledig geclusterd; order=0: uniform verspreid
                order = 1.0 - clamp(stddev / 0.408, 0.0, 1.0);
            }

            stats_out[type_idx] = vec4<f32>(
                total,
                clamp(energy, 0.0, 3000.0),
                order,
                clamp(cx, 0.0, 1.0),
            );
        }

    } else {
        // --- Globale stats (workgroup 7): 8×8 grid cluster-benadering ---

        atomicStore(&cell_counts[lid], 0u);
        scratch_total[lid] = 0.0;
        workgroupBarrier();

        var local_total: f32 = 0.0;
        var idx: u32 = lid;
        loop {
            if idx >= sim_params.num_particles { break; }
            let p = particles[idx];
            if p.is_active == 1u {
                let nx = (p.pos.x - vp_left) / vp_w;
                let ny = (p.pos.y - vp_top)  / vp_h;
                if nx >= 0.0 && nx <= 1.0 && ny >= 0.0 && ny <= 1.0 {
                    local_total += 1.0;
                    let cell_x = u32(clamp(nx * 8.0, 0.0, 7.0));
                    let cell_y = u32(clamp(ny * 8.0, 0.0, 7.0));
                    atomicAdd(&cell_counts[cell_y * 8u + cell_x], 1u);
                }
            }
            idx += 64u;
        }

        scratch_total[lid] = local_total;
        workgroupBarrier();

        // Reduce totaal
        var stride: u32 = 32u;
        loop {
            if lid < stride {
                scratch_total[lid] += scratch_total[lid + stride];
            }
            workgroupBarrier();
            if stride == 1u { break; }
            stride = stride >> 1u;
        }

        if lid == 0u {
            let total = scratch_total[0];

            var cluster_count: f32 = 0.0;
            for (var c: u32 = 0u; c < 64u; c++) {
                if atomicLoad(&cell_counts[c]) > 0u {
                    cluster_count += 1.0;
                }
            }

            let avg_size = select(0.0, total / cluster_count, cluster_count > 0.0);
            stats_out[7] = vec4<f32>(total, cluster_count, avg_size, 0.0);
        }
    }
}

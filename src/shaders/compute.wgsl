struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    // type will be used to index into the rules array
    // and for coloring in the render pass.
    ptype: u32,
    // size: f32, // REVERTED
}

struct InteractionRule {
    attraction: f32,
    min_radius: f32,
    max_radius: f32,
    // Padding if needed, e.g. to make it vec4-sized if used in uniform,
    // but in storage buffer, less strict. 3*f32 = 12 bytes.
    // Add a padding f32 to make it 16 bytes for safety if it were uniform.
    // For storage buffer, this should be fine.
    _padding: f32,
    // To make it 16 bytes
}

struct SimParams {
    delta_time: f32,
    friction: f32,
    num_particles: u32,
    num_types: u32,

    virtual_world_width: f32,
    // e.g. 1000.0
    virtual_world_height: f32,
    // e.g. 1000.0
    canvas_render_width: f32,
    // e.g. 800.0 (used for rendering normalization)
    canvas_render_height: f32,
    // e.g. 800.0 (used for rendering normalization)
    virtual_world_offset_x: f32,
    // e.g. 100.0
    virtual_world_offset_y: f32,
    // e.g. 100.0
    boundary_mode: u32,
    // 0: disappear, 1: wrap (replaces wrap_mode)
    particle_render_size: f32,
    force_scale: f32,
    r_smooth: f32,
    // Moved r_smooth here for consistent ordering with TS
    flat_force: u32,
    // Moved flat_force here
    drift_x_per_second: f32,
    // New parameter for horizontal drift
    inter_type_attraction_scale: f32,
    // New parameter
    inter_type_radius_scale: f32,
    // New parameter
    time: f32,
    // Added time
    fisheyeStrength: f32,
    // Fisheye distortion strength
    backgroundColor: vec3<f32>,
    // New: background color
    _padding1: f32,
    // Padding to make total size 96 bytes (24 * 4)
}

// Particle data (input)
@group(0) @binding(0)
var<storage, read> particles_in: array<Particle>;
// Interaction rules: flat array, access via typeA * num_types + typeB
@group(0) @binding(1)
var<storage, read> rules: array<InteractionRule>;
// Simulation parameters
@group(0) @binding(2)
var<uniform> sim_params: SimParams;
// Particle data (output)
@group(0) @binding(3)
var<storage, read_write> particles_out: array<Particle>;

const PI: f32 = 3.141592653589793;
const EPSILON: f32 = 0.001;
// To avoid division by zero or sqrt(0)

@compute @workgroup_size(64) // Example workgroup size, can be tuned
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let p_idx = global_id.x;

    if (p_idx >= sim_params.num_particles) {
        return;
    }

    var particle_p = particles_in[p_idx];
    var total_force = vec2<f32>(0.0, 0.0);

    for (var q_idx: u32 = 0u; q_idx < sim_params.num_particles; q_idx = q_idx + 1u) {
        if (p_idx == q_idx) {
            continue;
        }

        let particle_q = particles_in[q_idx];

        var diff = particle_q.pos - particle_p.pos;

        // World wrapping for delta calculation (uses virtual world dimensions)
        if (sim_params.boundary_mode == 1u) {
            // 1u is wrap
            if (diff.x > sim_params.virtual_world_width * 0.5) {
                diff.x = diff.x - sim_params.virtual_world_width;
            }
            else if (diff.x < - sim_params.virtual_world_width * 0.5) {
                diff.x = diff.x + sim_params.virtual_world_width;
            }
            if (diff.y > sim_params.virtual_world_height * 0.5) {
                diff.y = diff.y - sim_params.virtual_world_height;
            }
            else if (diff.y < - sim_params.virtual_world_height * 0.5) {
                diff.y = diff.y + sim_params.virtual_world_height;
            }
        }
        // No special delta calculation for disappear mode, direct distance is fine.

        let dist_sq = dot(diff, diff);
        let rule_idx = particle_p.ptype * sim_params.num_types + particle_q.ptype;
        var rule = rules[rule_idx];
        var current_rule_attraction = rule.attraction;
        var current_rule_min_radius = rule.min_radius;
        var current_rule_max_radius = rule.max_radius;

        // Apply inter-type scaling if particle types are different
        if (particle_p.ptype != particle_q.ptype) {
            current_rule_attraction = rule.attraction * sim_params.inter_type_attraction_scale;
            current_rule_min_radius = rule.min_radius * sim_params.inter_type_radius_scale;
            current_rule_max_radius = rule.max_radius * sim_params.inter_type_radius_scale;
            // Ensure min_radius is not greater than max_radius after scaling
            if (current_rule_min_radius > current_rule_max_radius) {
                // Option 1: Swap them (simple fix)
                // let temp = current_rule_min_radius;
                // current_rule_min_radius = current_rule_max_radius;
                // current_rule_max_radius = temp;
                // Option 2: Clamp min_radius to max_radius (might be more stable)
                current_rule_min_radius = current_rule_max_radius;
            }
            // Ensure radii are positive
            current_rule_min_radius = max(EPSILON, current_rule_min_radius);
            current_rule_max_radius = max(EPSILON * 2.0, current_rule_max_radius);
        }

        if (dist_sq > current_rule_max_radius * current_rule_max_radius || dist_sq < EPSILON) {
            continue;
        }

        let dist = sqrt(dist_sq);
        let norm_diff = diff / dist;
        // Normalized direction vector

        var force_magnitude: f32 = 0.0;
        if (dist > current_rule_min_radius) {
            // Attraction/Repulsion based on rule.attraction
            if (sim_params.flat_force == 1u) {
                force_magnitude = current_rule_attraction;
            }
            else {
                let numer = 2.0 * abs(dist - 0.5 * (current_rule_max_radius + current_rule_min_radius));
                let denom = current_rule_max_radius - current_rule_min_radius;
                if (denom < EPSILON) {
                    // Avoid division by zero if min_radius is very close to max_radius
                    force_magnitude = current_rule_attraction;
                }
                else {
                    force_magnitude = current_rule_attraction * (1.0 - numer / denom);
                }
            }
        }
        else {
            // Strong repulsion if too close (within min_radius)
            // f = R_SMOOTH*minR*(1.0f/(minR + R_SMOOTH) - 1.0f / (r + R_SMOOTH));
            // This force is repulsive, so it should be positive if minR is positive.
            // The C++ code implies this force pushes particles apart.
            // A positive f means along norm_diff (q-p), so it pushes p towards q.
            // For repulsion, force should be against norm_diff.
            // Let's ensure the formula results in repulsion.
            // The formula from C++ seems to be for magnitude, direction is handled by dx/dy.
            // If r < minR, we want to push p away from q. So force is -norm_diff * magnitude.
            // The C++ code adds f*dx, f*dy to velocity. If f is positive, it's attraction.
            // So, for repulsion, f should be negative.
            // The formula `R_SMOOTH*minR*(1.0f/(minR + R_SMOOTH) - 1.0f / (r + R_SMOOTH))`
            // with r < minR, (r + R_SMOOTH) < (minR + R_SMOOTH), so 1/(r+RS) > 1/(minR+RS)
            // so (1/(minR+RS) - 1/(r+RS)) is negative.
            // Thus, the formula naturally gives a negative force for repulsion.
            force_magnitude = sim_params.r_smooth * current_rule_min_radius * (1.0 / (current_rule_min_radius + sim_params.r_smooth) - 1.0 / (dist + sim_params.r_smooth));
        }
        total_force = total_force + norm_diff * force_magnitude;
    }

    // Update velocity
    particle_p.vel = particle_p.vel + total_force * sim_params.force_scale * sim_params.delta_time;
    // Apply friction
    particle_p.vel = particle_p.vel * (1.0 - sim_params.friction);

    // Update position
    particle_p.pos = particle_p.pos + particle_p.vel * sim_params.delta_time;

    // Apply drift
    particle_p.pos.x = particle_p.pos.x + sim_params.drift_x_per_second * sim_params.delta_time;

    // Boundary conditions
    if (sim_params.boundary_mode == 1u) {
        // Wrap around virtual world
        if (particle_p.pos.x < 0.0) {
            particle_p.pos.x = particle_p.pos.x + sim_params.virtual_world_width;
        }
        if (particle_p.pos.x >= sim_params.virtual_world_width) {
            particle_p.pos.x = particle_p.pos.x - sim_params.virtual_world_width;
        }
        if (particle_p.pos.y < 0.0) {
            particle_p.pos.y = particle_p.pos.y + sim_params.virtual_world_height;
        }
        if (particle_p.pos.y >= sim_params.virtual_world_height) {
            particle_p.pos.y = particle_p.pos.y - sim_params.virtual_world_height;
        }
    }
    else if (sim_params.boundary_mode == 0u) {
        // Disappear and respawn
        let is_out_of_bounds = particle_p.pos.x < 0.0 || particle_p.pos.x >= sim_params.virtual_world_width || particle_p.pos.y < 0.0 || particle_p.pos.y >= sim_params.virtual_world_height;

        if (is_out_of_bounds) {
            // Reset velocity
            particle_p.vel = vec2<f32>(0.0, 0.0);

            // Randomize Y position for respawn
            particle_p.pos.y = random_float(global_id.x + particle_p.ptype) * sim_params.virtual_world_height;

            // Determine X respawn position based on drift direction
            if (sim_params.drift_x_per_second > EPSILON) {
                // Drifting significantly right, respawn left
                particle_p.pos.x = EPSILON;
            }
            else if (sim_params.drift_x_per_second < - EPSILON) {
                // Drifting significantly left, respawn right
                particle_p.pos.x = sim_params.virtual_world_width - EPSILON;
            }
            else {
                // No significant drift or drift is very close to zero, respawn on the right (consistent default)
                particle_p.pos.x = sim_params.virtual_world_width - EPSILON;
            }

            // particle_p.ptype = u32(random_float(global_id.x + u32(particle_p.pos.y)) * f32(sim_params.num_types)); // Optionally randomize type
        }
    }
    // No bounce mode implemented as per request.

    particles_out[p_idx] = particle_p;
}

// Simple pseudo-random number generator based on seed (e.g., particle index)
// Not cryptographically secure, but good enough for visual randomness.
fn random_float(seed: u32) -> f32 {
    var s = seed;
    s = (s ^ 61u) ^ (s >> 16u);
    s = s + (s << 3u);
    s = s ^ (s >> 4u);
    s = s * 0x27d4eb2du;
    s = s ^ (s >> 15u);
    return f32(s) / 4294967295.0;
    // Convert to [0, 1) float
}

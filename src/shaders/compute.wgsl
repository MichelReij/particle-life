struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    // type will be used to index into the rules array
    // and for coloring in the render pass.
    ptype: u32,
    size: f32,
    target_size: f32,
    transition_start: f32,
    // Start time of transition, 0 means no transition
    transition_type: u32,
    // 0 = grow, 1 = shrink
    is_active: u32,
    // Whether this particle is active/visible (bool as u32)
    _padding1: f32,
    _padding2: f32,
    // Ensure 16-byte alignment (48 bytes total)
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

// Lightning data structures (shared with lightning compute and rendering)
struct LightningSegment {
    start_pos: vec2<f32>,
    end_pos: vec2<f32>,
    thickness: f32,
    alpha: f32,
    generation: u32,
    appear_time: f32,
    is_visible: u32,
    _padding: u32,
    // Padding for alignment
    _padding2: u32,
    // Additional padding to align to 16-byte boundary (48 bytes total)
    _padding3: u32,
    // Final padding to reach 48 bytes (16-byte aligned)
}

struct LightningBolt {
    num_segments: u32,
    flash_id: u32,
    start_time: f32,
    next_lightning_time: f32,
    is_super_lightning: u32,
    // 1 if this is a super lightning, 0 if normal
    needs_rules_reset: u32,
    // 1 if interaction rules should be randomized, 0 otherwise
    _padding1: u32,
    // Padding for 16-byte alignment
    _padding2: u32,
    // Additional padding for alignment
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
    fisheye_strength: f32,
    // Fisheye distortion strength
    background_color_r: f32,
    // Background color red component
    background_color_g: f32,
    // Background color green component
    background_color_b: f32,
    // Background color blue component

    // Lenia-inspired parameters
    lenia_enabled: u32,
    // Boolean as u32: enable Lenia-style interactions
    lenia_growth_mu: f32,
    // Lenia growth function center (μ)
    lenia_growth_sigma: f32,
    // Lenia growth function spread (σ)
    lenia_kernel_radius: f32,
    // Lenia kernel radius in pixels

    // Lightning parameters
    lightning_frequency: f32,
    // Lightning strikes per second
    lightning_intensity: f32,
    // Lightning brightness/strength (0-1)
    lightning_duration: f32,
    // Duration of each lightning flash in seconds

    // Particle transition parameters for GPU-based size transitions
    transition_active: u32,
    // Boolean: is a transition currently active
    transition_start_time: f32,
    // When the transition started
    transition_duration: f32,
    // How long the transition should take
    transition_start_count: u32,
    // Particle count at start of transition
    transition_end_count: u32,
    // Target particle count
    transition_is_grow: u32,
    // Boolean: true for grow, false for shrink

    // Spatial grid optimization parameters
    spatial_grid_enabled: u32,
    // Boolean: enable spatial grid optimization (0=disabled, 1=enabled)
    spatial_grid_cell_size: f32,
    // Size of each grid cell in world units
    spatial_grid_width: u32,
    // Number of grid cells horizontally
    spatial_grid_height: u32,
    // Number of grid cells vertically
}

// Particle data (input)
@group(0) @binding(0)
var<storage, read> particles_in: array<Particle>;
// Interaction rules: flat array, access via typeA * num_types + typeB
@group(0) @binding(1)
var<storage, read_write> rules: array<InteractionRule>;
// Simulation parameters
@group(0) @binding(2)
var<uniform> sim_params: SimParams;
// Particle data (output)
@group(0) @binding(3)
var<storage, read_write> particles_out: array<Particle>;
// Lightning segments buffer (shared with rendering)
@group(0) @binding(4)
var<storage, read> lightning_segments: array<LightningSegment>;
// Lightning bolt buffer (shared with rendering) - single bolt structure
@group(0) @binding(5)
var<storage, read_write> lightning_bolt: LightningBolt;

const PI: f32 = 3.141592653589793;
const EPSILON: f32 = 0.001;
// To avoid division by zero or sqrt(0)

// === Spatial Grid Utility Functions ===

// Convert world position to grid cell coordinates
fn world_pos_to_grid_cell(pos: vec2<f32>) -> vec2<i32> {
    if (sim_params.spatial_grid_enabled == 0u) {
        return vec2<i32>(0, 0);
        // Return dummy value if grid is disabled
    }

    let cell_x = i32(floor(pos.x / sim_params.spatial_grid_cell_size));
    let cell_y = i32(floor(pos.y / sim_params.spatial_grid_cell_size));
    return vec2<i32>(cell_x, cell_y);
}

// Probabilistic culling for spatial grid optimization
fn should_process_spatial_interaction(p_idx: u32, q_idx: u32, p_cell: vec2<i32>, q_cell: vec2<i32>) -> bool {
    // Always process nearby particles
    let cell_dist_x = abs(p_cell.x - q_cell.x);
    let cell_dist_y = abs(p_cell.y - q_cell.y);
    let max_cell_dist = max(cell_dist_x, cell_dist_y);

    if (max_cell_dist <= 1) {
        return true;
        // Always process immediate neighbors
    }

    // For distant particles, use probabilistic sampling to reduce computation
    // This creates a deterministic but pseudo-random pattern based on particle indices
    let hash_input = (p_idx * 73u + q_idx * 101u) % 997u;
    // Large prime for better distribution
    let probability_threshold = 0.3;
    // Process 30% of distant interactions
    let random_value = f32(hash_input) / 997.0;

    return random_value < probability_threshold;
}

// Calculate edge damping factor for particles near world boundaries
fn calculate_edge_damping_factor(pos: vec2<f32>) -> f32 {
    // Define the damping zone near world edges
    let damping_zone = 50.0;
    // Distance from edge where damping starts
    let world_width = sim_params.virtual_world_width;
    let world_height = sim_params.virtual_world_height;

    // Calculate distance from each edge
    let dist_left = pos.x;
    let dist_right = world_width - pos.x;
    let dist_top = pos.y;
    let dist_bottom = world_height - pos.y;

    // Find minimum distance to any edge
    let min_edge_dist = min(min(dist_left, dist_right), min(dist_top, dist_bottom));

    // Calculate damping factor (1.0 = no damping, 0.0 = full damping)
    if (min_edge_dist >= damping_zone) {
        return 1.0;
        // No damping in the center
    }
    else {
        return min_edge_dist / damping_zone;
        // Linear damping near edges
    }
}

// === Lenia Functions ===

// Lenia kernel function: K(r) = exp(-r²/2σ²)
fn lenia_kernel(distance: f32, sigma: f32) -> f32 {
    let normalized_dist = distance / sigma;
    return exp(- 0.5 * normalized_dist * normalized_dist);
}

// Lenia growth function: μ(U) = 2 * exp(-(U-μ)²/2σ²) - 1
fn lenia_growth_function(density: f32, mu: f32, sigma: f32) -> f32 {
    let diff = density - mu;
    return 2.0 * exp(- 0.5 * diff * diff / (sigma * sigma)) - 1.0;
}

// Calculate local density around a particle using Lenia kernel
fn calculate_lenia_density(particle_pos: vec2<f32>, type_idx: u32, p_idx: u32) -> f32 {
    var density = 0.0;
    let kernel_radius = sim_params.lenia_kernel_radius;

    // Spatial grid optimization: only check particles in nearby cells
    let p_cell = world_pos_to_grid_cell(particle_pos);
    let cell_radius = i32(ceil(kernel_radius / sim_params.spatial_grid_cell_size)) + 1;

    for (var i: u32 = 0u; i < sim_params.num_particles; i = i + 1u) {
        let other_particle = particles_in[i];

        // Skip inactive particles in density calculations
        if (other_particle.is_active == 0u) {
            continue;
        }

        // Spatial grid culling: skip particles in distant cells
        if (sim_params.spatial_grid_enabled == 1u) {
            let q_cell = world_pos_to_grid_cell(other_particle.pos);
            let cell_dist_x = abs(p_cell.x - q_cell.x);
            let cell_dist_y = abs(p_cell.y - q_cell.y);
            let max_cell_dist = max(cell_dist_x, cell_dist_y);

            // Skip particles beyond the kernel radius in cell space
            if (max_cell_dist > cell_radius) {
                continue;
            }

            // Apply probabilistic culling for better performance
            if (!should_process_spatial_interaction(p_idx, i, p_cell, q_cell)) {
                continue;
            }
        }

        var diff = other_particle.pos - particle_pos;

        // Apply world wrapping for density calculation
        if (sim_params.boundary_mode == 0u) {
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

        let distance = length(diff);

        // Only consider particles within kernel radius
        if (distance < kernel_radius && distance > EPSILON) {
            let kernel_sigma = kernel_radius * 0.15;
            // More appropriate sigma
            let kernel_weight = lenia_kernel(distance, kernel_sigma);
            density = density + kernel_weight;
        }
    }
    // Normalize by kernel area instead of particle count for better scaling
    let kernel_area = PI * kernel_radius * kernel_radius;
    return density / kernel_area * 1000.0;
    // Scale to reasonable range (0-1)
}

// === Lightning Electromagnetic Force Functions ===

// Calculate electromagnetic force from lightning on a particle (buffer-based)
fn calculateLightningElectromagneticForce(particle_pos: vec2<f32>, particle_vel: vec2<f32>, time: f32, particle_type: u32) -> vec2<f32> {
    // Early exit if lightning is disabled or no active segments
    if (sim_params.lightning_frequency <= 0.0 || lightning_bolt.num_segments == 0u) {
        return vec2<f32>(0.0, 0.0);
    }

    var total_em_force = vec2<f32>(0.0, 0.0);

    // Process the single lightning bolt
    let bolt = lightning_bolt;

    // Check if this bolt is currently active
    let bolt_age = time - bolt.start_time;
    let flash_duration = sim_params.lightning_duration;

    if (bolt_age < 0.0 || bolt_age > flash_duration) {
        return total_em_force;
    }

    // Process each segment in this bolt
    for (var seg_idx = 0u; seg_idx < bolt.num_segments; seg_idx++) {
        let segment = lightning_segments[seg_idx];

        // Check if segment is visible
        if (segment.is_visible == 0u) {
            continue;
        }

        // Check if segment should be visible at current time
        let segment_age = time - segment.appear_time;

        // SINGLE FRAME ELECTROMAGNETIC EFFECT: Only apply force on the exact frame when segment becomes visible
        // This means segment_age should be between 0 and 1 frame duration
        let frame_duration = sim_params.delta_time;
        // Duration of one frame

        if (segment_age < 0.0 || segment_age > frame_duration) {
            continue;
        }

        // Calculate electromagnetic force from this segment
        let em_force = calculateSegmentElectromagneticForce(particle_pos, particle_vel, segment.start_pos, segment.end_pos, segment.generation, particle_type, time, segment_age);
        total_em_force = total_em_force + em_force;
    }

    return total_em_force;
}

// Calculate electromagnetic force from a single lightning segment
fn calculateSegmentElectromagneticForce(particle_pos: vec2<f32>, particle_vel: vec2<f32>, segment_start: vec2<f32>, segment_end: vec2<f32>, generation: u32, particle_type: u32, time: f32, segment_age: f32) -> vec2<f32> {
    // SIMPLIFIED REPULSION TEST: Work directly in UV coordinates
    // Convert particle position from world coordinates to UV coordinates
    // NOTE: Flip Y coordinate to match lightning rendering coordinate system
    let particle_uv = vec2<f32>(particle_pos.x / sim_params.virtual_world_width, 1.0 - (particle_pos.y / sim_params.virtual_world_height));

    // Lightning segments are already in UV coordinates (0.0-1.0)
    // Find closest point on segment to particle (in UV space)
    let segment_vec = segment_end - segment_start;
    let segment_length = length(segment_vec);

    if (segment_length < 0.001) {
        // 0.1% in UV space
        return vec2<f32>(0.0, 0.0);
    }

    let segment_dir = segment_vec / segment_length;
    let to_particle = particle_uv - segment_start;
    let projection = dot(to_particle, segment_dir);

    // Clamp projection to segment bounds
    let clamped_projection = clamp(projection, 0.0, segment_length);
    let closest_point = segment_start + segment_dir * clamped_projection;

    // Calculate distance in UV coordinates
    let force_vec_uv = particle_uv - closest_point;
    let distance_uv = length(force_vec_uv);

    // Only affect particles within smaller radius for more localized effect
    let influence_radius_uv = 0.03;

    if (distance_uv >= influence_radius_uv) {
        // || distance_uv < 0.001) {
        return vec2<f32>(0.0, 0.0);
    }

    // Simple repulsion: push particles away from lightning
    let repulsion_direction = normalize(force_vec_uv);
    let distance_factor = (influence_radius_uv - distance_uv) / influence_radius_uv;

    // Much weaker repulsion strength for gentle pushing effect instead of teleporting
    let repulsion_strength = 500.0 * sim_params.inter_type_attraction_scale * (1.0 + f32(generation) * 0.3);
    let repulsion_force_uv = repulsion_direction * distance_factor * repulsion_strength;

    // Convert force back to world coordinates
    let repulsion_force_world = vec2<f32>(repulsion_force_uv.x * sim_params.virtual_world_width, repulsion_force_uv.y * sim_params.virtual_world_height);

    return repulsion_force_world;
}

// === Random Number Generation ===

// Simple hash function for pseudo-random numbers
fn hash(seed: u32) -> u32 {
    var x = seed;
    x = x ^ (x >> 16u);
    x = x * 0x45d9f3bu;
    x = x ^ (x >> 16u);
    x = x * 0x45d9f3bu;
    x = x ^ (x >> 16u);
    return x;
}

// Generate random float in range [0, 1)
fn random_float(seed: u32) -> f32 {
    return f32(hash(seed)) / f32(0xFFFFFFFFu);
}

// Generate random float in range [min, max)
fn random_range(seed: u32, min_val: f32, max_val: f32) -> f32 {
    return min_val + random_float(seed) * (max_val - min_val);
}

// === Super Lightning Interaction Rules Randomization ===

// Check if super lightning is active and randomize interaction rules
fn check_and_randomize_rules() {
    // Check if there's a super lightning bolt that needs rules reset
    if (lightning_bolt.needs_rules_reset == 1u) {
        // Use lightning flash_id as base seed for randomization
        let base_seed = lightning_bolt.flash_id;

        // Randomize all interaction rules
        for (var type_a = 0u; type_a < sim_params.num_types; type_a++) {
            for (var type_b = 0u; type_b < sim_params.num_types; type_b++) {
                let rule_idx = type_a * sim_params.num_types + type_b;
                let seed = base_seed + rule_idx * 71u;
                // 71 is a prime for better distribution

                // Generate new random interaction rule
                // Force: random between -1.0 and 1.0
                let new_force = random_range(seed, - 1.0, 1.0);

                // MinRadius: random between 5.0 and 15.0
                let new_min_radius = random_range(seed + 1u, 5.0, 15.0);

                // MaxRadius: random between 20.0 and 80.0
                let new_max_radius = random_range(seed + 2u, 20.0, 80.0);

                // Update the rule
                rules[rule_idx].attraction = new_force;
                rules[rule_idx].min_radius = new_min_radius;
                rules[rule_idx].max_radius = new_max_radius;
            }
        }

        // Reset the flag to prevent re-randomization on subsequent frames
        lightning_bolt.needs_rules_reset = 0u;
    }
}

@compute @workgroup_size(64) // Example workgroup size, can be tuned
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let p_idx = global_id.x;

    // Only thread 0 checks for super lightning and randomizes rules (avoid race conditions)
    if (p_idx == 0u) {
        check_and_randomize_rules();
    }

    // Synchronize workgroup to ensure rule changes are visible to all threads
    workgroupBarrier();

    if (p_idx >= sim_params.num_particles) {
        return;
    }

    var particle_p = particles_in[p_idx];

    // Skip inactive particles - they don't participate in physics
    if (particle_p.is_active == 0u) {
        // Just copy the inactive particle to output without processing
        particles_out[p_idx] = particle_p;
        return;
    }

    // Debug: Ensure is_active is always 1 for active particles
    if (particle_p.is_active != 1u) {
        // Force it to 1 if it's not 0 or 1 (corrupted data)
        particle_p.is_active = 1u;
    }

    var total_force = vec2<f32>(0.0, 0.0);

    // Calculate Lenia forces if enabled
    if (sim_params.lenia_enabled == 1u) {
        // Calculate local density around this particle
        let local_density = calculate_lenia_density(particle_p.pos, particle_p.ptype, p_idx);

        // Apply Lenia growth function to determine growth/decay
        let growth_force = lenia_growth_function(local_density, sim_params.lenia_growth_mu, sim_params.lenia_growth_sigma);

        // Calculate density gradient for directional force
        let sample_radius = 5.0;
        // Small radius for gradient calculation
        let density_right = calculate_lenia_density(particle_p.pos + vec2<f32>(sample_radius, 0.0), particle_p.ptype, p_idx);
        let density_left = calculate_lenia_density(particle_p.pos - vec2<f32>(sample_radius, 0.0), particle_p.ptype, p_idx);
        let density_up = calculate_lenia_density(particle_p.pos + vec2<f32>(0.0, sample_radius), particle_p.ptype, p_idx);
        let density_down = calculate_lenia_density(particle_p.pos - vec2<f32>(0.0, sample_radius), particle_p.ptype, p_idx);

        // Calculate gradient (direction of steepest density increase)
        let gradient_x = (density_right - density_left) / (2.0 * sample_radius);
        let gradient_y = (density_up - density_down) / (2.0 * sample_radius);
        let density_gradient = vec2<f32>(gradient_x, gradient_y);

        // Apply growth-based movement (move toward favorable density if growth_force > 0)
        let lenia_force = density_gradient * growth_force * 1000.0;
        // Increased force multiplier for stronger Lenia influence

        total_force = total_force + lenia_force;
    }

    // Particle-particle interactions with spatial grid optimization
    let p_cell = world_pos_to_grid_cell(particle_p.pos);

    for (var q_idx: u32 = 0u; q_idx < sim_params.num_particles; q_idx = q_idx + 1u) {
        if (p_idx == q_idx) {
            continue;
        }

        let particle_q = particles_in[q_idx];

        // Skip interactions with inactive particles
        if (particle_q.is_active == 0u) {
            continue;
        }

        // Spatial grid culling: skip distant particles for better performance
        if (sim_params.spatial_grid_enabled == 1u) {
            let q_cell = world_pos_to_grid_cell(particle_q.pos);

            // Apply spatial culling based on interaction distance
            if (!should_process_spatial_interaction(p_idx, q_idx, p_cell, q_cell)) {
                continue;
            }
        }

        var diff = particle_q.pos - particle_p.pos;

        // World wrapping for delta calculation (uses virtual world dimensions)
        if (sim_params.boundary_mode == 2u) {
            // 2u is wrap
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
        // No special delta calculation for bounce mode or disappear mode, direct distance is fine.

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
        // Add electromagnetic cascade effect: fast-moving particles (recently affected by lightning)
        // create stronger interactions with nearby particles
        let other_particle_speed = length(particle_q.vel);
        let electromagnetic_boost = 1.0 + (other_particle_speed * 0.001);
        // Small boost for energetic particles

        force_magnitude = force_magnitude * electromagnetic_boost;

        total_force = total_force + norm_diff * force_magnitude;
    }

    // Add lightning electromagnetic forces
    let lightning_em_force = calculateLightningElectromagneticForce(particle_p.pos, particle_p.vel, sim_params.time, particle_p.ptype);

    total_force = total_force + lightning_em_force;

    // Apply edge damping to reduce forces near boundaries (prevents clustering)
    // Drift is preserved, only attraction/repulsion forces are dampened
    let edge_damping = calculate_edge_damping_factor(particle_p.pos);
    total_force = total_force * edge_damping;

    // Update velocity
    particle_p.vel = particle_p.vel + total_force * sim_params.force_scale * sim_params.delta_time;
    // Apply friction
    particle_p.vel = particle_p.vel * (1.0 - sim_params.friction);

    // Update position
    particle_p.pos = particle_p.pos + particle_p.vel * sim_params.delta_time;

    // Apply drift
    particle_p.pos.x = particle_p.pos.x + sim_params.drift_x_per_second * sim_params.delta_time;

    // Boundary conditions
    if (sim_params.boundary_mode == 0u) {
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
    else if (sim_params.boundary_mode == 1u) {
        // Hybrid mode: Horizontal wrap + Vertical bounce
        let bounce_damping = 0.8;
        let bounce_margin = 0.0005;
        // Much smaller margin: ~1.2px in virtual world, ~0.4px in canvas

        // Horizontal: Wrap around (like wrap mode)
        if (particle_p.pos.x < 0.0) {
            particle_p.pos.x = particle_p.pos.x + sim_params.virtual_world_width;
        }
        if (particle_p.pos.x >= sim_params.virtual_world_width) {
            particle_p.pos.x = particle_p.pos.x - sim_params.virtual_world_width;
        }

        // Vertical: Bounce with small margin to prevent sticking
        // Top boundary (y = 0)
        if (particle_p.pos.y < bounce_margin) {
            particle_p.pos.y = bounce_margin;
            if (particle_p.vel.y < 0.0) {
                particle_p.vel.y = - particle_p.vel.y * bounce_damping;
            }
        }
        // Bottom boundary (y = height)
        if (particle_p.pos.y >= sim_params.virtual_world_height - bounce_margin) {
            particle_p.pos.y = sim_params.virtual_world_height - bounce_margin;
            if (particle_p.vel.y > 0.0) {
                particle_p.vel.y = - particle_p.vel.y * bounce_damping;
            }
        }
    }
    else if (sim_params.boundary_mode == 2u) {
        // Disappear and respawn
        let is_out_of_bounds = particle_p.pos.x < 0.0 || particle_p.pos.x >= sim_params.virtual_world_width || particle_p.pos.y < 0.0 || particle_p.pos.y >= sim_params.virtual_world_height;

        if (is_out_of_bounds) {
            // Give particle initial velocity in drift direction to prevent sticking
            particle_p.vel = vec2<f32>(sim_params.drift_x_per_second * 1.8, 0.0);

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
                // No significant drift, respawn on the right (consistent default)
                particle_p.pos.x = sim_params.virtual_world_width - EPSILON;
            }

            // particle_p.ptype = u32(random_float(global_id.x + u32(particle_p.pos.y)) * f32(sim_params.num_types)); // Optionally randomize type
        }
    }

    // Handle per-particle transitions
    if (particle_p.transition_start > 0.0) {
        let elapsed = sim_params.time - particle_p.transition_start;
        let progress = clamp(elapsed / sim_params.transition_duration, 0.0, 1.0);

        if (progress < 1.0) {
            // Transition in progress
            if (particle_p.transition_type == 0u) {
                // Grow transition: activate immediately and interpolate size
                particle_p.is_active = 1u;
                // Activate at start of grow transition
                let min_visible_size = 3.0;
                particle_p.size = min_visible_size + (particle_p.target_size - min_visible_size) * progress;
            }
            else {
                // Shrink transition: stay active but interpolate size down
                // Don't deactivate until transition completes
                let min_visible_size = 0.1;
                particle_p.size = particle_p.target_size * (1.0 - progress) + min_visible_size * progress;
            }
        }
        else {
            // Transition complete
            if (particle_p.transition_type == 0u) {
                // Grow complete: set to target size and clear transition
                particle_p.size = particle_p.target_size;
                particle_p.transition_start = 0.0;
                // is_active already set to 1u above
            }
            else {
                // Shrink complete: deactivate particle and clear transition
                particle_p.is_active = 0u;
                // Deactivate at END of shrink transition
                particle_p.size = 0.1;
                particle_p.transition_start = 0.0;
            }
        }
    }

    // Final safety clamps to prevent visual issues
    particle_p.target_size = clamp(particle_p.target_size, 5.0, 22.0);
    particle_p.size = clamp(particle_p.size, 1.0, particle_p.target_size);

    particles_out[p_idx] = particle_p;
}

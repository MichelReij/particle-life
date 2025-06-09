struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    // type will be used to index into the rules array
    // and for coloring in the render pass.
    ptype: u32,
    size: f32,
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
    startPos: vec2<f32>,
    endPos: vec2<f32>,
    thickness: f32,
    alpha: f32,
    generation: u32,
    appearTime: f32,
    isVisible: u32,
    _padding: f32,
    // Ensure 16-byte alignment
}

struct LightningBolt {
    numSegments: u32,
    flashId: u32,
    startTime: f32,
    _padding: f32,
    // Ensure 16-byte alignment
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
// Lightning segments buffer (shared with rendering)
@group(0) @binding(4)
var<storage, read> lightning_segments: array<LightningSegment>;
// Lightning bolt buffer (shared with rendering) - single bolt structure
@group(0) @binding(5)
var<storage, read> lightning_bolt: LightningBolt;

const PI: f32 = 3.141592653589793;
const EPSILON: f32 = 0.001;
// To avoid division by zero or sqrt(0)

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
fn calculate_lenia_density(particle_pos: vec2<f32>, type_idx: u32) -> f32 {
    var density = 0.0;
    let kernel_radius = sim_params.lenia_kernel_radius;

    for (var i: u32 = 0u; i < sim_params.num_particles; i = i + 1u) {
        let other_particle = particles_in[i];
        var diff = other_particle.pos - particle_pos;

        // Apply world wrapping for density calculation
        if (sim_params.boundary_mode == 1u) {
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
            let kernel_weight = lenia_kernel(distance, kernel_radius * 0.3);
            density = density + kernel_weight;
        }
    }

    // Normalize density by maximum possible (all particles at center)
    return density / f32(sim_params.num_particles);
}

// === Lightning Electromagnetic Force Functions ===

// Hash function for lightning generation (matches fragment shader)
fn hash(x: f32) -> f32 {
    var p = x;
    p = fract(p * 0.1031);
    p *= p + 33.33;
    p *= p + p;
    return fract(p);
}

// Struct to represent a branch in the lightning system
struct LightningBranch {
    pos: vec2<f32>,
    dir: vec2<f32>,
    generation: u32,
    appear_time: f32,
}

;

// Calculate electromagnetic force from lightning on a particle (buffer-based)
fn calculateLightningElectromagneticForce(particle_pos: vec2<f32>, particle_vel: vec2<f32>, time: f32, particle_type: u32) -> vec2<f32> {
    // Check if lightning is enabled
    if (sim_params.lightning_frequency <= 0.0) {
        return vec2<f32>(0.0, 0.0);
    }

    var total_em_force = vec2<f32>(0.0, 0.0);

    // Process the single lightning bolt
    let bolt = lightning_bolt;

    // Skip inactive bolt
    if (bolt.numSegments == 0u) {
        return total_em_force;
    }

    // Check if this bolt is currently active
    let bolt_age = time - bolt.startTime;
    let flash_duration = sim_params.lightning_duration;

    if (bolt_age < 0.0 || bolt_age > flash_duration) {
        return total_em_force;
    }

    // Process each segment in this bolt
    for (var seg_idx = 0u; seg_idx < bolt.numSegments; seg_idx++) {
        let segment = lightning_segments[seg_idx];

        // Check if segment is visible
        if (segment.isVisible == 0u) {
            continue;
        }

        // Check if segment should be visible at current time
        let segment_age = time - segment.appearTime;
        let segment_duration = 0.4;
        // Same as in lightning compute shader

        if (segment_age < 0.0 || segment_age > segment_duration) {
            continue;
        }

        // Calculate electromagnetic force from this segment
        let em_force = calculateSegmentElectromagneticForce(particle_pos, particle_vel, segment.startPos, segment.endPos, segment.generation, particle_type, time, segment_age);
        total_em_force = total_em_force + em_force;
    }

    return total_em_force;
}

// Calculate electromagnetic force from a single lightning segment
fn calculateSegmentElectromagneticForce(particle_pos: vec2<f32>, particle_vel: vec2<f32>, segment_start: vec2<f32>, segment_end: vec2<f32>, generation: u32, particle_type: u32, time: f32, segment_age: f32) -> vec2<f32> {
    // Find closest point on segment to particle
    let segment_vec = segment_end - segment_start;
    let segment_length = length(segment_vec);

    if (segment_length < EPSILON) {
        return vec2<f32>(0.0, 0.0);
    }

    let segment_dir = segment_vec / segment_length;
    let to_particle = particle_pos - segment_start;
    let projection = dot(to_particle, segment_dir);

    // Clamp projection to segment bounds
    let clamped_projection = clamp(projection, 0.0, segment_length);
    let closest_point = segment_start + segment_dir * clamped_projection;

    // Calculate distance and force direction
    let force_vec = particle_pos - closest_point;
    let distance = length(force_vec);

    // Calculate influence radius - branch tips have more concentrated but intense fields
    let base_influence_radius = 150.0;
    let radius_scale = 1.0 - f32(generation) * 0.15;
    let influence_radius = base_influence_radius * max(0.5, radius_scale);

    if (distance >= influence_radius || distance < EPSILON) {
        return vec2<f32>(0.0, 0.0);
    }

    let normalized_distance = distance / influence_radius;
    let distance_factor = 1.0 - pow(normalized_distance, 1.5);

    // === CHARGE SEPARATION EFFECT ===
    // Different particle types have different "charge" based on their type
    let particle_type_f = f32(particle_type);
    let charge_polarity = sin(particle_type_f * 2.0); // Range: -1 to 1
    // Some types attracted (negative charge), others repelled (positive charge)

    // === ENHANCED MAGNETIC FIELD SPIRALING ===
    // Lightning creates a magnetic field around the current path
    // Perpendicular direction for magnetic force (right-hand rule)
    let perpendicular = vec2<f32>(-segment_dir.y, segment_dir.x);

    // Enhanced magnetic force with velocity dependence (Lorentz force: F = q(v × B))
    let particle_speed = length(particle_vel);
    let velocity_factor = max(0.5, particle_speed * 0.02); // Increased minimum and scale

    // Add time-based oscillations for dynamic spiral patterns
    let time_oscillation = sin(time * 3.0 + particle_type_f) * 0.5 + 1.0; // Range: 0.5 to 1.5
    let segment_oscillation = sin(segment_age * 5.0) * 0.3 + 1.0; // Segment-specific pulsing

    // Calculate magnetic force strength - much stronger and distance-dependent
    let magnetic_base_strength = (1.0 - pow(normalized_distance, 0.8)) * sim_params.lightning_intensity;
    let magnetic_strength = magnetic_base_strength * 1500.0 * velocity_factor * time_oscillation * segment_oscillation;

    // Create multiple spiral components for more complex motion
    let spiral_direction = perpendicular * charge_polarity;
    let magnetic_force = spiral_direction * magnetic_strength;

    // Enhanced rotational component with time-varying angle
    let base_rotation_angle = charge_polarity * 2.0; // Increased base rotation
    let time_rotation = time * 2.0 + particle_type_f; // Time-varying rotation
    let dynamic_rotation_angle = base_rotation_angle + sin(time_rotation) * 1.0;

    let cos_rot = cos(dynamic_rotation_angle);
    let sin_rot = sin(dynamic_rotation_angle);
    let rotated_perpendicular = vec2<f32>(
        perpendicular.x * cos_rot - perpendicular.y * sin_rot,
        perpendicular.x * sin_rot + perpendicular.y * cos_rot
    );
    let enhanced_spiral_force = rotated_perpendicular * magnetic_strength * 0.8; // Increased contribution

    // Add velocity-aligned magnetic component for helical motion
    let velocity_dir = normalize(particle_vel + vec2<f32>(0.001, 0.001)); // Avoid division by zero
    let velocity_cross_field = vec2<f32>(-velocity_dir.y, velocity_dir.x);
    let helical_force = velocity_cross_field * magnetic_strength * charge_polarity * 0.6;

    // === ELECTRIC FIELD FORCE ===
    // Traditional attraction/repulsion based on charge
    let electric_direction = force_vec / distance;
    let electric_strength = distance_factor * sim_params.lightning_intensity * 700.0; // Increased slightly
    let electric_force = electric_direction * electric_strength * charge_polarity;

    // === GENERATION SCALING ===
    // Higher generations (branch tips) have stronger fields due to charge concentration
    let generation_scale = 1.0 + f32(generation) * 0.4;

    // === PLASMA EFFECTS FOR VERY CLOSE PARTICLES ===
    let plasma_threshold = 0.25;
    var total_force = magnetic_force + enhanced_spiral_force + helical_force + electric_force;

    if (normalized_distance < plasma_threshold) {
        let plasma_intensity = (plasma_threshold - normalized_distance) / plasma_threshold;
        let plasma_boost = 1.0 + plasma_intensity * 3.0; // Increased plasma boost
        total_force = total_force * plasma_boost;
    }

    // Apply 75% force reduction (halved twice: 0.5 * 0.5 = 0.25)
    return total_force * generation_scale * 0.25;
}

@compute @workgroup_size(64) // Example workgroup size, can be tuned
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let p_idx = global_id.x;

    if (p_idx >= sim_params.num_particles) {
        return;
    }

    var particle_p = particles_in[p_idx];
    var total_force = vec2<f32>(0.0, 0.0);

    // Calculate Lenia forces if enabled
    if (sim_params.lenia_enabled == 1u) {
        // Calculate local density around this particle
        let local_density = calculate_lenia_density(particle_p.pos, particle_p.ptype);

        // Apply Lenia growth function to determine growth/decay
        let growth_force = lenia_growth_function(local_density, sim_params.lenia_growth_mu, sim_params.lenia_growth_sigma);

        // Calculate density gradient for directional force
        let sample_radius = 5.0;
        // Small radius for gradient calculation
        let density_right = calculate_lenia_density(particle_p.pos + vec2<f32>(sample_radius, 0.0), particle_p.ptype);
        let density_left = calculate_lenia_density(particle_p.pos - vec2<f32>(sample_radius, 0.0), particle_p.ptype);
        let density_up = calculate_lenia_density(particle_p.pos + vec2<f32>(0.0, sample_radius), particle_p.ptype);
        let density_down = calculate_lenia_density(particle_p.pos - vec2<f32>(0.0, sample_radius), particle_p.ptype);

        // Calculate gradient (direction of steepest density increase)
        let gradient_x = (density_right - density_left) / (2.0 * sample_radius);
        let gradient_y = (density_up - density_down) / (2.0 * sample_radius);
        let density_gradient = vec2<f32>(gradient_x, gradient_y);

        // Apply growth-based movement (move toward favorable density if growth_force > 0)
        let lenia_force = density_gradient * growth_force * 100.0;
        // Scale factor for effect strength
        total_force = total_force + lenia_force;
    }

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
        // Add electromagnetic cascade effect: fast-moving particles (recently affected by lightning)
        // create stronger interactions with nearby particles
        let other_particle_speed = length(particle_q.vel);
        let electromagnetic_boost = 1.0 + (other_particle_speed * 0.001);
        // Small boost for energetic particles

        force_magnitude = force_magnitude * electromagnetic_boost;

        total_force = total_force + norm_diff * force_magnitude;
    }

    // Add Lenia-inspired forces if enabled
    if (sim_params.lenia_enabled == 1u) {
        // Calculate local density and gradient
        let local_density = calculate_lenia_density(particle_p.pos, particle_p.ptype);

        // Calculate density gradient for directional movement
        let gradient_step = 5.0;
        // Small step for gradient calculation
        let density_right = calculate_lenia_density(particle_p.pos + vec2<f32>(gradient_step, 0.0), particle_p.ptype);
        let density_left = calculate_lenia_density(particle_p.pos - vec2<f32>(gradient_step, 0.0), particle_p.ptype);
        let density_up = calculate_lenia_density(particle_p.pos + vec2<f32>(0.0, gradient_step), particle_p.ptype);
        let density_down = calculate_lenia_density(particle_p.pos - vec2<f32>(0.0, gradient_step), particle_p.ptype);

        let gradient_x = (density_right - density_left) / (2.0 * gradient_step);
        let gradient_y = (density_up - density_down) / (2.0 * gradient_step);

        // Apply growth function to determine movement direction
        let growth_force = lenia_growth_function(local_density, sim_params.lenia_growth_mu, sim_params.lenia_growth_sigma);

        // Move towards optimal density regions
        let lenia_force = vec2<f32>(gradient_x, gradient_y) * growth_force * 200.0;
        // Scale factor for visibility

        total_force = total_force + lenia_force;
    }

    // Add lightning electromagnetic forces
    let lightning_em_force = calculateLightningElectromagneticForce(particle_p.pos, particle_p.vel, sim_params.time, particle_p.ptype);
    total_force = total_force + lightning_em_force;

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

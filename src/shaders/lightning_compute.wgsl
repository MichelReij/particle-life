// Lightning Generation Compute Shader
// Generates lightning segments and stores them in a buffer for both rendering and physics

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

    // Padding for 16-byte alignment (128 bytes total)
    _padding: f32,
    _padding2: f32,
}

// Lightning segment data structure
struct LightningSegment {
    start_pos: vec2<f32>,
    // Segment start position (UV coordinates)
    end_pos: vec2<f32>,
    // Segment end position (UV coordinates)
    thickness: f32,
    // Segment thickness in UV units
    alpha: f32,
    // Segment alpha/opacity
    generation: u32,
    // Branch generation (0, 1, 2, 3)
    appear_time: f32,
    // When this segment should appear
    is_visible: u32,
    // 1 if visible, 0 if not (boolean as u32)
    _padding: u32,
    // Padding to reach 48 bytes (16-byte aligned)
    _padding2: u32,
    // Additional padding for alignment
    _padding3: u32,
    // Final padding to reach 48 bytes total
}

// Lightning bolt data structure
struct LightningBolt {
    num_segments: u32,
    // Number of active segments in this bolt
    flash_id: u32,
    // Unique flash ID for this bolt
    start_time: f32,
    // When this bolt started
    next_lightning_time: f32,
    // When the next lightning should occur
}

@group(0) @binding(0)
var<uniform> sim_params: SimParams;

@group(0) @binding(1)
var<storage, read_write> lightning_segments: array<LightningSegment>;

@group(0) @binding(2)
var<storage, read_write> lightning_bolt: LightningBolt;

// More sophisticated hash-based random number generation
fn hash32(p: u32) -> u32 {
    var x = p;
    x = (x ^ (x >> 16u)) * 0x45d9f3bu;
    x = (x ^ (x >> 16u)) * 0x45d9f3bu;
    x = x ^ (x >> 16u);
    return x;
}

fn hash_to_float(x: u32) -> f32 {
    return f32(hash32(x)) / 4294967296.0;
    // 2^32
}

// Generate high-quality random seeds with multiple entropy sources
fn generate_segment_seed(time_base: f32, segment_idx: u32, seed_type: u32, extra_entropy: f32) -> f32 {
    // Convert float time to integer for better hashing
    let time_int = u32(time_base * 1000.0) + u32(extra_entropy * 10000.0);

    // Combine multiple entropy sources
    let combined_seed = hash32(time_int) ^ hash32(segment_idx * 0x9E3779B9u) ^ hash32(seed_type * 0x85EBCA6Bu) ^ hash32(u32(extra_entropy * 1000000.0));

    return hash_to_float(combined_seed);
}

// Generate multiple random values from a single seed for consistency
fn multi_random(seed: u32, count: u32) -> array<f32, 8> {
    var values: array<f32, 8>;
    for (var i = 0u; i < min(count, 8u); i = i + 1u) {
        values[i] = hash_to_float(hash32(seed + i * 0x9E3779B9u));
    }
    return values;
}



@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // Only use the first thread for lightning generation
    if (global_id.x != 0u || global_id.y != 0u || global_id.z != 0u) {
        return;
    }

    // Check if lightning is enabled
    if (sim_params.lightning_frequency <= 0.0) {
        lightning_bolt.num_segments = 0u;
        return;
    }

    let electricalActivity = sim_params.inter_type_attraction_scale;
    if (electricalActivity <= 0.0) {
        lightning_bolt.num_segments = 0u;
        return;
    }

    // Generate realistic lightning bolts based on time and electrical activity
    let time = sim_params.time;

    // Calculate interval based on electrical activity with much more variation
    // electricalActivity typically ranges from 0.0 to ~3.0 based on UI slider
    // Map to intervals: 15s (min activity ~0.0) to 2s (max activity ~3.0)
    let normalized_activity = clamp(electricalActivity / 3.0, 0.0, 1.0);
    let base_interval = 16.0 - (normalized_activity * 15.0);

    // Initialize next lightning time if not set (first time or after reset)
    if (lightning_bolt.next_lightning_time <= 0.0) {
        // For the first lightning, start much sooner to avoid long initial wait
        let init_seed = generate_segment_seed(time, 0u, 999u, electricalActivity);
        let quick_delay = init_seed * 3.0; // 0-3s initial delay
        lightning_bolt.next_lightning_time = time + quick_delay;
    }

    // Check if it's time for the next lightning bolt
    if (time >= lightning_bolt.next_lightning_time) {

        // Generate next lightning interval with multiple sources of variation
        let interval_seed1 = generate_segment_seed(time, lightning_bolt.flash_id, 1000u, electricalActivity);
        let interval_seed2 = generate_segment_seed(time + 1.0, lightning_bolt.flash_id, 1001u, electricalActivity * 2.0);
        let interval_seed3 = generate_segment_seed(time + 2.0, lightning_bolt.flash_id, 1002u, electricalActivity * 3.0);

        // Use different distributions for more natural variation
        // Exponential-like distribution for more realistic storm patterns
        let random_factor1 = pow(interval_seed1, 2.0); // Quadratic for more short intervals
        let random_factor2 = interval_seed2; // Linear
        let random_factor3 = sqrt(interval_seed3); // Square root for more long intervals

        // Combine multiple random factors for complex timing patterns
        let combined_random = (random_factor1 * 0.5) + (random_factor2 * 0.3) + (random_factor3 * 0.2);

        // Weather pattern simulation - create longer-term storm cycles
        let weather_cycle = sin(time * 0.05) * 0.5 + 0.5; // ~2-minute cycles
        let storm_intensity = sin(time * 0.02 + electricalActivity) * 0.3 + 0.7; // ~5-minute cycles
        let atmospheric_pressure = cos(time * 0.08 + 1.57) * 0.2 + 0.8; // ~1.5-minute cycles

        // Storm cell movement - simulate moving thunderstorm cells
        let storm_cell_x = sin(time * 0.03) * 0.5 + 0.5;
        let storm_cell_y = cos(time * 0.025) * 0.5 + 0.5;
        let storm_cell_proximity = 1.0 - length(vec2<f32>(storm_cell_x - 0.5, storm_cell_y - 0.5)) * 2.0;
        let storm_cell_factor = clamp(storm_cell_proximity, 0.2, 1.0);

        // Cumulative charge buildup - longer gaps increase probability
        let time_since_last = time - lightning_bolt.start_time;
        let charge_buildup = clamp(time_since_last / 20.0, 0.0, 2.0); // Builds up over 20 seconds
        let charge_factor = 1.0 + charge_buildup * 0.5; // Up to 50% increase in likelihood

        // Create bursts and lulls: sometimes lightning comes in clusters
        let burst_seed = generate_segment_seed(time, lightning_bolt.flash_id, 2000u, electricalActivity);

        // Enhanced burst/lull logic with weather influence
        let weather_burst_bias = weather_cycle * storm_intensity * atmospheric_pressure * storm_cell_factor;
        let burst_threshold = 0.3 + (weather_burst_bias - 0.5) * 0.2; // Weather affects burst probability
        let normal_threshold = 0.7 + (weather_burst_bias - 0.5) * 0.1; // Weather affects normal/lull balance

        var final_variation: f32;

        if (burst_seed < burst_threshold) {
            // Burst mode - very short intervals, influenced by weather
            let burst_intensity = weather_burst_bias * charge_factor;
            final_variation = (0.3 + combined_random * 1.2) / burst_intensity;
        } else if (burst_seed < normal_threshold) {
            // Normal mode - medium intervals with weather variation
            let normal_factor = (weather_burst_bias * 0.5 + 0.5) * charge_factor;
            final_variation = (1.5 + combined_random * 5.0) / normal_factor;
        } else {
            // Lull mode - longer intervals, less affected by charge buildup
            let lull_factor = weather_burst_bias * 0.7 + 0.3;
            final_variation = (4.0 + combined_random * 12.0) / (lull_factor * sqrt(charge_factor));
        }

        // Apply complex environmental scaling
        let environmental_factor = weather_cycle * storm_intensity * atmospheric_pressure * storm_cell_factor;
        final_variation *= (1.0 - normalized_activity * 0.6); // Activity scaling
        final_variation *= (1.0 - environmental_factor * 0.4); // Environmental scaling
        final_variation = max(final_variation, 0.2); // Minimum 0.2s interval

        lightning_bolt.next_lightning_time = time + base_interval + final_variation;

        // Time-based random seed generation for bolt creation
        let bolt_seed = fract(sin(time * 12.9898 + electricalActivity * 78.233 + f32(lightning_bolt.flash_id) * 91.2347) * 43758.5453);
        let bolt_seed_int = u32(bolt_seed * 4294967296.0);

        // Generate a new lightning bolt
        lightning_bolt.flash_id = lightning_bolt.flash_id + 1u;
        lightning_bolt.start_time = time;
        lightning_bolt.num_segments = 0u;

        // Clear all segment data for clean initialization
        for (var clear_idx = 0u; clear_idx < 20u; clear_idx = clear_idx + 1u) {
            lightning_segments[clear_idx].start_pos = vec2<f32>(0.0, 0.0);
            lightning_segments[clear_idx].end_pos = vec2<f32>(0.0, 0.0);
            lightning_segments[clear_idx].thickness = 0.0;
            lightning_segments[clear_idx].alpha = 0.0;
            lightning_segments[clear_idx].generation = 0u;
            lightning_segments[clear_idx].appear_time = 999999.0;
            // Far future - ensures segment starts invisible
            lightning_segments[clear_idx].is_visible = 0u;
            lightning_segments[clear_idx]._padding = 0u;
            lightning_segments[clear_idx]._padding2 = 0u;
            lightning_segments[clear_idx]._padding3 = 0u;
        }

        // Lightning starts within 0.2UV radius around center (0.5, 0.5) for round screen
        let position_randoms = multi_random(bolt_seed_int, 8u);

        // Generate random angle and distance within 0.2UV radius
        let random_angle = position_randoms[0] * 6.28318530718;
        // 0 to 2π
        let random_distance = sqrt(position_randoms[1]) * 0.2;
        // Square root for uniform distribution in circle

        let start_pos = vec2<f32>(0.5 + cos(random_angle) * random_distance, // Center at 0.5 + random offset
        0.5 + sin(random_angle) * random_distance);

        // First segment can go in any direction
        let initial_angle = position_randoms[2] * 6.28318530718;
        // 0 to 2π

        // Vary initial length within specified range
        let initial_length = 0.02 + position_randoms[3] * 0.01;
        // 0.03-0.05 UV units (specified range)

        let first_end = vec2<f32>(start_pos.x + cos(initial_angle) * initial_length, start_pos.y + sin(initial_angle) * initial_length);

        // Add the first segment with randomized thickness
        lightning_bolt.num_segments = 0u;
        // Start fresh
        if (lightning_bolt.num_segments < 20u) {
            lightning_segments[lightning_bolt.num_segments].start_pos = start_pos;
            lightning_segments[lightning_bolt.num_segments].end_pos = first_end;
            // Randomize main trunk thickness
            lightning_segments[lightning_bolt.num_segments].thickness = 0.0006 + position_randoms[4] * 0.0005;
            // 0.001-0.003 range
            lightning_segments[lightning_bolt.num_segments].alpha = 0.9 + position_randoms[5] * 0.1;
            // 0.9-1.0 alpha
            lightning_segments[lightning_bolt.num_segments].generation = 0u;
            lightning_segments[lightning_bolt.num_segments].appear_time = lightning_bolt.start_time + 0.01; // Small delay to match staggered timing
            lightning_segments[lightning_bolt.num_segments].is_visible = 0u;
                // Start invisible - will be made visible by lifecycle logic
                lightning_segments[lightning_bolt.num_segments]._padding = 0u;
                lightning_segments[lightning_bolt.num_segments]._padding2 = 0u;
                lightning_segments[lightning_bolt.num_segments]._padding3 = 0u;
            lightning_bolt.num_segments = lightning_bolt.num_segments + 1u;
        }

        // Generate branches recursively with much improved randomization
        var branch_queue: array<vec4<f32>, 20>;
        // x, y, angle, generation
        var parent_queue: array<u32, 20>;
        // Track parent segment index for each queue entry
        var queue_size = 0u;

        // Clear the parent queue to prevent contamination from previous bolts
        for (var clear_idx = 0u; clear_idx < 20u; clear_idx = clear_idx + 1u) {
            parent_queue[clear_idx] = 999u;
            // Invalid parent index
        }

        // Add initial branch to queue
        branch_queue[0] = vec4<f32>(first_end.x, first_end.y, initial_angle, 0.0);
        parent_queue[0] = 0u;
        // First segment is parent of first branch
        queue_size = 1u;

        // Process branches
        for (var queue_idx = 0u; queue_idx < queue_size && queue_idx < 20u; queue_idx = queue_idx + 1u) {
            let branch_info = branch_queue[queue_idx];
            let branch_pos = vec2<f32>(branch_info.x, branch_info.y);
            let parent_angle = branch_info.z;
            let generation = u32(branch_info.w);
            let parent_segment_idx = parent_queue[queue_idx];

            if (generation >= 7u) {
                continue;
            }
            // Max 7 generations

            // Much more sophisticated branching logic
            let branch_seed_base = hash32(bolt_seed_int + queue_idx * 0x9E3779B9u + generation * 0x85EBCA6Bu);
            let branch_randoms = multi_random(branch_seed_base, 8u);
            var num_spawns = 0u;

            if (generation <= 1u) {
                // Main trunk generations: Usually 1-2 branches
                if (branch_randoms[0] < 0.9) {
                    num_spawns = 2u;
                    // 90% chance: 2 branches
                }
                else {
                    num_spawns = 1u;
                    // 10% chance: 1 branch
                }
            }
            else if (generation == 2u) {
                // Secondary branches: Mix of 0-2 branches
                if (branch_randoms[0] < 0.01) {
                    num_spawns = 0u;
                    // 1% chance: terminate
                }
                else if (branch_randoms[0] < 0.3) {
                    num_spawns = 1u;
                    // 29% chance: 1 branch
                }
                else {
                    num_spawns = 2u;
                    // 70% chance: 2 branches
                }
            }
            else {
                // Tertiary+ branches: Mostly terminate, some continue
                if (branch_randoms[0] < 0.1) {
                    num_spawns = 0u;
                    // 10% chance: terminate
                }
                else if (branch_randoms[0] < 0.4) {
                    num_spawns = 1u;
                    // 30% chance: 1 branch
                }
                else {
                    num_spawns = 2u;
                    // 60% chance: 2 branches
                }
            }

            // Store first child angle for optimal second child positioning
            var first_child_angle: f32 = 0.0;

            // Generate spawned branches with realistic lightning physics
            for (var spawn_idx = 0u; spawn_idx < num_spawns; spawn_idx = spawn_idx + 1u) {
                if (lightning_bolt.num_segments >= 20u) {
                    break;
                }
                if (queue_size >= 20u) {
                    // Updated to match new queue size
                    break;
                }

                // Use unique random seeds for each branch with much more entropy
                let segment_seed_base = hash32(branch_seed_base + spawn_idx * 0x45d9f3bu + queue_idx * 0x9E3779B9u);
                let segment_randoms = multi_random(segment_seed_base, 8u);

                var new_angle: f32;

                if (num_spawns >= 2u && spawn_idx == 1u) {
                    // Second child: ensure good separation from first child
                    // Calculate the angle from parent to first child
                    var first_to_parent_diff = first_child_angle - parent_angle;

                    // Normalize to [-π, π] range
                    while (first_to_parent_diff > 3.14159265359) {
                        first_to_parent_diff = first_to_parent_diff - 6.28318530718;
                    }
                    while (first_to_parent_diff < - 3.14159265359) {
                        first_to_parent_diff = first_to_parent_diff + 6.28318530718;
                    }

                    // Ensure minimum separation angle (45 degrees = 0.7854 radians)
                    let min_separation = 0.7854;
                    // 45 degrees minimum
                    var separation_angle = abs(first_to_parent_diff);

                    if (separation_angle < min_separation) {
                        // If first child is too close to parent direction, adjust it
                        let sign = select(- 1.0, 1.0, first_to_parent_diff >= 0.0);
                        first_to_parent_diff = sign * min_separation;
                    }

                    // Place second child on the opposite side with guaranteed separation
                    new_angle = parent_angle - first_to_parent_diff;

                    // Add smaller randomization to avoid perfect symmetry
                    let random_offset = (segment_randoms[0] - 0.5) * 0.3;
                    // ±0.15 radians (~±9 degrees)
                    new_angle = new_angle + random_offset;
                }
                else {
                    // First child or single child: use random angle as before
                    // More realistic lightning branching angles (much smaller)
                    var angle_deviation: f32;
                    if (generation == 0u) {
                        // Main trunk: small deviation (10-35 degrees)
                        angle_deviation = 10.0 + segment_randoms[0] * 25.0;
                    }
                    else if (generation == 1u) {
                        // Primary branches: moderate spread (15-45 degrees)
                        angle_deviation = 15.0 + segment_randoms[0] * 30.0;
                    }
                    else {
                        // Secondary+ branches: wider angles (20-60 degrees)
                        angle_deviation = 20.0 + segment_randoms[0] * 40.0;
                    }

                    // Convert to radians
                    let angle_offset_radians = angle_deviation * 0.017453292519943;

                    // More natural turn direction with bias
                    var turn_direction: f32;
                    let turn_bias = segment_randoms[1];

                    // Random turn direction
                    turn_direction = select(- 1.0, 1.0, turn_bias > 0.5);

                    new_angle = parent_angle + turn_direction * angle_offset_radians;

                    // Store first child angle for second child calculation
                    if (spawn_idx == 0u) {
                        first_child_angle = new_angle;
                    }
                }

                // More realistic segment length variation based on generation
                var segment_length: f32;
                if (generation == 0u) {
                    // Main trunk: shorter segments (0.01-0.02 UV)
                    segment_length = 0.01 + segment_randoms[2] * 0.01;
                }
                else if (generation == 1u) {
                    // Primary branches: medium length (0.02-0.03 UV)
                    segment_length = 0.02 + segment_randoms[2] * 0.01;
                }
                else {
                    // Secondary+ branches: longest (0.02-0.05 UV)
                    segment_length = 0.02 + segment_randoms[2] * 0.02;
                }

                // Add slight length variation based on electrical activity
                segment_length *= (0.9 + electricalActivity * 0.2);

                // Calculate segment end position directly from angle and length
                let new_end = vec2<f32>(branch_pos.x + cos(new_angle) * segment_length, branch_pos.y + sin(new_angle) * segment_length);

                // Much more realistic thickness scaling
                var segment_thickness: f32;
                if (generation == 0u) {
                    segment_thickness = 0.0005 + segment_randoms[3] * 0.0004;
                    // 0.0005 - 0.0009
                }
                else if (generation == 1u) {
                    segment_thickness = 0.0004 + segment_randoms[3] * 0.0003;
                    // 0.0004 - 0.0007
                }
                else if (generation == 2u) {
                    segment_thickness = 0.0003 + segment_randoms[3] * 0.0002;
                    // 0.0003 - 0.0005
                }
                else {
                    segment_thickness = 0.0002 + segment_randoms[3] * 0.0001;
                    // 0.0002 - 0.0003
                }

                // Vary alpha based on generation for natural fading
                let base_alpha = 1.0 - (f32(generation) * 0.15);
                let alpha_variation = 0.8 + segment_randoms[4] * 0.2;
                let segment_alpha = base_alpha * alpha_variation;

                // Add segment with staggered timing for more natural appearance
                let segment_idx = lightning_bolt.num_segments;

                lightning_segments[segment_idx].start_pos = branch_pos;
                lightning_segments[segment_idx].end_pos = new_end;
                lightning_segments[segment_idx].thickness = segment_thickness;
                lightning_segments[segment_idx].alpha = segment_alpha;
                lightning_segments[segment_idx].generation = generation + 1u;

                // Stagger appearance time with more randomness
                let base_delay = f32(generation) * 0.08;
                // Base delay per generation
                let random_delay = segment_randoms[5] * 0.08;
                // 0-160ms random delay
                lightning_segments[segment_idx].appear_time = lightning_bolt.start_time + base_delay + random_delay;

                lightning_segments[segment_idx].is_visible = 0u;
                // Start invisible - will be made visible by lifecycle logic
                lightning_segments[segment_idx]._padding = 0u;
                lightning_segments[segment_idx]._padding2 = 0u;
                lightning_segments[segment_idx]._padding3 = 0u;

                lightning_bolt.num_segments = lightning_bolt.num_segments + 1u;

                // Add to queue for further branching
                if (queue_size < 20u) {
                    // Track this segment as parent for future branches
                    branch_queue[queue_size] = vec4<f32>(new_end.x, new_end.y, new_angle, f32(generation + 1u));
                    parent_queue[queue_size] = segment_idx;
                    // This new segment becomes the parent
                    queue_size = queue_size + 1u;
                }
            }
        }
    }
    else {
        // More sophisticated segment lifecycle management
        let lightning_age = time - lightning_bolt.start_time;

        // Don't update lifecycle in the same frame as creation to avoid race conditions
        if (lightning_age > 0.016) { // Skip first frame (16ms at 60fps)
            var any_segments_visible = false;

            // Update visibility with more natural fading and flickering
            for (var i = 0u; i < lightning_bolt.num_segments; i = i + 1u) {
                let segment_age = time - lightning_segments[i].appear_time;

            // Variable duration based on generation (main trunk lasts longer)
            // Increased durations for better study
            var segment_duration: f32;
            if (lightning_segments[i].generation == 0u) {
                segment_duration = 1.5 + hash_to_float(hash32(i * 0x9E3779B9u)) * 0.5;
                // 1.5 - 2.0s (main trunk)
            }
            else if (lightning_segments[i].generation == 1u) {
                segment_duration = 1.3 + hash_to_float(hash32(i * 0x85EBCA6Bu)) * 0.5;
                // 1.3 - 1.8s (primary branches)
            }
            else {
                segment_duration = 1.1 + hash_to_float(hash32(i * 0x45d9f3bu)) * 0.5;
                // 1.1 - 1.6s (secondary branches)
            }

            if (segment_age > 0.0 && segment_age < segment_duration) {
                lightning_segments[i].is_visible = 1u;
                any_segments_visible = true;

                // Simple fade: just apply fade factor to existing alpha, don't recalculate base
                let fade_factor = 1.0 - (segment_age / segment_duration);
                let fade_curve = fade_factor * fade_factor;

                // For new segments (first 0.1s), keep original alpha intact
                // For older segments, apply fade to a known bright base
                if (segment_age < 0.1) {
                    // Keep original alpha (already set correctly during creation)
                    // Do nothing - alpha is already correct
                } else {
                    // Apply fade, but use a bright base alpha similar to creation
                    let bright_base = 0.95; // Bright base like during creation
                    lightning_segments[i].alpha = bright_base * fade_curve;
                }
            }
            else {
                lightning_segments[i].is_visible = 0u;
                lightning_segments[i].alpha = 0.0;
            }
        }

            // Clear bolt when all segments are gone
            if (!any_segments_visible) {
                lightning_bolt.num_segments = 0u;
            }
        } // End of lifecycle management guard
    }
}

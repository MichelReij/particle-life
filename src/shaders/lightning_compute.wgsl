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
    // Padding for alignment
    _padding2: u32,
    // Additional padding to align to 16-byte boundary (48 bytes total)
    _padding3: u32,
    // Final padding to reach 48 bytes (16-byte aligned)
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
    collision_checks_count: u32,
    // Counter for collision checks performed (for debugging)
    _padding2: u32,
    // Padding for alignment
    _padding3: u32,
    // Additional padding to align to 16-byte boundary (32 bytes total)
    _padding4: u32,
    // Additional padding to align to 16-byte boundary (32 bytes total)
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

// Calculate the shortest distance between a point and a line segment
fn point_to_segment_distance(point: vec2<f32>, seg_start: vec2<f32>, seg_end: vec2<f32>) -> f32 {
    let seg_vec = seg_end - seg_start;
    let seg_length_sq = dot(seg_vec, seg_vec);

    if (seg_length_sq < 0.000001) {
        // Very short segment, treat as point
        return length(point - seg_start);
    }

    let t = clamp(dot(point - seg_start, seg_vec) / seg_length_sq, 0.0, 1.0);
    let projection = seg_start + t * seg_vec;
    return length(point - projection);
}

// Calculate the shortest distance between two line segments
// Special handling for segments that share endpoints (parent-child connections)
fn segment_to_segment_distance(a_start: vec2<f32>, a_end: vec2<f32>, b_start: vec2<f32>, b_end: vec2<f32>) -> f32 {
    let endpoint_threshold = 0.0001;
    // Very small threshold for endpoint matching

    // Check if segments share an endpoint (parent-child connection)
    // In this case, return a small distance to allow connection but prevent crossing
    if (length(a_start - b_start) < endpoint_threshold || length(a_start - b_end) < endpoint_threshold || length(a_end - b_start) < endpoint_threshold || length(a_end - b_end) < endpoint_threshold) {
        return 0.0001;
        // Small distance to allow connection
    }

    // Check distance from each endpoint to the other segment
    let dist1 = point_to_segment_distance(a_start, b_start, b_end);
    let dist2 = point_to_segment_distance(a_end, b_start, b_end);
    let dist3 = point_to_segment_distance(b_start, a_start, a_end);
    let dist4 = point_to_segment_distance(b_end, a_start, a_end);

    return min(min(dist1, dist2), min(dist3, dist4));
}

// Check if two points are the same (within tolerance)
fn points_equal(p1: vec2<f32>, p2: vec2<f32>) -> bool {
    let tolerance = 0.001;
    // Adjusted for UV coordinate scale
    return (abs(p1.x - p2.x) < tolerance && abs(p1.y - p2.y) < tolerance);
}

// Orientation function for three ordered points (p, q, r)
// Returns: 0 = collinear, 1 = clockwise, 2 = counterclockwise
fn orientation(p: vec2<f32>, q: vec2<f32>, r: vec2<f32>) -> u32 {
    let val = (q.y - p.y) * (r.x - q.x) - (q.x - p.x) * (r.y - q.y);
    if (abs(val) < 0.001) {
        // Increased tolerance for UV coordinates
        return 0u;
        // collinear
    }
    return select(2u, 1u, val > 0.0);
    // clockwise or counterclockwise
}

// Check if point q lies on line segment pr (assuming they are collinear)
fn on_segment(p: vec2<f32>, q: vec2<f32>, r: vec2<f32>) -> bool {
    return (q.x <= max(p.x, r.x) && q.x >= min(p.x, r.x) && q.y <= max(p.y, r.y) && q.y >= min(p.y, r.y));
}

// Check if two line segments intersect using orientation method
// Segment 1: p1->q1, Segment 2: p2->q2
// IMPORTANT: This excludes endpoint-only intersections (valid connections)
fn segments_intersect(p1: vec2<f32>, q1: vec2<f32>, p2: vec2<f32>, q2: vec2<f32>) -> bool {
    // Check if segments share endpoints (valid connections, not intersections)
    if (points_equal(p1, p2) || points_equal(p1, q2) || points_equal(q1, p2) || points_equal(q1, q2)) {
        return false;
        // Endpoints touching is valid, not an intersection
    }

    let o1 = orientation(p1, q1, p2);
    let o2 = orientation(p1, q1, q2);
    let o3 = orientation(p2, q2, p1);
    let o4 = orientation(p2, q2, q1);

    // General case: segments intersect if orientations are different
    if (o1 != o2 && o3 != o4) {
        return true;
    }

    // Special cases: segments are collinear and overlap (but not just touching at endpoints)
    // p1, q1 and p2 are collinear and p2 lies on segment p1q1
    if (o1 == 0u && on_segment(p1, p2, q1) && !points_equal(p2, p1) && !points_equal(p2, q1)) {
        return true;
    }
    // p1, q1 and q2 are collinear and q2 lies on segment p1q1
    if (o2 == 0u && on_segment(p1, q2, q1) && !points_equal(q2, p1) && !points_equal(q2, q1)) {
        return true;
    }
    // p2, q2 and p1 are collinear and p1 lies on segment p2q2
    if (o3 == 0u && on_segment(p2, p1, q2) && !points_equal(p1, p2) && !points_equal(p1, q2)) {
        return true;
    }
    // p2, q2 and q1 are collinear and q1 lies on segment p2q2
    if (o4 == 0u && on_segment(p2, q1, q2) && !points_equal(q1, p2) && !points_equal(q1, q2)) {
        return true;
    }

    return false;
    // No intersection
}

// Check if a proposed segment would collide with existing segments
// Check backwards from most recent segment down to just after parent
fn check_segment_collision(new_start: vec2<f32>, new_end: vec2<f32>, min_distance: f32, current_segments: u32, parent_segment_idx: u32) -> bool {
    // Check backwards from current_segments-1 down to parent_segment_idx+1
    // This checks only recently created peer segments, not older generations
    if (current_segments <= parent_segment_idx + 1u) {
        return false;
        // No segments to check
    }

    for (var i = current_segments - 1u; i > parent_segment_idx; i--) {
        let existing_start = lightning_segments[i].start_pos;
        let existing_end = lightning_segments[i].end_pos;

        // Use proper line segment intersection algorithm
        if (segments_intersect(new_start, new_end, existing_start, existing_end)) {
            return true;
            // Collision detected
        }
    }

    return false;
    // No collision
}

// [REMOVED] Comprehensive collision check function to reduce recursion count
// Now using simplified collision detection with limited attempts

// Find a valid position for a new segment with proper collision detection
fn find_valid_segment_position(start_pos: vec2<f32>, preferred_angle: f32, length: f32, min_distance: f32, current_segments: u32, parent_segment_idx: u32, random_values: array<f32, 8>) -> vec3<f32> {
    // Try the preferred angle first
    var test_end = vec2<f32>(start_pos.x + cos(preferred_angle) * length, start_pos.y + sin(preferred_angle) * length);

    if (!check_segment_collision(start_pos, test_end, min_distance, current_segments, parent_segment_idx)) {
        return vec3<f32>(test_end.x, test_end.y, 0.0);
        // No collision
    }

    // If preferred angle collides, try alternative angles
    let max_attempts = 8u;
    for (var attempt = 0u; attempt < max_attempts; attempt++) {
        // Generate alternative angle based on random values
        let angle_deviation = (random_values[attempt] - 0.5) * 1.57;
        // ±90 degrees max deviation
        let test_angle = preferred_angle + angle_deviation;

        test_end = vec2<f32>(start_pos.x + cos(test_angle) * length, start_pos.y + sin(test_angle) * length);

        if (!check_segment_collision(start_pos, test_end, min_distance, current_segments, parent_segment_idx)) {
            return vec3<f32>(test_end.x, test_end.y, 0.0);
            // Found collision-free position
        }
    }

    // If all attempts failed, return the preferred position but mark as collision
    test_end = vec2<f32>(start_pos.x + cos(preferred_angle) * length, start_pos.y + sin(preferred_angle) * length);
    return vec3<f32>(test_end.x, test_end.y, 1.0);
    // Collision detected (flag = 1.0)
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

    // Calculate interval based on electrical activity
    // electricalActivity typically ranges from 0.0 to ~3.0 based on UI slider
    // Map to intervals: 15s (min activity ~0.0) to 2s (max activity ~3.0) - much shorter for testing
    let normalized_activity = clamp(electricalActivity / 3.0, 0.0, 1.0);
    // Normalize to 0.0-1.0
    let base_interval = 30.0 - (normalized_activity * 22.0);
    // 15s to 2s range

    // Initialize next lightning time if not set (first time or after reset)
    if (lightning_bolt.next_lightning_time <= 0.0) {
        // For the first lightning, start much sooner to avoid long initial wait
        let init_seed = generate_segment_seed(time, 0u, 999u, electricalActivity);
        let quick_delay = init_seed * 2.0;
        // Only 0-2s initial delay
        lightning_bolt.next_lightning_time = time + quick_delay;
    }

    // Check if it's time for the next lightning bolt
    if (time >= lightning_bolt.next_lightning_time) {

        // Generate next lightning interval with reduced random variation
        let interval_seed = generate_segment_seed(time, lightning_bolt.flash_id, 1000u, electricalActivity);
        let random_variation = interval_seed * 3.0;
        // Reduced to 0-3s random variation
        lightning_bolt.next_lightning_time = time + base_interval + random_variation;

        // Time-based random seed generation for bolt creation
        let bolt_seed = fract(sin(time * 12.9898 + electricalActivity * 78.233 + f32(lightning_bolt.flash_id) * 91.2347) * 43758.5453);
        let bolt_seed_int = u32(bolt_seed * 4294967296.0);

        // Generate a new lightning bolt
        lightning_bolt.flash_id = lightning_bolt.flash_id + 1u;
        lightning_bolt.start_time = time;
        lightning_bolt.num_segments = 0u;
        lightning_bolt.collision_checks_count = 0u;
        // Reset collision counter for new bolt

        // Clear all segment data to prevent collision detection artifacts
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
            lightning_segments[lightning_bolt.num_segments].thickness = 0.0015 + position_randoms[4] * 0.0005;
            // 0.001-0.003 range
            lightning_segments[lightning_bolt.num_segments].alpha = 0.9 + position_randoms[5] * 0.1;
            // 0.9-1.0 alpha
            lightning_segments[lightning_bolt.num_segments].generation = 0u;
            lightning_segments[lightning_bolt.num_segments].appear_time = lightning_bolt.start_time;
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

                // Calculate minimum collision distance based on generation
                // REASONABLE distances for proper collision detection
                var min_collision_distance: f32;
                if (generation == 0u) {
                    min_collision_distance = 0.001;
                    // Small but detectable
                }
                else if (generation == 1u) {
                    min_collision_distance = 0.0008;
                    // Slightly smaller
                }
                else {
                    min_collision_distance = 0.0005;
                    // Smallest for fine branches
                }

                // Use collision detection to find position and check for collisions
                // Returns vec3: (end_x, end_y, collision_flag)
                let position_result = find_valid_segment_position(branch_pos, new_angle, segment_length, min_collision_distance, lightning_bolt.num_segments, parent_segment_idx, segment_randoms);
                let new_end = vec2<f32>(position_result.x, position_result.y);
                let collision_detected = position_result.z > 0.5;
                // collision_flag

                // Recalculate actual length after collision avoidance
                let actual_length = length(new_end - branch_pos);

                // Much more realistic thickness scaling
                var segment_thickness: f32;
                if (generation == 0u) {
                    segment_thickness = 0.0005 + segment_randoms[3] * 0.0004;
                    // 0.00150 - 0.00250
                }
                else if (generation == 1u) {
                    segment_thickness = 0.0004 + segment_randoms[3] * 0.0003;
                    // 0.00125 - 0.00175
                }
                else if (generation == 2u) {
                    segment_thickness = 0.0003 + segment_randoms[3] * 0.0002;
                    // 0.00125 - 0.00150
                }
                else {
                    segment_thickness = 0.0002 + segment_randoms[3] * 0.0001;
                    // Minimum thickness
                }

                // Vary alpha based on generation and collision status
                let base_alpha = 1.0 - (f32(generation) * 0.15);
                let alpha_variation = 0.8 + segment_randoms[4] * 0.2;
                var segment_alpha = base_alpha * alpha_variation;

                // Add segment with staggered timing for more natural appearance
                let segment_idx = lightning_bolt.num_segments;

                // PROPER COLLISION DETECTION: Use actual collision detection results
                var final_collision_detected = collision_detected;

                // COLLISION DEBUG: Use negative alpha to mark collision segments as RED
                if (final_collision_detected) {
                    segment_alpha = - segment_alpha;
                    // Negative alpha = RED in shader
                }
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

                // More sophisticated fading with multiple components
                let fade_factor = 1.0 - (segment_age / segment_duration);
                let fade_curve = fade_factor * fade_factor;
                // Quadratic fade for more natural look

                // Reduced flicker for easier study
                let slow_pulse = sin(time * 8.0 + f32(i) * 0.5) * 0.05;
                // Very gentle slow pulse
                let random_variation = hash_to_float(hash32(u32(time * 10.0) + i)) * 0.05 - 0.025;
                // Minimal random variation

                let flicker_component = 0.95 + slow_pulse + random_variation;
                let clamped_flicker = max(0.85, min(1.0, flicker_component));
                // Much more stable, minimal flicker

                // Base alpha varies per segment to avoid uniformity
                let base_alpha_variation = 0.9 + hash_to_float(hash32(i * 0x1234567u)) * 0.1;
                // 0.9-1.0 (more consistent)

                // PRESERVE COLLISION FLAG: Check if this was a collision segment (negative alpha)
                let was_collision = lightning_segments[i].alpha < 0.0;
                let new_alpha = fade_curve * clamped_flicker * base_alpha_variation;

                // Restore collision flag if it was originally a collision segment
                if (was_collision) {
                    lightning_segments[i].alpha = - new_alpha;
                    // Keep collision flag (negative)
                }
                else {
                    lightning_segments[i].alpha = new_alpha;
                    // Normal segment (positive)
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
    }
}

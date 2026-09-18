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

    // Particle transition parameters for GPU-based size transitions
    transition_active: u32,
    transition_start_time: f32,
    transition_duration: f32,
    transition_start_count: u32,
    transition_end_count: u32,
    transition_is_grow: u32,

    // Spatial grid optimization parameters
    spatial_grid_enabled: u32,
    spatial_grid_cell_size: f32,
    spatial_grid_width: u32,
    spatial_grid_height: u32,

    // Viewport/zoom parameters for rendering optimization
    viewport_center_x: f32,
    // Center of viewport in virtual world coordinates
    viewport_center_y: f32,
    // Center of viewport in virtual world coordinates
    viewport_width: f32,
    // Width of visible area in virtual world coordinates
    viewport_height: f32,
    // Height of visible area in virtual world coordinates
    viewport_radius: f32,
    // Radius of circular viewport in virtual world coordinates (for round screen)

    // Padding to ensure 16-byte alignment (3 × f32 = 12 bytes)
    night_alpha: f32,
    _viewport_padding2: f32,
    _viewport_padding3: f32,
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
    is_super_lightning: u32,
    // 1 if this is a super lightning, 0 if normal
    needs_rules_reset: u32,
    // 1 if interaction rules should be reset, 0 if not

    // --- Preview: the NEXT bolt's full geometry, pre-generated up to
    // PRE_GEN_LEAD seconds before it's actually due (see main()), and stored
    // in lightning_segments[40..80] rather than the live 0..40 range. At the
    // real strike moment it's simply copied into the live range ("promoted")
    // instead of being generated on the spot — so the pre-flash "thundercloud"
    // darkening (lightning_frag_buffer.wgsl) can be centred on the bolt's
    // real extent well before it fires, not just a single predicted point.
    preview_ready: u32,
    // 1 once a preview has been generated for the current next_lightning_time
    preview_num_segments: u32,
    preview_center_x: f32,
    preview_center_y: f32,
    preview_radius: f32,
    preview_is_super: u32,

    // --- Bounding circle around the LIVE (already-fired) bolt's segments —
    // copied from the preview at promotion time (or computed directly, in
    // the rare fallback where no preview was ready). Lets the post-flash
    // darkening in lightning_frag_buffer.wgsl cover the whole visible bolt.
    bolt_center_x: f32,
    bolt_center_y: f32,
    bolt_radius: f32,
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

// Generates one full bolt — trunk plus every recursively-branching segment —
// into lightning_segments[base_index .. base_index+40), using seed_time and
// seed_flash_id (rather than the live sim_params.time/lightning_bolt.flash_id)
// to anchor every random draw. This is what lets the exact same function
// serve two purposes: called with base_index=0, seed_time=(the current
// moment) it generates a bolt that fires immediately (the original,
// unchanged behaviour); called with base_index=40,
// seed_time=next_lightning_time it generates a PREVIEW of the upcoming bolt
// up to PRE_GEN_LEAD seconds early, bit-for-bit identical to what that bolt
// will look like once its own turn comes to be promoted into the live range.
// Returns (bounding-circle centre.x, centre.y, radius, segment count).
fn generate_bolt_segments(base_index: u32, seed_time: f32, seed_flash_id: u32, electrical_activity: f32, is_super: bool) -> vec4<f32> {
    // Clear this bolt's 40-slot range for clean initialization.
    for (var clear_idx = 0u; clear_idx < 40u; clear_idx = clear_idx + 1u) {
        let idx = base_index + clear_idx;
        lightning_segments[idx].start_pos = vec2<f32>(0.0, 0.0);
        lightning_segments[idx].end_pos = vec2<f32>(0.0, 0.0);
        lightning_segments[idx].thickness = 0.0;
        lightning_segments[idx].alpha = 0.0;
        lightning_segments[idx].generation = 0u;
        lightning_segments[idx].appear_time = 999999.0;
        // Far future - ensures segment starts invisible
        lightning_segments[idx].is_visible = 0u;
        lightning_segments[idx]._padding = 0u;
        lightning_segments[idx]._padding2 = 0u;
        lightning_segments[idx]._padding3 = 0u;
    }

    let bolt_seed = fract(sin(seed_time * 12.9898 + electrical_activity * 78.233 + f32(seed_flash_id) * 91.2347) * 43758.5453);
    let bolt_seed_int = u32(bolt_seed * 4294967296.0);

    // Lightning starts within 0.35UV radius around center (0.5, 0.5) for round screen
    let position_randoms = multi_random(bolt_seed_int, 8u);

    // Generate random angle and distance within 0.35UV radius
    let random_angle = position_randoms[0] * 6.28318530718;
    // 0 to 2π
    let random_distance = sqrt(position_randoms[1]) * 0.35;
    // Square root for uniform distribution in circle

    let start_pos = vec2<f32>(0.5 + cos(random_angle) * random_distance, // Center at 0.5 + random offset
    0.5 + sin(random_angle) * random_distance);

    // First segment can go in any direction
    let initial_angle = position_randoms[2] * 6.28318530718;
    // 0 to 2π

    // Compensate for smaller virtual world on smaller canvases so bolt size stays constant in CSS pixels
    let canvas_scale = 1080.0 / sim_params.canvas_render_width;

    // Vary initial length within specified range
    let initial_length = (0.02 + position_randoms[3] * 0.01) * canvas_scale;
    // 0.03-0.05 UV units (specified range)

    let first_end = vec2<f32>(start_pos.x + cos(initial_angle) * initial_length, start_pos.y + sin(initial_angle) * initial_length);

    // Add the first segment with randomized thickness
    var num_generated = 0u;
    if (num_generated < 40u) {
        let idx0 = base_index + num_generated;
        lightning_segments[idx0].start_pos = start_pos;
        lightning_segments[idx0].end_pos = first_end;

        // Super lightning has thicker, brighter bolts
        var segment_thickness: f32;
        let segment_alpha: f32 = 1.0;
        // Full alpha for visibility

        if (is_super) {
            // Super lightning: moderately thicker and brighter
            segment_thickness = (0.0012 + position_randoms[4] * 0.0008) * canvas_scale;
        }
        else {
            // Normal lightning thickness
            segment_thickness = (0.0009 + position_randoms[4] * 0.0007) * canvas_scale;
        }

        lightning_segments[idx0].thickness = segment_thickness;
        lightning_segments[idx0].alpha = segment_alpha;
        lightning_segments[idx0].generation = 0u;
        lightning_segments[idx0].appear_time = seed_time + 0.01;
        // Small delay to match staggered timing
        lightning_segments[idx0].is_visible = 0u;
        // Start invisible - will be made visible by lifecycle logic
        lightning_segments[idx0]._padding = 0u;
        lightning_segments[idx0]._padding2 = 0u;
        lightning_segments[idx0]._padding3 = 0u;
        num_generated = num_generated + 1u;
    }

    // Generate branches recursively with much improved randomization (increased array size for super lightning)
    var branch_queue: array<vec4<f32>, 40>;
    // x, y, angle, generation
    var parent_queue: array<u32, 40>;
    // Track parent segment index for each queue entry
    var queue_size = 0u;

    // Clear the parent queue to prevent contamination from previous bolts
    for (var clear_idx = 0u; clear_idx < 40u; clear_idx = clear_idx + 1u) {
        parent_queue[clear_idx] = 999u;
        // Invalid parent index
    }

    // Add initial branch to queue
    branch_queue[0] = vec4<f32>(first_end.x, first_end.y, initial_angle, 0.0);
    parent_queue[0] = 0u;
    // First segment is parent of first branch
    queue_size = 1u;

    // Process branches
    for (var queue_idx = 0u; queue_idx < queue_size && queue_idx < 40u; queue_idx = queue_idx + 1u) {
        let branch_info = branch_queue[queue_idx];
        let branch_pos = vec2<f32>(branch_info.x, branch_info.y);
        let parent_angle = branch_info.z;
        let generation = u32(branch_info.w);

        // Determine max generation based on super lightning - 2 extra generations for wider spread
        let max_generation = select(7u, 9u, is_super);

        if (generation >= max_generation) {
            continue;
        }

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
        else if (generation <= 4u) {
            // Secondary branches: Mix of 0-2 branches
            if (branch_randoms[0] < 0.3) {
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
                // 39% chance: 1 branch
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
            if (num_generated >= 40u) {
                break;
            }
            if (queue_size >= 40u) {
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

            // Zoom-aware segment length scaling for better detail at high zoom
            // Calculate the zoom factor to adjust segment detail
            let zoom_factor_detail = sim_params.virtual_world_width / sim_params.viewport_width;
            let detail_scale = 1.0 / sqrt(zoom_factor_detail);
            // More segments at higher zoom (square root for smoother scaling)

            // More realistic segment length variation based on generation (zoom-aware)
            var segment_length: f32;
            if (generation == 0u) {
                // Main trunk: shorter segments (0.018-0.035 UV) - scale with zoom for more detail
                segment_length = (0.015 + segment_randoms[2] * 0.015) * detail_scale * canvas_scale;
            }
            else if (generation == 1u) {
                // Primary branches: medium length (0.032-0.05 UV) - scale with zoom
                segment_length = (0.024 + segment_randoms[2] * 0.020) * detail_scale * canvas_scale;
            }
            else if (generation <= 7u) {
                // Secondary+ branches: longest (0.032-0.06 UV) - scale with zoom
                segment_length = (0.030 + segment_randoms[2] * 0.024) * detail_scale * canvas_scale;
            }
            else {
                // Super lightning extra generations (8-9): even longer for wider spread - scale with zoom
                segment_length = (0.032 + segment_randoms[2] * 0.025) * detail_scale * canvas_scale;
            }

            // Super lightning gets extra size boost for more dramatic effect
            if (is_super) {
                segment_length *= 2.5;
                // Make super lightning twice as large
            }
            else {
                // Add slight length variation based on electrical activity
                segment_length *= (0.7 + electrical_activity * 0.1);
            }

            // Calculate segment end position directly from angle and length
            let new_end = vec2<f32>(branch_pos.x + cos(new_angle) * segment_length, branch_pos.y + sin(new_angle) * segment_length);

            // Zoom-aware thickness scaling for crisp lightning at all zoom levels
            // Calculate the zoom factor to adjust thickness
            let zoom_factor = sim_params.virtual_world_width / sim_params.viewport_width;
            let base_thickness_scale = 1.0 / zoom_factor;
            // Thinner at higher zoom for crisp details

            // Much more realistic thickness scaling (thicker for better anti-aliasing)
            var segment_thickness: f32;
            if (generation == 0u) {
                segment_thickness = (0.0008 + segment_randoms[3] * 0.0006) * base_thickness_scale * canvas_scale;
            }
            else if (generation == 1u) {
                segment_thickness = (0.0007 + segment_randoms[3] * 0.0005) * base_thickness_scale * canvas_scale;
            }
            else if (generation == 2u) {
                segment_thickness = (0.0006 + segment_randoms[3] * 0.0004) * base_thickness_scale * canvas_scale;
            }
            else {
                segment_thickness = (0.0005 + segment_randoms[3] * 0.0003) * base_thickness_scale * canvas_scale;
            }

            // Vary alpha based on generation for natural fading
            let base_alpha = 1.0 - (f32(generation) * 0.15);
            let alpha_variation = 0.8 + segment_randoms[4] * 0.2;
            let segment_alpha = base_alpha * alpha_variation;

            // Add segment with staggered timing for more natural appearance
            let segment_idx = base_index + num_generated;

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
            lightning_segments[segment_idx].appear_time = seed_time + base_delay + random_delay;

            lightning_segments[segment_idx].is_visible = 0u;
            // Start invisible - will be made visible by lifecycle logic
            lightning_segments[segment_idx]._padding = 0u;
            lightning_segments[segment_idx]._padding2 = 0u;
            lightning_segments[segment_idx]._padding3 = 0u;

            num_generated = num_generated + 1u;

            // Add to queue for further branching
            if (queue_size < 40u) {
                // Track this segment as parent for future branches
                branch_queue[queue_size] = vec4<f32>(new_end.x, new_end.y, new_angle, f32(generation + 1u));
                parent_queue[queue_size] = segment_idx;
                // This new segment becomes the parent
                queue_size = queue_size + 1u;
            }
        }
    }

    // Compute a bounding circle around every segment this bolt actually ended
    // up with (trunk + all branches).
    var bounds_min = vec2<f32>(1e6, 1e6);
    var bounds_max = vec2<f32>(-1e6, -1e6);
    for (var b = 0u; b < num_generated; b = b + 1u) {
        let idx = base_index + b;
        bounds_min = min(bounds_min, min(lightning_segments[idx].start_pos, lightning_segments[idx].end_pos));
        bounds_max = max(bounds_max, max(lightning_segments[idx].start_pos, lightning_segments[idx].end_pos));
    }
    let bolt_center = (bounds_min + bounds_max) * 0.5;
    let bolt_radius = length(bounds_max - bounds_min) * 0.5;

    return vec4<f32>(bolt_center.x, bolt_center.y, bolt_radius, f32(num_generated));
}

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // Only use the first thread for lightning generation
    if (global_id.x != 0u || global_id.y != 0u || global_id.z != 0u) {
        return;
    }

    // Check if lightning is enabled. While disabled (HTV, or the slider at
    // 0 in WLP), sim_params.time keeps advancing but this whole shader
    // returns early every frame — so next_lightning_time/preview_ready were
    // being left completely frozen at whatever they were the instant
    // lightning turned off. Real time could then advance well past that
    // stale next_lightning_time before lightning is re-enabled, so the very
    // first frame back would see time already past (or just about to
    // reach) a strike that was actually scheduled ages ago — either firing
    // an oddly-timed bolt straight away, or (more visibly) the pre-flash
    // "thundercloud" darkening in lightning_frag_buffer.wgsl briefly
    // appearing with no bolt following it, since that darkening keys off
    // the same stale next_lightning_time. Resetting both here forces a
    // completely fresh schedule (same as the very first bolt ever, via the
    // next_lightning_time <= 0.0 branch below) the moment lightning turns
    // back on.
    if (sim_params.lightning_frequency <= 0.0) {
        lightning_bolt.num_segments = 0u;
        lightning_bolt.next_lightning_time = 0.0;
        lightning_bolt.preview_ready = 0u;
        return;
    }

    // NOTE: this used to read sim_params.inter_type_attraction_scale, a
    // derived bell-curve quantity (~-1.0 to +0.5, WLP-optimum-centred) that
    // is NOT the same thing as electrical activity despite the variable name
    // here — it went negative (disabling lightning entirely, see the check
    // below) across most of the electrical-activity slider's range,
    // including at max activity, and compressed the interval math below
    // toward its slowest setting even in the best case. lightning_frequency
    // is already the correct raw normalized (0-1) electrical activity value
    // for WLP (and already 0 for HTV, so this stays WLP-only).
    let electricalActivity = sim_params.lightning_frequency;
    if (electricalActivity <= 0.0) {
        lightning_bolt.num_segments = 0u;
        lightning_bolt.next_lightning_time = 0.0;
        lightning_bolt.preview_ready = 0u;
        return;
    }

    // Generate realistic lightning bolts based on time and electrical activity
    let time = sim_params.time;

    // *** SUPER LIGHTNING MODE ***
    // When super lightning is active, create much more intense storms

    // Calculate normalized activity first (needed in both modes).
    // electricalActivity (lightning_frequency) is already normalized 0-1 —
    // no further division needed (this used to divide by 3.0 again, which
    // silently compressed everything down to the 0-0.33 range).
    let normalized_activity = clamp(electricalActivity, 0.0, 1.0);

    // Normal lightning behavior - super lightning is just normal lightning with more branches
    let super_lightning_multiplier = 1.0;

    // Calculate interval based on electrical activity with much more variation.
    // electricalActivity is normalized 0-1 (see the fix above) — 16s interval
    // at zero activity down to 1s at max activity.
    let base_interval = 16.0 - (normalized_activity * 15.0);

    // Initialize next lightning time if not set (first time or after reset)
    if (lightning_bolt.next_lightning_time <= 0.0) {
        // For the first lightning, start much sooner to avoid long initial wait
        let init_seed = generate_segment_seed(time, 0u, 999u, electricalActivity);
        let quick_delay = init_seed * 3.0;
        // 0-3s initial delay
        lightning_bolt.next_lightning_time = time + quick_delay;
        lightning_bolt.preview_ready = 0u;
    }

    // Pre-generate the NEXT bolt's full geometry (trunk + every branch) up to
    // PRE_GEN_LEAD seconds before it's actually due, once per scheduled slot.
    // Written into lightning_segments[40..80] — entirely separate from the
    // live 0..40 range, so it never disturbs whatever bolt is currently
    // fading out there. This is what lets the pre-flash "thundercloud"
    // darkening (lightning_frag_buffer.wgsl) know the coming strike's real
    // extent well in advance, instead of just a single guessed point.
    let pre_gen_lead = 3.0;
    if (lightning_bolt.preview_ready == 0u && lightning_bolt.next_lightning_time > 0.0 && time >= lightning_bolt.next_lightning_time - pre_gen_lead) {
        let preview_flash_id = lightning_bolt.flash_id + 1u;
        // 5% chance for super lightning, rolled now so the preview and the
        // bolt it becomes are bit-for-bit identical.
        let preview_super_roll = fract(sin(lightning_bolt.next_lightning_time * 87.9123 + electricalActivity * 52.845 + f32(preview_flash_id) * 63.7429) * 37281.9876);
        let preview_is_super_bool = preview_super_roll < 0.05;

        let preview_result = generate_bolt_segments(40u, lightning_bolt.next_lightning_time, preview_flash_id, electricalActivity, preview_is_super_bool);
        lightning_bolt.preview_center_x = preview_result.x;
        lightning_bolt.preview_center_y = preview_result.y;
        lightning_bolt.preview_radius = preview_result.z;
        lightning_bolt.preview_num_segments = u32(preview_result.w);
        lightning_bolt.preview_is_super = select(0u, 1u, preview_is_super_bool);
        lightning_bolt.preview_ready = 1u;
    }

    // Check if it's time for the next lightning bolt
    if (time >= lightning_bolt.next_lightning_time) {

        // Generate next lightning interval with multiple sources of variation
        let interval_seed1 = generate_segment_seed(time, lightning_bolt.flash_id, 1000u, electricalActivity);
        let interval_seed2 = generate_segment_seed(time + 1.0, lightning_bolt.flash_id, 1001u, electricalActivity * 2.0);
        let interval_seed3 = generate_segment_seed(time + 2.0, lightning_bolt.flash_id, 1002u, electricalActivity * 3.0);

        // Use different distributions for more natural variation
        // Exponential-like distribution for more realistic storm patterns
        let random_factor1 = pow(interval_seed1, 2.0);
        // Quadratic for more short intervals
        let random_factor2 = interval_seed2;
        // Linear
        let random_factor3 = sqrt(interval_seed3);
        // Square root for more long intervals

        // Combine multiple random factors for complex timing patterns
        let combined_random = (random_factor1 * 0.5) + (random_factor2 * 0.3) + (random_factor3 * 0.2);

        // Weather pattern simulation - create longer-term storm cycles
        let weather_cycle = sin(time * 0.05) * 0.5 + 0.5;
        // ~2-minute cycles
        let storm_intensity = sin(time * 0.02 + electricalActivity) * 0.3 + 0.7;
        // ~5-minute cycles
        let atmospheric_pressure = cos(time * 0.08 + 1.57) * 0.2 + 0.8;
        // ~1.5-minute cycles

        // Storm cell movement - simulate moving thunderstorm cells
        let storm_cell_x = sin(time * 0.03) * 0.5 + 0.5;
        let storm_cell_y = cos(time * 0.025) * 0.5 + 0.5;
        let storm_cell_proximity = 1.0 - length(vec2<f32>(storm_cell_x - 0.5, storm_cell_y - 0.5)) * 2.0;
        let storm_cell_factor = clamp(storm_cell_proximity, 0.2, 1.0);

        // Cumulative charge buildup - longer gaps increase probability
        let time_since_last = time - lightning_bolt.start_time;
        let charge_buildup = clamp(time_since_last / 20.0, 0.0, 2.0);
        // Builds up over 20 seconds
        let charge_factor = 1.0 + charge_buildup * 0.5;
        // Up to 50% increase in likelihood

        // Create bursts and lulls: sometimes lightning comes in clusters
        let burst_seed = generate_segment_seed(time, lightning_bolt.flash_id, 2000u, electricalActivity);

        // Enhanced burst/lull logic with weather influence
        let weather_burst_bias = weather_cycle * storm_intensity * atmospheric_pressure * storm_cell_factor;
        let burst_threshold = 0.3 + (weather_burst_bias - 0.5) * 0.2;
        // Weather affects burst probability
        let normal_threshold = 0.7 + (weather_burst_bias - 0.5) * 0.1;
        // Weather affects normal/lull balance

        var final_variation: f32;

        if (burst_seed < burst_threshold) {
            // Burst mode - very short intervals, influenced by weather
            let burst_intensity = max(weather_burst_bias * charge_factor, 0.1);  // Prevent division by zero
            final_variation = (0.3 + combined_random * 1.2) / burst_intensity;
        }
        else if (burst_seed < normal_threshold) {
            // Normal mode - medium intervals with weather variation
            let normal_factor = max((weather_burst_bias * 0.5 + 0.5) * charge_factor, 0.1);  // Prevent division by zero
            final_variation = (1.5 + combined_random * 5.0) / normal_factor;
        }
        else {
            // Lull mode - longer intervals, less affected by charge buildup
            let lull_factor = max(weather_burst_bias * 0.7 + 0.3, 0.1);  // Prevent division by zero
            final_variation = (4.0 + combined_random * 12.0) / (lull_factor * max(sqrt(charge_factor), 0.1));
        }

        // Apply complex environmental scaling with safety checks
        let environmental_factor = weather_cycle * storm_intensity * atmospheric_pressure * storm_cell_factor;
        final_variation *= (1.0 - normalized_activity * 0.6);
        // Activity scaling
        final_variation *= (1.0 - environmental_factor * 0.4);
        // Environmental scaling
        final_variation = max(final_variation, 0.2);
        // Minimum 0.2s interval

        // Safety check: prevent extremely long intervals that could break the cycle
        final_variation = min(final_variation, 120.0);  // Maximum 2 minutes interval

        // Promote the pre-generated preview into the live range — bit-for-bit
        // the same bolt that's been sitting ready (and driving the pre-flash
        // darkening) for up to the last 3s. If none was ready in time (a very
        // short interval left no room for the lead), fall back to generating
        // on the spot, exactly like the original single-step code always did.
        let this_flash_id = lightning_bolt.flash_id + 1u;
        if (lightning_bolt.preview_ready == 1u) {
            for (var i = 0u; i < 40u; i = i + 1u) {
                lightning_segments[i] = lightning_segments[40u + i];
            }
            lightning_bolt.num_segments = lightning_bolt.preview_num_segments;
            lightning_bolt.is_super_lightning = lightning_bolt.preview_is_super;
            lightning_bolt.bolt_center_x = lightning_bolt.preview_center_x;
            lightning_bolt.bolt_center_y = lightning_bolt.preview_center_y;
            lightning_bolt.bolt_radius = lightning_bolt.preview_radius;
        } else {
            // 5% chance for super lightning - roll the dice!
            let fallback_super_roll = fract(sin(time * 87.9123 + electricalActivity * 52.845 + f32(this_flash_id) * 63.7429) * 37281.9876);
            let fallback_is_super = fallback_super_roll < 0.05;
            let fallback_result = generate_bolt_segments(0u, time, this_flash_id, electricalActivity, fallback_is_super);
            lightning_bolt.num_segments = u32(fallback_result.w);
            lightning_bolt.is_super_lightning = select(0u, 1u, fallback_is_super);
            lightning_bolt.bolt_center_x = fallback_result.x;
            lightning_bolt.bolt_center_y = fallback_result.y;
            lightning_bolt.bolt_radius = fallback_result.z;
        }

        lightning_bolt.flash_id = this_flash_id;
        lightning_bolt.start_time = time;
        lightning_bolt.needs_rules_reset = 0u;
        // Initialize to 0, will be set to 1 when last generation is reached

        // Safety check: ensure next_lightning_time doesn't overflow or become invalid
        let next_time = time + base_interval + final_variation;
        if (next_time > time && next_time < time + 300.0) {  // Sanity check: must be reasonable future time
            lightning_bolt.next_lightning_time = next_time;
        } else {
            // Fallback: simple 5-second interval if calculation went wrong
            lightning_bolt.next_lightning_time = time + 5.0;
        }

        // A new slot means a new preview needs to be generated for it.
        lightning_bolt.preview_ready = 0u;
    }
    else {
        // More sophisticated segment lifecycle management
        let lightning_age = time - lightning_bolt.start_time;

        // Don't update lifecycle in the same frame as creation to avoid race conditions
        if (lightning_age > 0.016) {
            // Skip first frame (16ms at 60fps)
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

                    // Beautiful gradual fade-out instead of abrupt disappearance
                    let fade_start = segment_duration * 0.6;
                    // Start fading at 60% of duration
                    let base_alpha = 1.0 - (f32(lightning_segments[i].generation) * 0.15);
                    // Generation-based base alpha

                    if (segment_age < fade_start) {
                        // Full brightness during first 60% of lifetime
                        lightning_segments[i].alpha = base_alpha;
                    }
                    else {
                        // Gradual fade during last 40% of lifetime
                        let fade_progress = (segment_age - fade_start) / (segment_duration - fade_start);
                        let fade_factor = 1.0 - smoothstep(0.0, 1.0, fade_progress);
                        lightning_segments[i].alpha = base_alpha * fade_factor;
                    }
                }
                else {
                    lightning_segments[i].is_visible = 0u;
                    lightning_segments[i].alpha = 0.0;
                }
            }

            // SUPER LIGHTNING RULES RESET: Trigger when lightning is most visually prominent
            if (lightning_bolt.is_super_lightning == 1u && lightning_bolt.needs_rules_reset == 0u) {
                // Count visible segments to determine visual prominence
                var visible_segments = 0u;
                var has_high_generation = false;
                var total_visible_alpha = 0.0;

                for (var j = 0u; j < lightning_bolt.num_segments; j = j + 1u) {
                    if (lightning_segments[j].is_visible == 1u) {
                        visible_segments = visible_segments + 1u;
                        total_visible_alpha += lightning_segments[j].alpha;
                    }
                    if (lightning_segments[j].generation >= 2u && lightning_segments[j].is_visible == 1u) {
                        has_high_generation = true;
                    }
                }

                // Calculate average visibility (0.0 to 1.0)
                var avg_visibility = 0.0;
                if (visible_segments > 0u) {
                    avg_visibility = total_visible_alpha / f32(visible_segments);
                }

                // Trigger rules reset when lightning reaches peak visual impact:
                // - At least 8 segments are visible (good branching development)
                // - High-generation branches are visible (complex structure)
                // - High average visibility (segments are bright and clear)
                // - OR fallback: many segments visible regardless of generation
                if ((visible_segments >= 8u && has_high_generation && avg_visibility >= 0.8) || (visible_segments >= 15u && avg_visibility >= 0.7)) {
                    lightning_bolt.needs_rules_reset = 1u;
                }
            }

            // Clear bolt when all segments are gone
            if (!any_segments_visible) {
                lightning_bolt.num_segments = 0u;
                lightning_bolt.is_super_lightning = 0u;
                // Reset super lightning flag
                lightning_bolt.needs_rules_reset = 0u;
                // Reset rules reset flag
            }
        }
        // End of lifecycle management guard
    }

    // SAFETY FALLBACK: If lightning cycle seems broken, reset it
    // This handles edge cases where next_lightning_time becomes invalid
    if (lightning_bolt.next_lightning_time <= 0.0 ||
        lightning_bolt.next_lightning_time < time ||
        lightning_bolt.next_lightning_time > time + 300.0) {
        // Reset with a reasonable delay
        lightning_bolt.next_lightning_time = time + 10.0;  // 10 second delay
        lightning_bolt.num_segments = 0u;
        lightning_bolt.is_super_lightning = 0u;
        lightning_bolt.needs_rules_reset = 0u;
        lightning_bolt.preview_ready = 0u; // new slot — needs its own preview
    }
}

// CORRECTED Lightning Electromagnetic Force Function
// This implements the EXACT same branching algorithm as the fragment shader

// Struct to represent a branch in the lightning system
struct LightningBranch {
    pos: vec2<f32>,
    dir: vec2<f32>,
    generation: u32,
    appear_time: f32,
}

;

// Calculate electromagnetic force from lightning on a particle - CORRECTED VERSION
fn calculateLightningElectromagneticForce(particle_pos: vec2<f32>, time: f32) -> vec2<f32> {
    // Check if lightning is enabled
    if (sim_params.lightning_frequency <= 0.0) {
        return vec2<f32>(0.0, 0.0);
    }

    // Calculate lightning timing
    let flash_interval = 1.0 / sim_params.lightning_frequency;
    let time_in_cycle = time % flash_interval;
    let flash_duration = sim_params.lightning_duration;

    // Only apply force during lightning flash
    if (time_in_cycle > flash_duration) {
        return vec2<f32>(0.0, 0.0);
    }

    // Generate the same lightning bolt as fragment shader
    let flash_id = floor(time / flash_interval);
    let base_seed = hash(flash_id * 73.421);

    // Lightning starting position and direction (matches fragment shader exactly)
    let pos_rand = hash(base_seed * 555.0);
    let pos_rand2 = hash(base_seed * 777.0);
    let dir_rand = hash(base_seed * 333.0);

    // Convert lightning UV coordinates to virtual world coordinates
    let uv_start_pos = vec2<f32>(0.1 + pos_rand * 0.8, 0.1 + pos_rand2 * 0.8);
    let lightning_start_pos = vec2<f32>(uv_start_pos.x * sim_params.virtual_world_width, uv_start_pos.y * sim_params.virtual_world_height);

    let angle = dir_rand * 6.28318;
    let initial_direction = vec2<f32>(cos(angle), sin(angle));

    // Lightning segment parameters (matches fragment shader exactly)
    let segment_interval = 0.07;
    let segment_duration = 0.4;
    let min_segment_length_px = 60.0;
    let max_segment_length_px = 150.0;
    let max_total_segments = 30;

    // Convert UV coordinates properly
    let resolution = vec2<f32>(sim_params.canvas_render_width, sim_params.canvas_render_height);
    let min_segment_length_uv = min_segment_length_px / min(resolution.x, resolution.y);
    let max_segment_length_uv = max_segment_length_px / min(resolution.x, resolution.y);

    // Convert UV segment lengths to virtual world coordinates
    let pixel_to_world_scale = sim_params.virtual_world_width / sim_params.canvas_render_width;
    let min_segment_length = min_segment_length_uv * sim_params.virtual_world_width;
    let max_segment_length = max_segment_length_uv * sim_params.virtual_world_width;

    var total_em_force = vec2<f32>(0.0, 0.0);

    // Implement true branching algorithm exactly like fragment shader
    // Arrays to store active branch endpoints (simulate dynamic arrays with fixed size)
    var branch_positions: array<vec2<f32>, 20>;
    var branch_directions: array<vec2<f32>, 20>;
    var branch_generations: array<u32, 20>;
    var branch_appear_times: array<f32, 20>;

    var active_branches = 0;

    // Initialize with root segment
    branch_positions[0] = lightning_start_pos;
    branch_directions[0] = initial_direction;
    branch_generations[0] = 0u;
    branch_appear_times[0] = 0.0;
    active_branches = 1;

    // Generate segments progressively exactly like fragment shader
    for (var step = 0; step < max_total_segments && active_branches > 0; step++) {
        let step_appear_time = f32(step) * segment_interval;

        // Skip if this step shouldn't appear yet
        if (time_in_cycle < step_appear_time) {
            break;
        }

        var new_branches = 0;
        var new_branch_positions: array<vec2<f32>, 20>;
        var new_branch_directions: array<vec2<f32>, 20>;
        var new_branch_generations: array<u32, 20>;
        var new_branch_appear_times: array<f32, 20>;

        // Process each active branch
        for (var i = 0; i < active_branches; i++) {
            let current_pos = branch_positions[i];
            let current_dir = branch_directions[i];
            let generation = branch_generations[i];
            let branch_appear_time = branch_appear_times[i];

            // Only process branches that should be visible
            if (time_in_cycle >= branch_appear_time) {
                // Calculate segment properties using exact same logic as fragment shader
                let seg_seed = hash(base_seed + f32(step) * 73.0 + f32(i) * 37.0);

                // Decide branching first (90% chance to branch)
                let branch_seed = hash(seg_seed * 99.999);
                let should_branch = branch_seed > 0.1 && generation < 4u && new_branches < 16;

                // Angle change logic matching fragment shader exactly
                var angle_change: f32;
                if (should_branch) {
                    // For branching segments, use moderate angle change
                    angle_change = (hash(seg_seed * 11.111) - 0.5) * 0.4;
                }
                else {
                    // For non-branching segments, maintain straighter paths
                    angle_change = (hash(seg_seed * 11.111) - 0.5) * 0.1;
                }

                let new_angle = atan2(current_dir.y, current_dir.x) + angle_change;
                var new_dir = vec2<f32>(cos(new_angle), sin(new_angle));

                // Calculate segment length with exact same logic as fragment shader
                let length_seed = hash(seg_seed * 222.222);
                let base_length_uv = min_segment_length_uv + length_seed * (max_segment_length_uv - min_segment_length_uv);
                let generation_decay = pow(0.8, f32(generation));
                let current_segment_length_uv = max(min_segment_length_uv, base_length_uv * generation_decay);

                // Convert to world coordinates
                let current_segment_length = current_segment_length_uv * sim_params.virtual_world_width;
                let segment_end = current_pos + new_dir * current_segment_length;

                // Calculate thickness and alpha exactly like fragment shader
                let raw_thickness = 3.0 * pow(0.7, f32(generation));
                let thickness = max(1.0, min(3.0, raw_thickness));
                let base_alpha = 1.0 * pow(0.9, f32(generation));

                // Check if segment is still within its visibility duration (exact logic from fragment shader)
                let segment_age = time_in_cycle - step_appear_time;
                let is_visible = segment_age < segment_duration;

                // Only apply electromagnetic force if segment is visible (matches fragment shader exactly)
                if (is_visible) {
                    let em_force = calculateSegmentElectromagneticForce(particle_pos, current_pos, segment_end, generation);
                    total_em_force = total_em_force + em_force;
                }

                // Branching logic exactly matching fragment shader
                if (should_branch) {
                    // Add branch(es) from the current segment endpoint
                    // Decide number of branches (1 or 2) - 40% chance for 2 branches
                    let num_new_branches = select(1, 2, hash(seg_seed * 77.777) > 0.6);

                    if (num_new_branches == 1) {
                        // Single branch - angle between 25-60 degrees from parent
                        if (new_branches < 19) {
                            let min_angle = 25.0 * 3.14159 / 180.0;
                            let max_angle = 60.0 * 3.14159 / 180.0;
                            let angle_range = max_angle - min_angle;
                            let branch_angle = min_angle + hash(seg_seed * 44.444) * angle_range;

                            // Choose left or right side randomly
                            let side = select(- 1.0, 1.0, hash(seg_seed * 66.666) > 0.5);
                            let final_branch_angle = new_angle + side * branch_angle;

                            new_branch_positions[new_branches] = segment_end;
                            new_branch_directions[new_branches] = vec2<f32>(cos(final_branch_angle), sin(final_branch_angle));
                            new_branch_generations[new_branches] = generation + 1u;
                            new_branch_appear_times[new_branches] = step_appear_time + segment_interval;
                            new_branches = new_branches + 1;
                        }
                    }
                    else {
                        // Two branches - both at angles between 25-60 degrees from parent
                        if (new_branches < 18) {
                            let min_angle = 25.0 * 3.14159 / 180.0;
                            let max_angle = 60.0 * 3.14159 / 180.0;
                            let angle_range = max_angle - min_angle;

                            // Generate two different angles within the valid range
                            let branch_angle1 = min_angle + hash(seg_seed * 55.555) * angle_range;
                            let branch_angle2 = min_angle + hash(seg_seed * 88.888) * angle_range;

                            let split_angle1 = new_angle + branch_angle1;
                            // Right side
                            let split_angle2 = new_angle - branch_angle2;
                            // Left side

                            // First branch
                            new_branch_positions[new_branches] = segment_end;
                            new_branch_directions[new_branches] = vec2<f32>(cos(split_angle1), sin(split_angle1));
                            new_branch_generations[new_branches] = generation + 1u;
                            new_branch_appear_times[new_branches] = step_appear_time + segment_interval;
                            new_branches = new_branches + 1;

                            // Second branch
                            new_branch_positions[new_branches] = segment_end;
                            new_branch_directions[new_branches] = vec2<f32>(cos(split_angle2), sin(split_angle2));
                            new_branch_generations[new_branches] = generation + 1u;
                            new_branch_appear_times[new_branches] = step_appear_time + segment_interval;
                            new_branches = new_branches + 1;
                        }
                    }
                }
                else {
                    // No branching - continue this branch with minimal direction change
                    if (new_branches < 20) {
                        new_branch_positions[new_branches] = segment_end;
                        new_branch_directions[new_branches] = new_dir;
                        new_branch_generations[new_branches] = generation;
                        new_branch_appear_times[new_branches] = step_appear_time + segment_interval;
                        new_branches = new_branches + 1;
                    }
                }
            }
        }

        // Update active branches for next iteration
        active_branches = new_branches;
        for (var j = 0; j < active_branches; j++) {
            branch_positions[j] = new_branch_positions[j];
            branch_directions[j] = new_branch_directions[j];
            branch_generations[j] = new_branch_generations[j];
            branch_appear_times[j] = new_branch_appear_times[j];
        }
    }

    return total_em_force;
}

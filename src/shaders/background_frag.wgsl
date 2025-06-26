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
}

@group(0) @binding(0)
var<uniform> sim_params: SimParams;

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    // Use background color directly - GPU handles sRGB conversion automatically
    let background_color = vec3<f32>(sim_params.background_color_r, sim_params.background_color_g, sim_params.background_color_b);
    var final_color = vec4<f32>(background_color, 1.0);
    // Base color

    // Temporarily disable clouds by setting num_gradients to 0
    let num_gradients = 0;
    // Changed from 24 to 0 to disable clouds
    let PI = 3.14159265359;

    for (var i = 0; i < num_gradients; i = i + 1) {
        let gradient_id = f32(i);
        let time_offset = gradient_id * 20.0;
        // Stagger animation

        // Unique random seeds/factors for this cloud, derived from gradient_id
        // These ensure different random-like behavior for each parameter of each cloud
        let seed1 = fract(gradient_id * 0.61803398875);
        // Golden ratio conjugate
        let seed2 = fract(gradient_id * 0.85355339059);
        // sqrt(2)/2 approx
        let seed3 = fract(gradient_id * 0.70710678118);
        // 1/sqrt(2)
        let seed4 = fract(gradient_id * 0.57735026919);
        // 1/sqrt(3)
        let seed5 = fract(gradient_id * 0.4472135955);
        // 1/sqrt(5)
        let seed6 = fract(gradient_id * 0.37796447301);
        // Another arbitrary fraction
        // seed7 can be introduced if more unique values are needed for future params.

        // --- Aspect Ratio Oscillation ---
        let base_aspect_ratio = 6.0;
        // Individual frequency: range ~[0.08, 0.32]
        let aspect_random_freq_factor = seed1 * 0.24 + 0.08;
        // Individual amplitude: range ~[0.4, 0.8]
        let aspect_random_amplitude_factor = seed2 * 0.4 + 0.4;
        let aspect_oscillation = (sin(sim_params.time * aspect_random_freq_factor + gradient_id * PI * 0.8) + 1.0) * 0.5;
        // Sin wave [0,1]
        // Modulate aspect ratio: e.g. base * ( (1-amplitude_factor) + oscillation * amplitude_factor * 2 )
        // If amplitude_factor = 0.6, this means aspect ratio varies between base * 0.4 and base * 1.6
        let current_aspect_ratio = base_aspect_ratio * ((1.0 - aspect_random_amplitude_factor) + aspect_oscillation * aspect_random_amplitude_factor * 2.0);

        // --- Radius Oscillation ---
        let radius_base = sim_params.canvas_render_height * 0.20;
        // Individual frequency for radius: range ~[0.002, 0.007]
        let radius_random_freq_factor = seed3 * 0.005 + 0.002;
        // Individual amplitude for radius variation: range ~[0.1, 0.3]
        let radius_random_amplitude_factor = seed4 * 0.2 + 0.1;
        // Radius oscillation: sin wave [-1, 1]
        let radius_oscillation_wave = sin(gradient_id * 1.2345 + sim_params.time * radius_random_freq_factor);
        // Modulate radius: radius_base * (1.0 + oscillation_wave * amplitude_factor)
        // e.g., if amplitude_factor = 0.2, radius varies between 80% and 120% of radius_base
        let radius = radius_base * (1.0 + radius_oscillation_wave * radius_random_amplitude_factor);

        // --- Angle Oscillation ---
        let angle_max_radians = 8.0 * PI / 180.0;
        // +/- 8 degrees
        // Individual frequency for angle: range ~[0.05, 0.25], using seed6
        let angle_random_freq_factor = seed6 * 0.20 + 0.05;
        let angle_phase_offset = gradient_id * PI * 1.37;
        // Unique phase for angle (using a different PI multiplier)
        let angle_oscillation_wave = sin(sim_params.time * angle_random_freq_factor + angle_phase_offset);
        // Sin wave [-1,1]
        let current_angle_rad = angle_oscillation_wave * angle_max_radians;

        let cos_a = cos(current_angle_rad);
        let sin_a = sin(current_angle_rad);

        // Horizontal movement and wrapping logic
        let cloud_half_width_pixels = radius * current_aspect_ratio;

        // Base X movement (continuous, not pre-fract'ed by fract() on the whole expression)
        // base_x_continuous_norm represents a normalized continuous position
        let mean_position_norm = fract(gradient_id * 0.61803398875);
        // Ensure base positions are within [0,1)
        let time_input_for_oscillation = sim_params.time * 0.0000000005 + time_offset * 0.00001;
        // Reintroduced
        let horizontal_oscillation = sin(time_input_for_oscillation) * 0.05;
        // Reintroduced, amplitude of 0.05 (5% of canvas width)
        // let horizontal_oscillation = 0.0; // Was temporarily disabled
        let base_x_continuous_norm = mean_position_norm + horizontal_oscillation;

        // Corrected drift effect calculation for 100% parallax
        // Use the accumulated virtual world offset, normalized by virtual world width.
        // This directly reflects the "camera" movement.
        let drift_effect_normalized = sim_params.virtual_world_offset_x / sim_params.virtual_world_width;

        let unwrapped_center_x_norm = base_x_continuous_norm - drift_effect_normalized;

        // Effective total width for wrapping = 1.0 (normalized canvas width) + normalized cloud width (half on each side)
        let cloud_half_width_norm = cloud_half_width_pixels / sim_params.canvas_render_width;
        let effective_wrapping_width_norm = 1.0 + 2.0 * cloud_half_width_norm;

        // Apply custom modulo to wrap around the effective_wrapping_width_norm.
        // Shift the coordinate system so that the wrapping occurs when the cloud is fully off-screen.
        let val_to_wrap = unwrapped_center_x_norm + cloud_half_width_norm;
        let period_of_wrap = effective_wrapping_width_norm;
        // WGSL compatible modulo for f32: result = dividend - divisor * floor(dividend / divisor)
        // This ensures the result is always in [0, period_of_wrap) if period_of_wrap is positive.
        var wrapped_val = val_to_wrap - period_of_wrap * floor(val_to_wrap / period_of_wrap);
        // Adjust if val_to_wrap is negative and result of above is 0 but should be period_of_wrap or if it's exactly on the boundary.
        // A simpler way for positive period:
        // wrapped_val = val_to_wrap % period_of_wrap;
        // if (wrapped_val < 0.0) { wrapped_val = wrapped_val + period_of_wrap; }
        // However, to be robust and match the intended logic of `value - period * floor(value / period)` for wrapping:
        // This formula correctly handles the desired floating point modulo behavior.
        wrapped_val = val_to_wrap - period_of_wrap * floor(val_to_wrap / period_of_wrap);

        let wrapped_center_x_norm = wrapped_val - cloud_half_width_norm;
        let final_center_x = wrapped_center_x_norm * sim_params.canvas_render_width;

        // Vertical movement with individual sinusoidal oscillation
        let y_random_freq_factor = fract(gradient_id * 0.7315 + 0.4823) * 0.008 + 0.007;
        // Individual frequency, range approx [0.007, 0.015]
        let y_phase_offset = 2.0 * PI * fract(gradient_id * 0.61803398875);
        // Better distributed phase shift
        let y_oscillation = sin(sim_params.time * y_random_freq_factor + y_phase_offset);
        // Sin wave [-1,1]

        // Map to normalized screen coordinates: center around 0.5, amplitude 0.75 to get range [-0.25, 1.25]
        let center_y_norm = 0.5 + y_oscillation * 0.75;
        let final_center_y = center_y_norm * sim_params.canvas_render_height;
        // Y can now be outside [0, canvasHeight]

        let gradient_center = vec2<f32>(final_center_x, final_center_y);

        let dx_orig = frag_coord.x - gradient_center.x;
        let dy_orig = frag_coord.y - gradient_center.y;

        // Apply rotation to the coordinate system of the cloud
        let dx_rotated = dx_orig * cos_a + dy_orig * sin_a;
        let dy_rotated = - dx_orig * sin_a + dy_orig * cos_a;

        // Apply aspect ratio scaling to the rotated coordinates
        // The length is calculated based on dx' scaled by aspect ratio, and dy' (scaled by 1.0)
        let effective_dist = length(vec2<f32>(dx_rotated / current_aspect_ratio, dy_rotated));

        let gradient_val = smoothstep(radius, 0.0, effective_dist);

        // --- Opacity Oscillation (target range [0.1, 0.4]) ---
        // Individual frequency for opacity: range ~[0.1, 0.4] (seed5 is used here)
        let opacity_random_freq_factor = seed5 * 0.3 + 0.1;
        // Phase for opacity oscillation
        let opacity_phase_offset = gradient_id * PI * 0.5;
        // Kept from previous version for consistency
        let opacity_oscillation = (sin(sim_params.time * opacity_random_freq_factor + opacity_phase_offset) + 1.0) * 0.5;
        // Sin wave [0,1]

        // current_max_opacity will now range from 0.03 (when opacity_oscillation is 0)
        // to 0.25 (when opacity_oscillation is 1)
        let current_max_opacity = 0.03 + opacity_oscillation * 0.18;
        let opacity = gradient_val * current_max_opacity;

        let gradient_color = vec4<f32>(1.0, 1.0, 1.0, opacity);

        // Blend this gradient with the final color
        final_color = vec4<f32>(mix(final_color.rgb, gradient_color.rgb, gradient_color.a), final_color.a);
    }

    // Convert back to sRGB for framebuffer output
    return final_color;
}

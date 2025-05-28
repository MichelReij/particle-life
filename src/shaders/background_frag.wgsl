struct SimulationParams {
    deltaTime: f32,
    friction: f32,
    numParticles: u32,
    numTypes: u32,
    virtualWorldWidth: f32,
    virtualWorldHeight: f32,
    canvasRenderWidth: f32,
    canvasRenderHeight: f32,
    virtualWorldOffsetX: f32,
    virtualWorldOffsetY: f32,
    boundaryMode: u32,
    particleRenderSize: f32,
    forceScale: f32,
    rSmooth: f32,
    flatForce: u32,
    driftXPerSecond: f32,
    interTypeAttractionScale: f32,
    interTypeRadiusScale: f32,
    time: f32, // Time in seconds for animation
    _padding0: f32,
};

@group(0) @binding(0) var<uniform> sim_params: SimulationParams;

// Function to convert HSL to RGB
// H: 0-1 (hue, maps to 0-360 degrees)
// S: 0-1 (saturation)
// L: 0-1 (lightness)
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> vec3<f32> {
    if (s == 0.0) {
        return vec3<f32>(l, l, l); // achromatic
    }
    let q = select(l * (1.0 + s), l + s - l * s, l < 0.5);
    let p = 2.0 * l - q;
    let r = hue_to_rgb_component(p, q, h + 1.0/3.0);
    let g = hue_to_rgb_component(p, q, h);
    let b = hue_to_rgb_component(p, q, h - 1.0/3.0);
    return vec3<f32>(r, g, b);
}

fn hue_to_rgb_component(p: f32, q: f32, t_in: f32) -> f32 {
    var t = t_in;
    if (t < 0.0) { t += 1.0; }
    if (t > 1.0) { t -= 1.0; }
    if (t < 1.0/6.0) { return p + (q - p) * 6.0 * t; }
    if (t < 1.0/2.0) { return q; }
    if (t < 2.0/3.0) { return p + (q - p) * (2.0/3.0 - t) * 6.0; }
    return p;
}

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let canvas_dims = vec2<f32>(sim_params.canvasRenderWidth, sim_params.canvasRenderHeight);
    var uv = frag_coord.xy / canvas_dims; // Normalize fragment coordinates

    // Apply parallax scrolling based on drift speed (50% of particle drift)
    // OLD: let parallax_offset_x = sim_params.driftXPerSecond * sim_params.time * 0.0025;
    // OLD: uv.x += parallax_offset_x;

    // NEW Parallax Calculation:
    // Use virtualWorldOffsetX (accumulated drift in pixels) for 100% parallax.
    let parallax_uv_offset_x = (sim_params.virtualWorldOffsetX * 1.0) / sim_params.canvasRenderWidth; // Changed 0.8 to 1.0

    // Corrected direction: If particles drift right (virtualWorldOffsetX increases),
    // background also drifts right. This means sampling an earlier (smaller) uv.x.
    uv.x -= parallax_uv_offset_x;

    // Wrap the main UV coordinates after parallax to ensure the view itself wraps.
    uv = fract(uv);

    // Define base positions and animation parameters for 16 radial gradients
    // Base positions in UV space [0,1]
    let base_centers = array<vec2<f32>, 16>(
        // Original 4
        vec2<f32>(0.25, 0.25), vec2<f32>(0.75, 0.25),
        vec2<f32>(0.25, 0.75), vec2<f32>(0.75, 0.75),
        // Next 4
        vec2<f32>(0.1, 0.4), vec2<f32>(0.9, 0.6),
        vec2<f32>(0.4, 0.1), vec2<f32>(0.6, 0.9),
        // New 8
        vec2<f32>(0.05, 0.15), vec2<f32>(0.95, 0.85),
        vec2<f32>(0.15, 0.05), vec2<f32>(0.85, 0.95),
        vec2<f32>(0.3, 0.6), vec2<f32>(0.7, 0.4),
        vec2<f32>(0.6, 0.3), vec2<f32>(0.4, 0.7)
    );

    // Animation properties (amplitude, speed, phase) - can be tuned
    let amplitudes = array<vec2<f32>, 16>(
        // Original 4
        vec2<f32>(0.1, 0.15), vec2<f32>(0.12, 0.08),
        vec2<f32>(0.08, 0.12), vec2<f32>(0.15, 0.1),
        // Next 4
        vec2<f32>(0.13, 0.1), vec2<f32>(0.1, 0.13),
        vec2<f32>(0.09, 0.11), vec2<f32>(0.11, 0.09),
        // New 8
        vec2<f32>(0.07, 0.12), vec2<f32>(0.14, 0.07),
        vec2<f32>(0.11, 0.06), vec2<f32>(0.06, 0.14),
        vec2<f32>(0.1, 0.1), vec2<f32>(0.09, 0.09),
        vec2<f32>(0.12, 0.13), vec2<f32>(0.13, 0.12)
    );
    let speeds = array<vec2<f32>, 16>(
        // Original 4
        vec2<f32>(0.2, 0.15), vec2<f32>(0.12, 0.22),
        vec2<f32>(0.18, 0.13), vec2<f32>(0.1, 0.25),
        // Next 4
        vec2<f32>(0.17, 0.14), vec2<f32>(0.11, 0.20),
        vec2<f32>(0.21, 0.10), vec2<f32>(0.13, 0.23),
        // New 8
        vec2<f32>(0.19, 0.16), vec2<f32>(0.10, 0.21),
        vec2<f32>(0.22, 0.11), vec2<f32>(0.14, 0.24),
        vec2<f32>(0.15, 0.18), vec2<f32>(0.16, 0.19),
        vec2<f32>(0.23, 0.12), vec2<f32>(0.12, 0.20)
    );
    let phases = array<vec2<f32>, 16>(
        // Original 4
        vec2<f32>(0.0, 1.57), vec2<f32>(0.5, 2.0),
        vec2<f32>(1.0, 0.2), vec2<f32>(1.7, 0.8),
        // Next 4
        vec2<f32>(0.2, 1.0), vec2<f32>(0.7, 2.5),
        vec2<f32>(1.2, 0.5), vec2<f32>(1.9, 1.1),
        // New 8
        vec2<f32>(0.1, 0.7), vec2<f32>(0.6, 2.2),
        vec2<f32>(1.1, 0.3), vec2<f32>(1.8, 1.3),
        vec2<f32>(0.3, 1.2), vec2<f32>(0.8, 2.7),
        vec2<f32>(1.3, 0.6), vec2<f32>(2.0, 1.0)
    );

    let gradient_radius = 0.55;
    var max_single_gradient_intensity = 0.0;

    for (var i = 0u; i < 16u; i = i + 1u) { // Loop for 16 gradients
        var animated_base_center = base_centers[i];
        animated_base_center.x += sin(sim_params.time * speeds[i].x + phases[i].x) * amplitudes[i].x;
        animated_base_center.y += cos(sim_params.time * speeds[i].y + phases[i].y) * amplitudes[i].y;

        let wrapped_center = fract(animated_base_center);

        var min_dist_sq = 1000000.0;
        for (var dx = -1.0; dx <= 1.0; dx = dx + 1.0) {
            for (var dy = -1.0; dy <= 1.0; dy = dy + 1.0) {
                let offset_center = wrapped_center + vec2<f32>(dx, dy);
                var diff = uv - offset_center;
                diff.x *= 0.5;
                diff.y *= 1.5;
                min_dist_sq = min(min_dist_sq, dot(diff, diff));
            }
        }
        let dist_to_center = sqrt(min_dist_sq);

        let current_gradient_intensity = 1.0 - smoothstep(gradient_radius * 0.05, gradient_radius, dist_to_center);
        max_single_gradient_intensity = max(max_single_gradient_intensity, current_gradient_intensity);
    }

    // Determine hue based on global drift speed
    let max_drift_for_hue = 100.0;
    let normalized_drift = clamp(abs(sim_params.driftXPerSecond) / max_drift_for_hue, 0.0, 1.0);
    let background_hue = 0.66 - (normalized_drift * 0.66); // Blue (0.66) to Red (0.0)

    let background_saturation = 0.5; // Saturation for the dark background
    let background_lightness = 0.03; // Very dark background
    let background_rgb = hsl_to_rgb(background_hue, background_saturation, background_lightness);

    // Gradients are white, their "intensity" translates to alpha
    let gradient_alpha = max_single_gradient_intensity * 0.3; // Opacity from 0.0 to 0.3
    let gradient_color_rgb = vec3<f32>(1.0, 1.0, 1.0); // White gradients

    // Blend background with white gradients
    let final_color_rgb = mix(background_rgb, gradient_color_rgb, gradient_alpha);

    return vec4<f32>(final_color_rgb, 1.0);
}

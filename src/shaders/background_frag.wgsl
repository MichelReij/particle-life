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

// Simple noise function (pseudo-random)
fn noise(uv: vec2<f32>) -> f32 {
    return fract(sin(dot(uv, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let canvas_dims = vec2<f32>(sim_params.canvasRenderWidth, sim_params.canvasRenderHeight);
    var uv = frag_coord.xy / canvas_dims;

    // Parallax scrolling for background based on drift speed
    // Adjust the 0.5 factor to change parallax intensity
    let parallax_offset_x = sim_params.driftXPerSecond * sim_params.time * 0.005; // Scaled for visual effect
    uv.x += parallax_offset_x * 0.5; // Apply 50% of drift to background

    // Cloud-like pattern using noise
    var color_intensity = 0.0;
    var scale = 4.0;
    var amplitude = 0.5;
    for (var i = 0; i < 4; i++) { // 4 octaves of noise
        color_intensity += noise(uv * scale + sim_params.time * 0.05 * f32(i+1)) * amplitude;
        scale *= 2.0;
        amplitude *= 0.5;
    }
    color_intensity = smoothstep(0.3, 0.7, color_intensity); // Adjust contrast

    // Hue calculation based on absolute drift speed
    // Max drift speed for hue mapping (e.g., 100 px/s)
    let max_drift_for_hue = 100.0;
    let normalized_drift = clamp(abs(sim_params.driftXPerSecond) / max_drift_for_hue, 0.0, 1.0);

    // Hue spectrum: Blue (0.66) -> Green (0.33) -> Yellow (0.16) -> Orange (0.08) -> Red (0.0)
    // We'll map normalized_drift (0 to 1) to hue (0.66 to 0.0)
    // Blue at 0 drift, Red at max_drift_for_hue
    let hue = 0.66 - (normalized_drift * 0.66); // Interpolates from blue (0.66) to red (0.0)

    let saturation = 0.7; // Keep saturation somewhat constant
    let lightness = 0.5 + color_intensity * 0.2; // Vary lightness with noise pattern

    let final_color_rgb = hsl_to_rgb(hue, saturation, lightness);

    return vec4<f32>(final_color_rgb, 1.0);
}

// Utility functions for buffer management and data conversion

pub fn validate_particle_count_change(
    current_count: u32,
    new_count: u32,
    max_particles: u32,
) -> u32 {
    // Safety validation: prevent sudden large changes (max 50% per operation)
    let max_change = (current_count as f32 * 0.5).max(100.0) as u32;

    if new_count > current_count {
        // Increasing particles
        let increase = new_count - current_count;
        if increase > max_change {
            current_count + max_change
        } else {
            new_count
        }
    } else if new_count < current_count {
        // Decreasing particles
        let decrease = current_count - new_count;
        if decrease > max_change {
            current_count - max_change
        } else {
            new_count
        }
    } else {
        new_count
    }
    .min(max_particles)
}

pub fn pressure_to_particle_count(pressure: f32, min_particles: u32, max_particles: u32) -> u32 {
    let clamped_pressure = pressure.max(0.0).min(350.0);
    let normalized = clamped_pressure / 350.0;
    let range = (max_particles - min_particles) as f32;
    let target = min_particles as f32 + normalized * range;

    // Round to nearest multiple of 64 for optimal GPU workgroup dispatch
    ((target / 64.0).round() * 64.0) as u32
}

/// Standard HSL → linear RGB (no gamma).
/// h: [0, 360],  s: [0, 100],  l: [0, 100]  →  (r, g, b): [0, 1]
pub fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    // Normalise hue to [0, 360) — handles negative values and values > 360
    let h = ((h % 360.0) + 360.0) % 360.0;
    let s = s / 100.0;
    let l = l / 100.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - ((h_prime % 2.0) - 1.0).abs());
    let (r1, g1, b1) = match h_prime as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    (r1 + m, g1 + m, b1 + m)
}

// Calculate background color based on drift speed
pub fn calculate_background_color_from_drift(drift_x_per_second: f32) -> [f32; 3] {
    let normalized_abs_drift = (drift_x_per_second.abs() / 80.0).min(1.0);

    // Hue transitions from blue (200°) at no drift to warm red-magenta (-10°/350°) at max drift
    let hue = 200.0 - normalized_abs_drift * 210.0;
    let saturation = 30.0;
    let lightness = 66.0;

    let (r, g, b) = hsl_to_rgb(hue, saturation, lightness);
    [r, g, b]
}

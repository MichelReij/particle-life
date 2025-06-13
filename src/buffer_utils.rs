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

// Convert HSLuv color to RGB
pub fn hsluv_to_rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    // Simplified HSLuv to RGB conversion
    // This is a basic implementation - for full HSLuv support, you'd want to use a proper library

    let h_norm = (h % 360.0) / 360.0;
    let s_norm = (s / 100.0).clamp(0.0, 1.0);
    let l_norm = (l / 100.0).clamp(0.0, 1.0);

    // Convert to HSL first, then to RGB
    let c = (1.0 - (2.0 * l_norm - 1.0).abs()) * s_norm;
    let x = c * (1.0 - ((h_norm * 6.0) % 2.0 - 1.0).abs());
    let m = l_norm - c / 2.0;

    let (r, g, b) = if h_norm < 1.0 / 6.0 {
        (c, x, 0.0)
    } else if h_norm < 2.0 / 6.0 {
        (x, c, 0.0)
    } else if h_norm < 3.0 / 6.0 {
        (0.0, c, x)
    } else if h_norm < 4.0 / 6.0 {
        (0.0, x, c)
    } else if h_norm < 5.0 / 6.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    [(r + m), (g + m), (b + m)]
}

// Calculate background color based on drift speed
pub fn calculate_background_color_from_drift(drift_x_per_second: f32) -> [f32; 3] {
    let normalized_abs_drift = (drift_x_per_second.abs() / 80.0).min(1.0);

    // Hue transitions from blue (215°) at no drift to red (15°) at max drift
    let hue = 215.0 - normalized_abs_drift * 200.0;
    let saturation = 33.0;
    let lightness = 66.0;

    hsluv_to_rgb(hue, saturation, lightness)
}

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

// Calculate background color based on drift speed
pub fn calculate_background_color_from_drift(drift_x_per_second: f32) -> [f32; 3] {
    let normalized_abs_drift = (drift_x_per_second.abs() / 80.0).min(1.0);

    // Hue transitions from blue (215°) at no drift to red (15°) at max drift
    let hue = 215.0 - normalized_abs_drift * 200.0;
    let saturation = 33.0;
    let lightness = 66.0;

    // Use the proper hsluv crate for conversion
    let (r, g, b) = hsluv::hsluv_to_rgb(hue as f64, saturation as f64, lightness as f64);
    [r as f32, g as f32, b as f32]
}

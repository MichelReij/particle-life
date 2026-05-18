// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

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


/// OKLCH → sRGB (geclampt naar [0, 1]).
/// l: [0, 1]  c: [0, ∞)  h_deg: [0, 360)  →  (r, g, b): [0, 1]
pub fn oklch_to_srgb(l: f32, c: f32, h_deg: f32) -> (f32, f32, f32) {
    let h = h_deg * std::f32::consts::PI / 180.0;
    let a = c * h.cos();
    let b = c * h.sin();

    // OkLab → LMS (cube roots)
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;

    let lms_l = l_ * l_ * l_;
    let lms_m = m_ * m_ * m_;
    let lms_s = s_ * s_ * s_;

    // LMS → linear sRGB
    let r_lin =  4.0767416621 * lms_l - 3.3077115913 * lms_m + 0.2309699292 * lms_s;
    let g_lin = -1.2684380046 * lms_l + 2.6097574011 * lms_m - 0.3413193965 * lms_s;
    let b_lin = -0.0041960863 * lms_l - 0.7034186147 * lms_m + 1.7076147010 * lms_s;

    // Linear sRGB → gamma-encoded sRGB
    let gamma = |v: f32| -> f32 {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
    };
    (gamma(r_lin), gamma(g_lin), gamma(b_lin))
}

// Calculate background color based on drift speed (OkLCH)
pub fn calculate_background_color_from_drift(drift_x_per_second: f32) -> [f32; 3] {
    let normalized_abs_drift = (drift_x_per_second.abs() / 80.0).min(1.0);

    // Hue transitions from blue (251°) at no drift to red (24.0°) at max drift
    let hue = 251.0 + normalized_abs_drift * (24.0 - 251.0);
    let (r, g, b) = oklch_to_srgb(0.64, 0.13, hue);
    [r, g, b]
}

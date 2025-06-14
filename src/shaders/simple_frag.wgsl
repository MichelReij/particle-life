// Simple fragment shader for particle rendering with circles
@fragment
fn main(@location(0) color: vec4<f32>, @location(1) uv: vec2<f32>) -> @location(0) vec4<f32> {
    // Create circular particles
    let center = vec2<f32>(0.5, 0.5);
    let distance_from_center = length(uv - center);

    // Discard pixels outside the circle
    if distance_from_center > 0.5 {
        discard;
    }

    // Apply smooth edge for anti-aliasing
    let edge_smoothness = 0.02;
    let alpha_factor = 1.0 - smoothstep(0.5 - edge_smoothness, 0.5, distance_from_center);

    var final_color = color;
    final_color.a *= alpha_factor;

    return final_color;
}

struct VertexOutput {
    @location(0) particle_color: vec4<f32>,
    @location(1) quad_uv: vec2<f32>,
    // UV coordinates for the quad
}

;

@fragment
fn main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate distance from center of quad (0.5, 0.5) to current fragment
    let center = vec2<f32>(0.5, 0.5);
    let dist = distance(in.quad_uv, center);

    // Create a perfect circle with smooth edges
    let radius = 0.5;
    // Full radius of the quad
    let edge_softness = 0.05;
    // Small value for anti-aliasing

    // Use smoothstep for anti-aliased edge
    let alpha_factor = 1.0 - smoothstep(radius - edge_softness, radius, dist);

    // Apply the circular mask to the particle color
    var final_color = in.particle_color;
    final_color.a *= alpha_factor;

    // Discard fragments that are completely transparent
    if (final_color.a < 0.01) {
        discard;
    }

    return final_color;
}

struct VertexOutput {
    @location(0) particle_color: vec4<f32>,
    @location(1) quad_uv: vec2<f32>,
    @location(2) particle_id: f32,
    @location(3) velocity_angle: f32,
}

@fragment
fn main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv_centered = in.quad_uv - vec2<f32>(0.5, 0.5);
    let dist = length(uv_centered);

    // Gaussian soft glow. The glow vertex shader uses wobble_margin=3.0, so
    // uv_centered covers ±1.5 and the particle body ends at dist≈0.5.
    // Falloff coefficient 1.2 keeps glow clearly visible well outside the particle body.
    // Peak alpha 0.45 ensures the halo is visible against dark backgrounds.
    let glow_alpha = exp(- dist * dist * 3.5) * 0.6 * in.particle_color.a;

    if (glow_alpha < 0.002) {
        discard;
    }

    return vec4<f32>(in.particle_color.rgb, glow_alpha);
}

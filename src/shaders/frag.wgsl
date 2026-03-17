struct VertexOutput {
    @location(0) particle_color: vec4<f32>,
    @location(1) quad_uv: vec2<f32>,
    // UV coordinates for the quad
}

;

@fragment
fn main(in: VertexOutput) -> @location(0) vec4<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let uv_centered = in.quad_uv - center;
    let dist = length(uv_centered);

    let radius = 0.5;

    if (dist > radius) {
        discard;
    }

    let normalized_dist = dist / radius;
    let z = sqrt(max(0.0, 1.0 - normalized_dist * normalized_dist));
    let normal = normalize(vec3<f32>(uv_centered.x * 2.0, uv_centered.y * 2.0, z));

    // Light from top-left-front, organic/diffuse
    let light_dir = normalize(vec3<f32>(- 0.4, 0.7, 1.0));
    let view_dir = vec3<f32>(0.0, 0.0, 1.0);

    // Deeper diffuse shadow for volume (0.5 min = real shadow on back side)
    let diffuse = mix(0.5, 1.0, max(0.0, dot(normal, light_dir)));

    // Faint specular - organic matter isn't shiny
    let reflect_dir = reflect(- light_dir, normal);
    let specular = pow(max(0.0, dot(reflect_dir, view_dir)), 18.0) * 0.08;

    // Dark cell-wall rim: membrane effect - darkens edge like a real cell
    let rim = 1.0 - max(0.0, dot(normal, view_dir));
    let cell_wall = pow(rim, 2.5) * 0.55;

    // Internal translucency glow: center is slightly brighter/warmer (cheap SSS)
    let center_glow = (1.0 - normalized_dist) * 0.18;

    // Gaussian soft edge fade - organic blob, not a hard plastic ball
    let alpha_factor = exp(- normalized_dist * normalized_dist * 6.0);

    var rgb = in.particle_color.rgb;
    rgb = rgb * diffuse;
    rgb = rgb + vec3<f32>(specular);
    rgb = rgb * (1.0 - cell_wall);
    // darken the rim
    rgb = rgb + rgb * center_glow;
    // brighten the core

    let alpha = in.particle_color.a * alpha_factor;

    if (alpha < 0.01) {
        discard;
    }

    return vec4<f32>(rgb, alpha);
}
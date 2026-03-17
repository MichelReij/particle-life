struct VertexOutput {
    @location(0) particle_color: vec4<f32>,
    @location(1) quad_uv: vec2<f32>,
    // UV coordinates for the quad
    @location(2) particle_id: f32,
    // Particle index for unique base shape
    @location(3) velocity_angle: f32,
    // Direction of movement, drives organic shape orientation
}

;

@fragment
fn main(in: VertexOutput) -> @location(0) vec4<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let uv_centered = in.quad_uv - center;
    let dist = length(uv_centered);

    let radius = 0.5;

    // Unique organic shape per particle: base phase from hash, orientation from velocity
    // Like an amoeba: unique shape that rotates as the particle changes direction of travel
    let hash = fract(sin(in.particle_id * 127.1 + 311.7) * 43758.5453);
    let phase1 = hash * 6.2832 + in.velocity_angle;
    let phase2 = hash * 15.7080 + in.velocity_angle * 1.3;

    // Organic shape: angular wobble makes particles amoeba-like instead of perfect circles
    let angle = atan2(uv_centered.y, uv_centered.x);
    let wobble = 1.0 + 0.03 * sin(angle * 3.0 + phase1) + 0.015 * sin(angle * 5.0 + phase2);
    let effective_dist = dist / wobble;

    if (effective_dist > radius) {
        discard;
    }

    let normalized_dist = effective_dist / radius;
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

    // Soft organic edge: full opacity in core, gentle fade in outer 20%
    let alpha_factor = smoothstep(1.0, 0.8, normalized_dist);

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
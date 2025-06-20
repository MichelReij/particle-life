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
    let uv_centered = in.quad_uv - center;
    let dist = length(uv_centered);

    let radius = 0.5;
    let edge_softness = 0.05;

    // Discard fragments outside the sphere
    if (dist > radius) {
        discard;
    }

    // Calculate 3D sphere surface normal
    // Map 2D UV to 3D sphere surface
    let normalized_dist = dist / radius;
    let z = sqrt(max(0.0, 1.0 - normalized_dist * normalized_dist));

    // Create 3D normal vector for lighting
    let normal = normalize(vec3<f32>(uv_centered.x * 2.0, // Scale to -1 to 1 range
    uv_centered.y * 2.0, // Scale to -1 to 1 range
    z));

    // Light setup - light coming from top-right-front
    let light_dir = normalize(vec3<f32>(0.6, 0.6, 1.0));
    let view_dir = vec3<f32>(0.0, 0.0, 1.0);

    // Diffuse lighting (surface facing light) - very subtle shadows
    let diffuse = max(0.85, dot(normal, light_dir));

    // Specular highlighting (shiny reflection) - minimal shine
    let reflect_dir = reflect(- light_dir, normal);
    let specular = pow(max(0.0, dot(reflect_dir, view_dir)), 32.0) * 0.05;

    // Rim lighting for better 3D effect - barely visible
    let rim = 1.0 - max(0.0, dot(normal, view_dir));
    let rim_light = pow(rim, 2.0) * 0.02;

    // Anti-aliased edge
    let alpha_factor = 1.0 - smoothstep(radius - edge_softness, radius, dist);

    // Apply lighting to particle color
    var final_color = in.particle_color;
    final_color = vec4<f32>(final_color.rgb * diffuse, final_color.a);
    // Apply diffuse lighting
    final_color = vec4<f32>(final_color.rgb + vec3<f32>(specular), final_color.a);
    // Add specular highlight
    final_color = vec4<f32>(final_color.rgb + vec3<f32>(rim_light), final_color.a);
    // Add rim lighting
    final_color.a *= alpha_factor;

    // Discard very transparent fragments
    if (final_color.a < 0.01) {
        discard;
    }

    return final_color;
}
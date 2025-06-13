// Vertex shader for particle rendering
struct Uniforms {
    view_matrix: mat4x4<f32>,
    projection_matrix: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,
}

struct InstanceInput {
    @location(1) particle_position: vec2<f32>,
    @location(2) particle_color: vec3<f32>,
    @location(3) particle_size: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;

    // Scale the quad vertex by particle size
    let scaled_pos = vertex.position * instance.particle_size;

    // Translate to particle position
    let world_pos = scaled_pos + instance.particle_position;

    out.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
    out.color = instance.particle_color;

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Simple circular particle with distance-based alpha
    let center = vec2<f32>(0.5, 0.5);
    let dist = distance(in.clip_position.xy, center);
    let alpha = 1.0 - smoothstep(0.0, 0.5, dist);

    return vec4<f32>(in.color, alpha);
}

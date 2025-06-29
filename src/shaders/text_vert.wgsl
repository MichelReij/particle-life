// Simple Text Overlay Vertex Shader
// Full-screen quad for text rendering

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) screen_pos: vec2<f32>,
}

@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Full-screen triangle
    var pos = array<vec2<f32>, 3>(vec2<f32>(- 1.0, - 1.0), vec2<f32>(3.0, - 1.0), vec2<f32>(- 1.0, 3.0));

    var out: VertexOutput;
    out.clip_position = vec4<f32>(pos[vertex_index], 0.0, 1.0);
    out.screen_pos = pos[vertex_index];
    return out;
}

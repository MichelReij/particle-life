// Lightning vertex shader for rendering lightning segments as lines

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Create a fullscreen quad using vertex index
    // 0: (-1, -1), 1: (1, -1), 2: (-1, 1), 3: (1, 1), 4: (-1, 1), 5: (1, -1)
    let x = f32(i32(vertex_index & 1u) * 2 - 1);
    let y = f32(i32((vertex_index >> 1u) & 1u) * 2 - 1);

    // Convert NDC (-1 to 1) to UV coordinates (0 to 1)
    let uv = vec2<f32>((x + 1.0) * 0.5, (y + 1.0) * 0.5);

    var output: VertexOutput;
    output.clip_position = vec4<f32>(x, y, 0.0, 1.0);
    output.uv = uv;

    return output;
}

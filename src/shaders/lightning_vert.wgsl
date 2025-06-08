// Lightning effect vertex shader
// Simple fullscreen quad for lightning overlay

@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {
    // Create a fullscreen quad using vertex index
    // 0: (-1, -1), 1: (1, -1), 2: (-1, 1), 3: (1, 1), 4: (-1, 1), 5: (1, -1)
    let x = f32(i32(vertex_index & 1u) * 2 - 1);
    let y = f32(i32((vertex_index >> 1u) & 1u) * 2 - 1);

    return vec4<f32>(x, y, 0.0, 1.0);
}

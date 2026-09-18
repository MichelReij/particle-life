// Lightning vertex shader for rendering lightning segments as lines

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Create a fullscreen quad using vertex index
    // Triangle 1: (0,1,2) = (-1,-1), (1,-1), (-1,1)
    // Triangle 2: (3,4,5) = (1,-1), (1,1), (-1,1)
    var positions = array<vec2<f32>, 6>(vec2<f32>(- 1.0, - 1.0), // 0: bottom-left
    vec2<f32>(1.0, - 1.0), // 1: bottom-right
    vec2<f32>(- 1.0, 1.0), // 2: top-left
    vec2<f32>(1.0, - 1.0), // 3: bottom-right
    vec2<f32>(1.0, 1.0), // 4: top-right
    vec2<f32>(- 1.0, 1.0));
    // 5: top-left

    let position = positions[vertex_index];

    // Convert NDC (-1 to 1) to UV coordinates (0 to 1). Y is flipped
    // (1.0 - ...) so uv.y=0 lands at the TOP of the screen and 1 at the
    // bottom, matching the raster/world-Y-down convention every other pass
    // uses (see vert.wgsl's own Y flip going the other way, world->clip) —
    // without this, lightning_frag_buffer.wgsl's world-to-viewport Y math
    // and this uv were on opposite conventions, so panning/zooming
    // vertically moved the bolt (and the thundercloud darkening, which
    // shares the same uv) in exactly the wrong direction relative to the
    // particles.
    let uv = vec2<f32>((position.x + 1.0) * 0.5, 1.0 - (position.y + 1.0) * 0.5);

    var output: VertexOutput;
    output.clip_position = vec4<f32>(position.x, position.y, 0.0, 1.0);
    output.uv = uv;

    return output;
}

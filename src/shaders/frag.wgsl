struct LayerUniforms {
  color: vec4f,
  offset: vec2f, // Parallax offset, not used in frag but part of the struct
};

// We expect grid_size at group 0, binding 0 (from previous setup)
// We expect layer_params (color, offset) at group 1, binding 0
@group(1) @binding(0) var<uniform> layer_params: LayerUniforms;

struct VertexOutput {
  @location(0) particle_color: vec4<f32>,
};

@fragment
fn main(in: VertexOutput) -> @location(0) vec4<f32> {
  return in.particle_color;
}

struct VertexOutput {
  @location(0) particle_color: vec4<f32>,
};

@fragment
fn main(in: VertexOutput) -> @location(0) vec4<f32> {
  return in.particle_color;
}

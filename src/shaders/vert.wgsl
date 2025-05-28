struct SimParams {
  delta_time: f32,
  friction: f32,
  num_particles: u32,
  num_types: u32,

  virtual_world_width: f32,
  virtual_world_height: f32,
  canvas_render_width: f32,
  canvas_render_height: f32,
  virtual_world_offset_x: f32,
  virtual_world_offset_y: f32,
  boundary_mode: u32,
  particle_render_size: f32,
  force_scale: f32,
  r_smooth: f32,
  flat_force: u32,
  drift_x_per_second: f32, // New parameter
  inter_type_attraction_scale: f32, // New parameter
  inter_type_radius_scale: f32,   // New parameter
  _padding_final: f32, // For 76-byte alignment
}

@group(0) @binding(0) var<uniform> sim_params: SimParams;

struct VertexInput {
  @location(3) quad_pos: vec2<f32>, // Vertex position of the quad (-1 to 1) - REVERTED from 4
  @builtin(instance_index) instance_idx: u32,
};

struct ParticleInstanceInput {
  @location(0) particle_pos: vec2<f32>,
  @location(1) particle_vel: vec2<f32>,
  @location(2) particle_type: u32,
  // @location(3) particle_size: f32, // REVERTED
};

struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) particle_color: vec4<f32>,
};

// Simple color palette (can be expanded or moved to a uniform buffer)
fn getColorForType(ptype: u32, num_types: u32) -> vec4<f32> {
    let hue_step = 360.0 / f32(num_types);
    let hue = f32(ptype) * hue_step;

    // Basic HSV to RGB conversion (simplified)
    let s = 0.9;
    let v = 0.95;

    let c = v * s;
    let h_prime = hue / 60.0;
    let x = c * (1.0 - abs(fract(h_prime / 2.0) * 2.0 - 1.0));
    var r_temp: f32 = 0.0;
    var g_temp: f32 = 0.0;
    var b_temp: f32 = 0.0;

    if (h_prime < 1.0) {
        r_temp = c; g_temp = x; b_temp = 0.0;
    } else if (h_prime < 2.0) {
        r_temp = x; g_temp = c; b_temp = 0.0;
    } else if (h_prime < 3.0) {
        r_temp = 0.0; g_temp = c; b_temp = x;
    } else if (h_prime < 4.0) {
        r_temp = 0.0; g_temp = x; b_temp = c;
    } else if (h_prime < 5.0) {
        r_temp = x; g_temp = 0.0; b_temp = c;
    } else {
        r_temp = c; g_temp = 0.0; b_temp = x;
    }

    let m = v - c;
    return vec4<f32>(r_temp + m, g_temp + m, b_temp + m, 0.5); // Alpha set to 0.5 for semi-transparency
}

@vertex
fn main(
  particle_attrs: ParticleInstanceInput,
  vertex_attrs: VertexInput
) -> VertexOutput {
  var out: VertexOutput;

  // Cull "dead" particles that were moved far away by the compute shader
  // The compute shader moves them to (-100000.0, -100000.0).
  // Check against a value like -50000.0 to catch these.
  if (particle_attrs.particle_pos.x < -50000.0) {
    out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0); // Position completely outside clip space [-1,1]
    out.particle_color = vec4<f32>(0.0, 0.0, 0.0, 0.0); // Make transparent
    return out;
  }

  // Use global particle_render_size from sim_params
  let particle_radius_pixels = sim_params.particle_render_size;

  // Particle position is in virtual world coordinates.
  // Translate to canvas-relative coordinates before normalizing.
  let canvas_relative_pos_x = particle_attrs.particle_pos.x - sim_params.virtual_world_offset_x;
  let canvas_relative_pos_y = particle_attrs.particle_pos.y - sim_params.virtual_world_offset_y;

  // Convert canvas-relative particle position to clip space (-1 to 1)
  let normalized_particle_pos = vec2<f32>(
    (canvas_relative_pos_x / sim_params.canvas_render_width) * 2.0 - 1.0,
    (1.0 - (canvas_relative_pos_y / sim_params.canvas_render_height)) * 2.0 - 1.0 // Invert Y
  );

  // Scale quad vertex by particle size and convert to clip space dimensions relative to canvas render size
  let scaled_quad_pos = vec2<f32>(
    vertex_attrs.quad_pos.x * (particle_radius_pixels / sim_params.canvas_render_width),
    vertex_attrs.quad_pos.y * (particle_radius_pixels / sim_params.canvas_render_height)
  );

  out.position = vec4<f32>(normalized_particle_pos + scaled_quad_pos, 0.0, 1.0);
  out.particle_color = getColorForType(particle_attrs.particle_type, sim_params.num_types);
  return out;
}

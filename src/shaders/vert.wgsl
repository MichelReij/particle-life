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
    drift_x_per_second: f32,
    // New parameter
    inter_type_attraction_scale: f32,
    // New parameter
    inter_type_radius_scale: f32,
    // New parameter
    time: f32,
    // Added time
    fisheyeStrength: f32,
    // Fisheye distortion strength
    backgroundColor: vec3<f32>,
    // New: background color
    _padding1: f32,
    // Padding to make total size 96 bytes (24 * 4)
}

@group(0) @binding(0)
var<uniform> sim_params: SimParams;

@group(0) @binding(1)
var<storage, read> particle_colors: array<vec4<f32>>;

struct VertexInput {
    @location(3) quad_pos: vec2<f32>,
    // Vertex position of the quad (-1 to 1) - REVERTED from 4
    @builtin(instance_index) instance_idx: u32,
}

;

struct ParticleInstanceInput {
    @location(0) particle_pos: vec2<f32>,
    @location(1) particle_vel: vec2<f32>,
    @location(2) particle_type: u32,
    // @location(3) particle_size: f32, // REVERTED
}

;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) particle_color: vec4<f32>,
    @location(1) quad_uv: vec2<f32>,
    // UV coordinates for circular particle rendering
}

;

// Get color for particle type from precomputed custom colors buffer
fn getColorForType(ptype: u32, num_types: u32) -> vec4<f32> {
    return particle_colors[ptype];
}

@vertex
fn main(particle_attrs: ParticleInstanceInput, vertex_attrs: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Cull "dead" particles that were moved far away by the compute shader
    // The compute shader moves them to (-100000.0, -100000.0).
    // Check against a value like -50000.0 to catch these.
    if (particle_attrs.particle_pos.x < - 50000.0) {
        out.position = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        // Position completely outside clip space [-1,1]
        out.particle_color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        // Make transparent
        out.quad_uv = vec2<f32>(0.0, 0.0);
        // Set UV for dead particles
        return out;
    }

    // Use global particle_render_size from sim_params
    let particle_radius_pixels = sim_params.particle_render_size;

    // Particle position is in virtual world coordinates.
    // Translate to canvas-relative coordinates before normalizing.
    let canvas_relative_pos_x = particle_attrs.particle_pos.x - sim_params.virtual_world_offset_x;
    let canvas_relative_pos_y = particle_attrs.particle_pos.y - sim_params.virtual_world_offset_y;

    // Convert canvas-relative particle position to clip space (-1 to 1)
    // Invert Y axis for proper screen coordinates
    let normalized_particle_pos = vec2<f32>((canvas_relative_pos_x / sim_params.canvas_render_width) * 2.0 - 1.0, (1.0 - (canvas_relative_pos_y / sim_params.canvas_render_height)) * 2.0 - 1.0);

    // Scale quad vertex by particle size and convert to clip space dimensions relative to canvas render size
    let scaled_quad_pos = vec2<f32>(vertex_attrs.quad_pos.x * (particle_radius_pixels / sim_params.canvas_render_width), vertex_attrs.quad_pos.y * (particle_radius_pixels / sim_params.canvas_render_height));

    out.position = vec4<f32>(normalized_particle_pos + scaled_quad_pos, 0.0, 1.0);
    out.particle_color = getColorForType(particle_attrs.particle_type, sim_params.num_types);

    // Calculate UV coordinates for the quad (convert from [-1,1] to [0,1])
    out.quad_uv = (vertex_attrs.quad_pos + 1.0) * 0.5;

    return out;
}

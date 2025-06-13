// Simple vertex shader for particle rendering
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
    inter_type_attraction_scale: f32,
    inter_type_radius_scale: f32,
    time: f32,
    fisheyeStrength: f32,
    backgroundColor: vec3<f32>,
    lenia_enabled: u32,
    lenia_growth_mu: f32,
    lenia_growth_sigma: f32,
    lenia_kernel_radius: f32,
}

struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    ptype: u32,
    size: f32,
}

@group(0) @binding(2)
var<uniform> sim_params: SimParams;

@group(0) @binding(0)
var<storage, read> particles: array<Particle>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;

    // Get particle data
    let particle = particles[vertex_index];

    // Convert world position to NDC
    let x = (particle.pos.x / sim_params.virtual_world_width) * 2.0 - 1.0;
    let y = (particle.pos.y / sim_params.virtual_world_height) * 2.0 - 1.0;

    output.position = vec4<f32>(x, y, 0.0, 1.0);

    // Simple color based on particle type
    let hue = f32(particle.ptype) * 60.0;
    output.color = vec4<f32>(
        sin(hue * 0.017453) * 0.5 + 0.5,
        cos(hue * 0.017453) * 0.5 + 0.5,
        0.8,
        1.0
    );

    return output;
}

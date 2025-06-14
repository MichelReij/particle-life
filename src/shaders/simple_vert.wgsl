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
    background_color_r: f32,
    background_color_g: f32,
    background_color_b: f32,
    lenia_enabled: u32,
    lenia_growth_mu: f32,
    lenia_growth_sigma: f32,
    lenia_kernel_radius: f32,
    lightning_frequency: f32,
    lightning_intensity: f32,
    lightning_duration: f32,
    _padding: f32,
}

struct Particle {
    pos: vec2<f32>,
    vel: vec2<f32>,
    ptype: u32,
    size: f32,
}

@group(0) @binding(0)
var<storage, read> particles: array<Particle>;

@group(0) @binding(1)
var<uniform> sim_params: SimParams;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

@vertex
fn main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;

    // Each particle is rendered as 6 vertices (2 triangles forming a quad)
    let particle_index = vertex_index / 6u;
    let vertex_in_quad = vertex_index % 6u;

    // Get particle data
    let particle = particles[particle_index];

    // Define quad vertices in local space (-1 to +1)
    var local_pos: vec2<f32>;
    var uv: vec2<f32>;

    // Generate 6 vertices for 2 triangles forming a quad
    switch vertex_in_quad {
        case 0u: { local_pos = vec2<f32>(-1.0, -1.0); uv = vec2<f32>(0.0, 0.0); } // Bottom-left
        case 1u: { local_pos = vec2<f32>( 1.0, -1.0); uv = vec2<f32>(1.0, 0.0); } // Bottom-right
        case 2u: { local_pos = vec2<f32>(-1.0,  1.0); uv = vec2<f32>(0.0, 1.0); } // Top-left
        case 3u: { local_pos = vec2<f32>( 1.0, -1.0); uv = vec2<f32>(1.0, 0.0); } // Bottom-right
        case 4u: { local_pos = vec2<f32>( 1.0,  1.0); uv = vec2<f32>(1.0, 1.0); } // Top-right
        case 5u: { local_pos = vec2<f32>(-1.0,  1.0); uv = vec2<f32>(0.0, 1.0); } // Top-left
        default: { local_pos = vec2<f32>(0.0, 0.0); uv = vec2<f32>(0.5, 0.5); }
    }

    // Scale by particle size and convert to screen space
    let scale_factor = sim_params.canvas_render_width / sim_params.virtual_world_width;
    let particle_size_screen = particle.size * scale_factor;

    // Convert particle size from pixels to NDC
    let size_ndc = particle_size_screen / sim_params.canvas_render_width * 2.0;

    // Scale local position by particle size
    local_pos *= size_ndc;

    // Convert world position to NDC
    let world_ndc_x = (particle.pos.x / sim_params.virtual_world_width) * 2.0 - 1.0;
    let world_ndc_y = (particle.pos.y / sim_params.virtual_world_height) * 2.0 - 1.0;

    // Final position
    output.position = vec4<f32>(world_ndc_x + local_pos.x, world_ndc_y + local_pos.y, 0.0, 1.0);
    output.uv = uv;

    // Simple color based on particle type
    let hue = f32(particle.ptype) * 60.0;
    output.color = vec4<f32>(
        sin(hue * 0.017453) * 0.5 + 0.5,
        cos(hue * 0.017453) * 0.5 + 0.5,
        0.8,
        0.6  // Set opacity to 0.6 as requested
    );

    return output;
}

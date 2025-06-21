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
    viewport_width: f32,
    viewport_height: f32,
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
    fisheye_strength: f32,
    // Fisheye distortion strength
    background_color_r: f32,
    // Background color red component
    background_color_g: f32,
    // Background color green component
    background_color_b: f32,
    // Background color blue component

    // Lenia-inspired parameters
    lenia_enabled: u32,
    // Boolean as u32: enable Lenia-style interactions
    lenia_growth_mu: f32,
    // Lenia growth function center (μ)
    lenia_growth_sigma: f32,
    // Lenia growth function spread (σ)
    lenia_kernel_radius: f32,
    // Lenia kernel radius in pixels
    lightning_frequency: f32,
    // Lightning strikes per second
    lightning_intensity: f32,
    // Lightning brightness/strength (0-1)
    lightning_duration: f32,
    // Duration of each lightning flash in seconds
    _padding: f32,
    // Padding to align to 16 bytes
    _padding2: f32,
    // Additional padding for 16-byte alignment (128 bytes total)
}

@group(0) @binding(2)
var<uniform> sim_params: SimParams;

@group(0) @binding(0)
var<storage, read> particle_colors: array<vec4<f32>>;

struct VertexInput {
    @location(4) quad_pos: vec2<f32>,
    // Vertex position of the quad (-1 to 1) - Updated to location 4
    @builtin(instance_index) instance_idx: u32,
}

;

struct ParticleInstanceInput {
    @location(0) particle_pos: vec2<f32>,
    @location(1) particle_vel: vec2<f32>,
    @location(2) particle_type: u32,
    @location(3) particle_size: f32,
    @location(5) target_size: f32,
    @location(6) transition_start: f32,
    @location(7) transition_type: u32,
    @location(8) is_active: u32,
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

    // Cull inactive particles using the is_active flag
    if (particle_attrs.is_active == 0u) {
        // Position far outside clip space and make completely transparent
        out.position = vec4<f32>(- 10.0, - 10.0, - 10.0, 1.0);
        // Make completely transparent
        out.particle_color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        // Set UV coordinates
        out.quad_uv = vec2<f32>(0.0, 0.0);
        return out;
    }

    // DEBUG: Color particles based on is_active value to diagnose the issue
    var debug_color = getColorForType(particle_attrs.particle_type, sim_params.num_types);

    // Debug color coding:
    // Normal color = is_active == 1u (expected)
    // Bright red = is_active != 0u and is_active != 1u (corrupted)
    // Bright blue = is_active == 0u (inactive, should be culled above)
    // Bright yellow = any other unexpected case

    if (particle_attrs.is_active == 0u) {
        debug_color = vec4<f32>(0.0, 0.0, 1.0, 1.0);
        // Blue for inactive (shouldn't happen since culled above)
    }
    else if (particle_attrs.is_active == 1u) {
        // Keep normal color - this is expected
    }
    else {
        // Red for corrupted values (not 0 or 1)
        debug_color = vec4<f32>(1.0, 0.0, 0.0, 1.0);
    }

    // Use per-particle size instead of global particle_render_size
    let particle_radius_pixels = clamp(particle_attrs.particle_size, 1.0, 30.0);
    // Safety clamp in vertex shader too!

    // Particle position is in virtual world coordinates (0-2400 range)
    // Convert directly to clip space (-1 to 1) based on the fixed 2400x2400 virtual world
    let normalized_particle_pos = vec2<f32>((particle_attrs.particle_pos.x / 2400.0) * 2.0 - 1.0, (1.0 - (particle_attrs.particle_pos.y / 2400.0)) * 2.0 - 1.0);

    // Scale quad vertex by particle size and convert to clip space dimensions
    // Fixed scaling for 2400x2400 virtual world
    let scaled_quad_pos = vec2<f32>(vertex_attrs.quad_pos.x * (particle_radius_pixels / 2400.0), vertex_attrs.quad_pos.y * (particle_radius_pixels / 2400.0));

    out.position = vec4<f32>(normalized_particle_pos + scaled_quad_pos, 0.0, 1.0);
    out.particle_color = debug_color;

    // Calculate UV coordinates for the quad (convert from [-1,1] to [0,1])
    out.quad_uv = (vertex_attrs.quad_pos + 1.0) * 0.5;

    return out;
}

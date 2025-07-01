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
    fisheye_strength: f32,
    background_color_r: f32,
    background_color_g: f32,
    background_color_b: f32,

    // Lenia-inspired parameters
    lenia_enabled: u32,
    lenia_growth_mu: f32,
    lenia_growth_sigma: f32,
    lenia_kernel_radius: f32,

    // Lightning parameters
    lightning_frequency: f32,
    lightning_intensity: f32,
    lightning_duration: f32,

    // Particle transition parameters for GPU-based size transitions
    transition_active: u32,
    transition_start_time: f32,
    transition_duration: f32,
    transition_start_count: u32,
    transition_end_count: u32,
    transition_is_grow: u32,

    // Spatial grid optimization parameters
    spatial_grid_enabled: u32,
    spatial_grid_cell_size: f32,
    spatial_grid_width: u32,
    spatial_grid_height: u32,
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

    // Use per-particle size directly (CPU already handles proper bounds)
    let particle_radius_pixels = particle_attrs.particle_size;
    // Trust the CPU calculations which include type multipliers and randomization

    // Particle position is in virtual world coordinates (0-virtual_world_width/height range)
    // Convert directly to clip space (-1 to 1) based on the virtual world dimensions from sim_params
    let normalized_particle_pos = vec2<f32>((particle_attrs.particle_pos.x / sim_params.virtual_world_width) * 2.0 - 1.0, (1.0 - (particle_attrs.particle_pos.y / sim_params.virtual_world_height)) * 2.0 - 1.0);

    // Scale quad vertex by particle size and convert to clip space dimensions
    // Dynamic scaling based on virtual world dimensions from sim_params
    let scaled_quad_pos = vec2<f32>(vertex_attrs.quad_pos.x * (particle_radius_pixels / sim_params.virtual_world_width), vertex_attrs.quad_pos.y * (particle_radius_pixels / sim_params.virtual_world_height));

    out.position = vec4<f32>(normalized_particle_pos + scaled_quad_pos, 0.0, 1.0);
    out.particle_color = debug_color;

    // Calculate UV coordinates for the quad (convert from [-1,1] to [0,1])
    out.quad_uv = (vertex_attrs.quad_pos + 1.0) * 0.5;

    return out;
}

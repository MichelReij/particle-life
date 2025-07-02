struct SimParams {
    delta_time: f32,
    friction: f32,
    num_particles: u32,
    num_types: u32,

    virtual_world_width: f32,
    // e.g. 4320.0 - full virtual simulation space width (4 × 1080)
    virtual_world_height: f32,
    // e.g. 4320.0 - full virtual simulation space height (4 × 1080)
    canvas_render_width: f32,
    // e.g. 1080.0 - final canvas display width for rendering normalization
    canvas_render_height: f32,
    // e.g. 1080.0 - final canvas display height for rendering normalization
    virtual_world_offset_x: f32,
    // e.g. 100.0
    virtual_world_offset_y: f32,
    // e.g. 100.0
    boundary_mode: u32,
    // 0: disappear, 1: wrap (replaces wrap_mode)
    particle_render_size: f32,
    force_scale: f32,
    r_smooth: f32,
    // Moved r_smooth here for consistent ordering with TS
    flat_force: u32,
    // Moved flat_force here
    drift_x_per_second: f32,
    // New parameter for horizontal drift
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

    // Lightning parameters
    lightning_frequency: f32,
    // Lightning strikes per second
    lightning_intensity: f32,
    // Lightning brightness/strength (0-1)
    lightning_duration: f32,
    // Duration of each lightning flash in seconds

    // Particle transition parameters for GPU-based size transitions
    transition_active: u32,
    // Boolean: is a transition currently active
    transition_start_time: f32,
    // When the transition started
    transition_duration: f32,
    // How long the transition should take
    transition_start_count: u32,
    // Particle count at start of transition
    transition_end_count: u32,
    // Target particle count
    transition_is_grow: u32,
    // Boolean: true for grow, false for shrink

    // Spatial grid optimization parameters
    spatial_grid_enabled: u32,
    // Boolean: enable spatial grid optimization (0=disabled, 1=enabled)
    spatial_grid_cell_size: f32,
    // Size of each grid cell in world units
    spatial_grid_width: u32,
    // Number of grid cells horizontally
    spatial_grid_height: u32,
    // Number of grid cells vertically

    // Viewport/zoom parameters for rendering optimization
    viewport_center_x: f32,
    // Center of viewport in virtual world coordinates
    viewport_center_y: f32,
    // Center of viewport in virtual world coordinates
    viewport_width: f32,
    // Width of visible area in virtual world coordinates
    viewport_height: f32,
    // Height of visible area in virtual world coordinates
    viewport_radius: f32,
    // Radius of circular viewport in virtual world coordinates (for round screen)

    // Padding to ensure 16-byte alignment (3 × f32 = 12 bytes)
    _viewport_padding1: f32,
    _viewport_padding2: f32,
    _viewport_padding3: f32,
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

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) particle_color: vec4<f32>,
    @location(1) quad_uv: vec2<f32>,
    // UV coordinates for circular particle rendering
}

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

    // VIEWPORT CULLING: Cull particles outside the visible zoom area
    // Calculate viewport bounds in virtual world coordinates
    let viewport_left = sim_params.viewport_center_x - sim_params.viewport_width * 0.5;
    let viewport_right = sim_params.viewport_center_x + sim_params.viewport_width * 0.5;
    let viewport_top = sim_params.viewport_center_y - sim_params.viewport_height * 0.5;
    let viewport_bottom = sim_params.viewport_center_y + sim_params.viewport_height * 0.5;

    // Add particle radius as margin to ensure particles partially in view are rendered
    let particle_radius = particle_attrs.particle_size;
    let margin = particle_radius * 2.0;
    // Extra margin for safety

    // RECTANGULAR CULLING: Check if particle is outside viewport bounds (with margin)
    if (particle_attrs.particle_pos.x < viewport_left - margin || particle_attrs.particle_pos.x > viewport_right + margin || particle_attrs.particle_pos.y < viewport_top - margin || particle_attrs.particle_pos.y > viewport_bottom + margin) {

        // Cull by positioning outside clip space
        out.position = vec4<f32>(- 10.0, - 10.0, - 10.0, 1.0);
        out.particle_color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        out.quad_uv = vec2<f32>(0.0, 0.0);
        return out;
    }

    // CIRCULAR CULLING: For round screen, also cull particles outside circular viewport
    let distance_from_center = length(particle_attrs.particle_pos - vec2<f32>(sim_params.viewport_center_x, sim_params.viewport_center_y));
    if (distance_from_center > sim_params.viewport_radius + margin) {
        // Cull particles outside the circular viewport
        out.position = vec4<f32>(- 10.0, - 10.0, - 10.0, 1.0);
        out.particle_color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
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

    // Use per-particle size directly - no scaling needed since we render directly to canvas
    // The particle sizes are already calculated correctly for the final display
    let particle_radius_pixels = particle_attrs.particle_size;

    // Particle position is in virtual world coordinates (0-virtual_world_width/height range)
    // Convert directly to canvas clip space, taking zoom and viewport into account

    // Transform from virtual world coords to viewport-relative coords
    let viewport_relative_x = (particle_attrs.particle_pos.x - viewport_left) / sim_params.viewport_width;
    let viewport_relative_y = (particle_attrs.particle_pos.y - viewport_top) / sim_params.viewport_height;

    // Convert to clip space (-1 to 1) - this now renders directly to canvas size
    let normalized_particle_pos = vec2<f32>(viewport_relative_x * 2.0 - 1.0, (1.0 - viewport_relative_y) * 2.0 - 1.0);

    // Scale quad vertex by particle size - scale relative to viewport size for proper zoom behavior
    // When zoomed in, the viewport is smaller, so particles should appear larger
    let viewport_scale_x = 2.0 / sim_params.viewport_width;
    // Scale relative to viewport width
    let viewport_scale_y = 2.0 / sim_params.viewport_height;
    // Scale relative to viewport height
    let scaled_quad_pos = vec2<f32>(vertex_attrs.quad_pos.x * particle_radius_pixels * viewport_scale_x, vertex_attrs.quad_pos.y * particle_radius_pixels * viewport_scale_y);

    out.position = vec4<f32>(normalized_particle_pos + scaled_quad_pos, 0.0, 1.0);
    out.particle_color = debug_color;

    // Calculate UV coordinates for the quad (convert from [-1,1] to [0,1])
    out.quad_uv = (vertex_attrs.quad_pos + 1.0) * 0.5;

    return out;
}

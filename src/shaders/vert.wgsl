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
    night_alpha: f32,
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
    @location(9) color_seed: u32, // nieuw bij elke spawn (compute.wgsl)
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) particle_color: vec4<f32>,
    @location(1) quad_uv: vec2<f32>,
    @location(2) particle_id: f32,
    @location(3) velocity_angle: f32,
    @location(4) particle_canvas_uv: vec2<f32>, // particle center in 0-1 canvas space (for vignette)
}

// Get color for particle type from precomputed custom colors buffer
fn getColorForType(ptype: u32, num_types: u32) -> vec4<f32> {
    return particle_colors[ptype];
}

// ─── Variatie binnen een deeltjestype ─────────────────────────────────────────
// Elk deeltje wijkt vast een klein beetje af van de kleur van zijn type, zoals
// individuen van één soort: lightness, chroma en hue in OkLCH, plus de opacity.
// Elke as krijgt een eigen afwijking binnen ±max, klokvormig verdeeld (hash_bell):
// de meeste deeltjes liggen dicht bij de typekleur, een enkeling haalt de rand.
// Uit het kleurzaadje dat het deeltje bij elke spawn krijgt (color_seed,
// compute.wgsl); zolang het leeft blijft zijn kleur dus gelijk.
// Max ±5% van de volle schaal, in de schaal van CSS-oklch() en het kleurenpaneel:
// L 0..1 → ±0.05, C 0..0.4 → ±0.02, H 0..360° → ±18°, opacity 0..1 → ±0.05.
const VARY_L: f32 = 0.05;
const VARY_C: f32 = 0.02;
const VARY_H_DEG: f32 = 18.0;
const VARY_ALPHA: f32 = 0.05;

// PCG-hash
fn pcg(x: u32) -> u32 {
    let state = x * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

// u32 → gelijkmatig verdeeld in [-1, 1]
fn unit_signed(h: u32) -> f32 {
    return f32(h) * (2.0 / 4294967295.0) - 1.0;
}

// Naar het midden verdeeld in [-1, 1]: het gemiddelde van twee gelijkmatige waarden
// (driehoeksverdeling, σ ≈ 0.41). Komt nooit buiten ±1: 36% valt binnen ±0.2
// (gelijkmatig: 20%), 16% buiten ±0.6 (gelijkmatig: 40%). Smaller: middel over
// meer waarden (drie: σ = 1/3, 43% binnen ±0.2, 7% buiten ±0.6).
fn hash_bell(x: u32) -> f32 {
    let a = pcg(x);
    let b = pcg(a);
    return (unit_signed(a) + unit_signed(b)) / 2.0;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let x = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(pow((x + 0.055) / 1.055, vec3<f32>(2.4)), x / 12.92, x <= vec3<f32>(0.04045));
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let x = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0));
    return select(1.055 * pow(x, vec3<f32>(1.0 / 2.4)) - 0.055, x * 12.92, x <= vec3<f32>(0.0031308));
}

// OkLab (Björn Ottosson) ↔ lineair sRGB
fn linear_srgb_to_oklab(c: vec3<f32>) -> vec3<f32> {
    let lms = vec3<f32>(
        0.4122214708 * c.r + 0.5363325363 * c.g + 0.0514459929 * c.b,
        0.2119034982 * c.r + 0.6806995451 * c.g + 0.1073969566 * c.b,
        0.0883024619 * c.r + 0.2817188376 * c.g + 0.6299787005 * c.b,
    );
    let l_ = pow(max(lms, vec3<f32>(0.0)), vec3<f32>(1.0 / 3.0));
    return vec3<f32>(
        0.2104542553 * l_.x + 0.7936177850 * l_.y - 0.0040720468 * l_.z,
        1.9779984951 * l_.x - 2.4285922050 * l_.y + 0.4505937099 * l_.z,
        0.0259040371 * l_.x + 0.7827717662 * l_.y - 0.8086757660 * l_.z,
    );
}

fn oklab_to_linear_srgb(lab: vec3<f32>) -> vec3<f32> {
    let l_ = lab.x + 0.3963377774 * lab.y + 0.2158037573 * lab.z;
    let m_ = lab.x - 0.1055613458 * lab.y - 0.0638541728 * lab.z;
    let s_ = lab.x - 0.0894841775 * lab.y - 1.2914855480 * lab.z;
    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;
    return vec3<f32>(
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    );
}

// Typekleur (sRGB zoals in particle_colors, + alpha) → kleur van dit ene deeltje
fn vary_per_particle(base: vec4<f32>, id: u32) -> vec4<f32> {
    let seed = id * 4u;
    let dl = hash_bell(seed) * VARY_L;
    let dc = hash_bell(seed + 1u) * VARY_C;
    let dh = radians(hash_bell(seed + 2u) * VARY_H_DEG);
    let da = hash_bell(seed + 3u) * VARY_ALPHA;

    let lab = linear_srgb_to_oklab(srgb_to_linear(base.rgb));
    let chroma = length(lab.yz);
    var ab = lab.yz;
    if (chroma > 1e-4) {
        // Hue draaien en chroma schalen; grijs blijft grijs (heeft geen tint)
        let rotated = vec2<f32>(ab.x * cos(dh) - ab.y * sin(dh), ab.x * sin(dh) + ab.y * cos(dh));
        ab = rotated * (max(chroma + dc, 0.0) / chroma);
    }
    let rgb = linear_to_srgb(oklab_to_linear_srgb(vec3<f32>(clamp(lab.x + dl, 0.0, 1.0), ab)));
    return vec4<f32>(rgb, clamp(base.a + da, 0.0, 1.0));
}

// Pipeline-overridable: 1.1 for normal rendering, larger values for the glow pass
override wobble_margin: f32 = 1.1;

@vertex
fn main(particle_attrs: ParticleInstanceInput, vertex_attrs: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Cull inactive particles and particles shrinking below visibility threshold
    // (prevents ghost glow during shrink transition when size approaches 0.1)
    if (particle_attrs.is_active == 0u || particle_attrs.particle_size < 1.0) {
        out.position = vec4<f32>(- 10.0, - 10.0, - 10.0, 1.0);
        out.particle_color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
        out.quad_uv = vec2<f32>(0.0, 0.0);
        return out;
    }

    // STANDARD CULLING: Check if particle position is outside the viewport bounds
    // No fisheye expansion needed - we'll let the grid handle fisheye via fragment shader
    let viewport_left = sim_params.viewport_center_x - sim_params.viewport_width * 0.5;
    let viewport_right = sim_params.viewport_center_x + sim_params.viewport_width * 0.5;
    let viewport_top = sim_params.viewport_center_y - sim_params.viewport_height * 0.5;
    let viewport_bottom = sim_params.viewport_center_y + sim_params.viewport_height * 0.5;

    // Add particle radius as margin to ensure particles partially in view are rendered.
    // Multiply by wobble_margin so glow pass (wobble_margin=3.0) doesn't cull particles
    // whose glow is still visible — preventing halos without a particle (ghosts).
    let particle_radius = particle_attrs.particle_size;
    let margin = particle_radius * wobble_margin * 2.0;

    // Check if particle is outside viewport bounds (with margin)
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

    // Get particle color. Alpha fades non-linearly over the transition's own
    // timing, rather than tracking the (roughly linear) size ramp directly —
    // without an opacity change at all here, a vanish/reappear transition was
    // pure geometry shrinkage, so the particle stayed fully opaque all the
    // way down to a hard cull below 1px (see above), then popped from "fully
    // opaque 1px dot" to "gone" (and the reverse on reappear) instead of
    // smoothly fading through transparency.
    //
    // Eased per feedback: disappearing (shrink) fades slowly at first and
    // accelerates toward the end (ease-in on the fade-OUT amount, t²);
    // appearing (grow) is the exact mirror — fades in quickly at first and
    // eases off as it nears full opacity (ease-out, 1-(1-t)²). For a
    // particle with no active transition this is a no-op (alpha stays 1).
    var particle_color = vary_per_particle(
        getColorForType(particle_attrs.particle_type, sim_params.num_types),
        particle_attrs.color_seed);
    var alpha_fade = 1.0;
    if (particle_attrs.transition_start > 0.0) {
        let elapsed = sim_params.time - particle_attrs.transition_start;
        let t = clamp(elapsed / sim_params.transition_duration, 0.0, 1.0);
        if (particle_attrs.transition_type == 1u) {
            // Shrink: ease-in fade-out.
            alpha_fade = 1.0 - t * t;
        } else {
            // Grow: ease-out fade-in.
            let inv = 1.0 - t;
            alpha_fade = 1.0 - inv * inv;
        }
    }
    particle_color.a *= alpha_fade;

    // Use per-particle size directly - no scaling needed since we render directly to canvas
    let particle_radius_pixels = particle_attrs.particle_size;

    // POSITION CALCULATION: Transform from world coords to viewport-relative coords
    // NO fisheye transformation here - particles will render straight, grid will have fisheye
    let viewport_relative_x = (particle_attrs.particle_pos.x - viewport_left) / sim_params.viewport_width;
    let viewport_relative_y = (particle_attrs.particle_pos.y - viewport_top) / sim_params.viewport_height;

    // Convert to clip space (-1 to 1) - this now renders directly to canvas size
    let normalized_particle_pos = vec2<f32>(viewport_relative_x * 2.0 - 1.0, (1.0 - viewport_relative_y) * 2.0 - 1.0);

    // Scale quad vertex by particle size - scale relative to viewport size for proper zoom behavior
    let viewport_scale_x = 2.0 / sim_params.viewport_width;
    let viewport_scale_y = 2.0 / sim_params.viewport_height;
    // wobble_margin is a pipeline override constant (default 1.1, overridden in glow pass)
    let scaled_quad_pos = vec2<f32>(vertex_attrs.quad_pos.x * particle_radius_pixels * wobble_margin * viewport_scale_x, vertex_attrs.quad_pos.y * particle_radius_pixels * wobble_margin * viewport_scale_y);

    out.position = vec4<f32>(normalized_particle_pos + scaled_quad_pos, 0.0, 1.0);
    out.particle_color = particle_color;

    // UV: scale by wobble_margin so frag uv_centered covers ±(wobble_margin*0.5) — enough room for max wobble
    out.quad_uv = vec2<f32>(vertex_attrs.quad_pos.x * 0.5 * wobble_margin + 0.5, vertex_attrs.quad_pos.y * 0.5 * wobble_margin + 0.5);
    out.particle_id = f32(vertex_attrs.instance_idx);
    // atan2(0, 0) is in WGSL niet gedefinieerd: Chrome geeft 0, maar Safari (WGSL →
    // Metal) gaf NaN voor stilstaande deeltjes — een losse particle zonder krachten
    // staat precies stil — en dan viel het lichaam weg en bleef alleen de halo over
    let vel = particle_attrs.particle_vel;
    out.velocity_angle = select(0.0, atan2(vel.y, vel.x), dot(vel, vel) > 1e-12);
    // Particle center in 0-1 canvas UV space (clip space → UV: (x+1)/2, (1-y)/2)
    out.particle_canvas_uv = vec2<f32>(
        (normalized_particle_pos.x + 1.0) * 0.5,
        (1.0 - normalized_particle_pos.y) * 0.5,
    );

    return out;
}

// Simple Text Overlay Fragment Shader
// Renders FPS and other info at the bottom center of the screen

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

    // Particle transition parameters
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

    // Viewport/zoom parameters for rendering optimization
    viewport_center_x: f32,
    viewport_center_y: f32,
    viewport_width: f32,
    viewport_height: f32,
    viewport_radius: f32,

    // Padding to ensure 16-byte alignment (3 × f32 = 12 bytes)
    night_alpha: f32,
    _viewport_padding2: f32,
    _viewport_padding3: f32,
}

// FPS data structure - updated from CPU
struct FpsData {
    fps: f32,
    frame_count: u32,
    particle_count: u32,
    time: f32,
}

@group(0) @binding(0)
var<uniform> sim_params: SimParams;

@group(0) @binding(1)
var<uniform> fps_data: FpsData;

// Simple 8x8 bitmap font for digits and letters
fn get_char_pixel(char_code: u32, x: u32, y: u32) -> f32 {
    switch char_code {
        case 48u : {
            // '0'
            let char_data = array<u32, 8>(0x3Cu, 0x66u, 0x66u, 0x66u, 0x66u, 0x66u, 0x66u, 0x3Cu);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 49u : {
            // '1'
            let char_data = array<u32, 8>(0x18u, 0x38u, 0x18u, 0x18u, 0x18u, 0x18u, 0x18u, 0x7Eu);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 50u : {
            // '2'
            let char_data = array<u32, 8>(0x3Cu, 0x66u, 0x06u, 0x0Cu, 0x18u, 0x30u, 0x60u, 0x7Eu);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 51u : {
            // '3'
            let char_data = array<u32, 8>(0x3Cu, 0x66u, 0x06u, 0x1Cu, 0x06u, 0x06u, 0x66u, 0x3Cu);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 52u : {
            // '4'
            let char_data = array<u32, 8>(0x0Cu, 0x1Cu, 0x2Cu, 0x4Cu, 0x7Eu, 0x0Cu, 0x0Cu, 0x0Cu);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 53u : {
            // '5'
            let char_data = array<u32, 8>(0x7Eu, 0x60u, 0x60u, 0x7Cu, 0x06u, 0x06u, 0x66u, 0x3Cu);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 54u : {
            // '6'
            let char_data = array<u32, 8>(0x3Cu, 0x66u, 0x60u, 0x7Cu, 0x66u, 0x66u, 0x66u, 0x3Cu);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 55u : {
            // '7'
            let char_data = array<u32, 8>(0x7Eu, 0x06u, 0x0Cu, 0x18u, 0x30u, 0x30u, 0x30u, 0x30u);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 56u : {
            // '8'
            let char_data = array<u32, 8>(0x3Cu, 0x66u, 0x66u, 0x3Cu, 0x66u, 0x66u, 0x66u, 0x3Cu);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 57u : {
            // '9'
            let char_data = array<u32, 8>(0x3Cu, 0x66u, 0x66u, 0x66u, 0x3Eu, 0x06u, 0x66u, 0x3Cu);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 70u : {
            // 'F'
            let char_data = array<u32, 8>(0x7Eu, 0x60u, 0x60u, 0x7Cu, 0x60u, 0x60u, 0x60u, 0x60u);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 80u : {
            // 'P'
            let char_data = array<u32, 8>(0x7Cu, 0x66u, 0x66u, 0x7Cu, 0x60u, 0x60u, 0x60u, 0x60u);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 83u : {
            // 'S'
            let char_data = array<u32, 8>(0x3Cu, 0x66u, 0x60u, 0x3Cu, 0x06u, 0x06u, 0x66u, 0x3Cu);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 58u : {
            // ':'
            let char_data = array<u32, 8>(0x00u, 0x18u, 0x18u, 0x00u, 0x00u, 0x18u, 0x18u, 0x00u);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 46u : {
            // '.'
            let char_data = array<u32, 8>(0x00u, 0x00u, 0x00u, 0x00u, 0x00u, 0x00u, 0x18u, 0x18u);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 102u : {
            // 'f'
            let char_data = array<u32, 8>(0x1Cu, 0x30u, 0x30u, 0x7Cu, 0x30u, 0x30u, 0x30u, 0x30u);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 112u : {
            // 'p'
            let char_data = array<u32, 8>(0x00u, 0x00u, 0x7Cu, 0x66u, 0x66u, 0x7Cu, 0x60u, 0x60u);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 115u : {
            // 's'
            let char_data = array<u32, 8>(0x00u, 0x00u, 0x3Eu, 0x60u, 0x3Cu, 0x06u, 0x7Cu, 0x00u);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 97u : {
            // 'a'
            let char_data = array<u32, 8>(0x00u, 0x00u, 0x3Cu, 0x06u, 0x3Eu, 0x66u, 0x3Eu, 0x00u);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 99u : {
            // 'c'
            let char_data = array<u32, 8>(0x00u, 0x00u, 0x3Cu, 0x66u, 0x60u, 0x66u, 0x3Cu, 0x00u);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 101u : {
            // 'e'
            let char_data = array<u32, 8>(0x00u, 0x00u, 0x3Cu, 0x66u, 0x7Eu, 0x60u, 0x3Cu, 0x00u);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 105u : {
            // 'i'
            let char_data = array<u32, 8>(0x18u, 0x00u, 0x18u, 0x18u, 0x18u, 0x18u, 0x3Cu, 0x00u);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 108u : {
            // 'l'
            let char_data = array<u32, 8>(0x18u, 0x18u, 0x18u, 0x18u, 0x18u, 0x18u, 0x3Cu, 0x00u);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 114u : {
            // 'r'
            let char_data = array<u32, 8>(0x00u, 0x00u, 0x36u, 0x6Cu, 0x60u, 0x60u, 0x60u, 0x00u);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        case 116u : {
            // 't'
            let char_data = array<u32, 8>(0x18u, 0x7Eu, 0x18u, 0x18u, 0x18u, 0x1Cu, 0x00u, 0x00u);
            return f32((char_data[y] >> (7u - x)) & 1u);
        }
        default : {
            return 0.0;
            // Space or unknown character
        }
    }
}

// Extract digit from number
fn get_digit(number: u32, position: u32) -> u32 {
    let div = u32(pow(10.0, f32(position)));
    return (number / div) % 10u;
}

// Cijfer-char_code voor `value` op decimale macht `power` (2=honderdtal, 1=tiental,
// 0=eenheden), met leidende-spatie-onderdrukking: alleen tonen als value groot genoeg
// is voor die positie, eenheden altijd tonen.
fn digit_code(value: u32, power: u32) -> u32 {
    let threshold = u32(pow(10.0, f32(power)));
    if (power == 0u || value >= threshold) {
        return 48u + get_digit(value, power);
    }
    return 32u;
}

@fragment
fn main(@location(0) screen_pos: vec2<f32>) -> @location(0) vec4<f32> {
    let canvas_width = sim_params.canvas_render_width;
    let canvas_height = sim_params.canvas_render_height;

    // Convert from screen space (-1 to 1) to canvas coordinates
    let canvas_x = (screen_pos.x + 1.0) * 0.5 * canvas_width;
    let canvas_y = (1.0 - screen_pos.y) * 0.5 * canvas_height;
    // Flip Y for proper orientation

    // Text parameters
    let char_width = 12.0;
    let char_height = 16.0;
    let margin = 16.0;
    let text_y = canvas_height - margin - char_height;
    // Onderste rij — buiten beeld op het ronde productiescherm

    var text_alpha = 0.0;

    if (canvas_y >= text_y && canvas_y < text_y + char_height) {
        let rel_y = canvas_y - text_y;
        let char_y = u32(rel_y * 8.0 / char_height);

        // FPS rechtsonder: "###fps" (6 tekens, 3 cijfers met leidende-spatie-
        // onderdrukking zodat >99 fps past zonder de "fps"-suffix te verschuiven)
        let fps_int = u32(fps_data.fps);
        let fps_width = 6.0 * char_width;
        let fps_start_x = canvas_width - margin - fps_width;
        let fps_rel_x = canvas_x - fps_start_x;

        // Particle-count linksonder: "#### particles" (4 cijfers + suffix, max 9999 —
        // MAX_PARTICLES=6400)
        let particle_count = fps_data.particle_count;
        let particles_width = 14.0 * char_width;
        let particles_start_x = margin;
        let particles_rel_x = canvas_x - particles_start_x;

        if (fps_rel_x >= 0.0 && fps_rel_x < fps_width) {
            let char_index = u32(fps_rel_x / char_width);
            let char_x = u32(fps_rel_x) % u32(char_width);

            if (char_x < 8u && char_y < 8u) {
                var char_code = 32u;
                switch char_index {
                    case 0u : { char_code = digit_code(fps_int, 2u); }
                    case 1u : { char_code = digit_code(fps_int, 1u); }
                    case 2u : { char_code = digit_code(fps_int, 0u); }
                    case 3u : { char_code = 102u; }
                    // 'f'
                    case 4u : { char_code = 112u; }
                    // 'p'
                    case 5u : { char_code = 115u; }
                    // 's'
                    default : { }
                }
                text_alpha = get_char_pixel(char_code, char_x, char_y);
            }
        }
        else if (particles_rel_x >= 0.0 && particles_rel_x < particles_width) {
            let char_index = u32(particles_rel_x / char_width);
            let char_x = u32(particles_rel_x) % u32(char_width);

            if (char_x < 8u && char_y < 8u) {
                var char_code = 32u;
                switch char_index {
                    case 0u : { char_code = digit_code(particle_count, 3u); }
                    case 1u : { char_code = digit_code(particle_count, 2u); }
                    case 2u : { char_code = digit_code(particle_count, 1u); }
                    case 3u : { char_code = digit_code(particle_count, 0u); }
                    // index 4 = spatie (default 32u)
                    case 5u : { char_code = 112u; }
                    // 'p'
                    case 6u : { char_code = 97u; }
                    // 'a'
                    case 7u : { char_code = 114u; }
                    // 'r'
                    case 8u : { char_code = 116u; }
                    // 't'
                    case 9u : { char_code = 105u; }
                    // 'i'
                    case 10u : { char_code = 99u; }
                    // 'c'
                    case 11u : { char_code = 108u; }
                    // 'l'
                    case 12u : { char_code = 101u; }
                    // 'e'
                    case 13u : { char_code = 115u; }
                    // 's'
                    default : { }
                }
                text_alpha = get_char_pixel(char_code, char_x, char_y);
            }
        }
    }

    if (text_alpha > 0.0) {
        // #cccccc grijs
        return vec4<f32>(0.8, 0.8, 0.8, 0.9);
    }
    else {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        // Transparent
    }
}

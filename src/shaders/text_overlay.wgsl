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
    let text_y = canvas_height - 30.0;
    // 30 pixels from bottom

    // Calculate FPS as integer: "##fps" (5 characters)
    let fps_int = u32(fps_data.fps);

    // Text layout: "##fps" (5 characters)
    let text_width = 5.0 * char_width;
    let text_start_x = (canvas_width - text_width) * 0.5;
    // Center horizontally

    var text_alpha = 0.0;

    // Check if we're in the text area
    if (canvas_y >= text_y && canvas_y < text_y + char_height) {
        let rel_x = canvas_x - text_start_x;
        let rel_y = canvas_y - text_y;

        if (rel_x >= 0.0 && rel_x < text_width) {
            let char_index = u32(rel_x / char_width);
            let char_x = u32(rel_x) % u32(char_width);
            let char_y = u32(rel_y * 8.0 / char_height);

            if (char_x < 8u && char_y < 8u) {
                var char_code = 32u;
                // space

                switch char_index {
                    case 0u : {
                        // Tens digit of FPS
                        let tens = get_digit(fps_int, 1u);
                        if (tens > 0u) {
                            char_code = 48u + tens;
                        }
                        else {
                            char_code = 32u;
                            // space for leading zero suppression
                        }
                    }
                    case 1u : {
                        // Units digit of FPS
                        char_code = 48u + get_digit(fps_int, 0u);
                    }
                    case 2u : {
                        char_code = 102u;
                        // 'f'
                    }
                    case 3u : {
                        char_code = 112u;
                        // 'p'
                    }
                    case 4u : {
                        char_code = 115u;
                        // 's'
                    }
                    default : { }
                }

                text_alpha = get_char_pixel(char_code, char_x, char_y);
            }
        }
    }

    if (text_alpha > 0.0) {
        // White text with subtle transparency for less intrusion
        return vec4<f32>(1.0, 1.0, 1.0, 0.6);
    }
    else {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        // Transparent
    }
}

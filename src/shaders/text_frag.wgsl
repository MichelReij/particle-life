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

@group(0) @binding(0)
var<uniform> sim_params: SimParams;

// Simple 8x8 bitmap font for digits and text
// This function returns 1.0 if the pixel should be lit for the given character
fn get_char_pixel(char_code: u32, x: u32, y: u32) -> f32 {
    // Simple 8x8 bitmap font for digits 0-9 and letters F, P, S
    // Each character is represented as 8 rows of 8 bits

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
        default : {
            return 0.0;
        }
    }
}

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let canvas_width = sim_params.canvas_render_width;
    let canvas_height = sim_params.canvas_render_height;

    // Position for text at bottom center
    let text_y = canvas_height - 40.0;
    // 40 pixels from bottom
    let char_width = 10.0;
    let char_height = 12.0;

    // Simple FPS text: "FPS: XX"
    let text_start_x = (canvas_width - 6.0 * char_width) * 0.5;
    // Center 6 characters

    var text_alpha = 0.0;

    // Check if we're in the text area
    if (frag_coord.y >= text_y && frag_coord.y < text_y + char_height) {
        let rel_x = frag_coord.x - text_start_x;
        let rel_y = frag_coord.y - text_y;

        if (rel_x >= 0.0 && rel_x < 6.0 * char_width) {
            let char_index = u32(rel_x / char_width);
            let char_x = u32(rel_x) % u32(char_width);
            let char_y = u32(rel_y * 8.0 / char_height);

            if (char_x < 8u && char_y < 8u) {
                var char_code = 32u;
                // space

                switch char_index {
                    case 0u : {
                        char_code = 70u;
                    }
                    // 'F'
                    case 1u : {
                        char_code = 80u;
                    }
                    // 'P'
                    case 2u : {
                        char_code = 83u;
                    }
                    // 'S'
                    case 3u : {
                        char_code = 58u;
                    }
                    // ':' (we'll treat as space)
                    case 4u : {
                        // First digit of FPS (simplified: always show 6)
                        char_code = 54u;
                        // '6'
                    }
                    case 5u : {
                        // Second digit of FPS (simplified: always show 0)
                        char_code = 48u;
                        // '0'
                    }
                    default : { }
                }

                text_alpha = get_char_pixel(char_code, char_x, char_y);
            }
        }
    }

    if (text_alpha > 0.0) {
        return vec4<f32>(1.0, 1.0, 1.0, 0.8);
        // White text with slight transparency
    }
    else {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
        // Transparent
    }
}

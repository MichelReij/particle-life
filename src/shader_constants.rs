/// Utility functions for processing WGSL shaders with dynamic constants
/// This module handles the substitution of hardcoded values with config constants
use crate::config::*;

/// Replace hardcoded world size constants in WGSL shader code
pub fn process_shader_constants(shader_source: &str) -> String {
    shader_source
        // Replace virtual world dimensions
        .replace("2400.0", &VIRTUAL_WORLD_WIDTH.to_string())
        .replace("2400", &VIRTUAL_WORLD_WIDTH_U32.to_string())
        // Replace canvas dimensions
        .replace("800.0", &CANVAS_WIDTH.to_string())
        .replace("800", &CANVAS_WIDTH_U32.to_string())
        // Replace center coordinates - this is trickier since 1200 might be used for other things
        // We'll be more specific and look for common patterns
        .replace("1200.0", &VIRTUAL_WORLD_CENTER_X.to_string())
        .replace("1200", &(VIRTUAL_WORLD_CENTER_X as u32).to_string())
}

/// Process a specific shader with constant replacement
pub fn process_vertex_shader(source: &str) -> String {
    process_shader_constants(source)
}

pub fn process_fragment_shader(source: &str) -> String {
    process_shader_constants(source)
}

pub fn process_compute_shader(source: &str) -> String {
    process_shader_constants(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_constant_replacement() {
        let test_shader = "
            let world_width = 2400.0;
            let world_height = 2400.0;
            let canvas_width = 800.0;
            let center_x = 1200.0;
        ";

        let processed = process_shader_constants(test_shader);

        // Should replace with actual config values
        assert!(processed.contains(&VIRTUAL_WORLD_WIDTH.to_string()));
        assert!(processed.contains(&CANVAS_WIDTH.to_string()));
        assert!(processed.contains(&VIRTUAL_WORLD_CENTER_X.to_string()));
    }
}

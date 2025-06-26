@group(0) @binding(0)
var scene_sampler: sampler;
@group(0) @binding(1)
var scene_texture: texture_2d<f32>;

struct ZoomUniforms {
    zoom_level: f32,
    center_x: f32,
    center_y: f32,
    native_gamma_correction: f32,
    // 1.0 for native gamma correction, 0.0 for browser (no extra correction)
}

;

@group(0) @binding(2)
var<uniform> zoom_uniforms: ZoomUniforms;

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    // frag_coord.xy is in final canvas coordinates (0 to 800)
    // We need to map this to a portion of the 2400x2400px rendered texture

    // Calculate the size of the area to sample from the 2400x2400px texture
    // At 1x zoom: sample full 2400x2400px
    // At 2x zoom: sample center 1200x1200px
    // At 3x zoom: sample center 800x800px
    // At 6x zoom: sample center 400x400px
    let sample_size = 2400.0 / zoom_uniforms.zoom_level;

    // Use the dynamic zoom center from uniforms
    let texture_center = vec2<f32>(zoom_uniforms.center_x, zoom_uniforms.center_y);

    // Calculate the top-left corner of our sample area
    let sample_top_left = texture_center - vec2<f32>(sample_size / 2.0, sample_size / 2.0);

    // Map fragment coordinates (0-800) to sample area coordinates
    let sample_pos = sample_top_left + (frag_coord.xy / 800.0) * sample_size;

    // Convert to UV coordinates for the 2400x2400px texture
    let sample_uv = sample_pos / vec2<f32>(2400.0, 2400.0);

    // Sample the scene texture
    let scene_color = textureSample(scene_texture, scene_sampler, sample_uv);

    // Add vignette effect
    // Convert fragment coordinates to UV coordinates in final canvas (0-1)
    let canvas_uv = frag_coord.xy / vec2<f32>(800.0, 800.0);

    // Center the UV coordinates around (0, 0) for vignette calculation
    let centered_uv = canvas_uv - 0.5;
    let dist_from_center = length(centered_uv);

    // Vignette parameters (based on canvas size)
    let vignette_radius = 0.7;
    // Relative to canvas size
    let vignette_softness = 0.4;
    // Softness of the vignette transition (increased for gentler fade)
    let max_vignette_alpha = 0.33;
    // Maximum vignette opacity

    // Calculate vignette alpha using smoothstep for smooth transition
    let vignette_alpha = smoothstep(vignette_radius - vignette_softness, vignette_radius, dist_from_center) * max_vignette_alpha;

    // Apply vignette (darken towards edges)
    let final_rgb = scene_color.rgb * (1.0 - vignette_alpha);

    // Platform-specific gamma correction for color matching
    // Native gets extra gamma correction to match browser appearance
    if (zoom_uniforms.native_gamma_correction > 0.5) {
        // Apply gamma correction using the uniform value
        let gamma_corrected = pow(final_rgb, vec3<f32>(zoom_uniforms.native_gamma_correction));
        return vec4<f32>(gamma_corrected, 1.0);
    }
    else {
        return vec4<f32>(final_rgb, 1.0);
    }
}
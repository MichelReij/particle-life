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
    virtual_world_width: f32,
    virtual_world_height: f32,
    canvas_width: f32,
    canvas_height: f32,
}

;

@group(0) @binding(2)
var<uniform> zoom_uniforms: ZoomUniforms;

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    // frag_coord.xy is in canvas coordinates (0 to canvas_width/height)
    // Since we now render directly to canvas size, we can sample directly from the scene texture
    // The zoom and viewport logic is already handled in the vertex shader

    // Convert fragment coordinates to UV coordinates for direct sampling
    let canvas_uv = frag_coord.xy / vec2<f32>(zoom_uniforms.canvas_width, zoom_uniforms.canvas_height);

    // Sample the scene texture directly (no coordinate transformation needed)
    let scene_color = textureSample(scene_texture, scene_sampler, canvas_uv);

    // Add vignette effect
    // canvas_uv is already calculated above for direct sampling

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
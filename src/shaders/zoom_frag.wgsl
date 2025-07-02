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
    // frag_coord.xy is in pixel coordinates (0 to canvas_width/height)
    // We need to sample from the central region of the larger fisheye buffer

    // Convert fragment coordinates to normalized UV coordinates (0.0 to 1.0)
    let canvas_uv = frag_coord.xy / vec2<f32>(zoom_uniforms.canvas_width, zoom_uniforms.canvas_height);

    // Get fisheye buffer dimensions (hardcoded constants from config.rs)
    let fisheye_buffer_width = 1404.0;
    let fisheye_buffer_height = 1404.0;
    let crop_offset_x = 162.0;
    // (1404 - 1080) / 2
    let crop_offset_y = 162.0;
    // (1404 - 1080) / 2

    // Map canvas UV (0,0 to 1,1) to the central crop region of the fisheye buffer
    // We want to sample the center 1080x1080 region from the 1404x1404 fisheye buffer
    // Center of fisheye buffer: (702, 702)
    // Crop region: from (162, 162) to (1242, 1242)

    // Simple direct mapping: canvas UV (0,0 to 1,1) to crop region (162,162 to 1242,1242)
    // Start UV: (162/1404, 162/1404) = (0.1154, 0.1154)
    // End UV: (1242/1404, 1242/1404) = (0.8846, 0.8846)
    let crop_start_uv = vec2<f32>(162.0 / 1404.0, 162.0 / 1404.0);
    let crop_end_uv = vec2<f32>(1242.0 / 1404.0, 1242.0 / 1404.0);

    // Linear interpolation from crop start to crop end based on canvas UV
    let fisheye_uv = crop_start_uv + canvas_uv * (crop_end_uv - crop_start_uv);

    // Sample from the central region of the fisheye buffer
    let scene_color = textureSample(scene_texture, scene_sampler, fisheye_uv);

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
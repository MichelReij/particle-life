@group(0) @binding(0)
var scene_sampler: sampler;
@group(0) @binding(1)
var scene_texture: texture_2d<f32>;

struct ZoomUniforms {
    zoom_level: f32,
    center_x: f32,
    center_y: f32,
    native_gamma_correction: f32,
    virtual_world_width: f32,
    virtual_world_height: f32,
    canvas_width: f32,
    canvas_height: f32,
    fisheye_buffer_width: f32,
    fisheye_buffer_height: f32,
    crop_offset_x: f32,
    crop_offset_y: f32,
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

    // Get fisheye buffer dimensions from uniforms (scale proportionally with canvas)
    let fw = zoom_uniforms.fisheye_buffer_width;
    let fh = zoom_uniforms.fisheye_buffer_height;
    let ox = zoom_uniforms.crop_offset_x;
    let oy = zoom_uniforms.crop_offset_y;
    let crop_start_uv = vec2<f32>(ox / fw, oy / fh);
    let crop_end_uv   = vec2<f32>((fw - ox) / fw, (fh - oy) / fh);

    // Linear interpolation from crop start to crop end based on canvas UV
    let fisheye_uv = crop_start_uv + canvas_uv * (crop_end_uv - crop_start_uv);

    // Sample from the central region of the fisheye buffer
    let scene_color = textureSample(scene_texture, scene_sampler, fisheye_uv);

    var final_rgb = scene_color.rgb;

    // Hard circular cutoff at the true visible edge (canvas is square, circle
    // is inscribed: radius = canvas_width/2). Particle glow halos are culled
    // vertex-side with a margin (see vert.wgsl's CIRCULAR CULLING) that lets
    // glow render slightly past this radius — invisible against a bright
    // background, but a very visible faint ring against the near-black night
    // background. This is the final pass before the swapchain (both native
    // and WASM), so clamping here guarantees nothing can ever bleed past the
    // true edge regardless of any upstream culling margin.
    let center = vec2<f32>(zoom_uniforms.canvas_width, zoom_uniforms.canvas_height) * 0.5;
    let dist_from_center = distance(frag_coord.xy, center);
    let visible_radius = zoom_uniforms.canvas_width * 0.5;
    if (dist_from_center > visible_radius) {
        final_rgb = vec3<f32>(0.0, 0.0, 0.0);
    }

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
// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// Corner overlay shader: samples a texture and blends it at a fixed pixel position.
// Used for copyright/CC notices in the corners of the canvas (hidden by circular viewport).

struct OverlayUniforms {
    // Top-left pixel position of the overlay in the fisheye buffer
    x: f32,
    y: f32,
    // Size in pixels in the fisheye buffer
    width: f32,
    height: f32,
}

@group(0) @binding(0) var overlay_sampler: sampler;
@group(0) @binding(1) var overlay_texture: texture_2d<f32>;
@group(0) @binding(2) var<uniform> overlay_uniforms: OverlayUniforms;

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let px = frag_coord.x;
    let py = frag_coord.y;

    let x0 = overlay_uniforms.x;
    let y0 = overlay_uniforms.y;
    let x1 = x0 + overlay_uniforms.width;
    let y1 = y0 + overlay_uniforms.height;

    if (px < x0 || px >= x1 || py < y0 || py >= y1) {
        discard;
    }

    let u = (px - x0) / overlay_uniforms.width;
    let v = (py - y0) / overlay_uniforms.height;

    return textureSample(overlay_texture, overlay_sampler, vec2<f32>(u, v));
}

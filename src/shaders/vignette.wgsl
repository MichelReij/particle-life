// Vignette pass: V-blur (separable with blur_h output) + mix sharp/blurred + darken.
// binding 1 = h-blurred texture, binding 2 = sharp (post) texture.

@group(0) @binding(0) var s: sampler;
@group(0) @binding(1) var blur_h_tex: texture_2d<f32>; // horizontally blurred
@group(0) @binding(2) var sharp_tex:  texture_2d<f32>; // original sharp frame

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(blur_h_tex));
    let uv   = frag_coord.xy / dims;
    let step = vec2<f32>(0.0, 1.3 / dims.y); // 3px step, vertical only

    // V-blur: same 9-tap Gaussian as blur_h.wgsl
    var blurred = textureSample(blur_h_tex, s, uv)              * 0.2742;
    blurred    += textureSample(blur_h_tex, s, uv + step * 1.0) * 0.2417;
    blurred    += textureSample(blur_h_tex, s, uv - step * 1.0) * 0.2417;
    blurred    += textureSample(blur_h_tex, s, uv + step * 2.0) * 0.0606;
    blurred    += textureSample(blur_h_tex, s, uv - step * 2.0) * 0.0606;
    blurred    += textureSample(blur_h_tex, s, uv + step * 3.0) * 0.0054;
    blurred    += textureSample(blur_h_tex, s, uv - step * 3.0) * 0.0054;
    blurred    += textureSample(blur_h_tex, s, uv + step * 4.0) * 0.0002;
    blurred    += textureSample(blur_h_tex, s, uv - step * 4.0) * 0.0002;

    let sharp = textureSample(sharp_tex, s, uv);

    let dist     = length(uv - vec2<f32>(0.5));
    let t        = smoothstep(0.05, 0.5, dist);
    let dark_t   = smoothstep(0.05, 0.5, dist); // wider transition for darkening
    let blur_t   = t * 0.6; // cap blur blend at 60% so corners stay readable

    return vec4<f32>(mix(sharp, blurred, blur_t).rgb * mix(1.0, 0.95, dark_t), 1.0);
}

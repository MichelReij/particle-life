// Horizontal Gaussian blur pass (separable). Output feeds into vignette.wgsl (V-blur).

@group(0) @binding(0) var s: sampler;
@group(0) @binding(1) var tex: texture_2d<f32>;

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(tex));
    let uv   = frag_coord.xy / dims;
    let step = vec2<f32>(1.5 / dims.x, 0.0); // 3px step, horizontal only

    // 9-tap Gaussian, sigma ≈ 4px (weights: 1, 0.6065, 0.1353 at 0, 1, 2 steps)
    var c = textureSample(tex, s, uv)                      * 0.2742;
    c    += textureSample(tex, s, uv + step * 1.0)         * 0.2417;
    c    += textureSample(tex, s, uv - step * 1.0)         * 0.2417;
    c    += textureSample(tex, s, uv + step * 2.0)         * 0.0606;
    c    += textureSample(tex, s, uv - step * 2.0)         * 0.0606;
    c    += textureSample(tex, s, uv + step * 3.0)         * 0.0054;
    c    += textureSample(tex, s, uv - step * 3.0)         * 0.0054;
    c    += textureSample(tex, s, uv + step * 4.0)         * 0.0002;
    c    += textureSample(tex, s, uv - step * 4.0)         * 0.0002;
    return c;
}

// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// Positie-indicator midden-onder: een donkere, halftransparante cirkel voor de hele
// virtuele wereld met daarop een lichtere cirkel voor wat je nu ziet. Hoe verder
// ingezoomd, hoe kleiner die lichte cirkel. Alles in schermpixels, dus scherp.

struct IndicatorUniforms {
    // Middelpunt en straal van de wereldcirkel (px)
    center: vec2<f32>,
    radius: f32,
    // Zichtbaarheid [0, 1]: uit bij volledig uitgezoomd
    alpha: f32,
    // Middelpunt van het beeld t.o.v. het wereldmidden, in wereldstralen
    view: vec2<f32>,
    // Straal van het beeld als deel van de wereldstraal (1 / zoom)
    view_radius: f32,
    _pad: f32,
}

@group(0) @binding(0) var<uniform> u: IndicatorUniforms;

const WORLD_COLOR: vec3<f32> = vec3<f32>(0.02, 0.04, 0.06);
const WORLD_ALPHA: f32 = 0.5;
const RIM_ALPHA: f32 = 0.22;
const VIEW_COLOR: vec3<f32> = vec3<f32>(0.85, 0.93, 0.96);
const VIEW_ALPHA: f32 = 0.45;
// Kleinste straal van de beeldcirkel (px), zodat hij ver ingezoomd zichtbaar blijft
const VIEW_MIN_PX: f32 = 2.5;

// Laag `c` met dekking `a` over (rgb, alpha) leggen, niet-voorvermenigvuldigd
fn over(base: vec4<f32>, c: vec3<f32>, a: f32) -> vec4<f32> {
    let out_a = a + base.a * (1.0 - a);
    if (out_a <= 0.0) {
        return vec4<f32>(0.0);
    }
    let rgb = (c * a + base.rgb * base.a * (1.0 - a)) / out_a;
    return vec4<f32>(rgb, out_a);
}

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let p = frag_coord.xy;
    let d = length(p - u.center);
    if (u.alpha <= 0.0 || d > u.radius + 1.0) {
        discard;
    }
    // Wereld: gevulde schijf met een dun lichter randje (1 px anti-aliasing)
    let world = 1.0 - smoothstep(u.radius - 1.0, u.radius, d);
    let rim = world * (1.0 - smoothstep(0.0, 1.0, abs(d - (u.radius - 1.0))));
    var col = over(vec4<f32>(0.0), WORLD_COLOR, WORLD_ALPHA * world);
    col = over(col, VIEW_COLOR, RIM_ALPHA * rim);

    // Beeld
    let vc = u.center + u.view * u.radius;
    let vr = max(u.view_radius * u.radius, VIEW_MIN_PX);
    let vis = 1.0 - smoothstep(vr - 1.0, vr, length(p - vc));
    col = over(col, VIEW_COLOR, VIEW_ALPHA * vis);

    let a = col.a * u.alpha;
    if (a < 0.002) {
        discard;
    }
    return vec4<f32>(col.rgb, a);
}

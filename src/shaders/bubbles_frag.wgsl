// Opstijgende bubbels van hydrothermale uitbarstingen (alleen diepzee / HTV).
// Gepland in eruptions.rs: dezelfde uitbarsting stuurt ook het vents-geluid en
// de lava op de neopixel-ring. Elke uitbarsting laat zolang de straal spuit
// bubbels los uit een bron in de onderste helft; ze stijgen schommelend op,
// waaieren uit, worden groter (minder druk) en vervagen. Op de bron gloeit een
// rood-oranje waas, met dezelfde kleuren en timing als de lava op de ring
// (NeoPixelDriver). Getekend in wereldcoördinaten, dus alles zoomt en pant mee
// met de deeltjes.

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
    lenia_enabled: u32,
    lenia_growth_mu: f32,
    lenia_growth_sigma: f32,
    lenia_kernel_radius: f32,
    lightning_frequency: f32,
    lightning_intensity: f32,
    lightning_duration: f32,
    transition_active: u32,
    transition_start_time: f32,
    transition_duration: f32,
    transition_start_count: u32,
    transition_end_count: u32,
    transition_is_grow: u32,
    spatial_grid_enabled: u32,
    spatial_grid_cell_size: f32,
    spatial_grid_width: u32,
    spatial_grid_height: u32,
    viewport_center_x: f32,
    viewport_center_y: f32,
    viewport_width: f32,
    viewport_height: f32,
    viewport_radius: f32,
    night_alpha: f32,
    wlp_start_time: f32,
    _viewport_padding3: f32,
}

struct Eruptions {
    // Per uitbarsting drie vec4's: (bron x, bron y, begin, grootte),
    // (spuitduur, seed, hitte, actief) en (opkomen, aanhouden, uitdoven,
    // helderheid) — zie EruptionScheduler::gpu_data()
    e: array<vec4<f32>, 12>,
}

@group(0) @binding(0)
var<uniform> sim_params: SimParams;
@group(0) @binding(1)
var<uniform> eruptions: Eruptions;

const MAX_ERUPTIONS: u32 = 4u;
const MAX_BUBBLES: u32 = 88u;
// Moet gelijk blijven aan BUBBLE_RISE_TIME in eruptions.rs
const RISE_TIME: f32 = 6.0;
// Snelste stijgsnelheid (wereld-UV per seconde) en grootste straal, voor de
// snelle uitsluiting van pixels buiten de pluim
const MAX_SPEED: f32 = 0.06;
const MAX_RADIUS: f32 = 0.05;
// Hoe ver bellen hooguit van de bron wegwaaieren (wereld-UV)
const MAX_SPREAD: f32 = 0.22;
const BUBBLE_COLOR: vec3<f32> = vec3<f32>(0.95, 0.98, 1.0);
// Lava-kleuren van de ring (LAVA_RED/YELLOW/LIGHTYELLOW in NeoPixelDriver.h)
const LAVA_RED: vec3<f32> = vec3<f32>(1.0, 0.098, 0.0);
const LAVA_YELLOW: vec3<f32> = vec3<f32>(1.0, 0.47, 0.0);
const LAVA_LIGHTYELLOW: vec3<f32> = vec3<f32>(1.0, 0.725, 0.235);
// Straal van de gloed (wereld-UV) en zijn maximale dekking
const GLOW_RADIUS: f32 = 0.075;
const GLOW_ALPHA: f32 = 0.75;

// Lava-kleur bij hitte [0, 1]: rood → geel → lichtgeel, zoals op de ring
fn lava_color(heat: f32) -> vec3<f32> {
    if heat <= 0.5 {
        return mix(LAVA_RED, LAVA_YELLOW, heat / 0.5);
    }
    return mix(LAVA_YELLOW, LAVA_LIGHTYELLOW, (heat - 0.5) / 0.5);
}

// Goedkope integer-hash → [0, 1)
fn hash(n: u32) -> f32 {
    var x = n * 747796405u + 2891336453u;
    x = ((x >> ((x >> 28u) + 4u)) ^ x) * 277803737u;
    x = (x >> 22u) ^ x;
    return f32(x) / 4294967296.0;
}

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    // Pixel → wereld-UV (de scènetextuur is canvas × 1.3, zie grid_frag.wgsl)
    let buf = vec2<f32>(sim_params.canvas_render_width, sim_params.canvas_render_height) * 1.3;
    let vp = vec2<f32>(sim_params.viewport_width, sim_params.viewport_height);
    let vp_origin = vec2<f32>(sim_params.viewport_center_x, sim_params.viewport_center_y) - vp * 0.5;
    let world = vec2<f32>(sim_params.virtual_world_width, sim_params.virtual_world_height);
    let uv = (vp_origin + frag_coord.xy / buf * vp) / world;
    // Eén pixel in wereld-UV, voor zachte randen op elke zoom
    let px = vp.x / (world.x * buf.x);

    var color = vec3<f32>(0.0);
    var alpha = 0.0;

    for (var i = 0u; i < MAX_ERUPTIONS; i++) {
        let a = eruptions.e[i * 3u];
        let b = eruptions.e[i * 3u + 1u];
        let g = eruptions.e[i * 3u + 2u];
        if b.w < 0.5 {
            continue;
        }
        let src = a.xy;
        let t = sim_params.time - a.z;
        let size = a.w;
        let emit = max(b.x, 0.1);
        let seed = u32(b.y * 65536.0);

        // Gloed op de bron: opkomen, aanhouden, uitdoven (zoals de lava op de
        // ring), en tijdens het opkomen iets uitdijend. Het hart is heter dan de
        // rand, net als bij de ring de oorsprong heter is dan de uiteinden.
        let ramp = max(g.x, 0.05);
        let fade_t = max(g.z, 0.05);
        var env = 0.0;
        if t < ramp {
            env = smoothstep(0.0, 1.0, t / ramp);
        } else if t < ramp + g.y {
            env = 1.0;
        } else {
            env = 1.0 - smoothstep(0.0, 1.0, (t - ramp - g.y) / fade_t);
        }
        if env > 0.0 {
            let glow_r = GLOW_RADIUS * (0.7 + 0.6 * size) * (0.6 + 0.4 * min(t / ramp, 1.0));
            let gd = length(uv - src) / glow_r;
            let core = exp(-gd * gd * 1.5);
            let ga = GLOW_ALPHA * g.w * env * core;
            if ga > 0.002 {
                let c = lava_color(b.z * core);
                let na = ga + alpha * (1.0 - ga);
                color = (c * ga + color * alpha * (1.0 - ga)) / max(na, 1e-4);
                alpha = na;
            }
        }

        // Snelle uitsluiting: buiten de pluim van deze uitbarsting niets te doen
        let reach = MAX_SPREAD * min(t, RISE_TIME) / RISE_TIME + 0.03 + MAX_RADIUS;
        if abs(uv.x - src.x) > reach || uv.y > src.y + reach
            || uv.y < src.y - MAX_SPEED * min(t, RISE_TIME) - reach {
            continue;
        }

        let count = u32(mix(32.0, f32(MAX_BUBBLES), size));
        for (var k = 0u; k < count; k++) {
            let h = seed * 131u + k * 7u;
            // Geboren zolang de straal spuit, iets vaker aan het begin
            let birth = emit * pow(hash(h), 1.3);
            let age = t - birth;
            // Levensduur sterk wisselend: veel bellen kort, een deel haalt de
            // volle RISE_TIME
            let life = RISE_TIME * mix(0.3, 1.0, pow(hash(h + 1u), 0.7));
            if age < 0.0 || age > life {
                continue;
            }
            // Veel kleine en weinig grote bellen; grote stijgen sneller
            let r0 = mix(0.0022, 0.009, pow(hash(h + 2u), 2.0)) * mix(0.8, 1.2, size);
            // Langzaam naar boven: ze stijgen vooral naar de kijker toe
            let speed = 0.02 + 2.5 * r0;
            // Snel groter tijdens het stijgen: ze komen naar je toe (en de druk
            // neemt af)
            let radius = r0 * (1.0 + mix(1.2, 3.5, hash(h + 8u)) * age / RISE_TIME);
            // Naar de kijker toe stijgen = in perspectief van de bron af
            // waaieren, steeds sneller naarmate de bel dichterbij komt; plus een
            // langzame drift naar boven en wat schommelen
            let dir_a = hash(h + 3u) * 6.283;
            let out = mix(0.15, 1.0, hash(h + 6u)) * MAX_SPREAD * 0.5
                * (age / RISE_TIME) * (1.0 + age / RISE_TIME);
            let wobble = sin(age * mix(2.5, 5.5, hash(h + 4u)) + hash(h + 5u) * 6.283)
                * 0.003 * (1.0 + 0.3 * age);
            let pos = src + vec2<f32>(cos(dir_a), sin(dir_a)) * out
                + vec2<f32>(wobble, -speed * age);

            let d = length(uv - pos);
            // Scherp begrensd (vervaging maakt er rook of mist van): een vage
            // kern, een dunne iets lichtere rand en een klein lichtpuntje. Pas in
            // het laatste stukje van haar leven vervaagt een bel terwijl ze
            // uitdooft — een luchtbel in water knapt niet, ze lost geleidelijk op
            let dissolve = smoothstep(0.8 * life, life, age);
            // Rand en zachte overgang in schermpixels, niet in verhouding tot de
            // bel: ingezoomd wordt de bel groter maar blijft de rand dun
            let aa = px + radius * 0.5 * dissolve;
            if d > radius + aa {
                continue;
            }
            let disk = 1.0 - smoothstep(radius - aa, radius, d);
            // (randen op volgorde houden: smoothstep met omgekeerde randen is ongedefinieerd)
            let rim = disk * smoothstep(radius - 2.0 * px - aa, radius - 0.5 * aa, d);
            let spot = 1.0 - smoothstep(radius * 0.08, radius * 0.25, length(uv - pos + vec2<f32>(radius * 0.35, radius * 0.35)));
            let fade = smoothstep(0.0, 0.25, age) * (1.0 - smoothstep(0.75 * life, life, age));
            let ba = clamp(0.08 * disk + 0.3 * rim + 0.35 * spot * disk, 0.0, 0.45) * fade;

            // Bel "over" wat er al ligt (gloed of andere bellen)
            let na = ba + alpha * (1.0 - ba);
            color = (BUBBLE_COLOR * ba + color * alpha * (1.0 - ba)) / max(na, 1e-4);
            alpha = na;
        }
    }

    return vec4<f32>(color, alpha);
}

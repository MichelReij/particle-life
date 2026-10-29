// Opstijgende bubbels van hydrothermale uitbarstingen (alleen diepzee / HTV).
// Gepland in eruptions.rs: dezelfde uitbarsting stuurt ook het vents-geluid en
// de lava op de neopixel-ring. Elke uitbarsting laat zolang de straal spuit
// bubbels los uit een bron in de onderste twee derde; ze stijgen langzaam op,
// waaieren in perspectief uit (ze komen naar de kijker toe), worden groter en
// lossen op. Op de bron gloeit een rood-oranje waas, met dezelfde kleuren en
// timing als de lava op de ring (NeoPixelDriver).
//
// Elke bel (en elke gloed) is een eigen vierkantje, net als de deeltjes: een
// pixel kost alleen werk als er werkelijk een bel overheen ligt. Eerder liep
// elke pixel binnen de pluim alle bellen langs; ingezoomd beslaat de pluim het
// hele scherm en zakte de framerate daardoor diep weg.
// Getekend in wereldcoördinaten, dus alles zoomt en pant mee met de deeltjes.

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

// Moeten gelijk blijven aan MAX_ERUPTIONS / MAX_BUBBLES in eruptions.rs: de
// renderer tekent MAX_ERUPTIONS gloeden plus MAX_ERUPTIONS × MAX_BUBBLES bellen
const MAX_ERUPTIONS: u32 = 4u;
const MAX_BUBBLES: u32 = 88u;
// Moet gelijk blijven aan BUBBLE_RISE_TIME in eruptions.rs
const RISE_TIME: f32 = 6.0;
// Hoe ver bellen hooguit van de bron wegwaaieren (wereld-UV)
const MAX_SPREAD: f32 = 0.22;
const BUBBLE_COLOR: vec3<f32> = vec3<f32>(0.95, 0.98, 1.0);
// Donkere band binnen de rand: licht van achter de bel buigt weg (breking)
const BUBBLE_SHADE: vec3<f32> = vec3<f32>(0.02, 0.05, 0.09);
// Richting van het licht (van boven, linksboven in beeld; y wijst naar beneden)
const LIGHT_DIR: vec2<f32> = vec2<f32>(-0.55, -0.83);
// Bellen komen in trosjes van zoveel tegelijk vrij
const CLUSTER: u32 = 3u;
// Straal van de kleinste en grootste bel (wereld-UV)
const R_MIN: f32 = 0.0018;
const R_MAX: f32 = 0.013;
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

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    // Afstand tot het midden van de bel of gloed (wereld-UV)
    @location(0) local: vec2<f32>,
    // Gloed: (0, straal, dekking, hitte). Bel: (1, straal, pixel, oplossen)
    @location(1) @interpolate(flat) p0: vec4<f32>,
    // Bel: (dekking door in- en uitfaden, fase van het trillen, vervorming, stand)
    @location(2) @interpolate(flat) p1: vec4<f32>,
}

fn hidden() -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(2.0, 2.0, 0.0, 1.0); // buiten beeld: wordt weggeknipt
    out.local = vec2<f32>(0.0);
    out.p0 = vec4<f32>(0.0);
    out.p1 = vec4<f32>(0.0);
    return out;
}

// Envelop van de gloed: opkomen, aanhouden, uitdoven (zoals de lava op de ring)
fn glow_envelope(t: f32, ramp: f32, hold: f32, fade_t: f32) -> f32 {
    if t < ramp {
        return smoothstep(0.0, 1.0, t / ramp);
    } else if t < ramp + hold {
        return 1.0;
    }
    return 1.0 - smoothstep(0.0, 1.0, (t - ramp - hold) / fade_t);
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VertexOut {
    // Eerst de gloeden (onder), dan de bellen
    var i = ii;
    var k = 0u;
    let is_glow = ii < MAX_ERUPTIONS;
    if !is_glow {
        i = (ii - MAX_ERUPTIONS) / MAX_BUBBLES;
        k = (ii - MAX_ERUPTIONS) % MAX_BUBBLES;
    }
    let a = eruptions.e[i * 3u];
    let b = eruptions.e[i * 3u + 1u];
    let g = eruptions.e[i * 3u + 2u];
    if b.w < 0.5 {
        return hidden();
    }

    let vp = vec2<f32>(sim_params.viewport_width, sim_params.viewport_height);
    let vp_origin = vec2<f32>(sim_params.viewport_center_x, sim_params.viewport_center_y) - vp * 0.5;
    let world = vec2<f32>(sim_params.virtual_world_width, sim_params.virtual_world_height);
    // Eén pixel in wereld-UV (de scènetextuur is canvas × 1.3, zie grid_frag.wgsl)
    let px = vp.x / (world.x * sim_params.canvas_render_width * 1.3);

    let src = a.xy;
    let t = sim_params.time - a.z;
    let size = a.w;

    var center: vec2<f32>;
    var half: f32;
    var out: VertexOut;
    if is_glow {
        // Het hart is heter dan de rand, net als bij de ring de oorsprong heter
        // is dan de uiteinden; tijdens het opkomen dijt de gloed iets uit
        let ramp = max(g.x, 0.05);
        let env = glow_envelope(t, ramp, g.y, max(g.z, 0.05));
        if env <= 0.0 {
            return hidden();
        }
        let glow_r = GLOW_RADIUS * (0.7 + 0.6 * size) * (0.6 + 0.4 * min(t / ramp, 1.0));
        center = src;
        half = glow_r * 2.2; // exp(-1.5 × 2.2²) ≈ 0: daarbuiten is niets meer te zien
        out.p0 = vec4<f32>(0.0, glow_r, GLOW_ALPHA * g.w * env, b.z);
        out.p1 = vec4<f32>(0.0);
    } else {
        let emit = max(b.x, 0.1);
        let count = u32(mix(32.0, f32(MAX_BUBBLES), size));
        if k >= count {
            return hidden();
        }
        let seed = u32(b.y * 65536.0);
        let h = seed * 131u + k * 7u;
        // Trosje: de bellen ervan komen kort na elkaar vrij en stijgen ongeveer
        // dezelfde kant op, als een snoer achter elkaar aan
        let hc = seed * 131u + (k / CLUSTER) * 7u + 100003u;
        // Geboren zolang de straal spuit, iets vaker aan het begin
        let birth = emit * pow(hash(hc), 1.3) + 0.15 * hash(h + 9u);
        let age = t - birth;
        // Levensduur sterk wisselend: veel bellen kort, een deel haalt de
        // volle RISE_TIME
        let life = RISE_TIME * mix(0.3, 1.0, pow(hash(h + 1u), 0.7));
        if age < 0.0 || age > life {
            return hidden();
        }
        // Veel kleine en een enkele grote bel; grote stijgen sneller
        let r0 = mix(R_MIN, R_MAX, pow(hash(h + 2u), 3.2)) * mix(0.8, 1.2, size);
        // 0 = klein (bolrond, stijgt recht), 1 = groot (vervormt en slingert)
        let big = smoothstep(0.0035, 0.011, r0);
        // Langzaam naar boven: ze stijgen vooral naar de kijker toe
        let speed = 0.02 + 2.5 * r0;
        // Snel groter tijdens het stijgen: ze komen naar je toe (en de druk
        // neemt af)
        let radius = r0 * (1.0 + mix(1.2, 3.5, hash(h + 8u)) * age / RISE_TIME);
        // Naar de kijker toe stijgen = in perspectief van de bron af
        // waaieren, steeds sneller naarmate de bel dichterbij komt; plus een
        // langzame drift naar boven en wat schommelen
        let dir_a = hash(hc + 3u) * 6.283 + (hash(h + 3u) - 0.5) * 0.6;
        let spread = mix(0.15, 1.0, hash(h + 6u)) * MAX_SPREAD * 0.5
            * (age / RISE_TIME) * (1.0 + age / RISE_TIME);
        // Zigzaggen of spiraalsgewijs slingeren: kleine bellen nauwelijks, grote
        // in een wijdere en tragere baan; elke bel met een eigen richting
        let swing_w = mix(6.0, 2.6, big) * mix(0.8, 1.2, hash(h + 4u));
        let swing_ph = age * swing_w + hash(h + 5u) * 6.283;
        let swing_r = mix(0.0006, 0.007, big) * (0.5 + 0.5 * min(age, 1.0));
        let flat_spiral = mix(0.15, 1.0, hash(h + 10u)); // 0.15: zigzag, 1: spiraal
        let swing_a = hash(h + 11u) * 6.283;
        let sw = vec2<f32>(cos(swing_ph), flat_spiral * sin(swing_ph)) * swing_r;
        let wobble = vec2<f32>(sw.x * cos(swing_a) - sw.y * sin(swing_a),
                               sw.x * sin(swing_a) + sw.y * cos(swing_a));
        center = src + vec2<f32>(cos(dir_a), sin(dir_a)) * spread
            + wobble + vec2<f32>(0.0, -speed * age);
        // Pas in het laatste stukje van haar leven vervaagt een bel terwijl ze
        // uitdooft — een luchtbel in water knapt niet, ze lost geleidelijk op
        let dissolve = smoothstep(0.8 * life, life, age);
        let fade = smoothstep(0.0, 0.25, age) * (1.0 - smoothstep(0.75 * life, life, age));
        // Grote bellen zijn nooit rond: afgeplat en trillend van vorm
        let deform = mix(0.015, 0.12, big) * mix(0.7, 1.3, hash(h + 12u));
        let tremble = age * mix(16.0, 7.0, big) + hash(h + 13u) * 6.283;
        half = radius * (1.0 + 0.5 * dissolve) * (1.0 + 1.5 * deform) + 2.0 * px;
        out.p0 = vec4<f32>(1.0, radius, px, dissolve);
        out.p1 = vec4<f32>(fade, tremble, deform, hash(h + 14u) * 6.283);
    }

    // Hoek van het vierkantje (twee driehoeken)
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
    );
    let local = corners[vi] * half;
    // Wereld-UV → plek in de viewport → clip-ruimte (y naar beneden in de wereld)
    let rel = ((center + local) * world - vp_origin) / vp;
    out.position = vec4<f32>(rel.x * 2.0 - 1.0, 1.0 - rel.y * 2.0, 0.0, 1.0);
    out.local = local;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    if in.p0.x < 0.5 {
        let gd = length(in.local) / in.p0.y;
        let core = exp(-gd * gd * 1.5);
        return vec4<f32>(lava_color(in.p0.w * core), in.p0.z * core);
    }

    let radius = in.p0.y;
    let px = in.p0.z;
    let tremble = in.p1.y;
    let deform = in.p1.z;
    let d = length(in.local);
    let dir = in.local / max(d, 1e-9);
    let th = atan2(in.local.y, in.local.x);
    // Omtrek: afgeplat (2e mode, schommelend) en een onregelmatige 3e mode,
    // beide trillend in de tijd
    let edge = radius * (1.0
        + deform * (0.6 + 0.4 * sin(tremble)) * cos(2.0 * (th - in.p1.w))
        + 0.35 * deform * sin(1.37 * tremble) * cos(3.0 * th + 0.7 * tremble));
    // q: 0 in het midden, 1 op de rand
    let q = d / edge;
    // Scherp begrensd (vervaging maakt er rook of mist van). Rand en zachte
    // overgang in schermpixels, niet in verhouding tot de bel: ingezoomd wordt
    // de bel groter maar blijft de rand dun
    let aa = (px + radius * 0.5 * in.p0.w) / edge;
    let disk = 1.0 - smoothstep(1.0 - aa, 1.0, q);
    let lit = dot(dir, LIGHT_DIR); // -1 van het licht af .. 1 naar het licht toe
    // Lichte rand (totale reflectie), aan de kant van het licht feller en dikker;
    // de dikte in schermpixels, zodat hij ingezoomd even dun en scherp blijft
    let rim_w = 2.0 * px / edge * (1.0 + 0.6 * max(lit, 0.0));
    let rim = disk * smoothstep(1.0 - rim_w - aa, 1.0 - 0.3 * rim_w, q) * (0.6 + 0.4 * lit);
    // Donkere band net binnen de rand, vooral aan de kant van het licht af
    let shade = disk * smoothstep(0.55, 0.85, q) * (1.0 - smoothstep(0.85, 1.0 - 0.5 * rim_w, q))
        * (0.55 - 0.45 * lit);
    // Lichtvlek: een sikkeltje langs de rand aan de kant van het licht, en een
    // zwak tegenlicht aan de overkant (het licht dat door de bel heen breekt).
    // Grootte in verhouding tot de bel, de rand ervan in schermpixels: ingezoomd
    // blijven ze scherp. (De donkere band is een geleidelijke schaduw en mag
    // meeschalen.)
    let tangent = vec2<f32>(-LIGHT_DIR.y, LIGHT_DIR.x);
    let rel = in.local / edge - LIGHT_DIR * 0.55;
    let crescent = 1.0 - smoothstep(0.13 - 1.4 * aa, 0.13,
        length(vec2<f32>(dot(rel, tangent) * 0.45, dot(rel, LIGHT_DIR) * 1.4)));
    let counter = 1.0 - smoothstep(0.1 - aa, 0.1, length(in.local / edge + LIGHT_DIR * 0.62));
    let bright = clamp(0.03 * disk + 0.38 * rim + 0.55 * crescent * disk + 0.18 * counter * disk, 0.0, 0.6);
    let dark = 0.16 * shade;
    let fade = in.p1.x * (1.0 - 0.6 * in.p0.w);
    let ba = (bright + dark) * fade;
    if ba <= 0.0 {
        discard;
    }
    return vec4<f32>((BUBBLE_COLOR * bright + BUBBLE_SHADE * dark) / (bright + dark), ba);
}

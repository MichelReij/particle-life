# WGSL / GPU Performance Analyse — Native build

Datum: 2026-06-30
Scope: `src/shaders/*.wgsl` + de pass-structuur in `src/webgpu_renderer.rs`, met de native binary
(1080×1080, default 6400 deeltjes, `MAX_PARTICLES = 6400`) als referentie.

## Samenvatting

Elke frame doet de renderer 2 compute passes en 10–12 fullscreen/instanced render passes
(`webgpu_renderer.rs:899-987`). Van al die stappen is er **één die met afstand domineert**:
de deeltjes-interactielus in `compute.wgsl`. Alle andere passes zijn pixel-shaders op een
1080×1080 (of 1404×1404 voor de fisheye-buffer) canvas — qua GPU-tijd verwaarloosbaar
vergeleken met de fysica.

## 1. De hoofdboosdoener: O(n²) interactielus in `compute.wgsl`

`compute.wgsl:684` — voor elk van de `n` actieve deeltjes wordt over **alle** `n` deeltjes
geloopt:

```wgsl
for (var q_idx: u32 = 0u; q_idx < sim_params.num_particles; q_idx = q_idx + 1u) {
    ...
    let particle_q = particles_in[q_idx];   // storage read, altijd uitgevoerd
    ...
    if (sim_params.spatial_grid_enabled == 1u) {
        let q_cell = world_pos_to_grid_cell(particle_q.pos);
        if (!should_process_spatial_interaction(p_idx, q_idx, p_cell, q_cell)) { continue; }
    }
    ...
}
```

Bij 6400 deeltjes is dat **6400 × 6400 ≈ 41 miljoen lus-iteraties per frame**, elk met een
buffer-read + cel-berekening. Bij 60 fps is dat ~2,5 miljard iteraties/seconde, parallel
verdeeld over 100 workgroups van 64 threads.

### De "spatial grid" is hier vooral cosmetisch

`spatial_grid_enabled` (default: aan, `simulation_params.rs:144`) en
`should_process_spatial_interaction()` (`compute.wgsl:205`) doen **geen echte bucket-lookup**.
Ze:

1. Lezen `particle_q` sowieso al uit de buffer, voor élk paar — de lus zelf blijft O(n²).
2. Berekenen on-the-fly de celcoördinaten van `q` en vergelijken die met die van `p`.
3. Voor naburige cellen (afstand ≤ 1): altijd verwerken.
4. Voor verdere cellen: een **probabilistische sample van 30%** (`probability_threshold = 0.3`,
   regel 220) — niet skippen op afstand, maar willekeurig een deel van de verre interacties
   negeren. Dat bespaart wel de kostbaardere sqrt/force-berekening verderop, maar **niet**
   de buffer-read en lus-overhead die je toch al per paar betaalt.

Ter vergelijking: de CPU-only `SpatialGrid` in `src/spatial_grid.rs` implementeert wél een
echte bucket-structuur (cellen met deeltjeslijsten, alleen 3×3 buren doorzoeken), maar die
wordt **niet gebruikt** door de GPU-physics — `particle_system.rs`/CPU-physics draait toch al
niet mee in de native build (zie CLAUDE.md: "Physics is GPU-only"). De GPU-kant herimplementeert
het concept losjes, maar zonder de O(n) winst.

Met cell_size = 80 en wereld 3240×3240 (`simulation_params.rs:145`, `config.rs:8-9`) krijg je
een 41×41-grid (1681 cellen, gemiddeld ~3,8 deeltjes/cel bij 6400 deeltjes). Een echte
bucket-implementatie zou de binnenste lus van ~6400 naar ~30-40 buurdeeltjes terugbrengen —
ruwweg een **150-200× reductie** van het werk in de hoofdlus.

## 2. Lenia-modus: verbergt een 2e–6e O(n²) lus, en staat standaard AAN

`simulation_params.rs:129`:
```rust
lenia_enabled: true, // Disabled by default to test basic particle interactions
```
De comment zegt "disabled by default", maar de waarde is `true`. `native_minimal.rs` overschrijft
dit nergens, dus de **native build draait standaard met Lenia aan**.

Als `lenia_enabled == 1u`, roept `compute.wgsl:654-679` `calculate_lenia_density()` **vijf keer**
per deeltje aan (1× voor de dichtheid zelf + 4× voor de gradiënt: rechts/links/boven/onder,
regels 664-667). Elke aanroep is zélf weer een volledige O(n)-scan over alle deeltjes
(`compute.wgsl:356`), met dezelfde (cosmetische) grid-cull als hierboven.

Resultaat: met Lenia aan kost één frame **niet** 1× de O(n²)-lus, maar **6×** — bovenop de
hoofdinteractielus. Dit is vermoedelijk de grootste enkelvoudige performance-regressie in de
huidige build.

## 3. Lightning compute — geen probleem

`lightning_compute.wgsl:160`: `@compute @workgroup_size(1)`, gedispatcht met
`dispatch_workgroups(1,1,1)` (`webgpu_renderer.rs:902`). Dit is **single-threaded**, maar alle
lussen erin zijn hard begrensd (`min(count, 8u)`, `< 40u`, `queue_idx < 40u`) of lopen over
`lightning_bolt.num_segments` (typisch enkele tientallen). Verwaarloosbare kost per frame.

## 4. Stats compute (sonificatie) — al correct geoptimaliseerd

Zoals beschreven in CLAUDE.md: 7 workgroups, elke 6 frames (~10Hz), 112 bytes readback,
non-blocking. Geen actie nodig.

## 5. Render passes — pixel-gebonden, niet deeltjes-gebonden

Per frame (`webgpu_renderer.rs:899-987`): Lightning Compute → Particle Compute → Background →
Grid → Glow → Particles → Night → Lightning render → Fisheye (alleen als
`fisheye_strength != 0`, anders een goedkope `copy_texture_to_texture`) → Zoom → BlurH →
Vignette → Corner Overlay.

- **Scene-buffer op 1.3× resolutie**: `FISHEYE_BUFFER_SCALE = 1.3` (`config.rs:50`) laat
  Background/Grid/Glow/Particles/Night/Lightning op 1404×1404 in plaats van 1080×1080 renderen
  (~1,69× pixelwerk t.o.v. native canvas). Op zich legitiem voor de fisheye-vervorming, maar
  het is de op-één-na grootste GPU-kost na de fysica.
- **BlurH/Vignette**: 9-tap separable Gaussian blur, standaard en goedkoop.
- **Fisheye/Zoom/Background/Grid/Night**: elk 1 fragment-shader pass, single of handvol
  texture-sample(s) per pixel. Geen loops van betekenis (`num_gradients` in
  `background_frag.wgsl:76` staat trouwens al op **0** — de cloud-loop is uitgeschakeld).
- **Glow + Particles**: instanced draws over `active_count` deeltjes (2× per frame) — dit is
  vertex/fragment werk per deeltje, niet O(n²), en dus geen aandachtspunt vergeleken met de
  compute-lus.

## Advies, op volgorde van verwachte impact

1. **Zet `lenia_enabled` standaard op `false`** (`simulation_params.rs:129`), tenzij het
   artwork bewust Lenia-gedrag wil tonen. Dit is de snelste winst: ~6× minder
   O(n²)-werk in de compute-pass, zonder enige architecturale wijziging.
   ```rust
   lenia_enabled: false,
   ```
   Als Lenia wél gewenst is op de native installatie: overweeg dan in elk geval de 4
   gradiënt-samples te vervangen door een goedkopere centrale-verschil-aanpak die de
   dichtheid uit de hoofdlus hergebruikt, in plaats van 4 losse volledige O(n)-scans.

2. **Implementeer een echte GPU-bucketed spatial grid** voor de hoofdinteractielus
   (en voor Lenia, als die aanblijft). Dit is het structurele fix: een aparte
   compute pass die deeltjes in cel-buckets sorteert (counting sort / prefix sum,
   `spatial_grid_cell_size = 80.0` geeft al een bruikbare 41×41-resolutie), gevolgd door een
   hoofdlus die alleen de ~9 buurcellen doorloopt in plaats van alle `n` deeltjes. Verwachte
   winst: ruwweg 150-200× minder iteraties in de hoofdlus bij 6400 deeltjes, ten koste van een
   kleine extra compute pass (sort) per frame.
   Dit is meer werk dan punt 1, maar lost het probleem op een manier op die blijft schalen als
   het deeltjesaantal ooit omhoog gaat.

3. **Verwijder of beperk de probabilistische 30%-sampling** (`compute.wgsl:220`) zodra er een
   echte bucket-grid is — die sampling bestaat nu om de kosten van cellen >1 afstand te
   maskeren, maar voegt sampling noise toe aan het gedrag. Met een echte grid heb je 'm niet
   meer nodig (verre cellen worden domweg niet meer bezocht).

4. **Optioneel**: verlaag `FISHEYE_BUFFER_SCALE` (nu 1,3) als de fisheye-vervorming visueel ook
   met minder marge werkt — bespaart ~40% pixelwerk op 6 van de render-passes. Alleen de moeite
   waard nadat punt 1–2 zijn opgelost, want dit is een veel kleinere kostenpost dan de fysica.

Geen actie nodig op: lightning compute, stats compute, blur/vignette, of de instanced
particle/glow render passes — die zijn al goedkoop relatief aan de fysica-lus.

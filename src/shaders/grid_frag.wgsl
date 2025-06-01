struct SimParams {
    // We might not need all params, but it's good practice to keep it consistent
    // if we later decide to make the grid dynamic (e.g., scale with canvas size from params).
    deltaTime: f32,
    friction: f32,
    numParticles: u32,
    numTypes: u32,
    virtualWorldWidth: f32,
    virtualWorldHeight: f32,
    canvasRenderWidth: f32,
    canvasRenderHeight: f32,
    virtualWorldOffsetX: f32,
    virtualWorldOffsetY: f32,
    boundaryMode: u32,
    particleRenderSize: f32,
    forceScale: f32,
    rSmooth: f32,
    flatForce: u32,
    driftXPerSecond: f32,
    interTypeAttractionScale: f32,
    interTypeRadiusScale: f32,
    time: f32,
    fisheyeStrength: f32,
    // Fisheye distortion strength
    backgroundColor: vec3<f32>,
    _padding1: f32,
}

;

@group(0) @binding(0)
var<uniform> sim_params: SimParams;
// No texture input needed for a static grid overlay, unless we were blending with previous pass manually.
// However, this pass will be blended by the pipeline settings on top of the canvas.

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let regular_line_thickness: f32 = 2.0;
    // Voorheen line_thickness
    let center_line_thickness: f32 = 4.0;
    // Nieuwe dikte voor centrale lijnen
    let spacing: f32 = 80.0;
    // Jouw huidige waarde
    let line_color_rgb = vec3<f32>(1.0, 1.0, 1.0);
    // Jouw huidige waarde (wit)
    let line_alpha: f32 = 0.1;
    // Jouw huidige waarde

    let center_x = sim_params.virtualWorldOffsetX + sim_params.canvasRenderWidth / 2.0;
    let center_y = sim_params.virtualWorldOffsetY + sim_params.canvasRenderHeight / 2.0;

    // Calculate grid lines that are aligned with the center axes
    // We need to find the grid spacing relative to the center position
    let offset_from_center_x = frag_coord.x - center_x;
    let offset_from_center_y = frag_coord.y - center_y;

    // Calculate distance to nearest grid line relative to center
    let mod_x = offset_from_center_x - spacing * floor(offset_from_center_x / spacing);
    let dist_to_regular_vertical_line = min(abs(mod_x), spacing - abs(mod_x));

    let mod_y = offset_from_center_y - spacing * floor(offset_from_center_y / spacing);
    let dist_to_regular_horizontal_line = min(abs(mod_y), spacing - abs(mod_y));

    var final_alpha: f32 = 0.0;

    // Controleer op dikke centrale verticale lijn
    let on_thick_center_vertical = abs(frag_coord.x - center_x) < center_line_thickness * 0.5;
    // Controleer op dikke centrale horizontale lijn
    let on_thick_center_horizontal = abs(frag_coord.y - center_y) < center_line_thickness * 0.5;

    // Controleer op reguliere (dunne) verticale rasterlijn
    let on_regular_vertical_grid = dist_to_regular_vertical_line < regular_line_thickness * 0.5;
    // Controleer op reguliere (dunne) horizontale rasterlijn
    let on_regular_horizontal_grid = dist_to_regular_horizontal_line < regular_line_thickness * 0.5;

    if (on_thick_center_vertical || on_thick_center_horizontal || on_regular_vertical_grid || on_regular_horizontal_grid) {
        final_alpha = line_alpha;
    }

    // For thicker lines, antialiasing might be nice, but smoothstep can be expensive.
    // Example with smoothstep for softer lines (replace above if/else):
    // let falloff = 1.0; // Pixels over which the line fades
    // let vertical_intensity = 1.0 - smoothstep(regular_line_thickness * 0.5 - falloff, regular_line_thickness * 0.5 + falloff, dist_to_regular_vertical_line);
    // let horizontal_intensity = 1.0 - smoothstep(regular_line_thickness * 0.5 - falloff, regular_line_thickness * 0.5 + falloff, dist_to_regular_horizontal_line);
    // let center_vertical_intensity = 1.0 - smoothstep(center_line_thickness * 0.5 - falloff, center_line_thickness * 0.5 + falloff, abs(frag_coord.x - center_x));
    // let center_horizontal_intensity = 1.0 - smoothstep(center_line_thickness * 0.5 - falloff, center_line_thickness * 0.5 + falloff, abs(frag_coord.y - center_y));
    // final_alpha = max(max(vertical_intensity, horizontal_intensity), max(center_vertical_intensity, center_horizontal_intensity)) * line_alpha;

    return vec4<f32>(line_color_rgb, final_alpha);
}

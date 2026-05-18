#!/usr/bin/env node
// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/
//
// Reads life_params.json → generates:
//   gen/life_params.ts   (TypeScript, for particle-life embed + UI)
//   gen/life_params.h    (C header, for meters + platformio)
//   gen/life_params.rs   (Rust module, for particle-life WASM)

"use strict";

const fs   = require("fs");
const path = require("path");

const src  = path.join(__dirname, "life_params.json");
const outDir = path.join(__dirname, "..", "src", "gen");
fs.mkdirSync(outDir, { recursive: true });

const data = JSON.parse(fs.readFileSync(src, "utf8"));

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function pct(value, min, max) {
    return ((value - min) / (max - min)) * 100;
}

// Convert a ranges array (physical units) to Stop[] (percent + hue).
// A "transition" range is expanded into two stops: start and end, each with
// the hue of the neighboring region, so gradientColor() can interpolate.
function rangesToStops(ranges, min, max, oklch) {
    const { h_blue, h_green, h_yellow, h_red } = oklch;
    const colorHue = { blue: h_blue, green: h_green, yellow: h_yellow, red: h_red };

    const stops = [];
    for (let i = 0; i < ranges.length; i++) {
        const r = ranges[i];
        const pMin = pct(r.min, min, max);
        const pMax = pct(r.max, min, max);

        if (r.color === "transition") {
            // Hue of the range before and after this transition
            const hFrom = i > 0 ? colorHue[ranges[i - 1].color] : h_red;
            const hTo   = i < ranges.length - 1 ? colorHue[ranges[i + 1].color] : h_red;
            stops.push([pMin, hFrom]);
            stops.push([pMax, hTo]);
        } else {
            const h = colorHue[r.color];
            stops.push([pMin, h]);
            stops.push([pMax, h]);
        }
    }

    // Deduplicate consecutive stops at the same percentage
    const deduped = [stops[0]];
    for (let i = 1; i < stops.length; i++) {
        if (Math.abs(stops[i][0] - deduped[deduped.length - 1][0]) > 0.001) {
            deduped.push(stops[i]);
        }
    }
    return deduped;
}

// Convert explicit css_stops [{min, color}] in physical units to Stop[] percents
function cssStopsToStops(cssStops, min, max, oklch) {
    const { h_blue, h_green, h_yellow, h_red } = oklch;
    const colorHue = { blue: h_blue, green: h_green, yellow: h_yellow, red: h_red };
    return cssStops.map(([val, color]) => [pct(val, min, max), colorHue[color]]);
}

// CSS gradient string from stops, direction "to top" (default for vertical sliders)
function stopsToGradient(stops, oklch, direction = "to top") {
    const { l, c } = oklch;
    const parts = stops.map(([p, h]) => `oklch(${l} ${c} ${h}) ${p.toFixed(1)}%`);
    return `linear-gradient(in oklch ${direction}, ${parts.join(", ")})`;
}

// ---------------------------------------------------------------------------
// Derive colour_zone_t values for C.
// For single-green ("bathtub") ranges: {red_lo, green_lo, green_hi, red_hi, green2_lo:0, green2_hi:0}
// For dual-green ("U-shape") ranges:   {red_lo, green_lo, green_hi, red_hi, green2_lo, green2_hi}
//   where green_lo/green_hi = first (low) green zone,
//         red_lo/red_hi = pure-red middle zone,
//         green2_lo/green2_hi = second (high) green zone.
// ---------------------------------------------------------------------------
function colourZone(ranges) {
    const greens = ranges.filter(r => r.color === "green");

    if (greens.length >= 2) {
        // U-shape: two separate green zones with red in the middle.
        const g1 = greens[0], g2 = greens[greens.length - 1];
        const reds = ranges.filter(r => r.color === "red");
        const red_lo  = reds.length ? Math.min(...reds.map(r => r.min)) : g1.max;
        const red_hi  = reds.length ? Math.max(...reds.map(r => r.max)) : g2.min;
        return { red_lo, green_lo: g1.min, green_hi: g1.max, red_hi, green2_lo: g2.min, green2_hi: g2.max };
    }

    // Standard single-green bathtub.
    let red_lo = null, green_lo = null, green_hi = null, red_hi = null;
    for (const r of ranges) {
        if (r.color === "green") {
            if (green_lo === null) green_lo = r.min;
            green_hi = r.max;
        }
        if (r.color === "transition") {
            if (green_lo === null && red_lo === null) red_lo = r.min;
            red_hi = r.max;
        }
    }
    if (red_lo === null) red_lo = ranges[0].min;
    if (red_hi === null) red_hi = ranges[ranges.length - 1].max;
    return { red_lo, green_lo, green_hi, red_hi, green2_lo: 0, green2_hi: 0 };
}

// Return the hypothesis-specific sub-object for a slider.
// For "DUAL" sliders the parameters live at the slider level itself.
function getDef(slider, hyp) {
    if (slider.hypothesis === "DUAL") return slider;
    return slider[hyp];
}

// ---------------------------------------------------------------------------
// TypeScript output
// ---------------------------------------------------------------------------

function genTS() {
    const { oklch, sliders } = data;
    const lines = [];
    lines.push(`// AUTO-GENERATED by shared/codegen.js — DO NOT EDIT MANUALLY`);
    lines.push(`// Source: shared/life_params.json`);
    lines.push(``);
    lines.push(`export const WLP_DEPTH_THRESHOLD = ${data.wlp_depth_threshold};`);
    lines.push(`export const OKLCH_L = ${oklch.l};`);
    lines.push(`export const OKLCH_C = ${oklch.c};`);
    lines.push(`export const H_BLUE   = ${oklch.h_blue};`);
    lines.push(`export const H_GREEN  = ${oklch.h_green};`);
    lines.push(`export const H_YELLOW = ${oklch.h_yellow};`);
    lines.push(`export const H_RED    = ${oklch.h_red};`);
    lines.push(``);
    lines.push(``);
    lines.push(`export type Stop = [number, number]; // [percent, hue]`);
    lines.push(``);
    lines.push(`export interface SliderVariant {`);
    lines.push(`    min: number;`);
    lines.push(`    max: number;`);
    lines.push(`    step: number;`);
    lines.push(`    label: Record<string, string>;`);
    lines.push(`    unit: Record<string, string>;`);
    lines.push(`    stops: Stop[];`);
    lines.push(`    gradient: string; // CSS linear-gradient string`);
    lines.push(`    optimum: number | null;`);
    lines.push(`    snapToMinThreshold: number | null;`);
    lines.push(`}`);
    lines.push(``);
    lines.push(`export const SLIDERS: { htv: SliderVariant; wlp: SliderVariant }[] = [`);

    for (const slider of sliders) {
        lines.push(`    {`);
        for (const hyp of ["htv", "wlp"]) {
            const v = getDef(slider, hyp);
            const min  = v.min  ?? slider.min;
            const max  = v.max  ?? slider.max;
            const step = v.step ?? slider.step;
            const stops = v.css_stops
                ? cssStopsToStops(v.css_stops, min, max, oklch)
                : rangesToStops(v.ranges, min, max, oklch);
            const gradient = stopsToGradient(stops, oklch);
            const optimum  = v.optimum !== undefined ? v.optimum : null;
            const snap     = v.snap_to_min_threshold !== undefined ? v.snap_to_min_threshold : null;
            lines.push(`        ${hyp}: {`);
            lines.push(`            min: ${min}, max: ${max}, step: ${step},`);
            lines.push(`            label: ${JSON.stringify(v.labels)},`);
            lines.push(`            unit: ${JSON.stringify(v.units)},`);
            lines.push(`            stops: ${JSON.stringify(stops)},`);
            lines.push(`            gradient: ${JSON.stringify(gradient)},`);
            lines.push(`            optimum: ${JSON.stringify(optimum)},`);
            lines.push(`            snapToMinThreshold: ${JSON.stringify(snap)},`);
            lines.push(`        },`);
        }
        lines.push(`    },`);
    }

    lines.push(`];`);
    lines.push(``);

    // Export UV sigma_sq constant
    const s2wlp = sliders[2].wlp;
    if (s2wlp.sigma_sq !== undefined) {
        lines.push(`export const UV_SIGMA_SQ = ${s2wlp.sigma_sq};`);
    }

    return lines.join("\n") + "\n";
}

// ---------------------------------------------------------------------------
// C header output
// ---------------------------------------------------------------------------

function genC() {
    const { oklch, sliders } = data;
    const lines = [];
    lines.push(`/* AUTO-GENERATED by shared/codegen.js — DO NOT EDIT MANUALLY */`);
    lines.push(`/* Source: shared/life_params.json */`);
    lines.push(`#ifndef LIFE_PARAMS_H`);
    lines.push(`#define LIFE_PARAMS_H`);
    lines.push(``);
    lines.push(`#include <stdint.h>`);
    lines.push(`#include <math.h>`);
    lines.push(``);
    lines.push(`/* OkLCH colour constants — single source of truth from life_params.json */`);
    const cf = v => Number.isInteger(v) ? `${v}.0f` : `${v}f`;
    lines.push(`#define OKLCH_L   ${cf(oklch.l)}`);
    lines.push(`#define OKLCH_C   ${cf(oklch.c)}`);
    lines.push(`#define H_BLUE    ${cf(oklch.h_blue)}`);
    lines.push(`#define H_GREEN   ${cf(oklch.h_green)}`);
    lines.push(`#define H_YELLOW  ${cf(oklch.h_yellow)}`);
    lines.push(`#define H_RED     ${cf(oklch.h_red)}`);
    lines.push(``);
    lines.push(`#define NUM_SLIDERS 4`);
    lines.push(``);

    // Hypothesis enum
    lines.push(`typedef enum { HYPO_HTV = 0, HYPO_WLP = 1 } hypothesis_t;`);
    lines.push(``);

    // Colour zone struct
    lines.push(`typedef struct {`);
    lines.push(`    float red_lo, green_lo, green_hi, red_hi;`);
    lines.push(`    float green2_lo, green2_hi; /* second green zone for U-shapes; both 0 when unused */`);
    lines.push(`} colour_zone_t;`);
    lines.push(``);

    // Slider meta struct
    lines.push(`typedef struct {`);
    lines.push(`    float min, max, step;`);
    lines.push(`    const char *label_nl, *label_en, *label_fr;`);
    lines.push(`    const char *unit_nl,  *unit_en,  *unit_fr;`);
    lines.push(`    colour_zone_t zone;`);
    lines.push(`    float optimum;           /* optimal physical value */`);
    lines.push(`    float snap_to_min_threshold; /* snap to min when value < this; NAN = disabled */`);
    lines.push(`} slider_def_t;`);
    lines.push(``);

    // Generate per-hypothesis arrays
    for (const hyp of ["htv", "wlp"]) {
        lines.push(`static const slider_def_t SLIDER_DEFS_${hyp.toUpperCase()}[NUM_SLIDERS] = {`);
        for (const slider of sliders) {
            const v    = getDef(slider, hyp);
            const min  = v.min  ?? slider.min;
            const max  = v.max  ?? slider.max;
            const step = v.step ?? slider.step;
            const z    = colourZone(v.ranges);
            const opt  = v.optimum !== undefined ? cf(v.optimum) : `NAN`;
            const snap = v.snap_to_min_threshold !== undefined ? cf(v.snap_to_min_threshold) : `NAN`;
            lines.push(`    /* slider ${slider.index} */`);
            lines.push(`    { ${cf(min)}, ${cf(max)}, ${cf(step)},`);
            lines.push(`      "${v.labels.nl}", "${v.labels.en}", "${v.labels.fr}",`);
            lines.push(`      "${v.units.nl}", "${v.units.en}", "${v.units.fr}",`);
            lines.push(`      { ${cf(z.red_lo)}, ${cf(z.green_lo)}, ${cf(z.green_hi)}, ${cf(z.red_hi)}, ${cf(z.green2_lo)}, ${cf(z.green2_hi)} },`);
            lines.push(`      ${opt}, ${snap} },`);
        }
        lines.push(`};`);
        lines.push(``);
    }

    // UV sigma_sq constant (gaussian curve for UV fitness function)
    const s2wlp = sliders[2].wlp;
    if (s2wlp.sigma_sq !== undefined) {
        lines.push(`#define UV_SIGMA_SQ ${cf(s2wlp.sigma_sq)}`);
        lines.push(``);
    }

    // Convenience macro to select active def
    lines.push(`#define SLIDER_DEF(hyp, idx) \\`);
    lines.push(`    ((hyp) == HYPO_WLP ? SLIDER_DEFS_WLP[(idx)] : SLIDER_DEFS_HTV[(idx)])`);
    lines.push(``);

    // Enum of all known parameters (hypothesis-independent identifiers)
    const allParams = [...new Set(
        sliders.flatMap(s => ["htv", "wlp"].map(h => getDef(s, h).param))
    )];
    lines.push(`typedef enum {`);
    allParams.forEach((p, i) => lines.push(`    LIFE_PARAM_${p.toUpperCase()} = ${i},`));
    lines.push(`} life_param_t;`);
    lines.push(``);

    // Physical slider index → param, per hypothesis
    for (const hyp of ["htv", "wlp"]) {
        lines.push(`/* Physical slider index → parameter for ${hyp.toUpperCase()} */`);
        lines.push(`static const life_param_t SLIDER_PARAM_MAP_${hyp.toUpperCase()}[NUM_SLIDERS] = {`);
        for (const slider of sliders) {
            const param = getDef(slider, hyp).param;
            lines.push(`    LIFE_PARAM_${param.toUpperCase()}, /* slider ${slider.index} */`);
        }
        lines.push(`};`);
        lines.push(``);
    }

    // Physical min/max per slider per hypothesis (for raw→physical conversion)
    for (const hyp of ["htv", "wlp"]) {
        lines.push(`static const float SLIDER_PHYS_MIN_${hyp.toUpperCase()}[NUM_SLIDERS] = {`);
        lines.push(`    ` + sliders.map(s => {
            const v = getDef(s, hyp); const min = v.min ?? s.min;
            return `${cf(min)} /* slider ${s.index}: ${v.param} */`;
        }).join(", "));
        lines.push(`};`);
        lines.push(`static const float SLIDER_PHYS_MAX_${hyp.toUpperCase()}[NUM_SLIDERS] = {`);
        lines.push(`    ` + sliders.map(s => {
            const v = getDef(s, hyp); const max = v.max ?? s.max;
            return `${cf(max)} /* slider ${s.index}: ${v.param} */`;
        }).join(", "));
        lines.push(`};`);
        lines.push(``);
    }

    // Optimum and snap-to-min threshold per slider per hypothesis (NAN = not applicable)
    for (const hyp of ["htv", "wlp"]) {
        lines.push(`static const float SLIDER_OPTIMUM_${hyp.toUpperCase()}[NUM_SLIDERS] = {`);
        lines.push(`    ` + sliders.map(s => {
            const v = getDef(s, hyp);
            const opt = v.optimum !== undefined ? cf(v.optimum) : `NAN`;
            return `${opt} /* slider ${s.index}: ${v.param} */`;
        }).join(", "));
        lines.push(`};`);
        lines.push(`static const float SLIDER_SNAP_THRESHOLD_${hyp.toUpperCase()}[NUM_SLIDERS] = {`);
        lines.push(`    ` + sliders.map(s => {
            const v = getDef(s, hyp);
            const snap = v.snap_to_min_threshold !== undefined ? cf(v.snap_to_min_threshold) : `NAN`;
            return `${snap} /* slider ${s.index}: ${v.param} */`;
        }).join(", "));
        lines.push(`};`);
        lines.push(``);
    }

    // Convenience macros
    lines.push(`#define SLIDER_PARAM(hyp, idx) \\`);
    lines.push(`    ((hyp) == HYPO_WLP ? SLIDER_PARAM_MAP_WLP[(idx)] : SLIDER_PARAM_MAP_HTV[(idx)])`);
    lines.push(`#define SLIDER_PHYS_MIN(hyp, idx) \\`);
    lines.push(`    ((hyp) == HYPO_WLP ? SLIDER_PHYS_MIN_WLP[(idx)] : SLIDER_PHYS_MIN_HTV[(idx)])`);
    lines.push(`#define SLIDER_PHYS_MAX(hyp, idx) \\`);
    lines.push(`    ((hyp) == HYPO_WLP ? SLIDER_PHYS_MAX_WLP[(idx)] : SLIDER_PHYS_MAX_HTV[(idx)])`);
    lines.push(`#define SLIDER_OPTIMUM(hyp, idx) \\`);
    lines.push(`    ((hyp) == HYPO_WLP ? SLIDER_OPTIMUM_WLP[(idx)] : SLIDER_OPTIMUM_HTV[(idx)])`);
    lines.push(`#define SLIDER_SNAP_THRESHOLD(hyp, idx) \\`);
    lines.push(`    ((hyp) == HYPO_WLP ? SLIDER_SNAP_THRESHOLD_WLP[(idx)] : SLIDER_SNAP_THRESHOLD_HTV[(idx)])`);
    lines.push(``);
    lines.push(`#endif /* LIFE_PARAMS_H */`);

    return lines.join("\n") + "\n";
}

// ---------------------------------------------------------------------------
// Rust output
// ---------------------------------------------------------------------------

function genRust() {
    const { sliders, oklch } = data;
    const lines = [];
    lines.push(`// AUTO-GENERATED by shared/codegen.js — DO NOT EDIT MANUALLY`);
    lines.push(`// Source: shared/life_params.json`);
    lines.push(``);
    lines.push(`pub const WLP_DEPTH_THRESHOLD: f32 = ${data.wlp_depth_threshold}.0;`);
    lines.push(``);
    const f32 = v => Number.isInteger(v) ? `${v}.0` : `${v}`;
    lines.push(`// OKLCH hue-waarden (slider-kleurschema)`);
    lines.push(`pub const H_BLUE:   f32 = ${f32(oklch.h_blue)};`);
    lines.push(`pub const H_GREEN:  f32 = ${f32(oklch.h_green)};`);
    lines.push(`pub const H_YELLOW: f32 = ${f32(oklch.h_yellow)};`);
    lines.push(`pub const H_RED:    f32 = ${f32(oklch.h_red)};`);
    lines.push(``);
    lines.push(`// OKLCH L/C voor simulatie-achtergrond (lichter en minder verzadigd dan slider-thumb)`);
    lines.push(`pub const BACKGROUND_L_HTV: f32 = ${f32(oklch.background_l_htv)};`);
    lines.push(`pub const BACKGROUND_C_HTV: f32 = ${f32(oklch.background_c_htv)};`);
    lines.push(`pub const BACKGROUND_L_WLP: f32 = ${f32(oklch.background_l_wlp)};`);
    lines.push(`pub const BACKGROUND_C_WLP: f32 = ${f32(oklch.background_c_wlp)};`);
    lines.push(``);

    const s2wlp = sliders[2].wlp;
    if (s2wlp.uv_optimum !== undefined) {
        lines.push(`pub const UV_OPTIMUM:  f32 = ${f32(s2wlp.uv_optimum)};`);
        lines.push(`pub const UV_SIGMA_SQ: f32 = ${f32(s2wlp.uv_sigma_sq)};`);
        lines.push(``);
    }

    // Physical ranges per slider, per hypothesis
    for (const hyp of ["htv", "wlp"]) {
        lines.push(`pub mod ${hyp} {`);
        for (const slider of sliders) {
            const v   = getDef(slider, hyp);
            const min = v.min  ?? slider.min;
            const max = v.max  ?? slider.max;
            const i   = slider.index;
            lines.push(`    pub const SLIDER${i}_MIN: f32 = ${min}.0;`);
            lines.push(`    pub const SLIDER${i}_MAX: f32 = ${max}.0;`);

            // Colour zone boundaries — parallel to C's colour_zone_t
            if (v.ranges) {
                const ranges = v.ranges;
                const blues  = ranges.filter(r => r.color === "blue");
                const greens = ranges.filter(r => r.color === "green");
                const reds   = ranges.filter(r => r.color === "red");

                if (blues.length)  lines.push(`    pub const SLIDER${i}_BLUE_END:    f32 = ${f32(blues[blues.length - 1].max)};`);
                if (greens.length) lines.push(`    pub const SLIDER${i}_GREEN_START: f32 = ${f32(greens[0].min)};`);
                if (greens.length) lines.push(`    pub const SLIDER${i}_GREEN_END:   f32 = ${f32(greens[greens.length - 1].max)};`);
                if (reds.length)   lines.push(`    pub const SLIDER${i}_RED_START:   f32 = ${f32(reds[reds.length - 1].min)};`);
                // Second green zone (U-shape, e.g. pressure)
                if (greens.length >= 2) {
                    lines.push(`    pub const SLIDER${i}_GREEN2_START: f32 = ${f32(greens[greens.length - 1].min)};`);
                    lines.push(`    pub const SLIDER${i}_GREEN2_END:   f32 = ${f32(greens[greens.length - 1].max)};`);
                }
            }
        }
        lines.push(`}`);
        lines.push(``);
    }

    return lines.join("\n") + "\n";
}

// ---------------------------------------------------------------------------
// Meters gauge definitions (meters_gauge_defs.h)
// Generates col_def_t-compatible structs + tick label arrays from the
// "meters" sub-objects in life_params.json, one per slider per hypothesis.
// ---------------------------------------------------------------------------

function genMetersC() {
    const { sliders } = data;
    const cf = v => Number.isInteger(v) ? `${v}.0f` : `${v}f`;
    const lines = [];
    lines.push(`/* AUTO-GENERATED by shared/codegen.js — DO NOT EDIT MANUALLY */`);
    lines.push(`/* Source: shared/life_params.json  →  meters display configuration */`);
    lines.push(`#ifndef METERS_GAUGE_DEFS_H`);
    lines.push(`#define METERS_GAUGE_DEFS_H`);
    lines.push(``);
    lines.push(`#include <stdbool.h>`);
    lines.push(`#include <stdint.h>`);
    lines.push(`#include "life_params.h"`);
    lines.push(``);
    lines.push(`/* Per-gauge display definition — all display parameters in one place. */`);
    lines.push(`typedef struct {`);
    lines.push(`    const char  *title_nl, *title_en, *title_fr;`);
    lines.push(`    const char  *unit_nl,  *unit_en,  *unit_fr;`);
    lines.push(`    float        scale_factor;   /* physical → LVGL needle integer */`);
    lines.push(`    int32_t      scale_min, scale_max; /* LVGL internal scale range */`);
    lines.push(`    float        disp_factor;    /* physical → displayed number     */`);
    lines.push(`    bool         decimals;`);
    lines.push(`    int32_t      disp_round;     /* round display value; 0 = off    */`);
    lines.push(`    uint32_t     total_ticks;`);
    lines.push(`    uint32_t     major_every;`);
    lines.push(`    const char **tick_labels;    /* NULL-terminated label array     */`);
    lines.push(`} meters_col_def_t;`);
    lines.push(``);

    // Tick label arrays
    for (const hyp of ["htv", "wlp"]) {
        lines.push(`/* ---- Tick labels: ${hyp.toUpperCase()} ---- */`);
        for (const s of sliders) {
            const v = getDef(s, hyp);
            const m = v.meters;
            const labels = m.tick_labels.map(l => `"${l}"`).join(", ");
            lines.push(`static const char *mg_tick_${s.index}_${hyp}[] = { ${labels}, NULL };`);
        }
        lines.push(``);
    }

    // Gauge def arrays, one per hypothesis
    for (const hyp of ["htv", "wlp"]) {
        lines.push(`static const meters_col_def_t METERS_GAUGE_DEFS_${hyp.toUpperCase()}[NUM_SLIDERS] = {`);
        for (const s of sliders) {
            const v = getDef(s, hyp);
            const m = v.meters;
            lines.push(`    { /* slider ${s.index}: ${v.param} (${hyp.toUpperCase()}) */`);
            lines.push(`        "${v.labels.nl}", "${v.labels.en}", "${v.labels.fr}",`);
            lines.push(`        "${v.units.nl}", "${v.units.en}", "${v.units.fr}",`);
            lines.push(`        ${cf(m.scale_factor)}, ${m.scale_min}, ${m.scale_max},`);
            lines.push(`        ${cf(m.disp_factor)}, ${m.decimals}, ${m.disp_round},`);
            lines.push(`        ${m.total_ticks}, ${m.major_every}, mg_tick_${s.index}_${hyp}`);
            lines.push(`    },`);
        }
        lines.push(`};`);
        lines.push(``);
    }

    lines.push(`#define METERS_GAUGE_DEFS(is_wlp) \\`);
    lines.push(`    ((is_wlp) ? METERS_GAUGE_DEFS_WLP : METERS_GAUGE_DEFS_HTV)`);
    lines.push(``);
    lines.push(`#endif /* METERS_GAUGE_DEFS_H */`);
    return lines.join("\n") + "\n";
}

// ---------------------------------------------------------------------------
// Write files
// ---------------------------------------------------------------------------

fs.writeFileSync(path.join(outDir, "life_params.ts"), genTS());
console.log("✓ src/gen/life_params.ts");

const cOutDir = path.join(__dirname, "gen");
fs.mkdirSync(cOutDir, { recursive: true });
const cHeader = genC();
fs.writeFileSync(path.join(cOutDir, "life_params.h"),  cHeader);
fs.writeFileSync(path.join(cOutDir, "life_params.rs"), genRust());
console.log("✓ shared/gen/life_params.h");
console.log("✓ shared/gen/life_params.rs");

const platformioInclude = path.join(__dirname, "../../../platformio/include/life_params.h");
if (fs.existsSync(path.dirname(platformioInclude))) {
    fs.writeFileSync(platformioInclude, cHeader);
    console.log("✓ platformio/include/life_params.h");
}

const metersInclude = path.join(__dirname, "../../../meters/main/life_params.h");
if (fs.existsSync(path.dirname(metersInclude))) {
    fs.writeFileSync(metersInclude, cHeader);
    console.log("✓ meters/main/life_params.h");
}

const metersGaugeDefs = path.join(__dirname, "../../../meters/main/meters_gauge_defs.h");
if (fs.existsSync(path.dirname(metersGaugeDefs))) {
    fs.writeFileSync(metersGaugeDefs, genMetersC());
    console.log("✓ meters/main/meters_gauge_defs.h");
}

// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// Gedeelde kleurlogica voor embed.ts en ui.ts.
// Alle waarden zijn afgeleid van life_params.json via codegen.

import { OKLCH_L, OKLCH_C, H_RED, SLIDERS, type Stop } from "./gen/life_params";

export type { Stop };

// Slider stops op slider-id — SLIDERS[0]=diepte, [1]=temp, [2]=pH/UV, [3]=elec
export const SLIDER_STOPS: Record<string, Stop[]> = {
    "presSlider":   SLIDERS[0].htv.stops as Stop[],
    "ol-pres":      SLIDERS[0].htv.stops as Stop[],
    "tempSlider":   SLIDERS[1].htv.stops as Stop[],
    "ol-temp":      SLIDERS[1].htv.stops as Stop[],
    "ol-temp-wlp":  SLIDERS[1].wlp.stops as Stop[],
    "phSlider":     SLIDERS[2].htv.stops as Stop[],
    "ol-ph":        SLIDERS[2].htv.stops as Stop[],
    "ol-ph-wlp":    SLIDERS[2].wlp.stops as Stop[],
    "elecSlider":   SLIDERS[3].htv.stops as Stop[],
    "ol-elec":      SLIDERS[3].htv.stops as Stop[],
    "ol-elec-wlp":  SLIDERS[3].wlp.stops as Stop[],
};

// Bereken de OkLCH-kleur die hoort bij een percentage op de slider-track.
export function gradientColor(pct: number, stops: Stop[]): string {
    if (stops.length === 0) return `oklch(${OKLCH_L} ${OKLCH_C} ${H_RED})`;
    if (pct <= stops[0][0])
        return `oklch(${OKLCH_L} ${OKLCH_C} ${stops[0][1]})`;
    if (pct >= stops[stops.length - 1][0])
        return `oklch(${OKLCH_L} ${OKLCH_C} ${stops[stops.length - 1][1]})`;
    for (let i = 0; i < stops.length - 1; i++) {
        const [p0, h0] = stops[i];
        const [p1, h1] = stops[i + 1];
        if (pct >= p0 && pct <= p1) {
            const t = (pct - p0) / (p1 - p0);
            let dh = h1 - h0;
            if (dh > 180) dh -= 360;
            if (dh < -180) dh += 360;
            return `oklch(${OKLCH_L} ${OKLCH_C} ${(h0 + t * dh).toFixed(1)})`;
        }
    }
    return `oklch(${OKLCH_L} ${OKLCH_C} ${H_RED})`;
}

// Zet --thumb-color op de slider op basis van de huidige waarde.
// invert=true voor sliders die omgekeerd lopen (bijv. diepte: boven=0, onder=max).
export function updateThumbColor(
    slider: HTMLInputElement,
    stopsKey?: string,
    invert = false,
): void {
    const key = stopsKey ?? slider.id;
    const stops = SLIDER_STOPS[key];
    if (!stops) return;
    let pct =
        ((parseFloat(slider.value) - parseFloat(slider.min)) /
            (parseFloat(slider.max) - parseFloat(slider.min))) *
        100;
    if (invert) pct = 100 - pct;
    slider.style.setProperty("--thumb-color", gradientColor(pct, stops));
}

// Zet de kleur van een waarde-display gelijk aan de thumb-kleur van de slider.
export function syncValueColor(slider: HTMLInputElement, displayId: string): void {
    const display = document.getElementById(displayId);
    if (display) display.style.color = slider.style.getPropertyValue("--thumb-color");
}

// Zet de CSS gradient-achtergrond op een slider op basis van de stops.
export function applySliderGradient(
    slider: HTMLInputElement,
    stopsKey: string,
    direction = "to top",
): void {
    const stops = SLIDER_STOPS[stopsKey];
    if (!stops) return;
    const parts = stops.map(
        ([p, h]) => `oklch(${OKLCH_L} ${OKLCH_C} ${h}) ${p.toFixed(1)}%`,
    );
    slider.style.background = `linear-gradient(in oklch ${direction}, ${parts.join(", ")})`;
}

// Shared hypothesis-aware slider storage logic for index.html (ui.ts) and embed.ts.

import { WLP_DEPTH_THRESHOLD, SLIDERS } from "./gen/life_params";
import { updateThumbColor } from "./color-utils";

export type Hypothesis = "htv" | "wlp";

// ── Storage ──────────────────────────────────────────────────────────────────

export interface HypothesisKeys {
    temp: { htv: string; wlp: string };
    ph:   { htv: string; wlp: string };
    elec: { htv: string; wlp: string };
    pres: string;
}

export function saveSlider(key: string, value: number): void {
    try { localStorage.setItem(key, value.toString()); } catch { /* ignore */ }
}

export function loadSlider(key: string, fallback: number): number {
    try {
        const s = localStorage.getItem(key);
        if (s !== null) { const n = parseFloat(s); if (!isNaN(n)) return n; }
    } catch { /* ignore */ }
    return fallback;
}

export function loadInitialHypothesis(keys: HypothesisKeys): Hypothesis {
    const p = loadSlider(keys.pres, 9999);
    return p < WLP_DEPTH_THRESHOLD ? "wlp" : "htv";
}

export function sliderKey(
    keys: HypothesisKeys,
    param: "temp" | "ph" | "elec",
    hypothesis: Hypothesis,
): string {
    return keys[param][hypothesis];
}

// ── Slider DOM ids ───────────────────────────────────────────────────────────

export interface SliderIds {
    temp: { slider: string; value: string; thumbStop: { htv: string; wlp: string } };
    ph:   { slider: string; value: string; label: string; thumbStop: { htv: string; wlp: string } };
    elec: { slider: string; value: string; thumbStop: { htv: string; wlp: string } };
}

// ── applyHypothesisToSliders ─────────────────────────────────────────────────
//
// Call when the hypothesis changes. Updates slider ranges, reloads saved values
// from the correct key, refreshes gradients and thumb colours.
// Returns the reloaded values so callers can push them to the engine.

export interface SliderValues {
    temperature: number;
    ph: number;
    electricalActivity: number;
}

export function applyHypothesisToSliders(
    hypothesis: Hypothesis,
    ids: SliderIds,
    keys: HypothesisKeys,
    phLabel: string,
    uvLabel: string,
    defaults: SliderValues,
): SliderValues {
    const result = { ...defaults };

    // ── Temperature ────────────────────────────────────────────────────────
    const tempSlider = document.getElementById(ids.temp.slider) as HTMLInputElement | null;
    const tempVal    = document.getElementById(ids.temp.value);
    if (tempSlider) {
        const newMax = hypothesis === "wlp" ? SLIDERS[1].wlp.max : SLIDERS[1].htv.max;
        tempSlider.max = newMax.toString();
        result.temperature = Math.min(
            loadSlider(sliderKey(keys, "temp", hypothesis), defaults.temperature),
            newMax,
        );
        tempSlider.value = result.temperature.toString();
        if (tempVal) tempVal.textContent = `${result.temperature}°C`;
        tempSlider.style.background = hypothesis === "wlp" ? SLIDERS[1].wlp.gradient.replace("to top", "to right") : "";
        updateThumbColor(tempSlider, ids.temp.thumbStop[hypothesis]);
    }

    // ── pH / UV ────────────────────────────────────────────────────────────
    const phSlider  = document.getElementById(ids.ph.slider) as HTMLInputElement | null;
    const phValEl   = document.getElementById(ids.ph.value);
    const phLabelEl = document.getElementById(ids.ph.label) as HTMLElement | null;
    if (phSlider) {
        if (hypothesis === "wlp") {
            phSlider.min = "0"; phSlider.max = "11"; phSlider.step = "0.1";
            if (phLabelEl) phLabelEl.textContent = uvLabel;
        } else {
            phSlider.min = "0"; phSlider.max = "14"; phSlider.step = "0.1";
            if (phLabelEl) phLabelEl.textContent = phLabel;
        }
        result.ph = Math.min(
            loadSlider(sliderKey(keys, "ph", hypothesis), defaults.ph),
            parseFloat(phSlider.max),
        );
        phSlider.value = result.ph.toFixed(1);
        if (phValEl)
            phValEl.textContent = hypothesis === "wlp"
                ? result.ph.toFixed(1) + " UV"
                : result.ph.toFixed(1);
        phSlider.style.background = hypothesis === "wlp" ? SLIDERS[2].wlp.gradient.replace("to top", "to right") : "";
        updateThumbColor(phSlider, ids.ph.thumbStop[hypothesis]);
    }

    // ── Electrical activity ────────────────────────────────────────────────
    const elecSlider = document.getElementById(ids.elec.slider) as HTMLInputElement | null;
    const elecVal    = document.getElementById(ids.elec.value);
    if (elecSlider) {
        const newMax = hypothesis === "wlp" ? SLIDERS[3].wlp.max : SLIDERS[3].htv.max;
        elecSlider.max = newMax.toString();
        result.electricalActivity = Math.min(
            loadSlider(sliderKey(keys, "elec", hypothesis), defaults.electricalActivity),
            newMax,
        );
        elecSlider.value = result.electricalActivity.toString();
        const elecUnit = hypothesis === "wlp" ? SLIDERS[3].wlp.unit["en"] : SLIDERS[3].htv.unit["en"];
        if (elecVal) elecVal.textContent = `${result.electricalActivity.toFixed(2)} ${elecUnit}`;
        elecSlider.style.background = hypothesis === "wlp" ? SLIDERS[3].wlp.gradient.replace("to top", "to right") : "";
        updateThumbColor(elecSlider, ids.elec.thumbStop[hypothesis]);
    }

    return result;
}

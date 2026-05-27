// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

import init, { ParticleLifeEngine } from "./pkg/particle_life_wasm";
import {
    CANVAS_WIDTH,
    CANVAS_HEIGHT,
    VIRTUAL_WORLD_WIDTH,
    VIRTUAL_WORLD_HEIGHT,
    ZOOM_MIN,
    ZOOM_MAX,
} from "./config";
import { WLP_DEPTH_THRESHOLD, SLIDERS } from "./gen/life_params";
import { updateThumbColor } from "./color-utils";
import {
    type Hypothesis,
    type HypothesisKeys,
    type SliderIds,
    type SliderValues,
    saveSlider,
    loadSlider,
    loadInitialHypothesis,
    sliderKey,
    applyHypothesisToSliders,
} from "./hypothesis-sliders";

const WORLD_CENTER_X = VIRTUAL_WORLD_WIDTH / 2;
const WORLD_CENTER_Y = VIRTUAL_WORLD_HEIGHT / 2;

type Lang = "nl" | "en" | "fr";

const I18N: Record<
    Lang,
    {
        loading: string;
        hintZoom: string;
        hintPan: string;
        titleScreenshot: string;
        titleRecord: string;
        startText: string;
        playAnyway: string;
        temp: string;
        ph: string;
        uv: string;
        depth: string;
        electricity: string;
    }
> = {
    nl: {
        loading: "Simulatie laden…",
        hintZoom: "Scroll / knijp om te zoomen",
        hintPan: "Sleep om te bewegen",
        titleScreenshot: "Screenshot",
        titleRecord: "Video opnemen",
        startText:
            "Deze simulatie gebruikt je grafische processor intensief en werkt mogelijk niet goed op oudere apparaten. Ook verbruikt het veel energie, waardoor de batterij van je telefoon of laptop snel leegloopt.",
        playAnyway: "Starten",
        temp: "Temperatuur",
        ph: "pH",
        uv: "UV",
        depth: "Diepte (druk)",
        electricity: "Elektriciteit",
    },
    en: {
        loading: "Loading simulation…",
        hintZoom: "Scroll / pinch to zoom",
        hintPan: "Drag to pan",
        titleScreenshot: "Screenshot",
        titleRecord: "Record video",
        startText:
            "This simulation uses your graphics processor intensively and may not run well on older devices. It also consumes a lot of energy, which will drain the battery of your phone or laptop quickly.",
        playAnyway: "Play anyway",
        temp: "Temperature",
        ph: "pH",
        uv: "UV",
        depth: "Sea depth",
        electricity: "Electricity",
    },
    fr: {
        loading: "Chargement de la simulation…",
        hintZoom: "Molette / pincer pour zoomer",
        hintPan: "Glisser pour déplacer",
        titleScreenshot: "Capture d'écran",
        titleRecord: "Enregistrer la vidéo",
        startText:
            "Cette simulation sollicite intensément votre processeur graphique et peut ne pas fonctionner correctement sur les appareils plus anciens. Elle consomme également beaucoup d'énergie, ce qui déchargera rapidement la batterie de votre téléphone ou ordinateur portable.",
        playAnyway: "Démarrer quand même",
        temp: "Température",
        ph: "pH",
        uv: "UV",
        depth: "Profondeur",
        electricity: "Électricité",
    },
};

// Slider stops indexed by slider-id: sourced from generated life_params.ts
// SLIDERS[0]=depth, [1]=temp, [2]=pH/UV, [3]=elec — aangevuld in color-utils.ts
// ol-pres, ol-temp etc. zijn al geregistreerd; extra aliases hier niet nodig.

function getLang(): Lang {
    const wrap = document.getElementById("ol-wrap");
    const raw = (wrap?.dataset.lang ?? "en").toLowerCase();
    return raw === "nl" || raw === "fr" ? raw : "en";
}

// Derive base URL from our script tag so WASM loads from the right server
// regardless of which WordPress page embeds this snippet.
// Resolved lazily (after DOMContentLoaded) via querySelectorAll — more reliable
// than document.currentScript which is null inside webpack bundles in Safari.
function getScriptBase(): string {
    const scripts = document.querySelectorAll<HTMLScriptElement>("script[src]");
    for (const script of Array.from(scripts)) {
        if (script.src.includes("particle-life-embed")) {
            return script.src.substring(0, script.src.lastIndexOf("/") + 1);
        }
    }
    return "https://michelreij.nl/webapps/origin-of-life/";
}

const EMBED_CSS = `
.ol-material-icon {
    font-family: 'Material Symbols Outlined';
    font-weight: normal;
    font-style: normal;
    font-size: 18px;
    line-height: 1;
    letter-spacing: normal;
    text-transform: none;
    display: inline-block;
    white-space: nowrap;
    word-wrap: normal;
    direction: ltr;
    -webkit-font-feature-settings: 'liga';
    font-feature-settings: 'liga';
    -webkit-font-smoothing: antialiased;
}

#ol-wrap {
    display: flex;
    flex-direction: column;
    width: 100%;
    background: transparent;
    border-radius: 6px;
    overflow: hidden;
    font-family: system-ui, sans-serif;
    font-size: 13px;
    color: #8f7e48;
    box-sizing: border-box;
}
#ol-wrap *, #ol-wrap *::before, #ol-wrap *::after {
    box-sizing: border-box;
}
#ol-canvas-wrap {
    position: relative;
    width: 100%;
    aspect-ratio: 1 / 1;
    overflow: hidden;
    background: transparent;
}
#ol-canvas {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
    touch-action: none;
    cursor: grab;
    border-radius: 50%;
}
#ol-canvas.dragging {
    cursor: grabbing;
}
#ol-status {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    color: #8f7e48;
    font-size: 14px;
    pointer-events: none;
}
#ol-hint-zoom {
    position: absolute;
    top: 6px;
    left: 8px;
    font-size: 10px;
    color: #8f7e48;
    pointer-events: none;
    user-select: none;
}
#ol-hint-pan {
    position: absolute;
    top: 6px;
    right: 8px;
    font-size: 10px;
    color: #8f7e48;
    pointer-events: none;
    user-select: none;
}
#ol-screenshot-btn, #ol-record-btn, #ol-pause-btn {
    position: absolute;
    bottom: 10px;
    background: none;
    border: none;
    color: #8f7e48;
    font-size: 0;
    width: 40px;
    height: 40px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: opacity 0.15s, transform 0.1s;
    z-index: 10;
    opacity: 0.85;
    padding: 0;
}
#ol-screenshot-btn { right: 10px; }
#ol-record-btn     { right: 58px; }
#ol-pause-btn      { left: 10px; }
#ol-screenshot-btn .ol-material-icon, #ol-record-btn .ol-material-icon, #ol-pause-btn .ol-material-icon {
    font-size: 32px;
    line-height: 1;
}
#ol-screenshot-btn:hover, #ol-record-btn:hover, #ol-pause-btn:hover { opacity: 1; }
#ol-screenshot-btn:active, #ol-record-btn:active, #ol-pause-btn:active { transform: scale(0.88); }
#ol-record-btn.recording { color: #ff5555; opacity: 1; }
#ol-screenshot-btn:focus, #ol-record-btn:focus, #ol-pause-btn:focus { outline: none; }
@media (max-width: 480px) {
    #ol-hint-zoom, #ol-hint-pan, #ol-screenshot-btn, #ol-record-btn, #ol-pause-btn { display: none; }
}
canvas#ol-canvas {
    background-color: #0004;
    box-shadow: none;
}
#ol-hint-zoom,
#ol-hint-pan {
    font-size: var(--wp--preset--font-size--small);
    color: var(--wp--preset--color--contrast, #8f7e48);
}
#ol-screenshot-flash {
    position: absolute;
    inset: 0;
    background: white;
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.05s;
    border-radius: 50%;
}
#ol-start-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    z-index: 20;
    border-radius: 50%;
    padding: 12%;
    text-align: center;
    background: #000C url('${getScriptBase()}assets/images/not_started_yet_1080.png') center/cover no-repeat;
}
#ol-start-overlay p {
    color: #8f7e48;
    font-size: 12px;
    line-height: 1.6;
    margin: 0 0 16px;
}
#ol-start-overlay button {
    background: rgba(40,140,60,0.9);
    color: #fff;
    border: none;
    border-radius: 6px;
    padding: 8px 22px;
    font-size: 13px;
    cursor: pointer;
    transition: background 0.15s;
}
#ol-start-overlay button:hover {
    background: rgba(40,140,60,1);
}
#ol-controls {
    display: flex;
    flex-direction: row;
    gap: 0;
    padding: 10px 14px;
    background: transparent;
    border-top: none;
    align-items: flex-end;
    justify-content: space-evenly;
}
.ol-slider-group {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    flex: 1 1 0;
}
.ol-slider-group label {
    color: #8f7e48;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    white-space: nowrap;
    order: 3;
}
.ol-slider-group .ol-val {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    min-width: 3.5em;
    text-align: center;
    order: 2;
}
/* Vertical slider via writing-mode — native vertical, height = track length */
.ol-slider-group .ol-slider-wrap {
    order: 1;
    width: 34px;
    height: 240px;
    display: flex;
    align-items: center;
    justify-content: center;
}
.ol-slider-group input[type="range"] {
    -webkit-appearance: none;
    appearance: none;
    writing-mode: vertical-lr;
    direction: rtl;
    width: 12px;
    height: 240px;
    border-radius: 6px;
    border: 3px solid #FFF;
    outline: none;
    cursor: pointer;
    margin: 0;
}
.ol-slider-group input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--thumb-color, #4aaff0);
    border: 3px solid #FFF;
    cursor: pointer;
    box-shadow: 0 1px 3px rgba(255,255,255,0.5);
}
.ol-slider-group input[type="range"]::-moz-range-thumb {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--thumb-color, #4aaff0);
    border: 3px solid #FFF;
    cursor: pointer;
    box-shadow: 0 1px 3px rgba(255,255,255,0.5);
}
@media (pointer: coarse) {
    .ol-slider-group .ol-slider-wrap {
        width: 36px;
    }
    .ol-slider-group input[type="range"] {
        width: 10px;
        border-radius: 5px;
    }
    .ol-slider-group input[type="range"]::-webkit-slider-thumb {
        width: 26px;
        height: 26px;
    }
    .ol-slider-group input[type="range"]::-moz-range-thumb {
        width: 26px;
        height: 26px;
    }
}

/* Temp HTV */
#ol-temp { background: ${SLIDERS[1].htv.gradient}; }
#ol-temp::-webkit-slider-runnable-track { background: transparent; }
#ol-temp::-moz-range-track { background: transparent; }

/* Diepte — direction:ltr omdat de slider omgekeerd is (0 bovenaan) */
#ol-pres { direction: ltr; background: ${SLIDERS[0].htv.gradient.replace("to top", "to bottom")}; }
#ol-pres::-webkit-slider-runnable-track { background: transparent; }
#ol-pres::-moz-range-track { background: transparent; }

/* pH HTV */
#ol-ph { background: ${SLIDERS[2].htv.gradient}; }
/* UV WLP */
#ol-ph.wlp { background: ${SLIDERS[2].wlp.gradient}; }
#ol-ph::-webkit-slider-runnable-track { background: transparent; }
#ol-ph::-moz-range-track { background: transparent; }

/* Elec */
#ol-elec { background: ${SLIDERS[3].htv.gradient}; }
#ol-elec::-webkit-slider-runnable-track { background: transparent; }
#ol-elec::-moz-range-track { background: transparent; }
`;

function injectStyles() {
    if (document.getElementById("ol-embed-styles")) return;
    const style = document.createElement("style");
    style.id = "ol-embed-styles";
    style.textContent =
        `@font-face {
    font-family: 'Material Symbols Outlined';
    font-style: normal;
    src: url('${getScriptBase()}fonts/material-symbols-outlined.woff2') format('woff2');
}` + EMBED_CSS;
    document.head.appendChild(style);
}

function buildDOM() {
    const wrap = document.getElementById("ol-wrap");
    if (!wrap) return;

    const t = I18N[getLang()];

    wrap.innerHTML = `
        <div id="ol-canvas-wrap">
            <canvas id="ol-canvas"></canvas>
            <div id="ol-status">${t.loading}</div>
            <div id="ol-hint-zoom">${t.hintZoom}</div>
            <div id="ol-hint-pan">${t.hintPan}</div>
            <button id="ol-pause-btn" title="Pause / Play (P)"><span class="ol-material-icon">pause</span></button>
            <button id="ol-screenshot-btn" title="${t.titleScreenshot}"><span class="ol-material-icon">photo_camera</span></button>
            <button id="ol-record-btn" title="${t.titleRecord}"><span class="ol-material-icon">videocam</span></button>
            <div id="ol-screenshot-flash"></div>
            <div id="ol-start-overlay">
                <p>${t.startText}</p>
                <button id="ol-play-anyway">${t.playAnyway}</button>
            </div>
        </div>
        <div id="ol-controls">
            <div class="ol-slider-group">
                <label>${t.depth}</label>
                <span class="ol-val" id="ol-pres-val">1m</span>
                <div class="ol-slider-wrap"><input type="range" id="ol-pres" min="0" max="1000" value="1" step="1" /></div>
            </div>
            <div class="ol-slider-group">
                <label>${t.temp}</label>
                <span class="ol-val" id="ol-temp-val">20°C</span>
                <div class="ol-slider-wrap"><input type="range" id="ol-temp" min="3" max="160" value="20" step="1" /></div>
            </div>
            <div class="ol-slider-group">
                <label id="ol-ph-label">${t.ph}</label>
                <span class="ol-val" id="ol-ph-val">7.0</span>
                <div class="ol-slider-wrap"><input type="range" id="ol-ph" min="0" max="14" value="7" step="0.1" /></div>
            </div>
            <div class="ol-slider-group">
                <label>${t.electricity}</label>
                <span class="ol-val" id="ol-elec-val">1.02</span>
                <div class="ol-slider-wrap"><input type="range" id="ol-elec" min="0" max="3" value="1.02" step="0.01" /></div>
            </div>
        </div>
    `;
}

class EmbedApp {
    private engine: ParticleLifeEngine | null = null;
    private canvas: HTMLCanvasElement | null = null;
    private animationId: number | null = null;
    private engineBusy = false;
    private isPaused = false;
    private lastTime = 0;
    private activeHypothesis: Hypothesis = "htv";

    private readonly hypothesisKeys: HypothesisKeys = {
        temp: { htv: "ol-slider-ol-temp", wlp: "ol-slider-ol-temp-wlp" },
        ph: { htv: "ol-slider-ol-ph", wlp: "ol-slider-ol-ph-wlp" },
        elec: { htv: "ol-slider-ol-elec", wlp: "ol-slider-ol-elec-wlp" },
        pres: "ol-slider-ol-pres",
    };

    private readonly sliderIds: SliderIds = {
        temp: {
            slider: "ol-temp",
            value: "ol-temp-val",
            thumbStop: { htv: "ol-temp", wlp: "ol-temp-wlp" },
        },
        ph: {
            slider: "ol-ph",
            value: "ol-ph-val",
            label: "ol-ph-label",
            thumbStop: { htv: "ol-ph", wlp: "ol-ph-wlp" },
        },
        elec: {
            slider: "ol-elec",
            value: "ol-elec-val",
            thumbStop: { htv: "ol-elec", wlp: "ol-elec-wlp" },
        },
    };

    private sliderDefaults: SliderValues = {
        temperature: 15,
        ph: 7.0,
        electricalActivity: 0.1,
    };

    private zoom = 1.0;
    private panX = WORLD_CENTER_X;
    private panY = WORLD_CENTER_Y;
    private virtualWorldWidth = VIRTUAL_WORLD_WIDTH;
    private virtualWorldHeight = VIRTUAL_WORLD_HEIGHT;

    private isDragging = false;
    private dragStartX = 0;
    private dragStartY = 0;
    private dragStartPanX = 0;
    private dragStartPanY = 0;

    private lastPinchDist = 0;
    private lastPinchMidX = 0;
    private lastPinchMidY = 0;

    private mediaRecorder: MediaRecorder | null = null;
    private recordedChunks: Blob[] = [];

    async init() {
        injectStyles();
        buildDOM();

        this.canvas = document.getElementById("ol-canvas") as HTMLCanvasElement;
        if (!this.canvas) return;

        const canvasWrap = document.getElementById("ol-canvas-wrap");
        const containerWidth = canvasWrap
            ? canvasWrap.getBoundingClientRect().width
            : CANVAS_WIDTH;
        const canvasSize = Math.max(
            300,
            Math.min(Math.floor(containerWidth), CANVAS_WIDTH),
        );
        this.canvas.width = canvasSize;
        this.canvas.height = canvasSize;
        this.virtualWorldWidth =
            canvasSize * (VIRTUAL_WORLD_WIDTH / CANVAS_WIDTH);
        this.virtualWorldHeight =
            canvasSize * (VIRTUAL_WORLD_HEIGHT / CANVAS_HEIGHT);
        this.panX = this.virtualWorldWidth / 2;
        this.panY = this.virtualWorldHeight / 2;

        // Feature detection: skip loading WASM if browser lacks WebAssembly or WebGPU
        if (typeof WebAssembly === "undefined") {
            console.warn("Origin of Life: WebAssembly not supported.");
            this.setStatus("Your browser does not support WebAssembly.");
            return;
        }
        if (!navigator.gpu) {
            console.warn("Origin of Life: WebGPU not supported.");
            this.setStatus("Your browser does not support WebGPU.");
            return;
        }
        const gpuAdapter = await navigator.gpu.requestAdapter();
        if (!gpuAdapter) {
            console.warn("Origin of Life: No WebGPU adapter available.");
            this.setStatus("No compatible GPU found for WebGPU.");
            return;
        }

        const wasmUrl = `${getScriptBase()}pkg/particle_life_wasm_bg.wasm?v=${Date.now()}`;

        try {
            await init({ module_or_path: wasmUrl });
        } catch (e) {
            console.error("Origin of Life: WASM init failed:", e);
            this.setStatus("Failed to load simulation.");
            return;
        }

        this.engine = new ParticleLifeEngine();

        try {
            await this.engine.initialize_webgpu(this.canvas);
        } catch (e) {
            console.warn("Origin of Life: WebGPU init failed:", e);
        }

        // Load circle mask overlay PNG and upload as GPU texture
        try {
            const base = getScriptBase();
            const resp = await fetch(
                `${base}assets/images/copyright_mask_wasm_1080.png`,
            );
            const blob = await resp.blob();
            const bm = await createImageBitmap(blob);
            this.engine.set_overlay_images(bm, bm.width, bm.height);
        } catch (e) {
            console.warn("Origin of Life: overlay image failed to load:", e);
        }

        this.wireSliders();
        this.wireZoomPan();
        this.wireStartOverlay();
        this.wireCapture();

        const status = document.getElementById("ol-status");
        if (status) status.style.display = "none";
    }

    private setStatus(msg: string) {
        const el = document.getElementById("ol-status");
        if (el) el.textContent = msg;
    }

    private wireStartOverlay() {
        const overlay = document.getElementById("ol-start-overlay");
        const btn = document.getElementById("ol-play-anyway");
        if (!overlay || !btn) return;

        btn.addEventListener("click", () => {
            overlay.style.display = "none";
            this.lastTime = performance.now();
            this.startLoop();
        });
    }

    private wireSliders() {
        // Bepaal hypothese vroeg zodat temp/ph/elec de juiste storage-key laden
        this.activeHypothesis = loadInitialHypothesis(this.hypothesisKeys);

        // Stuur pressure als eerste naar de engine zodat is_wlp correct staat
        // vóór temp/ph/elec worden berekend — anders gebruikt apply_temperature de verkeerde hypothese
        if (this.engine) {
            const presSliderInit = document.getElementById("ol-pres") as HTMLInputElement | null;
            if (presSliderInit) {
                const pv = parseFloat(presSliderInit.value);
                this.engine.set_pressure(pv);
                this.engine.set_particle_count_from_pressure(pv);
            }
        }

        // Gradients direct toepassen als we in wlp starten — anders blijven ze op htv
        if (this.activeHypothesis === "wlp") {
            const t = I18N[getLang()];
            const vals = applyHypothesisToSliders(
                "wlp",
                this.sliderIds,
                this.hypothesisKeys,
                t.ph,
                t.uv,
                this.sliderDefaults,
            );
            this.sliderDefaults = vals;
        }

        // Generic wire for sliders without hypothesis-specific storage
        const wire = (
            id: string,
            valId: string,
            format: (v: number) => string,
            apply: (v: number) => void,
            stopsKey?: () => string,
        ) => {
            const slider = document.getElementById(
                id,
            ) as HTMLInputElement | null;
            const display = document.getElementById(valId);
            if (!slider) return;
            const saved = loadSlider("ol-slider-" + id, NaN);
            if (!isNaN(saved)) slider.value = saved.toString();
            const update = () => {
                const v = parseFloat(slider.value);
                if (display) display.textContent = format(v);
                updateThumbColor(slider, stopsKey ? stopsKey() : undefined);
                if (display)
                    display.style.color =
                        slider.style.getPropertyValue("--thumb-color");
                saveSlider("ol-slider-" + id, v);
                apply(v);
            };
            slider.addEventListener("input", update);
            update();
        };

        // Temperature — hypothesis-aware
        const tempSlider = document.getElementById(
            "ol-temp",
        ) as HTMLInputElement | null;
        const tempVal = document.getElementById("ol-temp-val");
        if (tempSlider) {
            const saved = loadSlider(
                sliderKey(this.hypothesisKeys, "temp", this.activeHypothesis),
                this.sliderDefaults.temperature,
            );
            tempSlider.value = saved.toString();
            this.sliderDefaults.temperature = saved;
            const update = () => {
                const v = parseFloat(tempSlider.value);
                if (tempVal) tempVal.textContent = `${v}°C`;
                updateThumbColor(
                    tempSlider,
                    this.activeHypothesis === "wlp" ? "ol-temp-wlp" : "ol-temp",
                );
                if (tempVal)
                    tempVal.style.color =
                        tempSlider.style.getPropertyValue("--thumb-color");
                saveSlider(
                    sliderKey(
                        this.hypothesisKeys,
                        "temp",
                        this.activeHypothesis,
                    ),
                    v,
                );
                this.sliderDefaults.temperature = v;
                if (this.engine && !this.engineBusy) {
                    this.engine.set_temperature(v);
                    const dur = 1800 - (1620 * (v - 3)) / 157;
                    this.engine.set_rules_lerp_duration(Math.round(dur));
                }
            };
            tempSlider.addEventListener("input", update);
            update();
        }

        // pH / UV — hypothesis-aware
        const phSlider = document.getElementById(
            "ol-ph",
        ) as HTMLInputElement | null;
        const phVal = document.getElementById("ol-ph-val");
        if (phSlider) {
            const saved = loadSlider(
                sliderKey(this.hypothesisKeys, "ph", this.activeHypothesis),
                this.sliderDefaults.ph,
            );
            phSlider.value = saved.toString();
            this.sliderDefaults.ph = saved;
            const update = () => {
                const v = parseFloat(phSlider.value);
                if (phVal)
                    phVal.textContent =
                        this.activeHypothesis === "wlp"
                            ? v.toFixed(1) + " UV"
                            : v.toFixed(1);
                updateThumbColor(
                    phSlider,
                    this.activeHypothesis === "wlp" ? "ol-ph-wlp" : "ol-ph",
                );
                if (phVal)
                    phVal.style.color =
                        phSlider.style.getPropertyValue("--thumb-color");
                saveSlider(
                    sliderKey(this.hypothesisKeys, "ph", this.activeHypothesis),
                    v,
                );
                this.sliderDefaults.ph = v;
                if (!this.engine || this.engineBusy) return;
                if (this.activeHypothesis === "wlp") this.engine.set_uv(v);
                else this.engine.set_ph(v);
            };
            phSlider.addEventListener("input", update);
            update();
        }

        // Electrical activity — hypothesis-aware
        const elecSlider = document.getElementById(
            "ol-elec",
        ) as HTMLInputElement | null;
        const elecVal = document.getElementById("ol-elec-val");
        if (elecSlider) {
            const saved = loadSlider(
                sliderKey(this.hypothesisKeys, "elec", this.activeHypothesis),
                this.sliderDefaults.electricalActivity,
            );
            elecSlider.value = saved.toString();
            this.sliderDefaults.electricalActivity = saved;
            const update = () => {
                const v = parseFloat(elecSlider.value);
                if (elecVal) elecVal.textContent = v.toFixed(2);
                updateThumbColor(
                    elecSlider,
                    this.activeHypothesis === "wlp" ? "ol-elec-wlp" : "ol-elec",
                );
                if (elecVal)
                    elecVal.style.color =
                        elecSlider.style.getPropertyValue("--thumb-color");
                saveSlider(
                    sliderKey(
                        this.hypothesisKeys,
                        "elec",
                        this.activeHypothesis,
                    ),
                    v,
                );
                this.sliderDefaults.electricalActivity = v;
                if (this.engine && !this.engineBusy)
                    this.engine.set_electrical_activity(v);
            };
            elecSlider.addEventListener("input", update);
            update();
        }

        // Pressure — niet hypothesis-specifiek
        wire(
            "ol-pres",
            "ol-pres-val",
            (v) => `${v}m`,
            (v) => {
                if (this.engine && !this.engineBusy) {
                    this.engine.set_pressure(v);
                    this.engine.set_particle_count_from_pressure(v);
                    this.applyHypothesis(
                        v < WLP_DEPTH_THRESHOLD ? "wlp" : "htv",
                    );
                }
            },
        );

        // Clamp depth slider: snap to 0 on blur when value < 50m
        const presSlider = document.getElementById(
            "ol-pres",
        ) as HTMLInputElement | null;
        presSlider?.addEventListener("blur", () => {
            if (
                presSlider &&
                parseFloat(presSlider.value) < WLP_DEPTH_THRESHOLD
            ) {
                presSlider.value = "0";
                presSlider.dispatchEvent(new Event("input"));
            }
        });
    }

    private applyHypothesis(hypothesis: Hypothesis) {
        if (hypothesis === this.activeHypothesis) return;
        this.activeHypothesis = hypothesis;

        const t = I18N[getLang()];
        const vals = applyHypothesisToSliders(
            hypothesis,
            this.sliderIds,
            this.hypothesisKeys,
            t.ph,
            t.uv,
            this.sliderDefaults,
        );
        this.sliderDefaults = vals;

        if (this.engine && !this.engineBusy) {
            this.engine.set_temperature(vals.temperature);
            if (hypothesis === "wlp") this.engine.set_uv(vals.ph);
            else this.engine.set_ph(vals.ph);
            this.engine.set_electrical_activity(vals.electricalActivity);
        }
    }

    private applyZoomPan() {
        if (this.engine && !this.engineBusy) {
            this.engine.set_zoom(this.zoom, this.panX, this.panY);
        }
    }

    private constrainPan() {
        const halfW = this.virtualWorldWidth / this.zoom / 2;
        const halfH = this.virtualWorldHeight / this.zoom / 2;
        this.panX = Math.max(
            halfW,
            Math.min(this.virtualWorldWidth - halfW, this.panX),
        );
        this.panY = Math.max(
            halfH,
            Math.min(this.virtualWorldHeight - halfH, this.panY),
        );
    }

    private wireZoomPan() {
        const canvas = this.canvas!;

        canvas.addEventListener(
            "wheel",
            (e) => {
                e.preventDefault();
                const rect = canvas.getBoundingClientRect();
                const cx = (e.clientX - rect.left) / rect.width;
                const cy = (e.clientY - rect.top) / rect.height;

                const vw = this.virtualWorldWidth / this.zoom;
                const vh = this.virtualWorldHeight / this.zoom;
                const worldX = this.panX + (cx - 0.5) * vw;
                const worldY = this.panY + (cy - 0.5) * vh;

                const factor = e.deltaY < 0 ? 1.03 : 1 / 1.03;
                this.zoom = Math.max(
                    ZOOM_MIN,
                    Math.min(ZOOM_MAX, this.zoom * factor),
                );

                const nvw = this.virtualWorldWidth / this.zoom;
                const nvh = this.virtualWorldHeight / this.zoom;
                this.panX = worldX - (cx - 0.5) * nvw;
                this.panY = worldY - (cy - 0.5) * nvh;
                this.constrainPan();
                this.applyZoomPan();
            },
            { passive: false },
        );

        canvas.addEventListener("mousedown", (e) => {
            if (e.button !== 0) return;
            this.isDragging = true;
            this.dragStartX = e.clientX;
            this.dragStartY = e.clientY;
            this.dragStartPanX = this.panX;
            this.dragStartPanY = this.panY;
            canvas.classList.add("dragging");
        });

        window.addEventListener("mousemove", (e) => {
            if (!this.isDragging) return;
            const rect = canvas.getBoundingClientRect();
            const sx = this.virtualWorldWidth / (this.zoom * rect.width);
            const sy = this.virtualWorldHeight / (this.zoom * rect.height);
            this.panX = this.dragStartPanX - (e.clientX - this.dragStartX) * sx;
            this.panY = this.dragStartPanY - (e.clientY - this.dragStartY) * sy;
            this.constrainPan();
            this.applyZoomPan();
        });

        window.addEventListener("mouseup", () => {
            if (this.isDragging) {
                this.isDragging = false;
                canvas.classList.remove("dragging");
            }
        });

        canvas.addEventListener(
            "touchstart",
            (e) => {
                e.preventDefault();
                if (e.touches.length === 1) {
                    this.isDragging = true;
                    this.dragStartX = e.touches[0].clientX;
                    this.dragStartY = e.touches[0].clientY;
                    this.dragStartPanX = this.panX;
                    this.dragStartPanY = this.panY;
                    this.lastPinchDist = 0;
                } else if (e.touches.length === 2) {
                    this.isDragging = false;
                    const t0 = e.touches[0],
                        t1 = e.touches[1];
                    this.lastPinchDist = Math.hypot(
                        t1.clientX - t0.clientX,
                        t1.clientY - t0.clientY,
                    );
                    this.lastPinchMidX = (t0.clientX + t1.clientX) / 2;
                    this.lastPinchMidY = (t0.clientY + t1.clientY) / 2;
                }
            },
            { passive: false },
        );

        canvas.addEventListener(
            "touchmove",
            (e) => {
                e.preventDefault();
                const rect = canvas.getBoundingClientRect();

                if (e.touches.length === 1 && this.isDragging) {
                    const sx =
                        this.virtualWorldWidth / (this.zoom * rect.width);
                    const sy =
                        this.virtualWorldHeight / (this.zoom * rect.height);
                    this.panX =
                        this.dragStartPanX -
                        (e.touches[0].clientX - this.dragStartX) * sx;
                    this.panY =
                        this.dragStartPanY -
                        (e.touches[0].clientY - this.dragStartY) * sy;
                    this.constrainPan();
                    this.applyZoomPan();
                } else if (e.touches.length === 2) {
                    const t0 = e.touches[0],
                        t1 = e.touches[1];
                    const dist = Math.hypot(
                        t1.clientX - t0.clientX,
                        t1.clientY - t0.clientY,
                    );
                    const midX = (t0.clientX + t1.clientX) / 2;
                    const midY = (t0.clientY + t1.clientY) / 2;

                    if (this.lastPinchDist > 0) {
                        const cx = (midX - rect.left) / rect.width;
                        const cy = (midY - rect.top) / rect.height;
                        const vw = this.virtualWorldWidth / this.zoom;
                        const vh = this.virtualWorldHeight / this.zoom;
                        const worldX = this.panX + (cx - 0.5) * vw;
                        const worldY = this.panY + (cy - 0.5) * vh;

                        this.zoom = Math.max(
                            ZOOM_MIN,
                            Math.min(
                                ZOOM_MAX,
                                this.zoom * (dist / this.lastPinchDist),
                            ),
                        );

                        const nvw = this.virtualWorldWidth / this.zoom;
                        const nvh = this.virtualWorldHeight / this.zoom;
                        this.panX = worldX - (cx - 0.5) * nvw;
                        this.panY = worldY - (cy - 0.5) * nvh;

                        // Also pan with midpoint movement between frames
                        const dmx = midX - this.lastPinchMidX;
                        const dmy = midY - this.lastPinchMidY;
                        this.panX -=
                            dmx *
                            (this.virtualWorldWidth / (this.zoom * rect.width));
                        this.panY -=
                            dmy *
                            (this.virtualWorldHeight /
                                (this.zoom * rect.height));

                        this.constrainPan();
                        this.applyZoomPan();
                    }

                    this.lastPinchDist = dist;
                    this.lastPinchMidX = midX;
                    this.lastPinchMidY = midY;
                }
            },
            { passive: false },
        );

        canvas.addEventListener(
            "touchend",
            (e) => {
                if (e.touches.length === 0) {
                    this.isDragging = false;
                    this.lastPinchDist = 0;
                } else if (e.touches.length === 1) {
                    this.isDragging = true;
                    this.dragStartX = e.touches[0].clientX;
                    this.dragStartY = e.touches[0].clientY;
                    this.dragStartPanX = this.panX;
                    this.dragStartPanY = this.panY;
                    this.lastPinchDist = 0;
                }
            },
            { passive: false },
        );
    }

    private togglePause() {
        this.isPaused = !this.isPaused;
        const btn = document.getElementById("ol-pause-btn");
        if (btn) btn.innerHTML = `<span class="ol-material-icon">${this.isPaused ? "play_arrow" : "pause"}</span>`;
    }

    private wireCapture() {
        document
            .getElementById("ol-pause-btn")
            ?.addEventListener("click", () => this.togglePause());
        document
            .getElementById("ol-screenshot-btn")
            ?.addEventListener("click", () => this.captureScreenshot());
        document
            .getElementById("ol-record-btn")
            ?.addEventListener("click", () => this.toggleRecording());

        document.addEventListener("keydown", (e: KeyboardEvent) => {
            const el = document.activeElement;
            if (el instanceof HTMLTextAreaElement ||
                (el instanceof HTMLInputElement && el.type !== "range")) return;
            if (e.key === "p" || e.key === "P") this.togglePause();
            if (e.key === "s" || e.key === "S") this.captureScreenshot();
            if (e.key === "v" || e.key === "V") this.toggleRecording();
        });
    }

    private captureScreenshot() {
        if (!this.canvas) return;
        const flash = document.getElementById(
            "ol-screenshot-flash",
        ) as HTMLElement | null;
        if (flash) {
            flash.style.opacity = "0.7";
            setTimeout(() => {
                flash.style.opacity = "0";
            }, 150);
        }
        this.canvas.toBlob((blob) => {
            if (!blob) return;
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a");
            a.href = url;
            a.download = `origin-of-life-${Date.now()}.png`;
            a.click();
            URL.revokeObjectURL(url);
        }, "image/png");
    }

    private toggleRecording() {
        if (this.mediaRecorder && this.mediaRecorder.state !== "inactive") {
            this.mediaRecorder.stop();
            return;
        }
        if (!this.canvas) return;
        const stream = this.canvas.captureStream(60);
        const mimeType = MediaRecorder.isTypeSupported("video/webm;codecs=vp9")
            ? "video/webm;codecs=vp9"
            : "video/webm";
        this.recordedChunks = [];
        this.mediaRecorder = new MediaRecorder(stream, { mimeType });
        this.mediaRecorder.ondataavailable = (e) => {
            if (e.data.size > 0) this.recordedChunks.push(e.data);
        };
        this.mediaRecorder.onstop = () => {
            const blob = new Blob(this.recordedChunks, { type: mimeType });
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a");
            a.href = url;
            a.download = `origin-of-life-${Date.now()}.webm`;
            a.click();
            URL.revokeObjectURL(url);
            this.recordedChunks = [];
            document
                .getElementById("ol-record-btn")
                ?.classList.remove("recording");
        };
        this.mediaRecorder.start();
        document.getElementById("ol-record-btn")?.classList.add("recording");
    }

    private startLoop() {
        this.lastTime = performance.now();

        const animate = (now: number) => {
            const dt = Math.min((now - this.lastTime) / 1000, 0.05);
            this.lastTime = now;

            if (!this.isPaused && this.engine && !this.engineBusy) {
                try {
                    this.engine.update_frame(dt);
                    this.engine.render();
                } catch (e) {
                    console.error("Origin of Life simulation error:", e);
                }
            }

            this.animationId = requestAnimationFrame(animate);
        };

        this.animationId = requestAnimationFrame(animate);
    }

    destroy() {
        if (this.animationId) cancelAnimationFrame(this.animationId);
        if (this.engine) {
            this.engine.free();
            this.engine = null;
        }
    }
}

// Works whether DOMContentLoaded has already fired or not
function whenReady(fn: () => void) {
    if (document.readyState === "loading") {
        document.addEventListener("DOMContentLoaded", fn, { once: true });
    } else {
        fn();
    }
}

whenReady(async () => {
    const app = new EmbedApp();
    await app.init();
    window.addEventListener("beforeunload", () => app.destroy());
});

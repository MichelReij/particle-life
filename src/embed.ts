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

const WORLD_CENTER_X = VIRTUAL_WORLD_WIDTH / 2;
const WORLD_CENTER_Y = VIRTUAL_WORLD_HEIGHT / 2;

type Lang = "nl" | "en" | "fr";

const I18N: Record<Lang, {
    loading: string;
    hint: string;
    playTitle: string;
    pauseTitle: string;
    startText: string;
    startOk: string;
    startCancel: string;
    stopText: string;
    stopOk: string;
    temp: string;
    ph: string;
    depth: string;
    electricity: string;
}> = {
    nl: {
        loading:      "Simulatie laden…",
        hint:         "scroll / knijp om te zoomen · sleep om te bewegen",
        playTitle:    "Start simulatie",
        pauseTitle:   "Stop simulatie",
        startText:    "Deze simulatie gebruikt je grafische processor intensief en werkt mogelijk niet goed op oudere apparaten. Ook verbruikt het veel energie, waardoor de batterij van je telefoon of laptop snel leegloopt.",
        startOk:      "Starten",
        startCancel:  "Annuleren",
        stopText:     "De simulatie is gepauzeerd. Je batterij is nu weer veilig.",
        stopOk:       "Sluiten",
        temp:         "Temperatuur",
        ph:           "pH",
        depth:        "Zeedepte",
        electricity:  "Elektriciteit",
    },
    en: {
        loading:      "Loading simulation…",
        hint:         "scroll / pinch to zoom · drag to pan",
        playTitle:    "Start simulation",
        pauseTitle:   "Stop simulation",
        startText:    "This simulation uses your graphics processor intensively and may not run well on older devices. It also consumes a lot of energy, which will drain the battery of your phone or laptop quickly.",
        startOk:      "Start",
        startCancel:  "Cancel",
        stopText:     "The simulation has stopped. Your battery is safe again.",
        stopOk:       "Close",
        temp:         "Temperature",
        ph:           "pH",
        depth:        "Sea depth",
        electricity:  "Electricity",
    },
    fr: {
        loading:      "Chargement de la simulation…",
        hint:         "molette / pincer pour zoomer · glisser pour déplacer",
        playTitle:    "Démarrer la simulation",
        pauseTitle:   "Arrêter la simulation",
        startText:    "Cette simulation sollicite intensément votre processeur graphique et peut ne pas fonctionner correctement sur les appareils plus anciens. Elle consomme également beaucoup d'énergie, ce qui déchargera rapidement la batterie de votre téléphone ou ordinateur portable.",
        startOk:      "Démarrer",
        startCancel:  "Annuler",
        stopText:     "La simulation est arrêtée. Votre batterie est à nouveau en sécurité.",
        stopOk:       "Fermer",
        temp:         "Température",
        ph:           "pH",
        depth:        "Profondeur",
        electricity:  "Électricité",
    },
};

// Gradient stop: [percentage 0-100, r, g, b]
type Stop = [number, number, number, number];

function gradientColor(pct: number, stops: Stop[]): string {
    if (pct <= stops[0][0]) return `rgb(${stops[0][1]},${stops[0][2]},${stops[0][3]})`;
    if (pct >= stops[stops.length - 1][0]) { const s = stops[stops.length - 1]; return `rgb(${s[1]},${s[2]},${s[3]})`; }
    for (let i = 0; i < stops.length - 1; i++) {
        const [p0, r0, g0, b0] = stops[i];
        const [p1, r1, g1, b1] = stops[i + 1];
        if (pct >= p0 && pct <= p1) {
            const t = (pct - p0) / (p1 - p0);
            return `rgb(${Math.round(r0 + t*(r1-r0))},${Math.round(g0 + t*(g1-g0))},${Math.round(b0 + t*(b1-b0))})`;
        }
    }
    return `rgb(192,57,43)`;
}

const RED:    [number,number,number] = [192,  57, 43];
const YELLOW: [number,number,number] = [243, 156, 18];
const GREEN:  [number,number,number] = [ 39, 174, 96];

// Stops per slider in percentage, matching the CSS gradients exactly
const SLIDER_STOPS: Record<string, Stop[]> = {
    "ol-temp": [
        [  0, ...RED],   [49.0, ...RED],
        [58.6, ...YELLOW], [64.9, ...GREEN],
        [71.3, ...GREEN], [77.7, ...YELLOW],
        [85.0, ...RED],  [100, ...RED],
    ],
    "ol-pres": [
        [  0, ...RED],  [20.0, ...RED],
        [35.0, ...YELLOW], [50.0, ...GREEN],
        [100, ...GREEN],
    ],
    "ol-ph": [
        [  0, ...RED],   [57.1, ...RED],
        [64.3, ...YELLOW], [71.4, ...GREEN],
        [78.6, ...GREEN], [85.7, ...YELLOW],
        [92.0, ...RED],  [100, ...RED],
    ],
    "ol-elec": [
        [  0, ...RED],   [60.0, ...RED],
        [66.7, ...YELLOW], [70.0, ...GREEN],
        [73.3, ...GREEN], [80.0, ...YELLOW],
        [87.0, ...RED],  [100, ...RED],
    ],
};

function updateThumbColor(slider: HTMLInputElement) {
    const stops = SLIDER_STOPS[slider.id];
    if (!stops) return;
    const pct = (parseFloat(slider.value) - parseFloat(slider.min)) /
                (parseFloat(slider.max)  - parseFloat(slider.min)) * 100;
    slider.style.setProperty("--thumb-color", gradientColor(pct, stops));
}

function getLang(): Lang {
    const wrap = document.getElementById("ol-wrap");
    const raw = (wrap?.dataset.lang ?? "en").toLowerCase();
    return (raw === "nl" || raw === "fr") ? raw : "en";
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
#ol-wrap {
    display: flex;
    flex-direction: column;
    width: 100%;
    background: transparent;
    border-radius: 6px;
    overflow: hidden;
    font-family: system-ui, sans-serif;
    font-size: 13px;
    color: #ccc;
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
    color: #7aaff0;
    font-size: 14px;
    pointer-events: none;
    text-shadow: 0 1px 4px rgba(0,0,0,0.8);
}
#ol-hint {
    position: absolute;
    bottom: 6px;
    right: 8px;
    font-size: 10px;
    color: #ccc;
    pointer-events: none;
    user-select: none;
}
#ol-playpause {
    position: absolute;
    top: 0;
    right: 0;
    width: 11%;
    aspect-ratio: 1 / 1;
    border-radius: 50%;
    border: none;
    background: rgba(40,140,60,0.85);
    color: #fff;
    font-size: 1.5em;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10;
    transition: background 0.15s;
    line-height: 1;
    padding: 0;
}
#ol-playpause:hover {
    background: rgba(40,140,60,1);
}
#ol-playpause.running {
    background: rgba(180,30,30,0.85);
}
#ol-playpause.running:hover {
    background: rgba(180,30,30,1);
}
#ol-modal-start, #ol-modal-stop {
    position: absolute;
    inset: 0;
    background: rgba(0,0,0,0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 20;
    border-radius: 50%;
}
#ol-modal {
    background: #1a1a2a;
    border: 1px solid #333;
    border-radius: 12px;
    padding: 18px 20px;
    max-width: 80%;
    color: #ddd;
    font-size: 12px;
    line-height: 1.5;
    text-align: center;
}
#ol-modal p {
    margin: 0 0 14px;
}
#ol-modal-ok {
    background: rgba(40,140,60,0.9);
    color: #fff;
    border: none;
    border-radius: 6px;
    padding: 7px 18px;
    font-size: 12px;
    cursor: pointer;
    margin-right: 8px;
}
#ol-modal-ok:hover {
    background: rgba(40,140,60,1);
}
#ol-modal-cancel {
    background: rgba(80,80,80,0.6);
    color: #ccc;
    border: none;
    border-radius: 6px;
    padding: 7px 18px;
    font-size: 12px;
    cursor: pointer;
}
#ol-modal-cancel:hover {
    background: rgba(80,80,80,0.9);
}
#ol-controls {
    display: flex;
    flex-wrap: wrap;
    gap: 8px 18px;
    padding: 10px 14px;
    background: transparent;
    border-top: none;
    align-items: center;
    justify-content: center;
}
.ol-slider-group {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    flex: 1 1 45%;
    max-width: 48%;
}
.ol-slider-group label {
    color: #999;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    white-space: nowrap;
    text-shadow: 0 1px 3px rgba(0,0,0,0.9);
}
.ol-slider-group .ol-val {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    min-width: 3.5em;
    text-align: center;
    text-shadow: 0 1px 3px rgba(0,0,0,0.9);
}
.ol-slider-group input[type="range"] {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 6px;
    border-radius: 3px;
    outline: none;
    cursor: pointer;
    margin: 0;
}
.ol-slider-group input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--thumb-color, #4aaff0);
    border: none;
    cursor: pointer;
    box-shadow: 0 1px 3px rgba(0,0,0,0.5);
}
.ol-slider-group input[type="range"]::-moz-range-thumb {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--thumb-color, #4aaff0);
    border: none;
    cursor: pointer;
    box-shadow: 0 1px 3px rgba(0,0,0,0.5);
}

/* Temp (3–160°C), range=157: rood<80, geel 80–95, groen 95–115, geel 115–125, rood>125
   (80-3)/157=49.0%  (95-3)/157=58.6%  (115-3)/157=71.3%  (125-3)/157=77.7% */
#ol-temp { background: linear-gradient(to right,
    #c0392b 49.0%, #f39c12 58.6%, #27ae60 64.9%,
    #27ae60 71.3%, #f39c12 77.7%, #c0392b 85%); }
#ol-temp::-webkit-slider-runnable-track { background: transparent; }
#ol-temp::-moz-range-track { background: transparent; }

/* Diepte (0–1000m): rood<200, geel 200–500, groen 500–1000 */
#ol-pres { background: linear-gradient(to right,
    #c0392b 20%, #f39c12 35%, #27ae60 50%); }
#ol-pres::-webkit-slider-runnable-track { background: transparent; }
#ol-pres::-moz-range-track { background: transparent; }

/* pH (0–14): rood<8, geel 8–9, groen 9–11, geel 11–12, rood>12
   8/14=57.1%  9/14=64.3%  11/14=78.6%  12/14=85.7% */
#ol-ph { background: linear-gradient(to right,
    #c0392b 57.1%, #f39c12 64.3%, #27ae60 71.4%,
    #27ae60 78.6%, #f39c12 85.7%, #c0392b 92%); }
#ol-ph::-webkit-slider-runnable-track { background: transparent; }
#ol-ph::-moz-range-track { background: transparent; }

/* Elec (0–3 kJ): rood<1.8, geel 1.8–2.0, groen 2.0–2.2, geel 2.2–2.4, rood>2.4
   1.8/3=60.0%  2.0/3=66.7%  2.2/3=73.3%  2.4/3=80.0% */
#ol-elec { background: linear-gradient(to right,
    #c0392b 60.0%, #f39c12 66.7%, #27ae60 70.0%,
    #27ae60 73.3%, #f39c12 80.0%, #c0392b 87%); }
#ol-elec::-webkit-slider-runnable-track { background: transparent; }
#ol-elec::-moz-range-track { background: transparent; }
`;

function injectStyles() {
    if (document.getElementById("ol-embed-styles")) return;
    const style = document.createElement("style");
    style.id = "ol-embed-styles";
    style.textContent = EMBED_CSS;
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
            <div id="ol-hint">${t.hint}</div>
            <button id="ol-playpause" title="${t.playTitle}">▶</button>
            <div id="ol-modal-start" style="display:none">
                <div id="ol-modal">
                    <p>${t.startText}</p>
                    <button id="ol-modal-start-ok">${t.startOk}</button>
                    <button id="ol-modal-start-cancel">${t.startCancel}</button>
                </div>
            </div>
            <div id="ol-modal-stop" style="display:none">
                <div id="ol-modal">
                    <p>${t.stopText}</p>
                    <button id="ol-modal-stop-ok">${t.stopOk}</button>
                </div>
            </div>
        </div>
        <div id="ol-controls">
            <div class="ol-slider-group">
                <label>${t.temp}</label>
                <input type="range" id="ol-temp" min="3" max="160" value="20" step="1" />
                <span class="ol-val" id="ol-temp-val">20°C</span>
            </div>
            <div class="ol-slider-group">
                <label>${t.ph}</label>
                <input type="range" id="ol-ph" min="0" max="14" value="7" step="0.1" />
                <span class="ol-val" id="ol-ph-val">7.0</span>
            </div>
            <div class="ol-slider-group">
                <label>${t.depth}</label>
                <input type="range" id="ol-pres" min="0" max="1000" value="1" step="1" />
                <span class="ol-val" id="ol-pres-val">1m</span>
            </div>
            <div class="ol-slider-group">
                <label>${t.electricity}</label>
                <input type="range" id="ol-elec" min="0" max="3" value="1.02" step="0.01" />
                <span class="ol-val" id="ol-elec-val">1.02</span>
            </div>
        </div>
    `;
}

class EmbedApp {
    private engine: ParticleLifeEngine | null = null;
    private canvas: HTMLCanvasElement | null = null;
    private animationId: number | null = null;
    private engineBusy = false;
    private lastTime = 0;
    private running = false;

    private zoom = 1.0;
    private panX = WORLD_CENTER_X;
    private panY = WORLD_CENTER_Y;

    private isDragging = false;
    private dragStartX = 0;
    private dragStartY = 0;
    private dragStartPanX = 0;
    private dragStartPanY = 0;

    private lastPinchDist = 0;
    private lastPinchMidX = 0;
    private lastPinchMidY = 0;

    async init() {
        injectStyles();
        buildDOM();

        this.canvas = document.getElementById("ol-canvas") as HTMLCanvasElement;
        if (!this.canvas) return;

        this.canvas.width = CANVAS_WIDTH;
        this.canvas.height = CANVAS_HEIGHT;

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

        this.wireSliders();
        this.wireZoomPan();
        this.wirePlayPause();

        const status = document.getElementById("ol-status");
        if (status) status.style.display = "none";
    }

    private setStatus(msg: string) {
        const el = document.getElementById("ol-status");
        if (el) el.textContent = msg;
    }

    private wirePlayPause() {
        const btn         = document.getElementById("ol-playpause") as HTMLButtonElement;
        const startModal  = document.getElementById("ol-modal-start");
        const stopModal   = document.getElementById("ol-modal-stop");
        const startOk     = document.getElementById("ol-modal-start-ok");
        const startCancel = document.getElementById("ol-modal-start-cancel");
        const stopOk      = document.getElementById("ol-modal-stop-ok");
        if (!btn || !startModal || !stopModal || !startOk || !startCancel || !stopOk) return;

        const t = I18N[getLang()];

        const startSim = () => {
            this.running = true;
            btn.textContent = "⏸";
            btn.title = t.pauseTitle;
            btn.classList.add("running");
            this.lastTime = performance.now();
            this.startLoop();
        };

        const stopSim = () => {
            this.running = false;
            btn.textContent = "▶";
            btn.title = t.playTitle;
            btn.classList.remove("running");
            if (this.animationId !== null) {
                cancelAnimationFrame(this.animationId);
                this.animationId = null;
            }
        };

        btn.addEventListener("click", () => {
            if (this.running) {
                stopSim();
                stopModal.style.display = "flex";
            } else {
                startModal.style.display = "flex";
            }
        });

        startOk.addEventListener("click", () => {
            startModal.style.display = "none";
            startSim();
        });

        startCancel.addEventListener("click", () => {
            startModal.style.display = "none";
        });

        stopOk.addEventListener("click", () => {
            stopModal.style.display = "none";
        });
    }

    private wireSliders() {
        const wire = (
            id: string,
            valId: string,
            format: (v: number) => string,
            apply: (v: number) => void,
        ) => {
            const slider = document.getElementById(id) as HTMLInputElement | null;
            const display = document.getElementById(valId);
            if (!slider) return;
            const update = () => {
                const v = parseFloat(slider.value);
                if (display) display.textContent = format(v);
                updateThumbColor(slider);
                if (display) display.style.color = slider.style.getPropertyValue("--thumb-color");
                apply(v);
            };
            slider.addEventListener("input", update);
            update();
        };

        wire("ol-temp", "ol-temp-val", (v) => `${v}°C`, (v) => {
            if (this.engine && !this.engineBusy) {
                this.engine.set_temperature(v);
                const dur = 1800 - (1620 * (v - 3)) / 157;
                this.engine.set_rules_lerp_duration(Math.round(dur));
            }
        });

        wire("ol-ph", "ol-ph-val", (v) => v.toFixed(1), (v) => {
            if (this.engine && !this.engineBusy) this.engine.set_ph(v);
        });

        wire("ol-pres", "ol-pres-val", (v) => `${v}m`, (v) => {
            if (this.engine && !this.engineBusy) {
                this.engine.set_pressure(v);
                this.engine.set_particle_count_from_pressure(v);
            }
        });

        wire("ol-elec", "ol-elec-val", (v) => v.toFixed(2), (v) => {
            if (this.engine && !this.engineBusy)
                this.engine.set_electrical_activity(v);
        });
    }

    private applyZoomPan() {
        if (this.engine && !this.engineBusy) {
            this.engine.set_zoom(this.zoom, this.panX, this.panY);
        }
    }

    private constrainPan() {
        const halfW = VIRTUAL_WORLD_WIDTH / this.zoom / 2;
        const halfH = VIRTUAL_WORLD_HEIGHT / this.zoom / 2;
        this.panX = Math.max(halfW, Math.min(VIRTUAL_WORLD_WIDTH - halfW, this.panX));
        this.panY = Math.max(halfH, Math.min(VIRTUAL_WORLD_HEIGHT - halfH, this.panY));
    }

    private wireZoomPan() {
        const canvas = this.canvas!;

        canvas.addEventListener("wheel", (e) => {
            e.preventDefault();
            const rect = canvas.getBoundingClientRect();
            const cx = (e.clientX - rect.left) / rect.width;
            const cy = (e.clientY - rect.top) / rect.height;

            const vw = VIRTUAL_WORLD_WIDTH / this.zoom;
            const vh = VIRTUAL_WORLD_HEIGHT / this.zoom;
            const worldX = this.panX + (cx - 0.5) * vw;
            const worldY = this.panY + (cy - 0.5) * vh;

            const factor = e.deltaY < 0 ? 1.03 : 1 / 1.03;
            this.zoom = Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, this.zoom * factor));

            const nvw = VIRTUAL_WORLD_WIDTH / this.zoom;
            const nvh = VIRTUAL_WORLD_HEIGHT / this.zoom;
            this.panX = worldX - (cx - 0.5) * nvw;
            this.panY = worldY - (cy - 0.5) * nvh;
            this.constrainPan();
            this.applyZoomPan();
        }, { passive: false });

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
            const sx = VIRTUAL_WORLD_WIDTH / (this.zoom * rect.width);
            const sy = VIRTUAL_WORLD_HEIGHT / (this.zoom * rect.height);
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

        canvas.addEventListener("touchstart", (e) => {
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
                const t0 = e.touches[0], t1 = e.touches[1];
                this.lastPinchDist = Math.hypot(t1.clientX - t0.clientX, t1.clientY - t0.clientY);
                this.lastPinchMidX = (t0.clientX + t1.clientX) / 2;
                this.lastPinchMidY = (t0.clientY + t1.clientY) / 2;
            }
        }, { passive: false });

        canvas.addEventListener("touchmove", (e) => {
            e.preventDefault();
            const rect = canvas.getBoundingClientRect();

            if (e.touches.length === 1 && this.isDragging) {
                const sx = VIRTUAL_WORLD_WIDTH / (this.zoom * rect.width);
                const sy = VIRTUAL_WORLD_HEIGHT / (this.zoom * rect.height);
                this.panX = this.dragStartPanX - (e.touches[0].clientX - this.dragStartX) * sx;
                this.panY = this.dragStartPanY - (e.touches[0].clientY - this.dragStartY) * sy;
                this.constrainPan();
                this.applyZoomPan();
            } else if (e.touches.length === 2) {
                const t0 = e.touches[0], t1 = e.touches[1];
                const dist = Math.hypot(t1.clientX - t0.clientX, t1.clientY - t0.clientY);
                const midX = (t0.clientX + t1.clientX) / 2;
                const midY = (t0.clientY + t1.clientY) / 2;

                if (this.lastPinchDist > 0) {
                    const cx = (midX - rect.left) / rect.width;
                    const cy = (midY - rect.top) / rect.height;
                    const vw = VIRTUAL_WORLD_WIDTH / this.zoom;
                    const vh = VIRTUAL_WORLD_HEIGHT / this.zoom;
                    const worldX = this.panX + (cx - 0.5) * vw;
                    const worldY = this.panY + (cy - 0.5) * vh;

                    this.zoom = Math.max(ZOOM_MIN, Math.min(ZOOM_MAX, this.zoom * (dist / this.lastPinchDist)));

                    const nvw = VIRTUAL_WORLD_WIDTH / this.zoom;
                    const nvh = VIRTUAL_WORLD_HEIGHT / this.zoom;
                    this.panX = worldX - (cx - 0.5) * nvw;
                    this.panY = worldY - (cy - 0.5) * nvh;

                    // Also pan with midpoint movement between frames
                    const dmx = midX - this.lastPinchMidX;
                    const dmy = midY - this.lastPinchMidY;
                    this.panX -= dmx * (VIRTUAL_WORLD_WIDTH / (this.zoom * rect.width));
                    this.panY -= dmy * (VIRTUAL_WORLD_HEIGHT / (this.zoom * rect.height));

                    this.constrainPan();
                    this.applyZoomPan();
                }

                this.lastPinchDist = dist;
                this.lastPinchMidX = midX;
                this.lastPinchMidY = midY;
            }
        }, { passive: false });

        canvas.addEventListener("touchend", (e) => {
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
        }, { passive: false });
    }

    private startLoop() {
        this.lastTime = performance.now();

        const animate = (now: number) => {
            const dt = Math.min((now - this.lastTime) / 1000, 0.05);
            this.lastTime = now;

            if (this.engine && !this.engineBusy) {
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
        if (this.engine) { this.engine.free(); this.engine = null; }
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

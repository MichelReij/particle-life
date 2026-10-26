// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// Mengpaneel voor de ambient drone: één fader per klankbron.
// Volgorde moet gelijk blijven aan dsp::mix in src/dsp.rs.

const CHANNELS: { key: string; label: string; title: string }[] = [
    { key: "cellos", label: "Cellos", title: "Bowed cellos" },
    { key: "theremin", label: "Theremin", title: "Theremin phrases" },
    { key: "brainwaves", label: "Brainwaves", title: "Deep, dark bass drone with slow chord changes" },
    { key: "droplets", label: "Droplets", title: "Tuned water droplets" },
    { key: "stream", label: "Stream", title: "Murmuring stream" },
    { key: "surf", label: "Surf", title: "Surf (warm little ponds only)" },
    { key: "thunder", label: "Thunder", title: "Rumble, thunder roll and boom" },
    { key: "lightning", label: "Lightning", title: "Crack and crackle" },
    { key: "sonar", label: "Sonar", title: "Sonar pings with echo (hydrothermal vents only)" },
    { key: "vents", label: "Vents", title: "Rumbling vents with occasional eruptions and bubbles (hydrothermal vents only)" },
    { key: "events", label: "Events", title: "Zoomed in: every visible clump as its own sound, plus bubbling while clumps merge or split" },
    { key: "reverb", label: "Reverb", title: "Reverb over all layers" },
    { key: "master", label: "Master", title: "Overall level" },
];

/** Onderste stand van een fader = uit. */
const MIN_DB = -40;
const MAX_DB = 12;
const STORAGE_KEY = "audio_mix_db_v1";
/** Bliksem-crunch: 0 = uit, CRUNCH_MAX = sample-and-hold over 16 samples. */
const CRUNCH_KEY = "lightningCrunch";
const CRUNCH_MAX = 15;
const CRUNCH_DEFAULT = 3;
const COLLAPSED_KEY = "panel_mixer_collapsed";

function dbToGain(db: number): number {
    return db <= MIN_DB ? 0 : Math.pow(10, db / 20);
}

function formatDb(db: number): string {
    if (db <= MIN_DB) return "off";
    return `${db > 0 ? "+" : ""}${db.toFixed(1)}`;
}

function loadLevels(): Record<string, number> {
    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        if (raw) return JSON.parse(raw);
    } catch {
        // Geen opslag beschikbaar of ongeldige inhoud: standaardwaarden
    }
    return {};
}

function saveLevels(levels: Record<string, number>): void {
    try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(levels));
    } catch {
        // Opslaan is een gemak, geen vereiste
    }
}

/**
 * Bouwt het mengpaneel in #audio-mixer en roept `onChange` aan met de lineaire
 * gains (volgorde: CHANNELS), gevolgd door de sample-and-hold-lengte van de
 * bliksem (dsp::mix::LIGHTNING_HOLD) — direct bij het opstarten met de bewaarde
 * stand, en daarna bij elke beweging.
 */
export function initAudioMixer(onChange: (gains: Float32Array) => void): void {
    const panel = document.getElementById("audio-mixer");
    const body = document.getElementById("audio-mixer-body");
    const toggle = document.getElementById("audio-mixer-toggle");
    const strips = document.getElementById("audio-mixer-strips");
    const reset = document.getElementById("audio-mixer-reset");
    if (!panel || !body || !toggle || !strips) return;

    const levels = loadLevels();
    const sliders: HTMLInputElement[] = [];

    const emit = () => {
        const gains = new Float32Array(CHANNELS.length + 1);
        CHANNELS.forEach((ch, i) => (gains[i] = dbToGain(levels[ch.key] ?? 0)));
        gains[CHANNELS.length] = (levels[CRUNCH_KEY] ?? CRUNCH_DEFAULT) + 1;
        onChange(gains);
        saveLevels(levels);
    };

    CHANNELS.forEach((ch) => {
        const strip = document.createElement("div");
        strip.className = "mixer-strip";
        strip.title = `${ch.title} — double-click for 0 dB`;

        const value = document.createElement("span");
        value.className = "mixer-value";

        const slider = document.createElement("input");
        slider.type = "range";
        slider.min = String(MIN_DB);
        slider.max = String(MAX_DB);
        slider.step = "0.5";
        slider.value = String(levels[ch.key] ?? 0);
        slider.className = "mixer-fader";
        slider.setAttribute("aria-label", ch.label);

        const label = document.createElement("span");
        label.className = "mixer-label";
        label.textContent = ch.label;

        const update = () => {
            const db = parseFloat(slider.value);
            levels[ch.key] = db;
            value.textContent = formatDb(db);
            strip.classList.toggle("muted", db <= MIN_DB);
        };
        slider.addEventListener("input", () => {
            update();
            emit();
        });
        slider.addEventListener("dblclick", () => {
            slider.value = "0";
            update();
            emit();
        });
        update();

        strip.append(value, slider, label);
        strips.appendChild(strip);
        sliders.push(slider);
    });

    // Bliksem-crunch: sample-and-hold na de vervorming
    const crunchRow = document.createElement("div");
    crunchRow.className = "mixer-crunch";
    crunchRow.title =
        "Lightning crunch: the distorted lightning is only updated every N samples " +
        "(sample-and-hold) — digital, lo-fi grit. 0 = off.";
    const crunchLabel = document.createElement("span");
    crunchLabel.textContent = "Lightning crunch";
    const crunch = document.createElement("input");
    crunch.type = "range";
    crunch.min = "0";
    crunch.max = String(CRUNCH_MAX);
    crunch.step = "1";
    crunch.value = String(levels[CRUNCH_KEY] ?? CRUNCH_DEFAULT);
    crunch.setAttribute("aria-label", "Lightning crunch");
    const crunchValue = document.createElement("span");
    crunchValue.className = "mixer-value";
    const updateCrunch = () => {
        const v = parseInt(crunch.value, 10);
        levels[CRUNCH_KEY] = v;
        crunchValue.textContent = v === 0 ? "off" : `÷${v + 1}`;
    };
    crunch.addEventListener("input", () => {
        updateCrunch();
        emit();
    });
    updateCrunch();
    crunchRow.append(crunchLabel, crunch, crunchValue);
    body.insertBefore(crunchRow, reset ?? null);

    reset?.addEventListener("click", () => {
        sliders.forEach((s) => {
            s.value = "0";
            s.dispatchEvent(new Event("input"));
        });
    });

    // Inklappen, zelfde gedrag als de andere panelen
    const setCollapsed = (collapsed: boolean) => {
        body.style.display = collapsed ? "none" : "";
        toggle.textContent = collapsed ? "▶" : "▼";
        try {
            localStorage.setItem(COLLAPSED_KEY, collapsed ? "1" : "0");
        } catch {
            // zie saveLevels
        }
    };
    let startCollapsed = false;
    try {
        startCollapsed = localStorage.getItem(COLLAPSED_KEY) === "1";
    } catch {
        // zie loadLevels
    }
    setCollapsed(startCollapsed);
    toggle.addEventListener("click", () => setCollapsed(body.style.display !== "none"));

    emit();
}

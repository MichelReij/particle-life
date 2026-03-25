// ui.ts - User Interface Controls and DOM Interactions
// This module handles all HTML/DOM interactions, slider controls, and localStorage

import { SimulationParams, BoundaryMode } from "./particle-life-types";
// import { Joy } from "./lib/joy";
declare var Joy: any; // Temporary fix for Joy library
import {
    VIRTUAL_WORLD_CENTER_X,
    VIRTUAL_WORLD_CENTER_Y,
    VIRTUAL_WORLD_WIDTH,
    VIRTUAL_WORLD_HEIGHT,
    CANVAS_WIDTH_U32,
    CANVAS_HEIGHT_U32,
} from "./config";

// Environmental parameters for the UI controls
let temperature = 20; // Default temperature
let electricalActivity = 1.02; // Default electrical activity
let uvLight = 25; // Default UV light (kept for backward compat, unused)
let ph = 7.0; // Default pH (optimum for life is ~10)
let pressure = 1; // Default pressure

// Zoom center variables for joystick navigation
let zoomCenterX = VIRTUAL_WORLD_CENTER_X; // Center X coordinate in virtual world (default: center)
let zoomCenterY = VIRTUAL_WORLD_CENTER_Y; // Center Y coordinate in virtual world (default: center)

// JoyStick variables
let joystick: any;
let joystickForceX = 0.0;
let joystickForceY = 0.0;
let joystickInfluence = 200.0; // Maximum force influence from joystick

// FPS calculation variables
let frameCount = 0;
let lastFPSTime = 0;
let fpsDisplayElement: HTMLElement | null;

// localStorage functionality for persistent settings
const STORAGE_KEYS = {
    temperature: "particleLife_temperature",
    electricalActivity: "particleLife_electricalActivity",
    ph: "particleLife_ph",
    pressure: "particleLife_pressure",
    zoom: "particleLife_zoom",
    drift: "particleLife_drift",
    forceScale: "particleLife_forceScale",
    friction: "particleLife_friction",
    rSmooth: "particleLife_rSmooth",
    interTypeAttractionScale: "particleLife_interTypeAttractionScale",
    interTypeRadiusScale: "particleLife_interTypeRadiusScale",
    fisheyeStrength: "particleLife_fisheyeStrength",
    particleRenderSize: "particleLife_particleRenderSize",
    opacity: "particleLife_opacity",
};

// === Storage Functions ===
export function saveToLocalStorage(key: string, value: number): void {
    try {
        localStorage.setItem(key, value.toString());
        if (key === STORAGE_KEYS.zoom) {
            console.log(`💾 Saving zoom to localStorage: ${value}`);
        }
    } catch (e) {
        console.warn("Failed to save to localStorage:", e);
    }
}

export function loadFromLocalStorage(
    key: string,
    defaultValue: number,
): number {
    try {
        const stored = localStorage.getItem(key);
        if (stored !== null) {
            const parsed = parseFloat(stored);
            if (!isNaN(parsed)) {
                if (key === STORAGE_KEYS.zoom) {
                    console.log(
                        `📖 Loading zoom from localStorage: ${parsed} (default: ${defaultValue})`,
                    );
                }
                return parsed;
            }
        }
    } catch (e) {
        console.warn("Failed to load from localStorage:", e);
    }
    if (key === STORAGE_KEYS.zoom) {
        console.log(
            `📖 No zoom in localStorage, using default: ${defaultValue}`,
        );
    }
    return defaultValue;
}

// === Parameter Mapping Functions ===
// Particle count constants (moved from deleted particle-lenia.ts)
const MAX_PARTICLES = 6400;
const MIN_PARTICLES = 1600;

/**
 * Convert pressure value to particle count
 * Pressure range: 0-1000 bar -> Particle count: 1600-6400
 */
function pressureToParticleCount(pressure: number): number {
    // Linear mapping from pressure (0-1000 bar) to particle count (1600-6400)
    const normalized = pressure / 1000; // Normalize to 0-1
    return Math.round(
        MIN_PARTICLES + normalized * (MAX_PARTICLES - MIN_PARTICLES),
    );
}

// Temperature mapping functions
function temperatureToDrift(temp: number): number {
    // Linear mapping: temp [3, 160] → drift [0, -120]
    // At temp = 3°C: drift = 0 px/s
    // At temp = 160°C: drift = -120 px/s
    // Scale factor applied to maintain same effect: (40-3)/(160-3) = 37/157 ≈ 0.2357
    const effectiveTemp = 3 + (temp - 3) * (37 / 157); // Map [3,160] to [3,40] equivalent
    return -((effectiveTemp - 3) * 120) / 37;
}

function temperatureToFriction(temp: number): number {
    // Exponential mapping: temp [3, 160] → friction [0.98, 0.05]
    // At temp = 3°C: friction = 0.98 (highest resistance, near total fixation)
    // At temp = 160°C: friction = 0.05 (lowest resistance)
    // Scale factor applied to maintain same effect: (40-3)/(160-3) = 37/157 ≈ 0.2357
    const effectiveTemp = 3 + (temp - 3) * (37 / 157); // Map [3,160] to [3,40] equivalent
    const normalizedTemp = (effectiveTemp - 3) / 37; // Normalize to [0, 1]
    return 0.98 * Math.exp(-3.0 * normalizedTemp); // Exponential decay from 0.98 to 0.05
}

function hslToRgb(
    h: number,
    s: number,
    l: number,
): { r: number; g: number; b: number } {
    // h: 0–360, s: 0–1, l: 0–1 → r/g/b: 0–1
    const a = s * Math.min(l, 1 - l);
    const f = (n: number) => {
        const k = (n + h / 30) % 12;
        return l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
    };
    return { r: f(0), g: f(8), b: f(4) };
}

function temperatureToBackgroundColor(temp: number): {
    r: number;
    g: number;
    b: number;
} {
    // Hue: 220 (blauw) → 0 (rood)
    // Bereikt max blauw al bij 80°C, max rood bij 140°C
    // Saturation en lightness zijn constant
    const S = 0.22;
    const L = 0.66;
    const hueTempClamped = Math.max(80, Math.min(140, temp));
    const normalizedHue = (hueTempClamped - 80) / (140 - 80);
    const hue = 220 + normalizedHue * (0 - 220); // 220 → 0
    return hslToRgb(hue, S, L);
}

// Pressure mapping functions - NOW ONLY CONTROLS rSmooth, forceScale, and particle count
function pressureToRSmooth(pressure: number): number {
    // Non-linear exponential mapping: pressure [0, 1000] → rSmooth [20, 0.1]
    // At pressure = 0: rSmooth = 20 (highest resistance)
    // At pressure = 1000 bar: rSmooth = 0.1 (lowest resistance)
    const normalizedPressure = pressure / 1000;
    return 20 * Math.exp(-5.3 * normalizedPressure);
}

function pressureToForceScale(pressure: number): number {
    // Linear mapping: pressure [0, 1000] → forceScale [100, 500]
    return 100 + (pressure * 400) / 1000;
}

// Electrical Activity mapping functions - NOW CONTROLS ATTRACTION SCALE
function electricalActivityToInterTypeAttractionScale(
    electricalActivity: number,
): number {
    // Non-linear cubic mapping: Electrical Activity [0, 3] → interTypeAttractionScale [-1.0, 3.0]
    // At zero electrical activity particles repel (scale = -1.0).
    // As activity rises the curve accelerates toward full attraction (scale = 3.0).
    // Formula: lerp(-1, 3, t³)  where t = electricalActivity / 3
    const t = electricalActivity / 3.0; // Normalize to [0, 1]
    const cubic = t * t * t; // Cubic ease-in for strong non-linearity
    return -1.0 + cubic * 4.0; // Map [0, 1] → [-1.0, 3.0]
}

// === Parameter Update Callbacks ===
// These functions will be set by the main module to handle simulation updates
let parameterUpdateCallbacks = {
    updateDriftAndBackground: (value: number) => {},
    updateBackgroundColor: (r: number, g: number, b: number) => {},
    updateBackgroundColorFromTemperature: (temp: number) => {},
    updateSimulationParameter: (paramName: string, value: number) => {},
    updateBooleanParameter: (paramName: string, value: boolean) => {},
    getParameter: (paramName: string) => 0,
    getBooleanParameter: (paramName: string) => false,
    updateZoom: (level: number, centerX?: number, centerY?: number) => {},
    updateParticleCount: (pressure: number) => {},
    // New comprehensive parameter methods
    setTemperature: (temp: number) => {},
    setPressure: (pressure: number) => {},
    setPH: (ph: number) => {},
    setElectricalActivity: (electrical: number) => {},
    setZoom: (level: number, centerX?: number, centerY?: number) => {},
    setParticleOpacity: (opacity: number) => {},
    setTypeColor: (typeIdx: number, r: number, g: number, b: number) => {},
    getTypeColorsRgb: (): Float32Array | null => null,
    // Rule regeneration
    regenerateRules: () => {},
};

export function setParameterUpdateCallbacks(callbacks: {
    updateDriftAndBackground: (value: number) => void;
    updateBackgroundColor: (r: number, g: number, b: number) => void;
    updateBackgroundColorFromTemperature: (temp: number) => void;
    updateSimulationParameter: (paramName: string, value: number) => void;
    updateBooleanParameter?: (paramName: string, value: boolean) => void;
    getParameter?: (paramName: string) => number;
    getBooleanParameter?: (paramName: string) => boolean;
    updateZoom: (level: number, centerX?: number, centerY?: number) => void;
    updateParticleCount: (pressure: number) => void;
    // New comprehensive parameter methods
    setTemperature?: (temp: number) => void;
    setPressure?: (pressure: number) => void;
    setPH?: (ph: number) => void;
    setElectricalActivity?: (electrical: number) => void;
    setZoom?: (level: number, centerX?: number, centerY?: number) => void;
    setParticleOpacity?: (opacity: number) => void;
    setTypeColor?: (typeIdx: number, r: number, g: number, b: number) => void;
    getTypeColorsRgb?: () => Float32Array | null;
    // Rule regeneration
    regenerateRules?: () => void;
}) {
    parameterUpdateCallbacks = {
        ...parameterUpdateCallbacks,
        ...callbacks,
        // Provide defaults for optional methods
        updateBooleanParameter:
            callbacks.updateBooleanParameter ||
            parameterUpdateCallbacks.updateBooleanParameter,
        getParameter:
            callbacks.getParameter || parameterUpdateCallbacks.getParameter,
        getBooleanParameter:
            callbacks.getBooleanParameter ||
            parameterUpdateCallbacks.getBooleanParameter,
        setTemperature:
            callbacks.setTemperature || parameterUpdateCallbacks.setTemperature,
        setPressure:
            callbacks.setPressure || parameterUpdateCallbacks.setPressure,
        setPH: callbacks.setPH || parameterUpdateCallbacks.setPH,
        setElectricalActivity:
            callbacks.setElectricalActivity ||
            parameterUpdateCallbacks.setElectricalActivity,
        setZoom: callbacks.setZoom || parameterUpdateCallbacks.setZoom,
        setParticleOpacity:
            callbacks.setParticleOpacity ||
            parameterUpdateCallbacks.setParticleOpacity,
        setTypeColor:
            callbacks.setTypeColor || parameterUpdateCallbacks.setTypeColor,
        getTypeColorsRgb:
            callbacks.getTypeColorsRgb ||
            parameterUpdateCallbacks.getTypeColorsRgb,
        // Rule regeneration
        regenerateRules:
            callbacks.regenerateRules ||
            parameterUpdateCallbacks.regenerateRules,
    };
}

// === Parameter Update Functions ===

function updateDriftAndFrictionFromTemperature(temp: number): void {
    // Use comprehensive temperature method if available (Rust engine)
    if (parameterUpdateCallbacks.setTemperature) {
        parameterUpdateCallbacks.setTemperature(temp);
        // Synchronize all parameter displays to reflect changes
        synchronizeAllParameterDisplays();
        return;
    }

    // Fallback to individual parameter updates (TypeScript engine)
    const newDrift = temperatureToDrift(temp);
    const newFriction = temperatureToFriction(temp);

    // Update drift using callback
    parameterUpdateCallbacks.updateDriftAndBackground(newDrift);

    // Update background color using HSL from Rust
    parameterUpdateCallbacks.updateBackgroundColorFromTemperature(temp);

    // Update friction parameter
    parameterUpdateCallbacks.updateSimulationParameter("friction", newFriction);

    // Update drift slider and display
    const driftSlider = document.getElementById(
        "driftSlider",
    ) as HTMLInputElement;
    const driftValueDisplay = document.getElementById("driftValue");
    if (driftSlider && driftValueDisplay) {
        driftSlider.value = newDrift.toString();
        driftValueDisplay.textContent = newDrift.toFixed(2);
    }

    // Update friction slider and display
    const frictionSlider = document.getElementById(
        "frictionSlider",
    ) as HTMLInputElement;
    const frictionValueDisplay = document.getElementById("frictionValue");
    if (frictionSlider && frictionValueDisplay) {
        frictionSlider.value = newFriction.toString();
        frictionValueDisplay.textContent = newFriction.toFixed(2);
    }
}

function updateParametersFromPressure(pressure: number): void {
    // Use comprehensive pressure method if available (Rust engine)
    if (parameterUpdateCallbacks.setPressure) {
        parameterUpdateCallbacks.setPressure(pressure);
        // Update particle count display
        updateParticleCountDisplay(pressure);
        // Synchronize all parameter displays to reflect changes
        synchronizeAllParameterDisplays();
        return;
    }

    // Fallback to individual parameter updates (TypeScript engine)
    const newRSmooth = pressureToRSmooth(pressure);
    const newForceScale = pressureToForceScale(pressure);
    // REMOVED: Inter-type parameters are now controlled by UV slider

    // Add debug logging to track pressure changes
    console.log(
        `🎚️ Pressure slider changed: ${pressure} → particle count will be: ${pressureToParticleCount(
            pressure,
        )}`,
    );

    // Update particle density based on pressure (new feature!)
    parameterUpdateCallbacks.updateParticleCount(pressure);

    // Update particle count display
    updateParticleCountDisplay(pressure);

    // Update parameters using callbacks (REMOVED inter-type parameters)
    parameterUpdateCallbacks.updateSimulationParameter("rSmooth", newRSmooth);
    parameterUpdateCallbacks.updateSimulationParameter(
        "forceScale",
        newForceScale,
    );

    // Update UI displays (REMOVED inter-type parameter displays)
    updateSliderDisplay("rSmoothSlider", "rSmoothValue", newRSmooth, 2);
    updateSliderDisplay(
        "forceScaleSlider",
        "forceScaleValue",
        newForceScale,
        2,
    );
}

function updateParametersFromPH(phValue: number): void {
    // Use comprehensive pH method if available (Rust engine)
    if (parameterUpdateCallbacks.setPH) {
        parameterUpdateCallbacks.setPH(phValue);
        // Synchronize all parameter displays to reflect changes
        synchronizeAllParameterDisplays();
        return;
    }

    // Fallback (TypeScript engine): pH effects handled by recompute_cross_dependencies in Rust
    // No direct TypeScript mapping needed — all pH effects go through Rust
}

function updateParametersFromElectricalActivity(
    electricalActivity: number,
): void {
    // Use comprehensive electrical activity method if available (Rust engine)
    if (parameterUpdateCallbacks.setElectricalActivity) {
        parameterUpdateCallbacks.setElectricalActivity(electricalActivity);
        // Synchronize all parameter displays to reflect changes
        synchronizeAllParameterDisplays();
        return;
    }

    // Fallback to individual parameter updates (TypeScript engine)
    const newInterTypeAttractionScale =
        electricalActivityToInterTypeAttractionScale(electricalActivity);

    // Update inter-type attraction scale using callback (Electrical Activity now controls attraction scale)
    parameterUpdateCallbacks.updateSimulationParameter(
        "interTypeAttractionScale",
        newInterTypeAttractionScale,
    );

    // Update UI display for attraction scale
    updateSliderDisplay(
        "interTypeAttractionScaleSlider",
        "interTypeAttractionScaleValue",
        newInterTypeAttractionScale,
        2,
    );
}

// === Parameter Synchronization System ===
// Updates all detail sliders to reflect current engine parameter values
function synchronizeAllParameterDisplays(): void {
    // Get all current parameter values from the engine
    const params = {
        driftXPerSecond:
            parameterUpdateCallbacks.getParameter("driftXPerSecond"),
        friction: parameterUpdateCallbacks.getParameter("friction"),
        forceScale: parameterUpdateCallbacks.getParameter("forceScale"),
        rSmooth: parameterUpdateCallbacks.getParameter("rSmooth"),
        interTypeAttractionScale: parameterUpdateCallbacks.getParameter(
            "interTypeAttractionScale",
        ),
        interTypeRadiusScale: parameterUpdateCallbacks.getParameter(
            "interTypeRadiusScale",
        ),
        fisheyeStrength:
            parameterUpdateCallbacks.getParameter("fisheyeStrength"),
        particleRenderSize:
            parameterUpdateCallbacks.getParameter("particleRenderSize"),
        leniaGrowthMu: parameterUpdateCallbacks.getParameter("leniaGrowthMu"),
        leniaGrowthSigma:
            parameterUpdateCallbacks.getParameter("leniaGrowthSigma"),
        leniaKernelRadius:
            parameterUpdateCallbacks.getParameter("leniaKernelRadius"),
        lightningFrequency:
            parameterUpdateCallbacks.getParameter("lightningFrequency"),
        lightningIntensity:
            parameterUpdateCallbacks.getParameter("lightningIntensity"),
        lightningDuration:
            parameterUpdateCallbacks.getParameter("lightningDuration"),
        // Boolean parameters
        flatForce: parameterUpdateCallbacks.getBooleanParameter("flatForce"),
        leniaEnabled:
            parameterUpdateCallbacks.getBooleanParameter("leniaEnabled"),
    };

    // Update all slider values and displays
    updateSliderDisplay("driftSlider", "driftValue", params.driftXPerSecond, 2);
    updateSliderDisplay("frictionSlider", "frictionValue", params.friction, 3);
    updateSliderDisplay(
        "forceScaleSlider",
        "forceScaleValue",
        params.forceScale,
        2,
    );
    updateSliderDisplay("rSmoothSlider", "rSmoothValue", params.rSmooth, 2);
    updateSliderDisplay(
        "interTypeAttractionScaleSlider",
        "interTypeAttractionScaleValue",
        params.interTypeAttractionScale,
        3,
    );
    updateSliderDisplay(
        "interTypeRadiusScaleSlider",
        "interTypeRadiusScaleValue",
        params.interTypeRadiusScale,
        3,
    );
    updateSliderDisplay(
        "fisheyeStrengthSlider",
        "fisheyeStrengthValue",
        params.fisheyeStrength,
        2,
    );
    updateSliderDisplay(
        "particleRenderSizeSlider",
        "particleRenderSizeValue",
        params.particleRenderSize,
        1,
    );
    updateSliderDisplay(
        "leniaGrowthMuSlider",
        "leniaGrowthMuValue",
        params.leniaGrowthMu,
        3,
    );
    updateSliderDisplay(
        "leniaGrowthSigmaSlider",
        "leniaGrowthSigmaValue",
        params.leniaGrowthSigma,
        3,
    );
    updateSliderDisplay(
        "leniaKernelRadiusSlider",
        "leniaKernelRadiusValue",
        params.leniaKernelRadius,
        1,
    );
    updateSliderDisplay(
        "lightningFrequencySlider",
        "lightningFrequencyValue",
        params.lightningFrequency,
        2,
    );
    updateSliderDisplay(
        "lightningIntensitySlider",
        "lightningIntensityValue",
        params.lightningIntensity,
        2,
    );
    updateSliderDisplay(
        "lightningDurationSlider",
        "lightningDurationValue",
        params.lightningDuration,
        2,
    );

    // Update boolean parameter displays
    updateBooleanDisplay(
        "flatForceCheckbox",
        "flatForceStatus",
        params.flatForce,
    );
    updateBooleanDisplay(
        "leniaEnabledCheckbox",
        "leniaEnabledStatus",
        params.leniaEnabled,
    );
}

// Helper function to update a boolean parameter display
function updateBooleanDisplay(
    checkboxId: string,
    statusId: string,
    value: boolean,
): void {
    const checkbox = document.getElementById(checkboxId) as HTMLInputElement;
    const status = document.getElementById(statusId);

    if (checkbox) {
        checkbox.checked = value;
    }
    if (status) {
        status.textContent = value ? "On" : "Off";
    }
}

// Export the synchronization function for use by parameter setters
export { synchronizeAllParameterDisplays };

// === Utility Functions ===

function updateSliderDisplay(
    sliderId: string,
    displayId: string,
    value: number,
    precision: number,
): void {
    const slider = document.getElementById(sliderId) as HTMLInputElement;
    const display = document.getElementById(displayId);
    if (slider && display) {
        slider.value = value.toString();
        display.textContent = value.toFixed(precision);
    }
}

/**
 * Updates the particle count display in the UI
 */
function updateParticleCountDisplay(pressure: number): void {
    const particleCountElement = document.getElementById("particleCount");
    if (particleCountElement) {
        const particleCount = pressureToParticleCount(pressure);
        particleCountElement.textContent = particleCount.toString();
    }
}

// === Zoom and Navigation Functions ===

// Function to constrain zoom center based on zoom level
function constrainZoomCenter(currentZoomLevel: number): {
    x: number;
    y: number;
} {
    // Calculate the size of the viewport (visible area) in virtual world units
    const viewportWidth = VIRTUAL_WORLD_WIDTH / currentZoomLevel;
    const viewportHeight = VIRTUAL_WORLD_HEIGHT / currentZoomLevel;

    // Calculate the half-width and half-height of the viewport
    const halfViewportWidth = viewportWidth / 2.0;
    const halfViewportHeight = viewportHeight / 2.0;

    // Calculate the minimum and maximum allowed zoom center positions
    // The center can be positioned such that the viewport edge touches the world edge
    const minCenterX = halfViewportWidth; // Left edge of viewport at world left edge (x=0)
    const maxCenterX = VIRTUAL_WORLD_WIDTH - halfViewportWidth; // Right edge at world right edge
    const minCenterY = halfViewportHeight; // Top edge of viewport at world top edge (y=0)
    const maxCenterY = VIRTUAL_WORLD_HEIGHT - halfViewportHeight; // Bottom edge at world bottom edge

    // Clamp the zoom center to ensure viewport never goes outside world bounds
    const clampedX = Math.max(minCenterX, Math.min(maxCenterX, zoomCenterX));
    const clampedY = Math.max(minCenterY, Math.min(maxCenterY, zoomCenterY));

    zoomCenterX = clampedX;
    zoomCenterY = clampedY;

    return { x: zoomCenterX, y: zoomCenterY };
}

export function getZoomCenter(): { x: number; y: number } {
    return { x: zoomCenterX, y: zoomCenterY };
}

export function setZoomCenter(x: number, y: number): void {
    zoomCenterX = x;
    zoomCenterY = y;
}

// === JoyStick Implementation ===

// JoyStick types (inline to avoid import issues)
interface JoyStickData {
    xPosition: number;
    yPosition: number;
    cardinalDirection: string;
    x: number;
    y: number;
}

async function initJoyStick(currentZoomLevel: number) {
    console.log("Initializing JoyStick...");

    try {
        // Wait for the JoyStick library to be available globally
        if (typeof (window as any).JoyStick === "undefined") {
            console.log("Waiting for JoyStick library to load...");
            // Wait up to 5 seconds for the library to load
            for (let i = 0; i < 50; i++) {
                await new Promise((resolve) => setTimeout(resolve, 100));
                if (typeof (window as any).JoyStick !== "undefined") {
                    break;
                }
            }
        }

        const JoyStickConstructor = (window as any).JoyStick;

        if (!JoyStickConstructor) {
            throw new Error("JoyStick constructor not found after waiting");
        }

        console.log("JoyStick constructor found, creating instance...");

        // Initialize JoyStick with callback function
        joystick = new JoyStickConstructor(
            "joyDiv",
            {
                width: 150,
                height: 150,
                internalFillColor: "#E3C463",
                internalLineWidth: 2,
                internalStrokeColor: "#B8A150",
                externalStrokeColor: "#E3C463",
                autoReturnToCenter: true,
            },
            function (stickData: JoyStickData) {
                // Relative mode: store deflection as velocity (-1 to +1), Y-axis inverted
                joystickForceX = stickData.x / 100.0;
                joystickForceY = -stickData.y / 100.0;
            },
        );

        console.log("JoyStick initialized successfully");

        // Add click detection: a quick tap (< 8px movement, < 400ms) resets pan to center
        const joyCanvas = document.querySelector(
            "#joyDiv canvas",
        ) as HTMLCanvasElement;
        if (joyCanvas) {
            let pressX = 0,
                pressY = 0,
                pressTime = 0;
            joyCanvas.addEventListener("pointerdown", (e) => {
                pressX = e.clientX;
                pressY = e.clientY;
                pressTime = Date.now();
            });
            joyCanvas.addEventListener("pointerup", (e) => {
                const dx = e.clientX - pressX;
                const dy = e.clientY - pressY;
                const dist = Math.sqrt(dx * dx + dy * dy);
                const elapsed = Date.now() - pressTime;
                if (dist < 8 && elapsed < 400) {
                    resetPanToCenter();
                    console.log("🕹️ Joystick click → pan reset to center");
                }
            });
        }
    } catch (error) {
        console.error("Failed to initialize JoyStick:", error);

        // Show error message to user
        const joyDiv = document.getElementById("joyDiv");
        if (joyDiv) {
            joyDiv.innerHTML =
                '<div style="color: red; text-align: center; padding: 20px;">JoyStick failed to load</div>';
        }
    }
}

// === Joystick Pan Update (called every frame from main.ts) ===

/**
 * Apply relative joystick panning. Call once per animation frame with the
 * elapsed time in milliseconds. Speed = 50 % of the visible viewport per
 * second at full deflection, so it feels the same depth regardless of zoom.
 */
export function updateJoystickPan(deltaTime: number): void {
    const deadZone = 0.02;
    if (
        Math.abs(joystickForceX) < deadZone &&
        Math.abs(joystickForceY) < deadZone
    ) {
        return;
    }

    const zoomSlider = document.getElementById(
        "zoomSlider",
    ) as HTMLInputElement;
    const zoom = zoomSlider ? parseFloat(zoomSlider.value) : 1.0;

    // Viewport size in virtual-world units at current zoom
    const viewportWidth = VIRTUAL_WORLD_WIDTH / zoom;
    const viewportHeight = VIRTUAL_WORLD_HEIGHT / zoom;

    // 50 % of visible area per second at max deflection
    const speedFactor = 0.5;

    zoomCenterX += joystickForceX * viewportWidth * speedFactor * deltaTime;
    zoomCenterY += joystickForceY * viewportHeight * speedFactor * deltaTime;

    const constrained = constrainZoomCenter(zoom);
    zoomCenterX = constrained.x;
    zoomCenterY = constrained.y;

    if (parameterUpdateCallbacks.setZoom) {
        parameterUpdateCallbacks.setZoom(zoom, zoomCenterX, zoomCenterY);
    } else {
        parameterUpdateCallbacks.updateZoom(zoom, zoomCenterX, zoomCenterY);
    }

    updateZoomCenterInfo(zoom);
}

/**
 * Smoothly animate the pan center back to the middle of the virtual world.
 * Triggered by a joystick button click (web: tap on stick canvas,
 * native: joystick button via ESP32).
 */
export function resetPanToCenter(): void {
    const startX = zoomCenterX;
    const startY = zoomCenterY;
    const targetX = VIRTUAL_WORLD_CENTER_X;
    const targetY = VIRTUAL_WORLD_CENTER_Y;

    if (startX === targetX && startY === targetY) return;

    const duration = 500; // ms
    const startTime = performance.now();

    const zoomSlider = document.getElementById(
        "zoomSlider",
    ) as HTMLInputElement;
    const zoom = zoomSlider ? parseFloat(zoomSlider.value) : 1.0;

    const tick = (now: number) => {
        const elapsed = now - startTime;
        const t = Math.min(elapsed / duration, 1);
        // ease-in-out cubic
        const ease = t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;

        zoomCenterX = startX + (targetX - startX) * ease;
        zoomCenterY = startY + (targetY - startY) * ease;

        if (parameterUpdateCallbacks.setZoom) {
            parameterUpdateCallbacks.setZoom(zoom, zoomCenterX, zoomCenterY);
        } else {
            parameterUpdateCallbacks.updateZoom(zoom, zoomCenterX, zoomCenterY);
        }
        updateZoomCenterInfo(zoom);

        if (t < 1) {
            requestAnimationFrame(tick);
        } else {
            console.log(
                `🎯 Pan reset to world center (${zoomCenterX}, ${zoomCenterY})`,
            );
        }
    };

    requestAnimationFrame(tick);
}

// === FPS Display ===

export function updateFPS(deltaTime: number): void {
    frameCount++;
    const currentTime = performance.now();

    if (currentTime - lastFPSTime >= 1000) {
        // Update every second
        const fps = Math.round(
            (frameCount * 1000) / (currentTime - lastFPSTime),
        );

        if (!fpsDisplayElement) {
            fpsDisplayElement = document.getElementById("fpsDisplay");
        }

        if (fpsDisplayElement) {
            fpsDisplayElement.textContent = `${fps}`;
        }

        frameCount = 0;
        lastFPSTime = currentTime;
    }
}

// === UI Initialization ===

export function initializeUI(
    simParams: SimulationParams,
    currentZoomLevel: number,
): void {
    console.log("Initializing UI controls...");

    // Initialize FPS display element
    fpsDisplayElement = document.getElementById("fpsDisplay");

    // Initialize sliders with saved values and set up event listeners
    initializeDriftSlider(simParams);
    initializeForceScaleSlider(simParams);
    initializeFrictionSlider(simParams);
    initializeRSmoothSlider(simParams);
    initializeInterTypeScaleSliders(simParams);
    // initializeFisheyeSlider(simParams); // Removed - fisheye strength is now fixed at 1.5
    initializeParticleRenderSizeSlider();
    initializeZoomSlider(currentZoomLevel);
    initializeLeniaControls(simParams);
    initializeEnvironmentalSliders();

    // Initialize JoyStick after a short delay to ensure DOM is ready
    setTimeout(() => {
        initJoyStick(currentZoomLevel);
        updateZoomCenterInfo(currentZoomLevel);
    }, 100);

    console.log("UI initialization complete");
}

// === Individual Slider Initialization Functions ===

function initializeDriftSlider(simParams: SimulationParams): void {
    const driftSlider = document.getElementById(
        "driftSlider",
    ) as HTMLInputElement;
    const driftValueDisplay = document.getElementById("driftValue");

    if (driftSlider && driftValueDisplay) {
        // Get current value from engine, fall back to localStorage or default
        let currentValue =
            parameterUpdateCallbacks.getParameter("driftXPerSecond");
        if (currentValue === 0) {
            currentValue = loadFromLocalStorage(
                STORAGE_KEYS.drift,
                simParams.driftXPerSecond,
            );
            parameterUpdateCallbacks.updateDriftAndBackground(currentValue);
        }

        driftSlider.value = currentValue.toString();
        driftValueDisplay.textContent = currentValue.toFixed(2);

        driftSlider.addEventListener("input", (event) => {
            const newDrift = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            parameterUpdateCallbacks.updateDriftAndBackground(newDrift);
            driftValueDisplay.textContent = newDrift.toFixed(2);
            saveToLocalStorage(STORAGE_KEYS.drift, newDrift);
        });
    }
}

function initializeForceScaleSlider(simParams: SimulationParams): void {
    const forceScaleSlider = document.getElementById(
        "forceScaleSlider",
    ) as HTMLInputElement;
    const forceScaleValueDisplay = document.getElementById("forceScaleValue");

    if (forceScaleSlider && forceScaleValueDisplay) {
        // Get current value from engine, fall back to localStorage or default
        let currentValue = parameterUpdateCallbacks.getParameter("forceScale");
        if (currentValue === 0) {
            currentValue = loadFromLocalStorage(
                STORAGE_KEYS.forceScale,
                simParams.forceScale,
            );
        }

        forceScaleSlider.value = currentValue.toString();
        forceScaleValueDisplay.textContent = currentValue.toFixed(2);

        forceScaleSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            parameterUpdateCallbacks.updateSimulationParameter(
                "forceScale",
                newValue,
            );
            forceScaleValueDisplay.textContent = newValue.toFixed(2);
            saveToLocalStorage(STORAGE_KEYS.forceScale, newValue);
        });
    }
}

function initializeFrictionSlider(simParams: SimulationParams): void {
    const frictionSlider = document.getElementById(
        "frictionSlider",
    ) as HTMLInputElement;
    const frictionValueDisplay = document.getElementById("frictionValue");

    if (frictionSlider && frictionValueDisplay) {
        // Get current value from engine, fall back to localStorage or default
        let currentValue = parameterUpdateCallbacks.getParameter("friction");
        if (currentValue === 0) {
            currentValue = loadFromLocalStorage(
                STORAGE_KEYS.friction,
                simParams.friction,
            );
        }

        frictionSlider.value = currentValue.toString();
        frictionValueDisplay.textContent = currentValue.toFixed(3);

        frictionSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            parameterUpdateCallbacks.updateSimulationParameter(
                "friction",
                newValue,
            );
            frictionValueDisplay.textContent = newValue.toFixed(3);
            saveToLocalStorage(STORAGE_KEYS.friction, newValue);
        });
    }
}

function initializeRSmoothSlider(simParams: SimulationParams): void {
    const rSmoothSlider = document.getElementById(
        "rSmoothSlider",
    ) as HTMLInputElement;
    const rSmoothValueDisplay = document.getElementById("rSmoothValue");

    if (rSmoothSlider && rSmoothValueDisplay) {
        // Get current value from engine, fall back to localStorage or default
        let currentValue = parameterUpdateCallbacks.getParameter("rSmooth");
        if (currentValue === 0) {
            currentValue = loadFromLocalStorage(
                STORAGE_KEYS.rSmooth,
                simParams.rSmooth,
            );
        }

        rSmoothSlider.value = currentValue.toString();
        rSmoothValueDisplay.textContent = currentValue.toFixed(2);

        rSmoothSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            parameterUpdateCallbacks.updateSimulationParameter(
                "rSmooth",
                newValue,
            );
            rSmoothValueDisplay.textContent = newValue.toFixed(2);
            saveToLocalStorage(STORAGE_KEYS.rSmooth, newValue);
        });
    }
}

function initializeInterTypeScaleSliders(simParams: SimulationParams): void {
    // Inter-Type Attraction Scale Slider
    const interTypeAttractionScaleSlider = document.getElementById(
        "interTypeAttractionScaleSlider",
    ) as HTMLInputElement;
    const interTypeAttractionScaleValueDisplay = document.getElementById(
        "interTypeAttractionScaleValue",
    );

    if (
        interTypeAttractionScaleSlider &&
        interTypeAttractionScaleValueDisplay
    ) {
        // Get current value from engine, fall back to localStorage or default
        let currentValue = parameterUpdateCallbacks.getParameter(
            "interTypeAttractionScale",
        );
        if (currentValue === 0) {
            currentValue = loadFromLocalStorage(
                STORAGE_KEYS.interTypeAttractionScale,
                simParams.interTypeAttractionScale,
            );
        }

        interTypeAttractionScaleSlider.value = currentValue.toString();
        interTypeAttractionScaleValueDisplay.textContent =
            currentValue.toFixed(2);

        interTypeAttractionScaleSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            parameterUpdateCallbacks.updateSimulationParameter(
                "interTypeAttractionScale",
                newValue,
            );
            interTypeAttractionScaleValueDisplay.textContent =
                newValue.toFixed(2);
            saveToLocalStorage(STORAGE_KEYS.interTypeAttractionScale, newValue);
        });
    }

    // Inter-Type Radius Scale Slider
    const interTypeRadiusScaleSlider = document.getElementById(
        "interTypeRadiusScaleSlider",
    ) as HTMLInputElement;
    const interTypeRadiusScaleValueDisplay = document.getElementById(
        "interTypeRadiusScaleValue",
    );

    if (interTypeRadiusScaleSlider && interTypeRadiusScaleValueDisplay) {
        // Get current value from engine, fall back to localStorage or default
        let currentValue = parameterUpdateCallbacks.getParameter(
            "interTypeRadiusScale",
        );
        if (currentValue === 0) {
            currentValue = loadFromLocalStorage(
                STORAGE_KEYS.interTypeRadiusScale,
                simParams.interTypeRadiusScale,
            );
        }

        interTypeRadiusScaleSlider.value = currentValue.toString();
        interTypeRadiusScaleValueDisplay.textContent = currentValue.toFixed(2);

        interTypeRadiusScaleSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            parameterUpdateCallbacks.updateSimulationParameter(
                "interTypeRadiusScale",
                newValue,
            );
            interTypeRadiusScaleValueDisplay.textContent = newValue.toFixed(2);
            saveToLocalStorage(STORAGE_KEYS.interTypeRadiusScale, newValue);
        });
    }
}

function initializeFisheyeSlider(simParams: SimulationParams): void {
    const fisheyeStrengthSlider = document.getElementById(
        "fisheyeStrengthSlider",
    ) as HTMLInputElement;
    const fisheyeStrengthValueDisplay = document.getElementById(
        "fisheyeStrengthValue",
    );

    if (fisheyeStrengthSlider && fisheyeStrengthValueDisplay) {
        // Get current value from engine, fall back to localStorage or default
        let currentValue =
            parameterUpdateCallbacks.getParameter("fisheyeStrength");
        if (currentValue === 0) {
            currentValue = loadFromLocalStorage(
                STORAGE_KEYS.fisheyeStrength,
                simParams.fisheyeStrength,
            );
        }

        fisheyeStrengthSlider.value = currentValue.toString();
        fisheyeStrengthValueDisplay.textContent = currentValue.toFixed(2);

        fisheyeStrengthSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            parameterUpdateCallbacks.updateSimulationParameter(
                "fisheyeStrength",
                newValue,
            );
            fisheyeStrengthValueDisplay.textContent = newValue.toFixed(2);
            saveToLocalStorage(STORAGE_KEYS.fisheyeStrength, newValue);
        });
    }
}

function initializeParticleRenderSizeSlider(): void {
    const particleRenderSizeSlider = document.getElementById(
        "particleRenderSizeSlider",
    ) as HTMLInputElement;
    const particleRenderSizeValueDisplay = document.getElementById(
        "particleRenderSizeValue",
    );

    if (particleRenderSizeSlider && particleRenderSizeValueDisplay) {
        // Get current value from engine, fall back to localStorage or default
        let currentValue =
            parameterUpdateCallbacks.getParameter("particleRenderSize");
        if (currentValue === 0) {
            currentValue = loadFromLocalStorage(
                STORAGE_KEYS.particleRenderSize,
                16.0, // Default value matching the HTML and PARTICLE_SIZE config
            );
        }

        particleRenderSizeSlider.value = currentValue.toString();
        particleRenderSizeValueDisplay.textContent = currentValue.toFixed(1);

        particleRenderSizeSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            parameterUpdateCallbacks.updateSimulationParameter(
                "particleRenderSize",
                newValue,
            );
            particleRenderSizeValueDisplay.textContent = newValue.toFixed(1);
            saveToLocalStorage(STORAGE_KEYS.particleRenderSize, newValue);
        });
    }
}

function initializeZoomSlider(currentZoomLevel: number): void {
    const zoomSlider = document.getElementById(
        "zoomSlider",
    ) as HTMLInputElement;
    const zoomValueDisplay = document.getElementById("zoomValue");

    if (zoomSlider && zoomValueDisplay) {
        // Always start at the minimum zoom level (1.0), ignore localStorage
        const initialZoom = Math.max(1.0, currentZoomLevel);
        console.log(
            `Initializing zoom slider: engine=${currentZoomLevel}, initial=${initialZoom}, HTML default=${zoomSlider.value}`,
        );

        zoomSlider.value = initialZoom.toString();
        zoomValueDisplay.textContent = initialZoom.toFixed(1); // Show 1 decimal

        // Immediately update the engine with the final zoom value to ensure consistency
        if (parameterUpdateCallbacks && parameterUpdateCallbacks.updateZoom) {
            parameterUpdateCallbacks.updateZoom(
                initialZoom,
                zoomCenterX,
                zoomCenterY,
            );
        }

        zoomSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            console.log(`🎚️ Zoom slider input event: ${newValue}`);
            zoomValueDisplay.textContent = newValue.toFixed(1); // Show 1 decimal
            // Don't save to localStorage - always reset to minimum on reload

            // Constrain zoom center based on new zoom level
            constrainZoomCenter(newValue);

            // Use comprehensive zoom method if available (Rust engine)
            if (parameterUpdateCallbacks.setZoom) {
                console.log(
                    `🔍 Calling setZoom(${newValue}, ${zoomCenterX}, ${zoomCenterY})`,
                );
                parameterUpdateCallbacks.setZoom(
                    newValue,
                    zoomCenterX,
                    zoomCenterY,
                );
            } else {
                console.log(
                    `🔍 Calling updateZoom fallback(${newValue}, ${zoomCenterX}, ${zoomCenterY})`,
                );
                // Fallback to updateZoom callback (TypeScript engine)
                parameterUpdateCallbacks.updateZoom(
                    newValue,
                    zoomCenterX,
                    zoomCenterY,
                );
            }

            updateZoomCenterInfo(newValue);
        });

        // Also add a 'change' event listener to ensure the zoom value persists when dragging ends
        zoomSlider.addEventListener("change", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            console.log(`Zoom slider change event: ${newValue}`);
            zoomValueDisplay.textContent = newValue.toFixed(1); // Show 1 decimal
            // Don't save to localStorage - always reset to minimum on reload

            // Constrain zoom center based on new zoom level
            constrainZoomCenter(newValue);

            // Use comprehensive zoom method if available (Rust engine)
            if (parameterUpdateCallbacks.setZoom) {
                parameterUpdateCallbacks.setZoom(
                    newValue,
                    zoomCenterX,
                    zoomCenterY,
                );
            } else {
                // Fallback to updateZoom callback (TypeScript engine)
                parameterUpdateCallbacks.updateZoom(
                    newValue,
                    zoomCenterX,
                    zoomCenterY,
                );
            }

            updateZoomCenterInfo(newValue);
        });
    }
}

function initializeLeniaControls(simParams: SimulationParams): void {
    // Lenia Enabled Checkbox
    const leniaEnabledCheckbox = document.getElementById(
        "leniaEnabledCheckbox",
    ) as HTMLInputElement;
    const leniaEnabledStatus = document.getElementById("leniaEnabledStatus");

    if (leniaEnabledCheckbox && leniaEnabledStatus) {
        // Get current value from engine, fall back to sim params
        let currentValue =
            parameterUpdateCallbacks.getBooleanParameter("leniaEnabled");
        if (!currentValue) {
            currentValue = simParams.leniaEnabled;
        }

        leniaEnabledCheckbox.checked = currentValue;
        leniaEnabledStatus.textContent = currentValue ? "On" : "Off";

        leniaEnabledCheckbox.addEventListener("change", (event) => {
            const newValue = (event.target as HTMLInputElement).checked;
            leniaEnabledStatus.textContent = newValue ? "On" : "Off";
            parameterUpdateCallbacks.updateBooleanParameter(
                "leniaEnabled",
                newValue,
            );
        });
    }

    // Lenia Growth Mu Slider
    const leniaGrowthMuSlider = document.getElementById(
        "leniaGrowthMuSlider",
    ) as HTMLInputElement;
    const leniaGrowthMuValue = document.getElementById("leniaGrowthMuValue");

    if (leniaGrowthMuSlider && leniaGrowthMuValue) {
        // Get current value from engine, fall back to sim params
        let currentValue =
            parameterUpdateCallbacks.getParameter("leniaGrowthMu");
        if (currentValue === 0) {
            currentValue = simParams.leniaGrowthMu;
        }

        leniaGrowthMuSlider.value = currentValue.toString();
        leniaGrowthMuValue.textContent = currentValue.toFixed(3);

        leniaGrowthMuSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            leniaGrowthMuValue.textContent = newValue.toFixed(3);
            parameterUpdateCallbacks.updateSimulationParameter(
                "leniaGrowthMu",
                newValue,
            );
        });
    }

    // Lenia Growth Sigma Slider
    const leniaGrowthSigmaSlider = document.getElementById(
        "leniaGrowthSigmaSlider",
    ) as HTMLInputElement;
    const leniaGrowthSigmaValue = document.getElementById(
        "leniaGrowthSigmaValue",
    );

    if (leniaGrowthSigmaSlider && leniaGrowthSigmaValue) {
        // Get current value from engine, fall back to sim params
        let currentValue =
            parameterUpdateCallbacks.getParameter("leniaGrowthSigma");
        if (currentValue === 0) {
            currentValue = simParams.leniaGrowthSigma;
        }

        leniaGrowthSigmaSlider.value = currentValue.toString();
        leniaGrowthSigmaValue.textContent = currentValue.toFixed(3);

        leniaGrowthSigmaSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            leniaGrowthSigmaValue.textContent = newValue.toFixed(3);
            parameterUpdateCallbacks.updateSimulationParameter(
                "leniaGrowthSigma",
                newValue,
            );
        });
    }

    // Lenia Kernel Radius Slider
    const leniaKernelRadiusSlider = document.getElementById(
        "leniaKernelRadiusSlider",
    ) as HTMLInputElement;
    const leniaKernelRadiusValue = document.getElementById(
        "leniaKernelRadiusValue",
    );

    if (leniaKernelRadiusSlider && leniaKernelRadiusValue) {
        // Get current value from engine, fall back to sim params
        let currentValue =
            parameterUpdateCallbacks.getParameter("leniaKernelRadius");
        if (currentValue === 0) {
            currentValue = simParams.leniaKernelRadius;
        }

        leniaKernelRadiusSlider.value = currentValue.toString();
        leniaKernelRadiusValue.textContent = currentValue.toFixed(1);

        leniaKernelRadiusSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            leniaKernelRadiusValue.textContent = newValue.toFixed(1);
            parameterUpdateCallbacks.updateSimulationParameter(
                "leniaKernelRadius",
                newValue,
            );
        });
    }
}

function initializeEnvironmentalSliders(): void {
    // Temperature Slider
    const tempSlider = document.getElementById(
        "tempSlider",
    ) as HTMLInputElement;
    const tempValueDisplay = document.getElementById("tempValue");

    if (tempSlider && tempValueDisplay) {
        temperature = loadFromLocalStorage(
            STORAGE_KEYS.temperature,
            temperature,
        );
        tempSlider.value = temperature.toString();
        tempValueDisplay.textContent = temperature.toString();

        // Apply initial temperature-based parameters on page load
        updateDriftAndFrictionFromTemperature(temperature);

        tempSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            temperature = newValue;
            tempValueDisplay.textContent = newValue.toString();
            saveToLocalStorage(STORAGE_KEYS.temperature, newValue);
            updateDriftAndFrictionFromTemperature(newValue);
        });
    }

    // Electrical Activity Slider
    const elecSlider = document.getElementById(
        "elecSlider",
    ) as HTMLInputElement;
    const elecValueDisplay = document.getElementById("elecValue");

    if (elecSlider && elecValueDisplay) {
        electricalActivity = loadFromLocalStorage(
            STORAGE_KEYS.electricalActivity,
            electricalActivity,
        );
        elecSlider.value = electricalActivity.toString();
        elecValueDisplay.textContent = electricalActivity.toFixed(2);

        // Apply initial electrical activity-based parameters on page load
        updateParametersFromElectricalActivity(electricalActivity);

        elecSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            electricalActivity = newValue;
            elecValueDisplay.textContent = newValue.toFixed(2);
            saveToLocalStorage(STORAGE_KEYS.electricalActivity, newValue);
            updateParametersFromElectricalActivity(newValue);
        });
    }

    // pH Slider
    const phSlider = document.getElementById("phSlider") as HTMLInputElement;
    const phValueDisplay = document.getElementById("phValue");

    if (phSlider && phValueDisplay) {
        ph = loadFromLocalStorage(STORAGE_KEYS.ph, ph);
        phSlider.value = ph.toString();
        phValueDisplay.textContent = ph.toFixed(1);

        // Apply initial pH-based parameters on page load
        updateParametersFromPH(ph);

        phSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            ph = newValue;
            phValueDisplay.textContent = newValue.toFixed(1);
            saveToLocalStorage(STORAGE_KEYS.ph, newValue);
            updateParametersFromPH(newValue);
        });
    }

    // Pressure Slider
    const presSlider = document.getElementById(
        "presSlider",
    ) as HTMLInputElement;
    const presValueDisplay = document.getElementById("presValue");

    if (presSlider && presValueDisplay) {
        pressure = loadFromLocalStorage(STORAGE_KEYS.pressure, pressure);
        // Invert slider position: top = surface (0 bar), bottom = max depth (1000 bar)
        presSlider.value = (1000 - pressure).toString();
        presValueDisplay.textContent = Math.round(pressure * 10) + "m";

        // Apply initial pressure-based parameters on page load
        updateParametersFromPressure(pressure);

        presSlider.addEventListener("input", (event) => {
            const sliderValue = parseFloat(
                (event.target as HTMLInputElement).value,
            );
            // Invert: slider at top (1000) = 0 bar surface, slider at bottom (0) = 1000 bar depth
            const newValue = 1000 - sliderValue;
            pressure = newValue;
            presValueDisplay.textContent = Math.round(newValue * 10) + "m";
            saveToLocalStorage(STORAGE_KEYS.pressure, newValue);
            updateParametersFromPressure(newValue);
        });
    }
}

function updateZoomCenterInfo(currentZoomLevel: number): void {
    const zoomCenterInfo = document.getElementById("zoomCenterInfo");
    if (zoomCenterInfo) {
        // Calculate the visible area dimensions at current zoom
        const visibleWidth = VIRTUAL_WORLD_WIDTH / currentZoomLevel;
        const visibleHeight = VIRTUAL_WORLD_HEIGHT / currentZoomLevel;

        // Use the same formula as in the joystick callback for consistency
        const maxMovementRange = Math.max(0, 112 * currentZoomLevel - 150);

        zoomCenterInfo.innerHTML =
            `Center: (${zoomCenterX.toFixed(0)}, ${zoomCenterY.toFixed(
                0,
            )})<br>` +
            `Visible: ${visibleWidth.toFixed(0)}×${visibleHeight.toFixed(
                0,
            )}<br>` +
            `Range: ${maxMovementRange.toFixed(0)}`;
    }
}

// === Canvas Setup ===

export function setupCanvas(): HTMLCanvasElement {
    // Clean up previous instances
    const CANVAS_ID = "__webgpu_particle_life_canvas__";
    const oldCanvas = document.getElementById(CANVAS_ID);
    if (oldCanvas && oldCanvas.parentNode) {
        oldCanvas.parentNode.removeChild(oldCanvas);
    }

    const canvas = document.createElement("canvas");
    canvas.id = CANVAS_ID;

    // Add canvas to the canvasContainer div in the Dashboard
    const canvasContainer = document.getElementById("canvasContainer");
    if (canvasContainer) {
        canvasContainer.appendChild(canvas);
    } else {
        // Fallback to body if canvasContainer not found
        document.body.appendChild(canvas);
        console.warn("canvasContainer not found, adding canvas to body");
    }

    canvas.width = CANVAS_WIDTH_U32;
    canvas.height = CANVAS_HEIGHT_U32;
    canvas.style.width = `${CANVAS_WIDTH_U32}px`;
    canvas.style.height = `${CANVAS_HEIGHT_U32}px`;

    return canvas;
}

// === Cleanup ===

export function cleanup(): void {
    // Clean up global singletons
    const GLOBAL_KEY = "__webgpu_particle_life_singleton__";
    if ((window as any)[GLOBAL_KEY]) {
        const prev = (window as any)[GLOBAL_KEY];
        if (prev.cancelAnimation) prev.cancelAnimation();
        if (prev.device && typeof prev.device.destroy === "function") {
            try {
                prev.device.destroy();
            } catch (e) {
                console.warn("Error destroying previous device:", e);
            }
        }
        if (prev.canvas && prev.canvas.parentNode) {
            prev.canvas.parentNode.removeChild(prev.canvas);
        }
    }

    // Clean up WebGPU device
    if (
        (window as any).__webgpuDevice &&
        typeof (window as any).__webgpuDevice.destroy === "function"
    ) {
        try {
            (window as any).__webgpuDevice.destroy();
        } catch (e) {
            console.warn("Error destroying existing __webgpuDevice:", e);
        }
    }
    (window as any).__webgpuDevice = undefined;
}

// ============================================================
// OKLCH Color Panel
// ============================================================

// OKLCH → sRGB conversion
function oklchToSrgb(
    l: number,
    c: number,
    h: number,
): [number, number, number] {
    const hRad = (h * Math.PI) / 180;
    const a = c * Math.cos(hRad);
    const b = c * Math.sin(hRad);

    // Oklab → linear RGB
    const l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    const m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    const s_ = l - 0.0894841775 * a - 1.291485548 * b;

    const lc = l_ * l_ * l_;
    const mc = m_ * m_ * m_;
    const sc = s_ * s_ * s_;

    const rLin = 4.0767416621 * lc - 3.3077115913 * mc + 0.2309699292 * sc;
    const gLin = -1.2684380046 * lc + 2.6097574011 * mc - 0.3413193965 * sc;
    const bLin = -0.0041960863 * lc - 0.7034186147 * mc + 1.6956086723 * sc;

    const toSrgb = (x: number) =>
        x <= 0.0031308
            ? 12.92 * x
            : 1.055 * Math.pow(Math.max(x, 0), 1 / 2.4) - 0.055;

    return [
        Math.max(0, Math.min(1, toSrgb(rLin))),
        Math.max(0, Math.min(1, toSrgb(gLin))),
        Math.max(0, Math.min(1, toSrgb(bLin))),
    ];
}

// sRGB → OKLCH (for initializing sliders from hex)
function srgbToOklch(
    r: number,
    g: number,
    b: number,
): [number, number, number] {
    const toLinear = (x: number) =>
        x <= 0.04045 ? x / 12.92 : Math.pow((x + 0.055) / 1.055, 2.4);

    const rLin = toLinear(r);
    const gLin = toLinear(g);
    const bLin = toLinear(b);

    const l_ = Math.cbrt(
        0.4122214708 * rLin + 0.5363325363 * gLin + 0.0514459929 * bLin,
    );
    const m_ = Math.cbrt(
        0.2119034982 * rLin + 0.6806995451 * gLin + 0.1073969566 * bLin,
    );
    const s_ = Math.cbrt(
        0.0883024619 * rLin + 0.2817188376 * gLin + 0.6299787005 * bLin,
    );

    const L = 0.2104542553 * l_ + 0.793617785 * m_ - 0.0040720468 * s_;
    const a = 1.9779984951 * l_ - 2.428592205 * m_ + 0.4505937099 * s_;
    const bk = -0.0259040371 * l_ + 0.7827717662 * m_ - 0.7568667852 * s_;

    const C = Math.sqrt(a * a + bk * bk);
    let H = (Math.atan2(bk, a) * 180) / Math.PI;
    if (H < 0) H += 360;

    return [L, C, H];
}

function srgbToHex(r: number, g: number, b: number): string {
    const toHex = (x: number) =>
        Math.round(Math.max(0, Math.min(1, x)) * 255)
            .toString(16)
            .padStart(2, "0");
    return "#" + toHex(r) + toHex(g) + toHex(b);
}

function hexToSrgb(hex: string): [number, number, number] {
    const r = parseInt(hex.slice(1, 3), 16) / 255;
    const g = parseInt(hex.slice(3, 5), 16) / 255;
    const b = parseInt(hex.slice(5, 7), 16) / 255;
    return [r, g, b];
}

// Current OKLCH state per type [L, C, H]
const typeOklch: [number, number, number][] = [
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
];

// Built-in defaults captured at startup from HTML slider values
const builtinTypeOklch: [number, number, number][] = [
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
];

const PALETTE_STORAGE_KEY = "particle_color_palettes";

function loadPalettes(): { name: string; colors: string[] }[] {
    try {
        return JSON.parse(localStorage.getItem(PALETTE_STORAGE_KEY) || "[]");
    } catch {
        return [];
    }
}

function savePalettes(palettes: { name: string; colors: string[] }[]): void {
    localStorage.setItem(PALETTE_STORAGE_KEY, JSON.stringify(palettes));
}

function refreshPaletteSelect(): void {
    const sel = document.getElementById("palette-select") as HTMLSelectElement;
    if (!sel) return;
    const palettes = loadPalettes();
    const current = sel.value;
    sel.innerHTML = '<option value="__builtin__">In-built colors</option>';
    palettes.forEach((p, i) => {
        const opt = document.createElement("option");
        opt.value = String(i);
        opt.textContent = p.name;
        sel.appendChild(opt);
    });
    // Restore previous selection if still valid, otherwise keep In-built colors
    if (
        current !== "" &&
        current !== "__builtin__" &&
        palettes[parseInt(current)]
    ) {
        sel.value = current;
    } else {
        sel.value = "__builtin__";
    }
}

function applyTypeColor(typeIdx: number): void {
    const [l, c, h] = typeOklch[typeIdx];
    const [r, g, b] = oklchToSrgb(l, c, h);

    // Update swatch
    const swatch = document.getElementById(`swatch-${typeIdx}`);
    if (swatch) swatch.style.backgroundColor = srgbToHex(r, g, b);

    // Send to Rust engine
    if (parameterUpdateCallbacks.setTypeColor) {
        parameterUpdateCallbacks.setTypeColor(typeIdx, r, g, b);
    }
}

export function initColorPanel(): void {
    const rows = document.querySelectorAll<HTMLElement>(".color-type-row");

    // Try to read initial colors from the engine (sRGB 0-1), convert to OKLCH
    const engineColors = parameterUpdateCallbacks.getTypeColorsRgb
        ? parameterUpdateCallbacks.getTypeColorsRgb()
        : null;

    rows.forEach((row) => {
        const typeIdx = parseInt(row.dataset.type || "0");
        const hSlider = row.querySelector<HTMLInputElement>(".oklch-h");
        const lSlider = row.querySelector<HTMLInputElement>(".oklch-l");
        const cSlider = row.querySelector<HTMLInputElement>(".oklch-c");
        const hVal = row.querySelector<HTMLElement>(".oklch-h-val");
        const lVal = row.querySelector<HTMLElement>(".oklch-l-val");
        const cVal = row.querySelector<HTMLElement>(".oklch-c-val");

        if (!hSlider || !lSlider || !cSlider) return;

        let l: number, c: number, h: number;

        if (engineColors && typeIdx * 3 + 2 < engineColors.length) {
            // Derive OKLCH from the engine's compiled-in RGB values
            const r = engineColors[typeIdx * 3];
            const g = engineColors[typeIdx * 3 + 1];
            const b = engineColors[typeIdx * 3 + 2];
            [l, c, h] = srgbToOklch(r, g, b);
        } else {
            // Fallback: use HTML slider defaults
            l = parseFloat(lSlider.defaultValue);
            c = parseFloat(cSlider.defaultValue);
            h = parseFloat(hSlider.defaultValue);
        }

        // Update sliders to reflect derived values
        hSlider.value = h.toFixed(0);
        lSlider.value = String(l);
        cSlider.value = String(c);
        if (hVal) hVal.textContent = h.toFixed(0);
        if (lVal) lVal.textContent = l.toFixed(2);
        if (cVal) cVal.textContent = c.toFixed(3);

        // Initialize typeOklch from the engine's built-in values
        typeOklch[typeIdx] = [l, c, h];
        // Capture built-in defaults for the "In-built colors" option
        builtinTypeOklch[typeIdx] = [l, c, h];
        applyTypeColor(typeIdx);

        const onInput = () => {
            const h = parseFloat(hSlider.value);
            const l = parseFloat(lSlider.value);
            const c = parseFloat(cSlider.value);
            typeOklch[typeIdx] = [l, c, h];
            if (hVal) hVal.textContent = h.toFixed(0);
            if (lVal) lVal.textContent = l.toFixed(2);
            if (cVal) cVal.textContent = c.toFixed(3);
            applyTypeColor(typeIdx);
        };

        hSlider.addEventListener("input", onInput);
        lSlider.addEventListener("input", onInput);
        cSlider.addEventListener("input", onInput);
    });

    // Toggle collapse - color panel
    const toggleBtn = document.getElementById("color-panel-toggle");
    const body = document.getElementById("color-panel-body");
    if (toggleBtn && body) {
        toggleBtn.addEventListener("click", () => {
            const collapsed = body.style.display === "none";
            body.style.display = collapsed ? "" : "none";
            toggleBtn.textContent = collapsed ? "▼" : "▶";
        });
    }

    // Toggle collapse - controls panel
    const controlsToggleBtn = document.getElementById("controls-toggle");
    const controlsBody = document.getElementById("controls-body");
    if (controlsToggleBtn && controlsBody) {
        controlsToggleBtn.addEventListener("click", () => {
            const collapsed = controlsBody.style.display === "none";
            controlsBody.style.display = collapsed ? "" : "none";
            controlsToggleBtn.textContent = collapsed ? "▼" : "▶";
        });
    }

    // Save palette
    document
        .getElementById("save-palette-btn")
        ?.addEventListener("click", () => {
            const palettes = loadPalettes();
            const name = `palette ${palettes.length + 1}`;
            const colors = typeOklch.map(([l, c, h]) =>
                srgbToHex(...oklchToSrgb(l, c, h)),
            );
            palettes.push({ name, colors });
            savePalettes(palettes);
            refreshPaletteSelect();
            // Auto-select the newly saved palette
            const sel = document.getElementById(
                "palette-select",
            ) as HTMLSelectElement;
            if (sel) sel.value = String(palettes.length - 1);
        });

    // Load palette helper
    const applySelectedPalette = () => {
        const sel = document.getElementById(
            "palette-select",
        ) as HTMLSelectElement;
        if (!sel || sel.value === "") return;

        // Restore built-in (hardcoded) colors
        if (sel.value === "__builtin__") {
            const rows =
                document.querySelectorAll<HTMLElement>(".color-type-row");
            builtinTypeOklch.forEach(([l, c, h], i) => {
                typeOklch[i] = [l, c, h];
                const row = rows[i];
                if (!row) return;
                const hSlider = row.querySelector<HTMLInputElement>(".oklch-h");
                const lSlider = row.querySelector<HTMLInputElement>(".oklch-l");
                const cSlider = row.querySelector<HTMLInputElement>(".oklch-c");
                const hVal = row.querySelector<HTMLElement>(".oklch-h-val");
                const lVal = row.querySelector<HTMLElement>(".oklch-l-val");
                const cVal = row.querySelector<HTMLElement>(".oklch-c-val");
                if (hSlider) {
                    hSlider.value = h.toFixed(0);
                    if (hVal) hVal.textContent = h.toFixed(0);
                }
                if (lSlider) {
                    lSlider.value = String(l);
                    if (lVal) lVal.textContent = l.toFixed(2);
                }
                if (cSlider) {
                    cSlider.value = String(c);
                    if (cVal) cVal.textContent = c.toFixed(3);
                }
                applyTypeColor(i);
            });
            return;
        }

        const palettes = loadPalettes();
        const palette = palettes[parseInt(sel.value)];
        if (!palette) return;

        const rows = document.querySelectorAll<HTMLElement>(".color-type-row");
        palette.colors.forEach((hex, i) => {
            if (i >= 6) return;
            const [r, g2, b] = hexToSrgb(hex);
            const [l, c, h] = srgbToOklch(r, g2, b);
            typeOklch[i] = [l, c, h];

            const row = rows[i];
            if (!row) return;
            const hSlider = row.querySelector<HTMLInputElement>(".oklch-h");
            const lSlider = row.querySelector<HTMLInputElement>(".oklch-l");
            const cSlider = row.querySelector<HTMLInputElement>(".oklch-c");
            const hVal = row.querySelector<HTMLElement>(".oklch-h-val");
            const lVal = row.querySelector<HTMLElement>(".oklch-l-val");
            const cVal = row.querySelector<HTMLElement>(".oklch-c-val");
            if (hSlider) {
                hSlider.value = h.toFixed(0);
                if (hVal) hVal.textContent = h.toFixed(0);
            }
            if (lSlider) {
                lSlider.value = String(l);
                if (lVal) lVal.textContent = l.toFixed(2);
            }
            if (cSlider) {
                cSlider.value = String(c);
                if (cVal) cVal.textContent = c.toFixed(3);
            }
            applyTypeColor(i);
        });
    };

    // Auto-load when selecting from dropdown
    document
        .getElementById("palette-select")
        ?.addEventListener("change", applySelectedPalette);

    // Delete palette
    document
        .getElementById("delete-palette-btn")
        ?.addEventListener("click", () => {
            const sel = document.getElementById(
                "palette-select",
            ) as HTMLSelectElement;
            if (!sel || sel.value === "" || sel.value === "__builtin__") return;
            const palettes = loadPalettes();
            palettes.splice(parseInt(sel.value), 1);
            savePalettes(palettes);
            refreshPaletteSelect();
        });

    // Rename palette
    document
        .getElementById("rename-palette-btn")
        ?.addEventListener("click", () => {
            const sel = document.getElementById(
                "palette-select",
            ) as HTMLSelectElement;
            if (!sel || sel.value === "" || sel.value === "__builtin__") return;
            const palettes = loadPalettes();
            const idx = parseInt(sel.value);
            const current = palettes[idx]?.name ?? "";
            const newName = prompt("Rename palette:", current);
            if (newName === null || newName.trim() === "") return;
            palettes[idx].name = newName.trim();
            savePalettes(palettes);
            refreshPaletteSelect();
            sel.value = String(idx);
        });

    refreshPaletteSelect();

    // Copy current colors as Rust source
    document
        .getElementById("copy-palette-btn")
        ?.addEventListener("click", () => {
            const typeNames = [
                "Blue",
                "Yellow",
                "Red",
                "Purple",
                "Green",
                "Cyan",
            ];
            const lines = typeOklch.map(([l, c, h], i) => {
                const [r, g, b] = oklchToSrgb(l, c, h);
                const hex = srgbToHex(r, g, b);
                const fmt = (x: number) => x.toFixed(4);
                return `    [${fmt(r)}, ${fmt(g)}, ${fmt(b)}], // ${hex} - ${typeNames[i]}`;
            });
            const text = `const DEFAULT_COLORS: [[f32; 3]; 6] = [\n${lines.join("\n")}\n];`;
            navigator.clipboard.writeText(text).then(() => {
                const btn = document.getElementById("copy-palette-btn");
                if (btn) {
                    const orig = btn.textContent;
                    btn.textContent = "Copied!";
                    setTimeout(() => {
                        btn.textContent = orig;
                    }, 1500);
                }
            });
        });

    // Opacity slider (in color panel)
    const opacitySlider = document.getElementById(
        "opacitySlider",
    ) as HTMLInputElement | null;
    const opacityValueDisplay = document.getElementById("opacityValue");

    if (opacitySlider && opacityValueDisplay) {
        // Restore from localStorage
        const stored = loadFromLocalStorage(STORAGE_KEYS.opacity, 0.55);
        opacitySlider.value = String(stored);
        opacityValueDisplay.textContent = stored.toFixed(2);
        if (parameterUpdateCallbacks.setParticleOpacity) {
            parameterUpdateCallbacks.setParticleOpacity(stored);
        }

        opacitySlider.addEventListener("input", () => {
            const val = parseFloat(opacitySlider.value);
            opacityValueDisplay.textContent = val.toFixed(2);
            saveToLocalStorage(STORAGE_KEYS.opacity, val);
            if (parameterUpdateCallbacks.setParticleOpacity) {
                parameterUpdateCallbacks.setParticleOpacity(val);
            }
        });
    }
}

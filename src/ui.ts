// ui.ts - User Interface Controls and DOM Interactions
// This module handles all HTML/DOM interactions, slider controls, and localStorage

import { SimulationParams, BoundaryMode } from "./particle-life-types";
// Import pressure-to-particle mapping for UI display
import { pressureToParticleCount } from "./particle-lenia";

// Environmental parameters for the UI controls
let temperature = 20; // Default temperature
let electricalActivity = 1.02; // Default electrical activity
let uvLight = 25; // Default UV light
let pressure = 1; // Default pressure

// Zoom center variables for joystick navigation
let zoomCenterX = 1200.0; // Center X coordinate in 2400x2400 world (default: center)
let zoomCenterY = 1200.0; // Center Y coordinate in 2400x2400 world (default: center)

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
    uvLight: "particleLife_uvLight",
    pressure: "particleLife_pressure",
    zoom: "particleLife_zoom",
    drift: "particleLife_drift",
    forceScale: "particleLife_forceScale",
    friction: "particleLife_friction",
    rSmooth: "particleLife_rSmooth",
    interTypeAttractionScale: "particleLife_interTypeAttractionScale",
    interTypeRadiusScale: "particleLife_interTypeRadiusScale",
    fisheyeStrength: "particleLife_fisheyeStrength",
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
    defaultValue: number
): number {
    try {
        const stored = localStorage.getItem(key);
        if (stored !== null) {
            const parsed = parseFloat(stored);
            if (!isNaN(parsed)) {
                if (key === STORAGE_KEYS.zoom) {
                    console.log(
                        `📖 Loading zoom from localStorage: ${parsed} (default: ${defaultValue})`
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
            `📖 No zoom in localStorage, using default: ${defaultValue}`
        );
    }
    return defaultValue;
}

// === Parameter Mapping Functions ===

// Temperature mapping functions
function temperatureToDrift(temp: number): number {
    // Linear mapping: temp [3, 40] → drift [0, -80]
    // At temp = 3°C: drift = 0 px/s
    // At temp = 40°C: drift = -80 px/s
    return -((temp - 3) * 80) / 37;
}

function temperatureToFriction(temp: number): number {
    // Exponential mapping: temp [3, 40] → friction [0.98, 0.05]
    // At temp = 3°C: friction = 0.98 (highest resistance, near total fixation)
    // At temp = 40°C: friction = 0.05 (lowest resistance)
    const normalizedTemp = (temp - 3) / 37; // Normalize to [0, 1]
    return 0.98 * Math.exp(-3.0 * normalizedTemp); // Exponential decay from 0.98 to 0.05
}

function temperatureToBackgroundColor(temp: number): {
    r: number;
    g: number;
    b: number;
} {
    // Temperature mapping to background color
    // Cold (3°C): Deep blue/purple
    // Room temp (20°C): Dark gray/black
    // Hot (40°C): Red/orange
    const normalizedTemp = Math.max(0, Math.min(1, (temp - 3) / 37)); // Clamp to [0, 1]

    if (normalizedTemp < 0.5) {
        // Cold to neutral: blue/purple to black
        const factor = normalizedTemp * 2; // 0 to 1
        return {
            r: factor * 0.1, // 0 to 0.1
            g: factor * 0.05, // 0 to 0.05
            b: (1 - factor) * 0.3 + factor * 0.0, // 0.3 to 0
        };
    } else {
        // Neutral to hot: black to red/orange
        const factor = (normalizedTemp - 0.5) * 2; // 0 to 1
        return {
            r: factor * 0.4, // 0 to 0.4
            g: factor * 0.1, // 0 to 0.1
            b: 0.0,
        };
    }
}

// Pressure mapping functions - NOW ONLY CONTROLS rSmooth, forceScale, and particle count
function pressureToRSmooth(pressure: number): number {
    // Non-linear exponential mapping: pressure [0, 350] → rSmooth [20, 0.1]
    // At pressure = 0: rSmooth = 20 (highest resistance)
    // At pressure = 350: rSmooth = 0.1 (lowest resistance)
    const normalizedPressure = pressure / 350;
    return 20 * Math.exp(-5.3 * normalizedPressure);
}

function pressureToForceScale(pressure: number): number {
    // Linear mapping: pressure [0, 350] → forceScale [100, 800]
    return 100 + (pressure * 700) / 350;
}

// UV Light mapping functions - NOW CONTROLS ONLY RADIUS SCALE
function uvToInterTypeRadiusScale(uv: number): number {
    // Linear mapping: UV [0, 50] → interTypeRadiusScale [0.1, 2.0]
    // At UV = 0: interTypeRadiusScale = 0.1 (minimum radius)
    // At UV = 50: interTypeRadiusScale = 2.0 (maximum radius)
    return 0.1 + (uv / 50.0) * (2.0 - 0.1);
}

// Electrical Activity mapping functions - NOW CONTROLS ATTRACTION SCALE
function electricalActivityToInterTypeAttractionScale(
    electricalActivity: number
): number {
    // Non-linear cubic mapping: Electrical Activity [0, 3] → interTypeAttractionScale [0, 3]
    // This creates a very pronounced acceleration curve where:
    // - Low electrical activity has very minimal effect on attraction
    // - Medium electrical activity has moderate effect
    // - High electrical activity has extremely dramatic effect on attraction
    // Formula: ITAS = (electricalActivity/3)³ × 3
    const normalizedElectrical = electricalActivity / 3.0; // Normalize to [0, 1]
    const cubicValue =
        normalizedElectrical * normalizedElectrical * normalizedElectrical; // Cube for strong non-linearity
    return cubicValue * 3.0; // Scale back to [0, 3]
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
    setUVLight: (uv: number) => {},
    setElectricalActivity: (electrical: number) => {},
    setZoom: (level: number, centerX?: number, centerY?: number) => {},
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
    setUVLight?: (uv: number) => void;
    setElectricalActivity?: (electrical: number) => void;
    setZoom?: (level: number, centerX?: number, centerY?: number) => void;
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
        setUVLight: callbacks.setUVLight || parameterUpdateCallbacks.setUVLight,
        setElectricalActivity:
            callbacks.setElectricalActivity ||
            parameterUpdateCallbacks.setElectricalActivity,
        setZoom: callbacks.setZoom || parameterUpdateCallbacks.setZoom,
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

    // Update background color using HSLuv from Rust
    parameterUpdateCallbacks.updateBackgroundColorFromTemperature(temp);

    // Update friction parameter
    parameterUpdateCallbacks.updateSimulationParameter("friction", newFriction);

    // Update drift slider and display
    const driftSlider = document.getElementById(
        "driftSlider"
    ) as HTMLInputElement;
    const driftValueDisplay = document.getElementById("driftValue");
    if (driftSlider && driftValueDisplay) {
        driftSlider.value = newDrift.toString();
        driftValueDisplay.textContent = newDrift.toFixed(2);
    }

    // Update friction slider and display
    const frictionSlider = document.getElementById(
        "frictionSlider"
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
            pressure
        )}`
    );

    // Update particle density based on pressure (new feature!)
    parameterUpdateCallbacks.updateParticleCount(pressure);

    // Update particle count display
    updateParticleCountDisplay(pressure);

    // Update parameters using callbacks (REMOVED inter-type parameters)
    parameterUpdateCallbacks.updateSimulationParameter("rSmooth", newRSmooth);
    parameterUpdateCallbacks.updateSimulationParameter(
        "forceScale",
        newForceScale
    );

    // Update UI displays (REMOVED inter-type parameter displays)
    updateSliderDisplay("rSmoothSlider", "rSmoothValue", newRSmooth, 2);
    updateSliderDisplay(
        "forceScaleSlider",
        "forceScaleValue",
        newForceScale,
        2
    );
}

function updateParametersFromUV(uv: number): void {
    // Use comprehensive UV method if available (Rust engine)
    if (parameterUpdateCallbacks.setUVLight) {
        parameterUpdateCallbacks.setUVLight(uv);
        // Synchronize all parameter displays to reflect changes
        synchronizeAllParameterDisplays();
        return;
    }

    // Fallback to individual parameter updates (TypeScript engine)
    const newInterTypeRadiusScale = uvToInterTypeRadiusScale(uv);

    // Update inter-type radius scale using callback (UV now only controls radius scale)
    parameterUpdateCallbacks.updateSimulationParameter(
        "interTypeRadiusScale",
        newInterTypeRadiusScale
    );

    // Update UI display for radius scale
    updateSliderDisplay(
        "interTypeRadiusScaleSlider",
        "interTypeRadiusScaleValue",
        newInterTypeRadiusScale,
        2
    );
}

function updateParametersFromElectricalActivity(
    electricalActivity: number
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
        newInterTypeAttractionScale
    );

    // Update UI display for attraction scale
    updateSliderDisplay(
        "interTypeAttractionScaleSlider",
        "interTypeAttractionScaleValue",
        newInterTypeAttractionScale,
        2
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
            "interTypeAttractionScale"
        ),
        interTypeRadiusScale: parameterUpdateCallbacks.getParameter(
            "interTypeRadiusScale"
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
        2
    );
    updateSliderDisplay("rSmoothSlider", "rSmoothValue", params.rSmooth, 2);
    updateSliderDisplay(
        "interTypeAttractionScaleSlider",
        "interTypeAttractionScaleValue",
        params.interTypeAttractionScale,
        3
    );
    updateSliderDisplay(
        "interTypeRadiusScaleSlider",
        "interTypeRadiusScaleValue",
        params.interTypeRadiusScale,
        3
    );
    updateSliderDisplay(
        "fisheyeStrengthSlider",
        "fisheyeStrengthValue",
        params.fisheyeStrength,
        2
    );
    updateSliderDisplay(
        "particleRenderSizeSlider",
        "particleRenderSizeValue",
        params.particleRenderSize,
        1
    );
    updateSliderDisplay(
        "leniaGrowthMuSlider",
        "leniaGrowthMuValue",
        params.leniaGrowthMu,
        3
    );
    updateSliderDisplay(
        "leniaGrowthSigmaSlider",
        "leniaGrowthSigmaValue",
        params.leniaGrowthSigma,
        3
    );
    updateSliderDisplay(
        "leniaKernelRadiusSlider",
        "leniaKernelRadiusValue",
        params.leniaKernelRadius,
        1
    );
    updateSliderDisplay(
        "lightningFrequencySlider",
        "lightningFrequencyValue",
        params.lightningFrequency,
        2
    );
    updateSliderDisplay(
        "lightningIntensitySlider",
        "lightningIntensityValue",
        params.lightningIntensity,
        2
    );
    updateSliderDisplay(
        "lightningDurationSlider",
        "lightningDurationValue",
        params.lightningDuration,
        2
    );

    // Update boolean parameter displays
    updateBooleanDisplay(
        "flatForceCheckbox",
        "flatForceStatus",
        params.flatForce
    );
    updateBooleanDisplay(
        "leniaEnabledCheckbox",
        "leniaEnabledStatus",
        params.leniaEnabled
    );
}

// Helper function to update a boolean parameter display
function updateBooleanDisplay(
    checkboxId: string,
    statusId: string,
    value: boolean
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
    precision: number
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
    // Calculate maximum movement range based on zoom factor
    // f(x) ≈ 111.24·x - 122.29, where x = zoom factor
    const maxMovementRange = Math.max(0, 111.24 * currentZoomLevel - 122.29);

    // Clamp zoom center to stay within allowed range
    const clampedX = Math.max(
        1200 - maxMovementRange,
        Math.min(1200 + maxMovementRange, zoomCenterX)
    );
    const clampedY = Math.max(
        1200 - maxMovementRange,
        Math.min(1200 + maxMovementRange, zoomCenterY)
    );

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
                // Get the current zoom level from the zoom slider to avoid stale captured value
                const zoomSlider = document.getElementById(
                    "zoomSlider"
                ) as HTMLInputElement;
                const actualCurrentZoomLevel = zoomSlider
                    ? parseFloat(zoomSlider.value)
                    : currentZoomLevel;

                console.log(
                    `🕹️ Joystick event: captured=${currentZoomLevel}, actual=${actualCurrentZoomLevel}, stick=(${stickData.x}, ${stickData.y})`
                );

                // Calculate maximum movement range based on zoom factor
                const maxMovementRange = Math.max(
                    0,
                    112 * actualCurrentZoomLevel - 150
                );

                // Convert joystick input (-100 to +100) to movement within the calculated range
                const moveX = (stickData.x / 100.0) * maxMovementRange;
                const moveY = -(stickData.y / 100.0) * maxMovementRange; // Inverted Y-axis

                // Calculate new zoom center positions
                zoomCenterX = 1200.0 + moveX;
                zoomCenterY = 1200.0 + moveY;

                // Update zoom uniforms via callback with the actual current zoom level
                parameterUpdateCallbacks.updateZoom(
                    actualCurrentZoomLevel,
                    zoomCenterX,
                    zoomCenterY
                );

                // Update zoom center info display
                const zoomCenterInfo =
                    document.getElementById("zoomCenterInfo");
                if (zoomCenterInfo) {
                    zoomCenterInfo.innerHTML = `Center: (${zoomCenterX.toFixed(
                        0
                    )}, ${zoomCenterY.toFixed(
                        0
                    )})<br>Range: ${maxMovementRange.toFixed(0)}`;
                }
            }
        );

        console.log("JoyStick initialized successfully");
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

// === FPS Display ===

export function updateFPS(deltaTime: number): void {
    frameCount++;
    const currentTime = performance.now();

    if (currentTime - lastFPSTime >= 1000) {
        // Update every second
        const fps = Math.round(
            (frameCount * 1000) / (currentTime - lastFPSTime)
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
    currentZoomLevel: number
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
    initializeFisheyeSlider(simParams);
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
        "driftSlider"
    ) as HTMLInputElement;
    const driftValueDisplay = document.getElementById("driftValue");

    if (driftSlider && driftValueDisplay) {
        // Get current value from engine, fall back to localStorage or default
        let currentValue =
            parameterUpdateCallbacks.getParameter("driftXPerSecond");
        if (currentValue === 0) {
            currentValue = loadFromLocalStorage(
                STORAGE_KEYS.drift,
                simParams.driftXPerSecond
            );
            parameterUpdateCallbacks.updateDriftAndBackground(currentValue);
        }

        driftSlider.value = currentValue.toString();
        driftValueDisplay.textContent = currentValue.toFixed(2);

        driftSlider.addEventListener("input", (event) => {
            const newDrift = parseFloat(
                (event.target as HTMLInputElement).value
            );
            parameterUpdateCallbacks.updateDriftAndBackground(newDrift);
            driftValueDisplay.textContent = newDrift.toFixed(2);
            saveToLocalStorage(STORAGE_KEYS.drift, newDrift);
        });
    }
}

function initializeForceScaleSlider(simParams: SimulationParams): void {
    const forceScaleSlider = document.getElementById(
        "forceScaleSlider"
    ) as HTMLInputElement;
    const forceScaleValueDisplay = document.getElementById("forceScaleValue");

    if (forceScaleSlider && forceScaleValueDisplay) {
        // Get current value from engine, fall back to localStorage or default
        let currentValue = parameterUpdateCallbacks.getParameter("forceScale");
        if (currentValue === 0) {
            currentValue = loadFromLocalStorage(
                STORAGE_KEYS.forceScale,
                simParams.forceScale
            );
        }

        forceScaleSlider.value = currentValue.toString();
        forceScaleValueDisplay.textContent = currentValue.toFixed(2);

        forceScaleSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value
            );
            parameterUpdateCallbacks.updateSimulationParameter(
                "forceScale",
                newValue
            );
            forceScaleValueDisplay.textContent = newValue.toFixed(2);
            saveToLocalStorage(STORAGE_KEYS.forceScale, newValue);
        });
    }
}

function initializeFrictionSlider(simParams: SimulationParams): void {
    const frictionSlider = document.getElementById(
        "frictionSlider"
    ) as HTMLInputElement;
    const frictionValueDisplay = document.getElementById("frictionValue");

    if (frictionSlider && frictionValueDisplay) {
        // Get current value from engine, fall back to localStorage or default
        let currentValue = parameterUpdateCallbacks.getParameter("friction");
        if (currentValue === 0) {
            currentValue = loadFromLocalStorage(
                STORAGE_KEYS.friction,
                simParams.friction
            );
        }

        frictionSlider.value = currentValue.toString();
        frictionValueDisplay.textContent = currentValue.toFixed(3);

        frictionSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value
            );
            parameterUpdateCallbacks.updateSimulationParameter(
                "friction",
                newValue
            );
            frictionValueDisplay.textContent = newValue.toFixed(3);
            saveToLocalStorage(STORAGE_KEYS.friction, newValue);
        });
    }
}

function initializeRSmoothSlider(simParams: SimulationParams): void {
    const rSmoothSlider = document.getElementById(
        "rSmoothSlider"
    ) as HTMLInputElement;
    const rSmoothValueDisplay = document.getElementById("rSmoothValue");

    if (rSmoothSlider && rSmoothValueDisplay) {
        // Get current value from engine, fall back to localStorage or default
        let currentValue = parameterUpdateCallbacks.getParameter("rSmooth");
        if (currentValue === 0) {
            currentValue = loadFromLocalStorage(
                STORAGE_KEYS.rSmooth,
                simParams.rSmooth
            );
        }

        rSmoothSlider.value = currentValue.toString();
        rSmoothValueDisplay.textContent = currentValue.toFixed(2);

        rSmoothSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value
            );
            parameterUpdateCallbacks.updateSimulationParameter(
                "rSmooth",
                newValue
            );
            rSmoothValueDisplay.textContent = newValue.toFixed(2);
            saveToLocalStorage(STORAGE_KEYS.rSmooth, newValue);
        });
    }
}

function initializeInterTypeScaleSliders(simParams: SimulationParams): void {
    // Inter-Type Attraction Scale Slider
    const interTypeAttractionScaleSlider = document.getElementById(
        "interTypeAttractionScaleSlider"
    ) as HTMLInputElement;
    const interTypeAttractionScaleValueDisplay = document.getElementById(
        "interTypeAttractionScaleValue"
    );

    if (
        interTypeAttractionScaleSlider &&
        interTypeAttractionScaleValueDisplay
    ) {
        // Get current value from engine, fall back to localStorage or default
        let currentValue = parameterUpdateCallbacks.getParameter(
            "interTypeAttractionScale"
        );
        if (currentValue === 0) {
            currentValue = loadFromLocalStorage(
                STORAGE_KEYS.interTypeAttractionScale,
                simParams.interTypeAttractionScale
            );
        }

        interTypeAttractionScaleSlider.value = currentValue.toString();
        interTypeAttractionScaleValueDisplay.textContent =
            currentValue.toFixed(2);

        interTypeAttractionScaleSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value
            );
            parameterUpdateCallbacks.updateSimulationParameter(
                "interTypeAttractionScale",
                newValue
            );
            interTypeAttractionScaleValueDisplay.textContent =
                newValue.toFixed(2);
            saveToLocalStorage(STORAGE_KEYS.interTypeAttractionScale, newValue);
        });
    }

    // Inter-Type Radius Scale Slider
    const interTypeRadiusScaleSlider = document.getElementById(
        "interTypeRadiusScaleSlider"
    ) as HTMLInputElement;
    const interTypeRadiusScaleValueDisplay = document.getElementById(
        "interTypeRadiusScaleValue"
    );

    if (interTypeRadiusScaleSlider && interTypeRadiusScaleValueDisplay) {
        // Get current value from engine, fall back to localStorage or default
        let currentValue = parameterUpdateCallbacks.getParameter(
            "interTypeRadiusScale"
        );
        if (currentValue === 0) {
            currentValue = loadFromLocalStorage(
                STORAGE_KEYS.interTypeRadiusScale,
                simParams.interTypeRadiusScale
            );
        }

        interTypeRadiusScaleSlider.value = currentValue.toString();
        interTypeRadiusScaleValueDisplay.textContent = currentValue.toFixed(2);

        interTypeRadiusScaleSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value
            );
            parameterUpdateCallbacks.updateSimulationParameter(
                "interTypeRadiusScale",
                newValue
            );
            interTypeRadiusScaleValueDisplay.textContent = newValue.toFixed(2);
            saveToLocalStorage(STORAGE_KEYS.interTypeRadiusScale, newValue);
        });
    }
}

function initializeFisheyeSlider(simParams: SimulationParams): void {
    const fisheyeStrengthSlider = document.getElementById(
        "fisheyeStrengthSlider"
    ) as HTMLInputElement;
    const fisheyeStrengthValueDisplay = document.getElementById(
        "fisheyeStrengthValue"
    );

    if (fisheyeStrengthSlider && fisheyeStrengthValueDisplay) {
        // Get current value from engine, fall back to localStorage or default
        let currentValue =
            parameterUpdateCallbacks.getParameter("fisheyeStrength");
        if (currentValue === 0) {
            currentValue = loadFromLocalStorage(
                STORAGE_KEYS.fisheyeStrength,
                simParams.fisheyeStrength
            );
        }

        fisheyeStrengthSlider.value = currentValue.toString();
        fisheyeStrengthValueDisplay.textContent = currentValue.toFixed(2);

        fisheyeStrengthSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value
            );
            parameterUpdateCallbacks.updateSimulationParameter(
                "fisheyeStrength",
                newValue
            );
            fisheyeStrengthValueDisplay.textContent = newValue.toFixed(2);
            saveToLocalStorage(STORAGE_KEYS.fisheyeStrength, newValue);
        });
    }
}

function initializeZoomSlider(currentZoomLevel: number): void {
    const zoomSlider = document.getElementById(
        "zoomSlider"
    ) as HTMLInputElement;
    const zoomValueDisplay = document.getElementById("zoomValue");

    if (zoomSlider && zoomValueDisplay) {
        // Always start at the minimum zoom level (1.45), ignore localStorage
        const initialZoom = Math.max(1.45, currentZoomLevel);
        console.log(
            `Initializing zoom slider: engine=${currentZoomLevel}, initial=${initialZoom}, HTML default=${zoomSlider.value}`
        );

        zoomSlider.value = initialZoom.toString();
        zoomValueDisplay.textContent = initialZoom.toFixed(1); // Show 1 decimal

        // Immediately update the engine with the final zoom value to ensure consistency
        if (parameterUpdateCallbacks && parameterUpdateCallbacks.updateZoom) {
            parameterUpdateCallbacks.updateZoom(
                initialZoom,
                zoomCenterX,
                zoomCenterY
            );
        }

        zoomSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value
            );
            console.log(`🎚️ Zoom slider input event: ${newValue}`);
            zoomValueDisplay.textContent = newValue.toFixed(1); // Show 1 decimal
            // Don't save to localStorage - always reset to minimum on reload

            // Constrain zoom center based on new zoom level
            constrainZoomCenter(newValue);

            // Use comprehensive zoom method if available (Rust engine)
            if (parameterUpdateCallbacks.setZoom) {
                console.log(
                    `🔍 Calling setZoom(${newValue}, ${zoomCenterX}, ${zoomCenterY})`
                );
                parameterUpdateCallbacks.setZoom(
                    newValue,
                    zoomCenterX,
                    zoomCenterY
                );
            } else {
                console.log(
                    `🔍 Calling updateZoom fallback(${newValue}, ${zoomCenterX}, ${zoomCenterY})`
                );
                // Fallback to updateZoom callback (TypeScript engine)
                parameterUpdateCallbacks.updateZoom(
                    newValue,
                    zoomCenterX,
                    zoomCenterY
                );
            }

            updateZoomCenterInfo(newValue);
        });

        // Also add a 'change' event listener to ensure the zoom value persists when dragging ends
        zoomSlider.addEventListener("change", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value
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
                    zoomCenterY
                );
            } else {
                // Fallback to updateZoom callback (TypeScript engine)
                parameterUpdateCallbacks.updateZoom(
                    newValue,
                    zoomCenterX,
                    zoomCenterY
                );
            }

            updateZoomCenterInfo(newValue);
        });
    }
}

function initializeLeniaControls(simParams: SimulationParams): void {
    // Lenia Enabled Checkbox
    const leniaEnabledCheckbox = document.getElementById(
        "leniaEnabledCheckbox"
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
                newValue
            );
        });
    }

    // Lenia Growth Mu Slider
    const leniaGrowthMuSlider = document.getElementById(
        "leniaGrowthMuSlider"
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
                (event.target as HTMLInputElement).value
            );
            leniaGrowthMuValue.textContent = newValue.toFixed(3);
            parameterUpdateCallbacks.updateSimulationParameter(
                "leniaGrowthMu",
                newValue
            );
        });
    }

    // Lenia Growth Sigma Slider
    const leniaGrowthSigmaSlider = document.getElementById(
        "leniaGrowthSigmaSlider"
    ) as HTMLInputElement;
    const leniaGrowthSigmaValue = document.getElementById(
        "leniaGrowthSigmaValue"
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
                (event.target as HTMLInputElement).value
            );
            leniaGrowthSigmaValue.textContent = newValue.toFixed(3);
            parameterUpdateCallbacks.updateSimulationParameter(
                "leniaGrowthSigma",
                newValue
            );
        });
    }

    // Lenia Kernel Radius Slider
    const leniaKernelRadiusSlider = document.getElementById(
        "leniaKernelRadiusSlider"
    ) as HTMLInputElement;
    const leniaKernelRadiusValue = document.getElementById(
        "leniaKernelRadiusValue"
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
                (event.target as HTMLInputElement).value
            );
            leniaKernelRadiusValue.textContent = newValue.toFixed(1);
            parameterUpdateCallbacks.updateSimulationParameter(
                "leniaKernelRadius",
                newValue
            );
        });
    }
}

function initializeEnvironmentalSliders(): void {
    // Temperature Slider
    const tempSlider = document.getElementById(
        "tempSlider"
    ) as HTMLInputElement;
    const tempValueDisplay = document.getElementById("tempValue");

    if (tempSlider && tempValueDisplay) {
        temperature = loadFromLocalStorage(
            STORAGE_KEYS.temperature,
            temperature
        );
        tempSlider.value = temperature.toString();
        tempValueDisplay.textContent = temperature.toString();

        // Apply initial temperature-based parameters on page load
        updateDriftAndFrictionFromTemperature(temperature);

        tempSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value
            );
            temperature = newValue;
            tempValueDisplay.textContent = newValue.toString();
            saveToLocalStorage(STORAGE_KEYS.temperature, newValue);
            updateDriftAndFrictionFromTemperature(newValue);
        });
    }

    // Electrical Activity Slider
    const elecSlider = document.getElementById(
        "elecSlider"
    ) as HTMLInputElement;
    const elecValueDisplay = document.getElementById("elecValue");

    if (elecSlider && elecValueDisplay) {
        electricalActivity = loadFromLocalStorage(
            STORAGE_KEYS.electricalActivity,
            electricalActivity
        );
        elecSlider.value = electricalActivity.toString();
        elecValueDisplay.textContent = electricalActivity.toFixed(2);

        // Apply initial electrical activity-based parameters on page load
        updateParametersFromElectricalActivity(electricalActivity);

        elecSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value
            );
            electricalActivity = newValue;
            elecValueDisplay.textContent = newValue.toFixed(2);
            saveToLocalStorage(STORAGE_KEYS.electricalActivity, newValue);
            updateParametersFromElectricalActivity(newValue);
        });
    }

    // UV Light Slider
    const uvSlider = document.getElementById("uvSlider") as HTMLInputElement;
    const uvValueDisplay = document.getElementById("uvValue");

    if (uvSlider && uvValueDisplay) {
        uvLight = loadFromLocalStorage(STORAGE_KEYS.uvLight, uvLight);
        uvSlider.value = uvLight.toString();
        uvValueDisplay.textContent = uvLight.toString();

        // Apply initial UV-based parameters on page load
        updateParametersFromUV(uvLight);

        uvSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value
            );
            uvLight = newValue;
            uvValueDisplay.textContent = newValue.toString();
            saveToLocalStorage(STORAGE_KEYS.uvLight, newValue);
            updateParametersFromUV(newValue);
        });
    }

    // Pressure Slider
    const presSlider = document.getElementById(
        "presSlider"
    ) as HTMLInputElement;
    const presValueDisplay = document.getElementById("presValue");

    if (presSlider && presValueDisplay) {
        pressure = loadFromLocalStorage(STORAGE_KEYS.pressure, pressure);
        presSlider.value = pressure.toString();
        presValueDisplay.textContent = pressure.toString();

        // Apply initial pressure-based parameters on page load
        updateParametersFromPressure(pressure);

        presSlider.addEventListener("input", (event) => {
            const newValue = parseFloat(
                (event.target as HTMLInputElement).value
            );
            pressure = newValue;
            presValueDisplay.textContent = newValue.toString();
            saveToLocalStorage(STORAGE_KEYS.pressure, newValue);
            updateParametersFromPressure(newValue);
        });
    }
}

function updateZoomCenterInfo(currentZoomLevel: number): void {
    const zoomCenterInfo = document.getElementById("zoomCenterInfo");
    if (zoomCenterInfo) {
        // Use the same formula as in the joystick callback for consistency
        const maxMovementRange = Math.max(0, 112 * currentZoomLevel - 150);
        zoomCenterInfo.innerHTML = `Center: (${zoomCenterX.toFixed(
            0
        )}, ${zoomCenterY.toFixed(0)})<br>Range: ${maxMovementRange.toFixed(
            0
        )}`;
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

    canvas.width = 800;
    canvas.height = 800;
    canvas.style.width = "800px";
    canvas.style.height = "800px";

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

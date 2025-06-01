// Custom color palette for particle types
const CUSTOM_COLORS = [
    // "#78729e",
    // "#cf9760",
    // "#698c61",
    // "#37597d",
    // "#287a8c",
    // "#a15064",
    // "#cf900a",
    // "#8196bd",
    "#559167",
    "#004e79",
    "#8398a7",
    "#99403a",
    "#e0954f",
];

// Convert hex color to RGB values (0-1 range)
function hexToRgb(hex: string): [number, number, number] {
    const result = /^#?([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/i.exec(hex);
    if (!result) {
        throw new Error(`Invalid hex color: ${hex}`);
    }
    return [
        parseInt(result[1], 16) / 255,
        parseInt(result[2], 16) / 255,
        parseInt(result[3], 16) / 255,
    ];
}

// Generate colors using the custom palette, cycling through colors for particle types
function generateParticleColors(numTypes: number): Float32Array {
    const colors = new Float32Array(numTypes * 4); // RGBA for each type

    for (let i = 0; i < numTypes; i++) {
        // Cycle through the custom colors
        const colorIndex = i % CUSTOM_COLORS.length;
        const [r, g, b] = hexToRgb(CUSTOM_COLORS[colorIndex]);

        // Store RGBA values
        const offset = i * 4;
        colors[offset + 0] = r; // Red
        colors[offset + 1] = g; // Green
        colors[offset + 2] = b; // Blue
        colors[offset + 3] = 0.6; // Alpha
    }

    return colors;
}

// Debug function to log the custom colors
function logParticleColors(numTypes: number) {
    console.log("Custom particle colors:");
    for (let i = 0; i < numTypes; i++) {
        const colorIndex = i % CUSTOM_COLORS.length;
        const hexColor = CUSTOM_COLORS[colorIndex];
        const [r, g, b] = hexToRgb(hexColor);
        console.log(
            `Type ${i}: ${hexColor} -> RGB(${(r * 255).toFixed(0)}, ${(
                g * 255
            ).toFixed(0)}, ${(b * 255).toFixed(0)})`
        );
    }
}

export { generateParticleColors, logParticleColors };

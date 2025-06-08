// Lightning effect fragment shader - BRANCHING VERSION
// Current behavior:
// - Lightning strikes every 1 second, lasts 0.5 seconds (fast lightning)
// - New segments appear every 0.1 seconds
// - 90% branching probability at each segment
// - Branching angles: 25-60 degrees from parent direction
// - No collision detection (removed for better visual flow)
// - Segments start with ±0.4 radian angle variation for visible branching
// - Up to 4 generations of branches allowed (0, 1, 2, 3)

struct SimulationParams {
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
    backgroundColor: vec3<f32>,

    // Lenia-inspired parameters
    leniaEnabled: u32,
    leniaGrowthMu: f32,
    leniaGrowthSigma: f32,
    leniaKernelRadius: f32,

    // Lightning parameters
    lightningFrequency: f32,
    lightningIntensity: f32,
    lightningDuration: f32,
}

@group(0) @binding(0)
var<uniform> sim_params: SimulationParams;

// Simple hash function
fn hash(x: f32) -> f32 {
    var p = x;
    p = fract(p * 0.1031);
    p *= p + 33.33;
    p *= p + p;
    return fract(p);
}

fn hash2(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// Generate lightning bolt with true branching - each segment can branch into 1 or 2 new segments
fn lightningBolt(uv: vec2<f32>, time: f32, boltId: f32, timeInFlash: f32) -> f32 {
    // Create seed for this bolt
    let baseSeed = hash(boltId * 73.421);

    // Random starting position and direction
    let posRand = hash(baseSeed * 555.0);
    let posRand2 = hash(baseSeed * 777.0);
    let dirRand = hash(baseSeed * 333.0);

    // Start from anywhere on screen
    let startPos = vec2<f32>(0.1 + posRand * 0.8, 0.1 + posRand2 * 0.8);

    // Random initial direction
    let angle = dirRand * 6.28318;
    let initialDirection = vec2<f32>(cos(angle), sin(angle));

    // Lightning parameters
    let maxTotalSegments = 30;
    // Total segments across all branches
    let minSegmentLengthPx = 60.0;
    // Minimum segment length in pixels
    let maxSegmentLengthPx = 150.0;
    // Maximum segment length in pixels
    let resolution = vec2<f32>(sim_params.canvasRenderWidth, sim_params.canvasRenderHeight);
    let minSegmentLengthUV = minSegmentLengthPx / min(resolution.x, resolution.y);
    let maxSegmentLengthUV = maxSegmentLengthPx / min(resolution.x, resolution.y);
    // Convert to UV coordinates
    let segmentInterval = 0.07;
    // New segment every 0.07 seconds
    let segmentDuration = 0.4;
    // Each segment visible for 0.4 seconds before disappearing

    var intensity = 0.0;
    var segmentCount = 0;

    // Arrays to store active branch endpoints (simulate dynamic arrays with fixed size)
    var branchPositions: array<vec2<f32>, 20>;
    var branchDirections: array<vec2<f32>, 20>;
    var branchGenerations: array<u32, 20>;
    var branchAppearTimes: array<f32, 20>;

    var activeBranches = 0;

    // Initialize with root segment
    branchPositions[0] = startPos;
    branchDirections[0] = initialDirection;
    branchGenerations[0] = 0u;
    branchAppearTimes[0] = 0.0;
    activeBranches = 1;

    // Generate segments progressively
    for (var step = 0; step < maxTotalSegments && activeBranches > 0; step++) {
        let stepAppearTime = f32(step) * segmentInterval;

        // Skip if this step shouldn't appear yet
        if (timeInFlash < stepAppearTime) {
            break;
        }

        var newBranches = 0;
        var newBranchPositions: array<vec2<f32>, 20>;
        var newBranchDirections: array<vec2<f32>, 20>;
        var newBranchGenerations: array<u32, 20>;
        var newBranchAppearTimes: array<f32, 20>;

        // Process each active branch
        for (var i = 0; i < activeBranches; i++) {
            let currentPos = branchPositions[i];
            let currentDir = branchDirections[i];
            let generation = branchGenerations[i];
            let branchAppearTime = branchAppearTimes[i];

            // Only process branches that should be visible
            if (timeInFlash >= branchAppearTime) {
                // Calculate segment properties
                let segSeed = hash(baseSeed + f32(step) * 73.0 + f32(i) * 37.0);

                // Decide branching first (90% chance to branch for highly visible testing)
                let branchSeed = hash(segSeed * 99.999);
                let shouldBranch = branchSeed > 0.1 && generation < 4u && newBranches < 16;

                // For continuing segments: use small angle change to maintain mostly straight paths
                // For branching: don't draw the "continuing" segment, just branch off
                var angleChange: f32;
                if (shouldBranch) {
                    // For branching segments, use moderate angle change for more dramatic lightning
                    angleChange = (hash(segSeed * 11.111) - 0.5) * 0.4;
                    // Increased for more visible branching
                }
                else {
                    // For non-branching segments, maintain straighter paths
                    angleChange = (hash(segSeed * 11.111) - 0.5) * 0.1;
                    // Small change for continuing segments
                }

                let newAngle = atan2(currentDir.y, currentDir.x) + angleChange;
                var newDir = vec2<f32>(cos(newAngle), sin(newAngle));

                // Calculate segment length with randomization and generation decay
                // Start with max length, decay by generation, but respect min/max constraints
                let lengthSeed = hash(segSeed * 222.222);
                let baseLengthUV = minSegmentLengthUV + lengthSeed * (maxSegmentLengthUV - minSegmentLengthUV);
                let generationDecay = pow(0.8, f32(generation));
                let currentSegmentLength = max(minSegmentLengthUV, baseLengthUV * generationDecay);
                let segmentEnd = currentPos + newDir * currentSegmentLength;

                // Calculate thickness and alpha (thinner and dimmer for higher generations)
                let thickness = 3.0 * pow(0.7, f32(generation));
                let baseAlpha = 1.0 * pow(0.9, f32(generation));

                // Check if segment is still within its visibility duration (no gradual fade)
                let segmentAge = timeInFlash - stepAppearTime;
                let isVisible = segmentAge < segmentDuration;

                // Only draw segment if it's still visible
                if (isVisible) {
                    intensity = max(intensity, drawSegment(uv, currentPos, segmentEnd, baseAlpha, thickness));
                }

                if (shouldBranch) {
                    // Don't draw a continuing main branch - instead draw the angled segment
                    //and create NEW branches from the endpoint

                    // Then add branch(es) from the current segment endpoint
                    // Decide number of branches (1 or 2) - slightly favor single branches for more realistic lightning
                    let numNewBranches = select(1, 2, hash(segSeed * 77.777) > 0.6);
                    // 40% chance for 2 branches

                    if (numNewBranches == 1) {
                        // Single branch - angle between 25-60 degrees from parent (more realistic)
                        if (newBranches < 19) {
                            let minAngle = 25.0 * 3.14159 / 180.0;
                            let maxAngle = 60.0 * 3.14159 / 180.0;
                            let angleRange = maxAngle - minAngle;
                            let branchAngle = minAngle + hash(segSeed * 44.444) * angleRange;

                            // Choose left or right side randomly
                            let side = select(- 1.0, 1.0, hash(segSeed * 66.666) > 0.5);
                            let finalBranchAngle = newAngle + side * branchAngle;

                            newBranchPositions[newBranches] = segmentEnd;
                            newBranchDirections[newBranches] = vec2<f32>(cos(finalBranchAngle), sin(finalBranchAngle));
                            newBranchGenerations[newBranches] = generation + 1u;
                            newBranchAppearTimes[newBranches] = stepAppearTime + segmentInterval;
                            newBranches = newBranches + 1;
                        }
                    }
                    else {
                        // Two branches - both at angles between 25-60 degrees from parent
                        if (newBranches < 18) {
                            // Need 2 slots available
                            let minAngle = 25.0 * 3.14159 / 180.0;
                            let maxAngle = 60.0 * 3.14159 / 180.0;
                            let angleRange = maxAngle - minAngle;

                            // Generate two different angles within the valid range
                            let branchAngle1 = minAngle + hash(segSeed * 55.555) * angleRange;
                            let branchAngle2 = minAngle + hash(segSeed * 88.888) * angleRange;

                            let splitAngle1 = newAngle + branchAngle1;
                            // Right side
                            let splitAngle2 = newAngle - branchAngle2;
                            // Left side

                            // First branch
                            newBranchPositions[newBranches] = segmentEnd;
                            newBranchDirections[newBranches] = vec2<f32>(cos(splitAngle1), sin(splitAngle1));
                            newBranchGenerations[newBranches] = generation + 1u;
                            newBranchAppearTimes[newBranches] = stepAppearTime + segmentInterval;
                            newBranches = newBranches + 1;

                            // Second branch
                            newBranchPositions[newBranches] = segmentEnd;
                            newBranchDirections[newBranches] = vec2<f32>(cos(splitAngle2), sin(splitAngle2));
                            newBranchGenerations[newBranches] = generation + 1u;
                            newBranchAppearTimes[newBranches] = stepAppearTime + segmentInterval;
                            newBranches = newBranches + 1;
                        }
                    }
                }
                else {
                    // No branching - continue this branch with minimal direction change
                    if (newBranches < 20) {
                        newBranchPositions[newBranches] = segmentEnd;
                        // Start from end of current segment
                        newBranchDirections[newBranches] = newDir;
                        // Use the slightly adjusted direction
                        newBranchGenerations[newBranches] = generation;
                        // Same generation
                        newBranchAppearTimes[newBranches] = stepAppearTime + segmentInterval;
                        // Next time step
                        newBranches = newBranches + 1;
                    }
                }
            }
        }

        // Update active branches for next iteration
        activeBranches = newBranches;
        for (var j = 0; j < activeBranches; j++) {
            branchPositions[j] = newBranchPositions[j];
            branchDirections[j] = newBranchDirections[j];
            branchGenerations[j] = newBranchGenerations[j];
            branchAppearTimes[j] = newBranchAppearTimes[j];
        }
    }

    return intensity;
}

// Helper function to draw a single segment
fn drawSegment(uv: vec2<f32>, start: vec2<f32>, end: vec2<f32>, alpha: f32, thickness: f32) -> f32 {
    let segmentDir = end - start;
    let segmentLength = length(segmentDir);

    if (segmentLength < 0.001) {
        return 0.0;
    }

    let normalizedDir = segmentDir / segmentLength;
    let toPoint = uv - start;
    let projLength = dot(toPoint, normalizedDir);

    if (projLength >= 0.0 && projLength <= segmentLength) {
        let closestPoint = start + normalizedDir * projLength;
        let distToSegment = length(uv - closestPoint);

        let pixelThickness = thickness / 800.0;
        // Convert pixels to UV coordinates
        let segmentIntensity = (1.0 - smoothstep(0.0, pixelThickness, distToSegment)) * alpha;
        return segmentIntensity;
    }

    return 0.0;
}

@fragment
fn main(@builtin(position) frag_coord: vec4<f32>) -> @location(0) vec4<f32> {
    let resolution = vec2<f32>(sim_params.canvasRenderWidth, sim_params.canvasRenderHeight);
    let uv = frag_coord.xy / resolution;

    // Check if lightning is enabled (frequency > 0)
    if (sim_params.lightningFrequency <= 0.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Calculate lightning timing using proper parameters
    let flashInterval = 1.0 / sim_params.lightningFrequency;
    // Convert frequency to interval
    let timeInCycle = sim_params.time % flashInterval;
    let flashDuration = sim_params.lightningDuration;

    // Only show lightning during flash period
    if (timeInCycle > flashDuration) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Generate lightning bolt
    let flashId = floor(sim_params.time / flashInterval);
    let boltIntensity = lightningBolt(uv, sim_params.time, flashId, timeInCycle);

    if (boltIntensity <= 0.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    // Lightning color - bright blue-white
    let lightningColor = vec3<f32>(0.8, 0.9, 1.0);
    let alpha = boltIntensity * sim_params.lightningIntensity;

    return vec4<f32>(lightningColor, alpha);
}

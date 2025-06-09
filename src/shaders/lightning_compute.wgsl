// Lightning Generation Compute Shader
// Generates lightning segments and stores them in a buffer for both rendering and physics

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

// Lightning segment data structure
struct LightningSegment {
    startPos: vec2<f32>,
    // Segment start position (UV coordinates)
    endPos: vec2<f32>,
    // Segment end position (UV coordinates)
    thickness: f32,
    // Segment thickness in pixels
    alpha: f32,
    // Segment alpha/opacity
    generation: u32,
    // Branch generation (0, 1, 2, 3)
    appearTime: f32,
    // When this segment should appear
    isVisible: u32,
    // 1 if visible, 0 if not (boolean as u32)
    _padding: u32,
    // Padding for alignment
}

// Lightning bolt data structure
struct LightningBolt {
    numSegments: u32,
    // Number of active segments in this bolt
    flashId: u32,
    // Unique flash ID for this bolt
    startTime: f32,
    // When this bolt started
    _padding: u32,
    // Padding for alignment
}

@group(0) @binding(0)
var<uniform> sim_params: SimulationParams;

@group(0) @binding(1)
var<storage, read_write> lightning_segments: array<LightningSegment>;

@group(0) @binding(2)
var<storage, read_write> lightning_bolt: LightningBolt;

// Hash function for lightning generation
fn hash(x: f32) -> f32 {
    var p = x;
    p = fract(p * 0.1031);
    p *= p + 33.33;
    p *= p + p;
    return fract(p);
}

// Floating point modulo function
fn fmod(x: f32, y: f32) -> f32 {
    return x - y * floor(x / y);
}

// Convert UV coordinates to virtual world coordinates
fn uvToWorld(uv: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(uv.x * sim_params.virtualWorldWidth, uv.y * sim_params.virtualWorldHeight);
}

// Convert virtual world coordinates to UV coordinates
fn worldToUV(world: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(world.x / sim_params.virtualWorldWidth, world.y / sim_params.virtualWorldHeight);
}

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // Only use the first thread for lightning generation
    if (global_id.x != 0u || global_id.y != 0u || global_id.z != 0u) {
        return;
    }

    // Check if lightning is enabled
    if (sim_params.lightningFrequency <= 0.0) {
        lightning_bolt.numSegments = 0u;
        return;
    }

    // Calculate lightning timing based on electrical activity
    // electrical activity (interTypeAttractionScale) ranges from 0-3
    // At max activity (3.0), interval should be ~8 seconds with randomization
    let electricalActivity = sim_params.interTypeAttractionScale;
    let baseInterval = mix(20.0, 8.0, min(electricalActivity / 3.0, 1.0)); // 20s at min activity, 8s at max
    
    // Calculate current flash cycle using accumulated time accounting for variable intervals
    // We need to track which flash cycle we're in more carefully
    var accumulatedTime = 0.0;
    var flashId = 0u;
    var flashStartTime = 0.0;
    var flashInterval = baseInterval;
    
    // Find which flash cycle we're currently in
    for (var i = 0u; i < 1000u; i++) { // Safety limit to prevent infinite loops
        // Calculate interval for this flash cycle
        let flashRandomSeed = hash(f32(i) * 12.345);
        let randomFactor = 0.75 + 0.5 * flashRandomSeed; // Range: 0.75 to 1.25
        flashInterval = baseInterval * randomFactor;
        
        if (sim_params.time < accumulatedTime + flashInterval) {
            // We're in this flash cycle
            flashId = i;
            flashStartTime = accumulatedTime;
            break;
        }
        
        accumulatedTime += flashInterval;
    }
    
    // Calculate time within this specific flash interval
    let timeInCycle = sim_params.time - flashStartTime;
    let flashDuration = sim_params.lightningDuration;

    // Clear segments if we're not in a lightning flash
    if (timeInCycle > flashDuration) {
        lightning_bolt.numSegments = 0u;
        return;
    }

    // Generate new lightning bolt when flash starts
    if (lightning_bolt.flashId != flashId) {
        // New lightning bolt starting
        lightning_bolt.flashId = flashId;
        lightning_bolt.startTime = flashStartTime;
        lightning_bolt.numSegments = 0u;
    }

    // Generate lightning bolt using the same algorithm as the original fragment shader
    let baseSeed = hash(f32(flashId) * 73.421); // Use flashId instead of time-based seed

    // Random starting position constrained to circle with radius 0.25UV (600px) from center
    let posRand = hash(baseSeed * 555.0);
    let posRand2 = hash(baseSeed * 777.0);
    let dirRand = hash(baseSeed * 333.0);
    
    // Generate point within circle of radius 0.25UV from center (0.5, 0.5)
    let radius = sqrt(posRand) * 0.25; // Square root for uniform distribution
    let theta = posRand2 * 6.28318; // Random angle
    let center = vec2<f32>(0.5, 0.5); // Screen center in UV coordinates
    let startPos = center + vec2<f32>(cos(theta), sin(theta)) * radius;

    // Random initial direction
    let angle = dirRand * 6.28318;
    let initialDirection = vec2<f32>(cos(angle), sin(angle));

    // Lightning parameters (matching fragment shader exactly)
    let maxTotalSegments = 30u;
    let minSegmentLengthPx = 40.0;
    let maxSegmentLengthPx = 90.0;
    let resolution = vec2<f32>(sim_params.canvasRenderWidth, sim_params.canvasRenderHeight);
    let minSegmentLengthUV = minSegmentLengthPx / min(resolution.x, resolution.y);
    let maxSegmentLengthUV = maxSegmentLengthPx / min(resolution.x, resolution.y);
    let segmentInterval = 0.07;
    let segmentDuration = 0.4;

    // Arrays to store active branch endpoints (simulate dynamic arrays with fixed size)
    var branchPositions: array<vec2<f32>, 20>;
    var branchDirections: array<vec2<f32>, 20>;
    var branchGenerations: array<u32, 20>;
    var branchAppearTimes: array<f32, 20>;

    var activeBranches: u32 = 0u;
    var segmentCount = 0u;

    // Initialize with root segment
    branchPositions[0] = startPos;
    branchDirections[0] = initialDirection;
    branchGenerations[0] = 0u;
    branchAppearTimes[0] = 0.0;
    activeBranches = 1u;

    // Generate segments progressively
    for (var step: u32 = 0u; step < maxTotalSegments && activeBranches > 0u; step++) {
        let stepAppearTime = f32(step) * segmentInterval;

        // Skip if this step shouldn't appear yet
        if (timeInCycle < stepAppearTime) {
            break;
        }

        var newBranches: u32 = 0u;
        var newBranchPositions: array<vec2<f32>, 20>;
        var newBranchDirections: array<vec2<f32>, 20>;
        var newBranchGenerations: array<u32, 20>;
        var newBranchAppearTimes: array<f32, 20>;

        // Process each active branch
        for (var i: u32 = 0u; i < activeBranches; i++) {
            let currentPos = branchPositions[i];
            let currentDir = branchDirections[i];
            let generation = branchGenerations[i];
            let branchAppearTime = branchAppearTimes[i];

            // Only process branches that should be visible
            if (timeInCycle >= branchAppearTime) {
                // Calculate segment properties
                let segSeed = hash(baseSeed + f32(step) * 73.0 + f32(i) * 37.0);

                // Decide branching first (90% chance to branch)
                let branchSeed = hash(segSeed * 99.999);
                let shouldBranch = branchSeed > 0.1 && generation < 4u && newBranches < 16;

                // Angle change logic
                var angleChange: f32;
                if (shouldBranch) {
                    angleChange = (hash(segSeed * 11.111) - 0.5) * 0.4;
                }
                else {
                    angleChange = (hash(segSeed * 11.111) - 0.5) * 0.1;
                }

                let newAngle = atan2(currentDir.y, currentDir.x) + angleChange;
                var newDir = vec2<f32>(cos(newAngle), sin(newAngle));

                // Calculate segment length with randomization and generation decay
                let lengthSeed = hash(segSeed * 222.222);
                let baseLengthUV = minSegmentLengthUV + lengthSeed * (maxSegmentLengthUV - minSegmentLengthUV);
                let generationDecay = pow(0.8, f32(generation));
                let currentSegmentLength = max(minSegmentLengthUV, baseLengthUV * generationDecay);
                let segmentEnd = currentPos + newDir * currentSegmentLength;

                // Calculate thickness and alpha (thinner and dimmer for higher generations)
                let rawThickness = 3.0 * pow(0.8, f32(generation));
                let thickness = max(1.0, min(3.0, rawThickness));
                let baseAlpha = 1.0 * pow(0.9, f32(generation));

                // Check if segment should appear yet and is still within flash duration
                let segmentAge = timeInCycle - stepAppearTime;
                let isVisible = segmentAge >= 0.0 && segmentAge < segmentDuration;

                // Store segment in buffer if we have space and it's visible
                // Convert UV coordinates to world coordinates for physics simulation
                if (segmentCount < arrayLength(&lightning_segments) && isVisible) {
                    lightning_segments[segmentCount].startPos = uvToWorld(currentPos);
                    lightning_segments[segmentCount].endPos = uvToWorld(segmentEnd);
                    lightning_segments[segmentCount].thickness = thickness;
                    lightning_segments[segmentCount].alpha = baseAlpha;
                    lightning_segments[segmentCount].generation = generation;
                    lightning_segments[segmentCount].appearTime = lightning_bolt.startTime + stepAppearTime; // FIX: Store absolute time, not relative
                    lightning_segments[segmentCount].isVisible = select(0u, 1u, isVisible);
                    lightning_segments[segmentCount]._padding = 0u;
                    segmentCount = segmentCount + 1u;
                }

                // Branching logic
                if (shouldBranch) {
                    // Decide number of branches (1 or 2) - 40% chance for 2 branches
                    let numNewBranches = select(1, 2, hash(segSeed * 77.777) > 0.6);

                    if (numNewBranches == 1) {
                        // Single branch - angle between 25-60 degrees from parent
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
                            newBranches = newBranches + 1u;
                        }
                    }
                    else {
                        // Two branches - both at angles between 25-60 degrees from parent
                        if (newBranches < 18) {
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
                            newBranches = newBranches + 1u;

                            // Second branch
                            newBranchPositions[newBranches] = segmentEnd;
                            newBranchDirections[newBranches] = vec2<f32>(cos(splitAngle2), sin(splitAngle2));
                            newBranchGenerations[newBranches] = generation + 1u;
                            newBranchAppearTimes[newBranches] = stepAppearTime + segmentInterval;
                            newBranches = newBranches + 1u;
                        }
                    }
                }
                else {
                    // No branching - continue this branch
                    if (newBranches < 20) {
                        newBranchPositions[newBranches] = segmentEnd;
                        newBranchDirections[newBranches] = newDir;
                        newBranchGenerations[newBranches] = generation;
                        newBranchAppearTimes[newBranches] = stepAppearTime + segmentInterval;
                        newBranches = newBranches + 1u;
                    }
                }
            }
        }

        // Update active branches for next iteration
        activeBranches = newBranches;
        for (var j: u32 = 0u; j < activeBranches; j++) {
            branchPositions[j] = newBranchPositions[j];
            branchDirections[j] = newBranchDirections[j];
            branchGenerations[j] = newBranchGenerations[j];
            branchAppearTimes[j] = newBranchAppearTimes[j];
        }
    }

    // Update total segment count
    lightning_bolt.numSegments = segmentCount;
}

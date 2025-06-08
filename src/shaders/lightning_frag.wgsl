// Lightning effect fragment shader - SIMPLIFIED VERSION
// Creates lightning bolts that build up segment by segment with proper timing

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

// Generate lightning bolt that builds up over time - segments appear every 0.5s and stay for 2s
fn lightningBolt(uv: vec2<f32>, time: f32, boltId: f32, timeInCycle: f32) -> f32 {
    // Create seed for this bolt
    let baseSeed = hash(boltId * 73.421 + time * 0.1);

    // Random starting position and direction
    let edgeRand = hash(baseSeed * 999.0);
    let posRand = hash(baseSeed * 555.0);
    let dirRand = hash(baseSeed * 333.0);

    var startPos: vec2<f32>;
    var direction: vec2<f32>;

    // Start from anywhere on screen (not just edges)
    let posRand2 = hash(baseSeed * 777.0);
    startPos = vec2<f32>(0.2 + posRand * 0.6, 0.2 + posRand2 * 0.6);
    // Anywhere in center area

    // Random direction (360 degrees)
    let angle = dirRand * 6.28318;
    // 0 to 2π
    direction = vec2<f32>(cos(angle), sin(angle));

    let numSegments = 20;
    let segmentLength = 0.03;
    let segmentInterval = 0.02;
    // New segment every 0.02 seconds (faster buildup)
    let segmentDuration = sim_params.lightningDuration;
    // Each segment visible for flash duration only

    var intensity = 0.0;
    var currentPos = startPos;
    var currentDir = direction;

    for (var i = 0; i < numSegments; i++) {
        let segmentIndex = f32(i);

        // When this segment should appear and disappear
        let appearTime = segmentIndex * segmentInterval;
        let fadeTime = appearTime + segmentDuration;

        // Calculate current position even for invisible segments (to maintain path)
        let segSeed = hash(baseSeed + segmentIndex * 12.345);
        let angleChange = (hash(segSeed * 11.111) - 0.5) * 1.0;
        // ±0.5 radians
        let newAngle = atan2(currentDir.y, currentDir.x) + angleChange;
        currentDir = vec2<f32>(cos(newAngle), sin(newAngle));

        let segmentEnd = currentPos + currentDir * segmentLength;

        // Check if this segment should be visible now
        if (timeInCycle >= appearTime && timeInCycle <= fadeTime) {
            // Calculate fade based on age
            let segmentAge = timeInCycle - appearTime;
            let fadeProgress = segmentAge / segmentDuration;
            let segmentAlpha = 1.0 - fadeProgress;
            // Fade out over time

            // Check if point is near this segment
            let toPoint = uv - currentPos;
            let projLength = dot(toPoint, currentDir);

            if (projLength >= 0.0 && projLength <= segmentLength) {
                let closestPoint = currentPos + currentDir * projLength;
                let distToSegment = length(uv - closestPoint);

                let thickness = 3.0 / 800.0;
                // 3 pixels thick
                let segmentIntensity = (1.0 - smoothstep(0.0, thickness, distToSegment)) * segmentAlpha;
                intensity = max(intensity, segmentIntensity);
            }
        }

        currentPos = segmentEnd;
    }

    return intensity;
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

# Lenia Integration Concepts for Particle Life

## 1. Density Field Approach
- Add a 2D density texture that tracks particle concentrations
- Particles contribute to local density based on their position
- Density field evolves using Lenia's growth function: μ(U) = 2 * exp(-(U-μ)²/2σ²) - 1
- Particles are influenced by local density gradients

## 2. Continuous Kernels
Replace hard min/max radius rules with smooth Gaussian kernels:
- K(r) = exp(-r²/2σ²) for neighbor detection
- Smooth transitions instead of sharp cutoffs
- Multiple kernel scales for different interaction types

## 3. Growth/Decay Mechanics
- Particles can "grow" (increase in size/strength) in favorable density
- Particles "decay" in unfavorable conditions
- Spawning/death based on local density patterns

## 4. Implementation Options:

### Option A: Compute Shader Density Field
- 2D texture for density
- Particles write to density in compute pass
- Separate compute pass for Lenia evolution
- Particles read density for behavior modification

### Option B: Particle-based Density
- Each particle carries density information
- Local density calculated via smooth kernels
- Growth/decay applied per particle

### Option C: Hybrid Approach
- Coarse density grid for global patterns
- Fine particle interactions for detail
- Multi-scale interactions like real Lenia

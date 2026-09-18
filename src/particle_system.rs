// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

use crate::config::*;
use crate::{InteractionRules, SimulationParams, SpatialGrid};
use rand::prelude::*;
use rand::rngs::SmallRng;

#[derive(Debug, Clone)]
pub struct Particle {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub particle_type: u32,
    pub size: f32,
    pub target_size: f32,
    pub transition_start: f32, // Start time of transition, 0 means no transition
    pub transition_type: u32,  // 0 = grow, 1 = shrink
    pub is_active: bool,       // Whether this particle is active/visible
}

// Size ranges for each particle type (multipliers of base size)
// We'll use the middle value of each range with ±20% randomization
//
// Types 8-10 are WLP-exclusive "flavor" particles (see compute.wgsl's respawn
// logic): HTV spawns only draw from types 0-7 exactly as before, WLP spawns
// draw from 0-4 (shared "same molecules" baseline) plus 8-10. Types 5-7 remain
// HTV-only. Kept as new slots rather than repainting 5-7 so already-visible
// particles never change appearance when the hypothesis switches — only new
// spawns (at the edge, via the existing respawn system) reflect it.
const NUM_TYPES: usize = 11;

// Probability weight per particle type, used by the CPU-side initial spawn
// (ParticleSystem::new) only. Deliberately zero for 8-10: the initial
// population always starts HTV-flavored regardless of the active hypothesis
// at boot — it self-corrects to the right mix within the first respawn cycle,
// since particles constantly cycle through the edge-respawn system. The
// hypothesis-aware weight tables that actually matter live in compute.wgsl
// (HTV_TYPE_WEIGHTS / WLP_TYPE_WEIGHTS), applied on every respawn.
const PARTICLE_TYPE_WEIGHTS: [f32; NUM_TYPES] = [
    0.22, // Type 0: Blue
    0.15, // Type 1: Yellow
    0.11, // Type 2: Red
    0.08, // Type 3: Purple
    0.18, // Type 4: Green
    0.08, // Type 5: Olive (HTV-only)
    0.15, // Type 6: Blue-gray (HTV-only)
    0.10, // Type 7: Blue-green (HTV-only)
    0.0,  // Type 8: WLP-only, never spawned at boot
    0.0,  // Type 9: WLP-only, never spawned at boot
    0.0,  // Type 10: WLP-only, never spawned at boot
];

const PARTICLE_TYPE_SIZE_MULTIPLIERS: [f32; NUM_TYPES] = [
    1.4, // Type 0: Blue
    1.9, // Type 1: Yellow
    0.5, // Type 2: Red
    0.8, // Type 3: Purple
    1.0, // Type 4: Green
    1.8, // Type 5: Olive
    0.7, // Type 6: Cyan
    1.2, // Type 7: Blue-green
    1.7, // Type 8: WLP-only — large "photosynthetic bloom"
    0.6, // Type 9: WLP-only — tiny "microbial"
    1.5, // Type 10: WLP-only — medium
];

// Default color palette (sRGB 0-1), tuned live via the in-app palette tool.
// Types 0-7 are HTV's palette; 8-10 are WLP-exclusive tones, contrasting with
// the HTV palette above them.
const DEFAULT_COLORS: [[f32; 3]; NUM_TYPES] = [
    [0.4435, 0.6471, 0.5528], // #71a58d - Blue
    [0.8570, 0.7567, 0.5449], // #dbc18b - Yellow
    [0.7540, 0.2949, 0.4556], // #c04b74 - Red
    [0.6598, 0.3962, 0.6094], // #a8659b - Purple
    [0.3153, 0.6550, 0.4474], // #50a772 - Green
    [0.5254, 0.5925, 0.4079], // #869768 - Olive green
    [0.4112, 0.5815, 0.7227], // #6994b8 - Orange
    [0.6332, 0.5649, 0.6777], // #a190ad - WLP: lavender
    [0.5224, 0.6671, 0.7465], // #85aabe - WLP: sky blue
    [0.6478, 0.6650, 0.1350], // #a5aa22 - WLP: olive-yellow
    [0.5414, 0.4696, 0.6847], // #8a78af - WLP: violet
];

#[derive(Debug)]
pub struct ParticleSystem {
    particles: Vec<Particle>,
    max_particles: u32,
    min_particles: u32,
    active_count: u32,
    num_types: u32,
    base_particle_size: f32,
    pub particle_opacity: f32,
    pub type_colors: [[f32; 3]; NUM_TYPES],
    spatial_grid: SpatialGrid,
}

impl ParticleSystem {
    pub fn new(params: &SimulationParams, _rules: &InteractionRules, rng: &mut SmallRng) -> Self {
        let max_particles = MAX_PARTICLES;
        let min_particles = MIN_PARTICLES;
        let num_types = NUM_TYPES as u32;

        let mut particles = Vec::with_capacity(max_particles as usize);

        // Initialize all particles (even inactive ones)
        for i in 0..max_particles {
            let weight_sum: f32 = PARTICLE_TYPE_WEIGHTS.iter().sum();
            let r = rng.gen::<f32>() * weight_sum;
            let mut cumulative = 0.0f32;
            let particle_type = PARTICLE_TYPE_WEIGHTS
                .iter()
                .enumerate()
                .find(|(_, &w)| {
                    cumulative += w;
                    r < cumulative
                })
                .map(|(i, _)| i as u32)
                .unwrap_or(num_types - 1);
            let base_multiplier = PARTICLE_TYPE_SIZE_MULTIPLIERS[particle_type as usize];

            // Add ±40% randomization to the base multiplier
            let randomization_factor = rng.gen_range(-0.4..0.4);
            let size_multiplier = base_multiplier * (1.0 + randomization_factor);

            let particle = Particle {
                position: [
                    rng.gen_range(0.0..params.virtual_world_width),
                    rng.gen_range(0.0..params.virtual_world_height),
                ],
                velocity: [rng.gen_range(-2.0..2.0), rng.gen_range(-2.0..2.0)],
                particle_type,
                size: params.particle_render_size * size_multiplier, // Use SimulationParams.particle_render_size
                target_size: params.particle_render_size * size_multiplier, // Store the intended size
                transition_start: 0.0,               // No transition initially
                transition_type: 0,                  // Default to grow type
                is_active: i < params.num_particles, // Only first num_particles are initially active
            };
            particles.push(particle);
        }

        // Create spatial grid for optimization (cell size = 100 pixels)
        // This will help reduce O(n²) particle interactions to roughly O(n log n)
        let spatial_grid = SpatialGrid::new(
            params.virtual_world_width,
            params.virtual_world_height,
            100.0, // Cell size - particles within ~100 pixels will be in same or adjacent cells
        );

        Self {
            particles,
            max_particles,
            min_particles,
            active_count: params.num_particles,
            num_types,
            base_particle_size: params.particle_render_size,
            particle_opacity: 0.8,
            type_colors: DEFAULT_COLORS,
            spatial_grid,
        }
    }

    pub fn get_active_count(&self) -> u32 {
        self.active_count
    }

    pub fn get_max_particles(&self) -> u32 {
        self.max_particles
    }

    pub fn get_min_particles(&self) -> u32 {
        self.min_particles
    }

    pub fn get_num_types(&self) -> u32 {
        self.num_types
    }

    pub fn set_particle_limits(&mut self, max: u32, min: u32) {
        self.max_particles = max;
        self.min_particles = min;
        if self.active_count > max {
            self.active_count = max;
        }
    }

    pub fn set_active_count(&mut self, count: u32) {
        let new_count = count.min(self.max_particles);
        self.active_count = new_count;

        // Update is_active flag for all affected particles
        for i in 0..self.max_particles as usize {
            if let Some(particle) = self.particles.get_mut(i) {
                particle.is_active = (i as u32) < new_count;
            }
        }
    }

    pub fn get_particle(&self, index: usize) -> Option<&Particle> {
        self.particles.get(index)
    }

    pub fn get_particle_mut(&mut self, index: usize) -> Option<&mut Particle> {
        if index < self.particles.len() {
            Some(&mut self.particles[index])
        } else {
            None
        }
    }

    // Convert particles to buffer format for GPU upload
    pub fn to_buffer(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(self.max_particles as usize * 48); // 10 fields + 8 bytes padding = 48 bytes per particle (16-byte aligned)

        for i in 0..self.max_particles as usize {
            if let Some(particle) = self.particles.get(i) {
                // Position (vec2f) - 8 bytes
                buffer.extend_from_slice(&particle.position[0].to_le_bytes());
                buffer.extend_from_slice(&particle.position[1].to_le_bytes());

                // Velocity (vec2f) - 8 bytes
                buffer.extend_from_slice(&particle.velocity[0].to_le_bytes());
                buffer.extend_from_slice(&particle.velocity[1].to_le_bytes());

                // Type (u32) - 4 bytes
                buffer.extend_from_slice(&particle.particle_type.to_le_bytes());

                // Size (f32) - 4 bytes
                buffer.extend_from_slice(&particle.size.to_le_bytes());

                // Target size (f32) - 4 bytes
                buffer.extend_from_slice(&particle.target_size.to_le_bytes());

                // Transition start (f32) - 4 bytes
                buffer.extend_from_slice(&particle.transition_start.to_le_bytes());

                // Transition type (u32) - 4 bytes
                buffer.extend_from_slice(&particle.transition_type.to_le_bytes());

                // Is active (u32) - 4 bytes (bool serialized as u32: 0 or 1)
                let is_active_u32 = if particle.is_active { 1u32 } else { 0u32 };
                buffer.extend_from_slice(&is_active_u32.to_le_bytes());

                // spawn_time = -1.0 (sentinel: initial particle, exempt from spawn damping)
                buffer.extend_from_slice(&(-1.0f32).to_le_bytes());
                buffer.extend_from_slice(&0.0f32.to_le_bytes()); // _padding2
            } else {
                // Fill with zeros for missing particles
                buffer.extend_from_slice(&[0u8; 48]);
            }
        }

        buffer
    }

    // Get particle colors buffer in RGBA format
    pub fn get_colors_buffer(&self) -> Vec<u8> {
        let default_opacity = self.particle_opacity;
        let mut buffer = Vec::with_capacity(self.num_types as usize * 16); // 4 floats per type

        for i in 0..self.num_types as usize {
            let color = self.type_colors[i % NUM_TYPES];

            // Store RGBA values as little-endian f32
            buffer.extend_from_slice(&color[0].to_le_bytes()); // Red
            buffer.extend_from_slice(&color[1].to_le_bytes()); // Green
            buffer.extend_from_slice(&color[2].to_le_bytes()); // Blue
            buffer.extend_from_slice(&default_opacity.to_le_bytes()); // Alpha
        }

        buffer
    }

    // Set particle size for transitions
    pub fn set_particle_size(&mut self, index: usize, size: f32) {
        if let Some(particle) = self.particles.get_mut(index) {
            particle.size = size;
        }
    }

    // Get particle size for transitions
    pub fn get_particle_size(&self, index: usize) -> f32 {
        self.particles.get(index).map(|p| p.size).unwrap_or(0.0)
    }

    // Update physics for all active particles
    pub fn update_physics(&mut self, params: &SimulationParams, rules: &InteractionRules) {
        // Clear and populate spatial grid with only active particles
        self.spatial_grid.clear();
        for i in 0..self.max_particles as usize {
            if let Some(particle) = self.particles.get(i) {
                if particle.is_active {
                    self.spatial_grid.insert(i, particle);
                }
            }
        }

        // Collect indices of active particles
        let active_indices: Vec<usize> = (0..self.max_particles as usize)
            .filter(|&i| self.particles.get(i).map(|p| p.is_active).unwrap_or(false))
            .collect();

        // Calculate forces for all active particles using spatial optimization
        let mut forces: Vec<[f32; 2]> = vec![[0.0, 0.0]; active_indices.len()];

        for (idx, &i) in active_indices.iter().enumerate() {
            if let Some(particle_a) = self.particles.get(i) {
                let mut total_force = [0.0f32, 0.0f32];

                // Get nearby particles using spatial grid instead of checking all particles
                let max_interaction_radius = 50.0; // Adjust based on your interaction rules
                let nearby_particles = self
                    .spatial_grid
                    .get_nearby_particles(particle_a, max_interaction_radius);

                // Calculate forces from nearby particles only
                for &j in &nearby_particles {
                    if i == j {
                        continue;
                    }

                    if let Some(particle_b) = self.particles.get(j) {
                        if !particle_b.is_active {
                            continue;
                        }
                        // Calculate distance vector
                        let dx = particle_b.position[0] - particle_a.position[0];
                        let dy = particle_b.position[1] - particle_a.position[1];

                        // Handle world wrapping if enabled
                        let (dx, dy) = if params.boundary_mode == 1 {
                            // Wrap mode
                            let dx = if dx > params.virtual_world_width / 2.0 {
                                dx - params.virtual_world_width
                            } else if dx < -params.virtual_world_width / 2.0 {
                                dx + params.virtual_world_width
                            } else {
                                dx
                            };

                            let dy = if dy > params.virtual_world_height / 2.0 {
                                dy - params.virtual_world_height
                            } else if dy < -params.virtual_world_height / 2.0 {
                                dy + params.virtual_world_height
                            } else {
                                dy
                            };
                            (dx, dy)
                        } else {
                            (dx, dy)
                        };

                        let distance = (dx * dx + dy * dy).sqrt();

                        // Skip if particles are too close (avoid division by zero)
                        if distance < 0.1 {
                            continue;
                        }

                        // Get interaction rule
                        let rule = rules.get_rule(
                            particle_a.particle_type as usize,
                            particle_b.particle_type as usize,
                        );

                        // Apply scaling factors
                        let min_radius = rule.min_radius * params.inter_type_radius_scale;
                        let max_radius = rule.max_radius * params.inter_type_radius_scale;
                        let attraction = rule.attraction * params.inter_type_attraction_scale;

                        // Skip if outside interaction range
                        if distance > max_radius {
                            continue;
                        }

                        // Calculate force magnitude
                        let force_magnitude = if params.flat_force {
                            // Flat force model
                            if distance < min_radius {
                                -attraction * params.force_scale
                            } else {
                                attraction * params.force_scale
                            }
                        } else {
                            // Smooth force model with r_smooth parameter
                            let normalized_distance = if distance < min_radius {
                                distance / min_radius
                            } else {
                                1.0 + (distance - min_radius) / (max_radius - min_radius)
                            };

                            let smooth_factor = if params.r_smooth > 0.0 {
                                1.0 / (1.0 + (normalized_distance * params.r_smooth).exp())
                            } else {
                                1.0
                            };

                            let base_force = if distance < min_radius {
                                -attraction // Repulsive when too close
                            } else {
                                attraction
                                    * (1.0 - (distance - min_radius) / (max_radius - min_radius))
                            };

                            base_force * smooth_factor * params.force_scale
                        };

                        // Apply force in direction of other particle
                        let norm_dx = dx / distance;
                        let norm_dy = dy / distance;

                        total_force[0] += force_magnitude * norm_dx;
                        total_force[1] += force_magnitude * norm_dy;
                    }
                }

                forces[idx] = total_force;
            }
        }

        // Apply forces and update positions
        for (idx, &i) in active_indices.iter().enumerate() {
            if let Some(particle) = self.particles.get_mut(i) {
                let force = forces[idx];

                // Update velocity with force and friction
                particle.velocity[0] =
                    particle.velocity[0] * (1.0 - params.friction) + force[0] * params.delta_time;
                particle.velocity[1] =
                    particle.velocity[1] * (1.0 - params.friction) + force[1] * params.delta_time;

                // Apply drift
                particle.velocity[0] += params.drift_x_per_second * params.delta_time;

                // Update position
                particle.position[0] += particle.velocity[0] * params.delta_time;
                particle.position[1] += particle.velocity[1] * params.delta_time;

                // Handle boundaries
                match params.boundary_mode {
                    1 => {
                        // Wrap mode
                        if particle.position[0] < 0.0 {
                            particle.position[0] += params.virtual_world_width;
                        } else if particle.position[0] >= params.virtual_world_width {
                            particle.position[0] -= params.virtual_world_width;
                        }

                        if particle.position[1] < 0.0 {
                            particle.position[1] += params.virtual_world_height;
                        } else if particle.position[1] >= params.virtual_world_height {
                            particle.position[1] -= params.virtual_world_height;
                        }
                    }
                    2 => {
                        // Bounce mode
                        if particle.position[0] < 0.0 {
                            particle.position[0] = 0.0;
                            particle.velocity[0] = -particle.velocity[0] * 0.8; // Damping
                        } else if particle.position[0] >= params.virtual_world_width {
                            particle.position[0] = params.virtual_world_width - 0.1;
                            particle.velocity[0] = -particle.velocity[0] * 0.8;
                        }

                        if particle.position[1] < 0.0 {
                            particle.position[1] = 0.0;
                            particle.velocity[1] = -particle.velocity[1] * 0.8;
                        } else if particle.position[1] >= params.virtual_world_height {
                            particle.position[1] = params.virtual_world_height - 0.1;
                            particle.velocity[1] = -particle.velocity[1] * 0.8;
                        }
                    }
                    _ => { // No boundary handling (particles can go outside)
                         // Do nothing - let particles move freely
                    }
                }
            }
        }
    }

    // Update particle sizes when SimulationParams.particle_render_size changes
    pub fn update_particle_sizes(&mut self, new_base_size: f32, _rng: &mut SmallRng) {
        // Calculate the scaling factor from old base size to new base size
        let size_scale_factor = if self.base_particle_size > 0.0 {
            new_base_size / self.base_particle_size
        } else {
            1.0
        };

        self.base_particle_size = new_base_size;

        // Update all particles by scaling their existing target_size
        // This preserves the original per-particle randomization
        for i in 0..self.max_particles as usize {
            if let Some(particle) = self.particles.get_mut(i) {
                // Scale both current size and target_size proportionally
                // This preserves the original ±20% randomization that was applied at creation
                particle.size *= size_scale_factor;
                particle.target_size *= size_scale_factor;
            }
        }
    }

    pub fn get_size_multiplier_for_type(&self, particle_type: u32) -> f32 {
        PARTICLE_TYPE_SIZE_MULTIPLIERS[particle_type as usize]
    }
}

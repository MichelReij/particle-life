use crate::{InteractionRules, SimulationParams, SpatialGrid};
use rand::prelude::*;
use rand::rngs::SmallRng;

#[derive(Debug, Clone)]
pub struct Particle {
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub particle_type: u32,
    pub size: f32,
}

// Size ranges for each particle type (multipliers of base size)
// We'll use the middle value of each range with ±20% randomization
const PARTICLE_TYPE_SIZE_MULTIPLIERS: [f32; 5] = [
    1.5, // Type 0: Blue   - large, dominant (middle of 1.4-1.6)
    1.2, // Type 1: Orange - medium-large (middle of 1.1-1.3)
    0.7, // Type 2: Red    - small, agile (middle of 0.6-0.8)
    0.9, // Type 3: Purple - smaller, compact (middle of 0.8-1.0)
    1.0, // Type 4: Green  - medium, balanced (middle of 0.9-1.1)
];

// Custom color palette matching TypeScript version
const CUSTOM_COLORS: [[f32; 3]; 5] = [
    [0.0141, 0.4549, 0.6784], // #0374ad - Blue
    [0.7804, 0.5216, 0.0745], // #c78513 - Orange
    [0.7490, 0.1098, 0.1098], // #bf1c1c - Red
    [0.4275, 0.1882, 0.7412], // #6d30bd - Purple
    [0.3216, 0.5843, 0.3020], // #52964d - Green
];

#[derive(Debug)]
pub struct ParticleSystem {
    particles: Vec<Particle>,
    max_particles: u32,
    min_particles: u32,
    active_count: u32,
    num_types: u32,
    base_particle_size: f32,
    spatial_grid: SpatialGrid,
}

impl ParticleSystem {
    pub fn new(params: &SimulationParams, _rules: &InteractionRules, rng: &mut SmallRng) -> Self {
        let max_particles = 6400;
        let min_particles = 1600;
        let num_types = 5;

        let mut particles = Vec::with_capacity(max_particles as usize);

        // Initialize all particles (even inactive ones)
        for i in 0..max_particles {
            let particle_type = (i % num_types) as u32;
            let base_multiplier = PARTICLE_TYPE_SIZE_MULTIPLIERS[particle_type as usize];

            // Add ±20% randomization to the base multiplier
            let randomization_factor = rng.gen_range(-0.2..0.2);
            let size_multiplier = base_multiplier * (1.0 + randomization_factor);

            let particle = Particle {
                position: [
                    rng.gen_range(0.0..params.virtual_world_width),
                    rng.gen_range(0.0..params.virtual_world_height),
                ],
                velocity: [rng.gen_range(-2.0..2.0), rng.gen_range(-2.0..2.0)],
                particle_type,
                size: params.particle_render_size * size_multiplier, // Use SimulationParams.particle_render_size
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
            base_particle_size: params.particle_render_size, // Store the base size from params
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

    pub fn set_active_count(&mut self, count: u32) {
        self.active_count = count.min(self.max_particles);
    }

    pub fn grow_particles(&mut self, new_count: u32, rng: &mut SmallRng) {
        let old_count = self.active_count;
        self.active_count = new_count.min(self.max_particles);

        // Initialize newly activated particles
        for i in old_count..self.active_count {
            let particle_type = (i % self.num_types) as u32;
            let base_multiplier = PARTICLE_TYPE_SIZE_MULTIPLIERS[particle_type as usize];

            // Add ±20% randomization to the base multiplier
            let randomization_factor = rng.gen_range(-0.2..0.2);
            let size_multiplier = base_multiplier * (1.0 + randomization_factor);

            if let Some(particle) = self.particles.get_mut(i as usize) {
                particle.position = [
                    rng.gen_range(0.0..2400.0), // Use default world size
                    rng.gen_range(0.0..2400.0),
                ];
                particle.velocity = [rng.gen_range(-2.0..2.0), rng.gen_range(-2.0..2.0)];
                particle.particle_type = particle_type;
                particle.size = self.base_particle_size * size_multiplier; // Use the base size from SimulationParams
            }
        }
    }

    pub fn get_particle(&self, index: usize) -> Option<&Particle> {
        self.particles.get(index)
    }

    pub fn get_particle_mut(&mut self, index: usize) -> Option<&mut Particle> {
        self.particles.get_mut(index)
    }

    // Convert particles to buffer format for GPU upload
    pub fn to_buffer(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(self.max_particles as usize * 24);

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
            } else {
                // Fill with zeros for missing particles
                buffer.extend_from_slice(&[0u8; 24]);
            }
        }

        buffer
    }

    // Get particle colors buffer in RGBA format
    pub fn get_colors_buffer(&self) -> Vec<u8> {
        let default_opacity = 0.6f32;
        let mut buffer = Vec::with_capacity(self.num_types as usize * 16); // 4 floats per type

        for i in 0..self.num_types as usize {
            let color = CUSTOM_COLORS[i % CUSTOM_COLORS.len()];

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
        let active_count = self.active_count as usize;

        // Clear and populate spatial grid
        self.spatial_grid.clear();
        for i in 0..active_count {
            if let Some(particle) = self.particles.get(i) {
                self.spatial_grid.insert(i, particle);
            }
        }

        // Calculate forces for all active particles using spatial optimization
        let mut forces: Vec<[f32; 2]> = vec![[0.0, 0.0]; active_count];

        for i in 0..active_count {
            if let Some(particle_a) = self.particles.get(i) {
                let mut total_force = [0.0f32, 0.0f32];

                // Get nearby particles using spatial grid instead of checking all particles
                let max_interaction_radius = 50.0; // Adjust based on your interaction rules
                let nearby_particles = self
                    .spatial_grid
                    .get_nearby_particles(particle_a, max_interaction_radius);

                // Calculate forces from nearby particles only
                for &j in &nearby_particles {
                    if i == j || j >= active_count {
                        continue;
                    }

                    if let Some(particle_b) = self.particles.get(j) {
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

                forces[i] = total_force;
            }
        }

        // Apply forces and update positions
        for i in 0..active_count {
            if let Some(particle) = self.particles.get_mut(i) {
                let force = forces[i];

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
    pub fn update_particle_sizes(&mut self, new_base_size: f32, rng: &mut SmallRng) {
        self.base_particle_size = new_base_size;

        // Update all active particles
        for i in 0..self.active_count as usize {
            if let Some(particle) = self.particles.get_mut(i) {
                let base_multiplier = PARTICLE_TYPE_SIZE_MULTIPLIERS[particle.particle_type as usize];

                // Add ±20% randomization to the base multiplier
                let randomization_factor = rng.gen_range(-0.2..0.2);
                let size_multiplier = base_multiplier * (1.0 + randomization_factor);
                particle.size = new_base_size * size_multiplier;
            }
        }
    }
}

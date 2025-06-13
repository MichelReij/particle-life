use crate::{console_log, ParticleSystem};

#[derive(Debug, Clone)]
pub struct ParticleTransition {
    pub start_index: u32,
    pub end_index: u32,
    pub start_time: f32,
    pub duration: f32,
    pub transition_type: TransitionType,
    pub target_sizes: Vec<f32>,
    pub needs_post_compute_finalization: bool,
}

#[derive(Debug, Clone)]
pub enum TransitionType {
    Grow,
    Shrink,
}

#[derive(Debug)]
pub struct ParticleTransitions {
    active_transitions: Vec<ParticleTransition>,
    transition_duration: f32,
}

impl ParticleTransitions {
    pub fn new() -> Self {
        Self {
            active_transitions: Vec::new(),
            transition_duration: 1.5, // 1.5 seconds for smooth transitions
        }
    }

    pub fn start_grow_transition(&mut self, start_index: u32, end_index: u32, current_time: f32) {
        console_log!(
            "🌱 Starting grow transition: {} -> {}",
            start_index,
            end_index
        );

        let particle_count = (end_index - start_index) as usize;
        let mut target_sizes = Vec::with_capacity(particle_count);

        // Calculate target sizes based on particle types
        for i in 0..particle_count {
            let particle_index = start_index + i as u32;
            let particle_type = (particle_index % 5) as usize; // 5 types
            let size_range = match particle_type {
                0 => (1.4, 1.6), // Blue - large
                1 => (1.1, 1.3), // Orange - medium-large
                2 => (0.6, 0.8), // Red - small
                3 => (0.8, 1.0), // Purple - smaller
                4 => (0.9, 1.1), // Green - medium
                _ => (1.0, 1.0),
            };

            // Use a simple calculation for target size
            let size_multiplier = (size_range.0 + size_range.1) / 2.0;
            target_sizes.push(12.0 * size_multiplier);
        }

        let transition = ParticleTransition {
            start_index,
            end_index,
            start_time: current_time,
            duration: self.transition_duration,
            transition_type: TransitionType::Grow,
            target_sizes,
            needs_post_compute_finalization: true,
        };

        self.cancel_overlapping_transitions(start_index, end_index);
        self.active_transitions.push(transition);
    }

    pub fn start_shrink_transition(&mut self, start_index: u32, end_index: u32, current_time: f32) {
        console_log!(
            "🍂 Starting shrink transition: {} -> {}",
            start_index,
            end_index
        );

        let particle_count = (end_index - start_index) as usize;
        let mut target_sizes = Vec::with_capacity(particle_count);

        // For shrink transitions, we start from current sizes
        for i in 0..particle_count {
            target_sizes.push(12.0); // Use a default size, will be updated from actual particle data
        }

        let transition = ParticleTransition {
            start_index,
            end_index,
            start_time: current_time,
            duration: self.transition_duration,
            transition_type: TransitionType::Shrink,
            target_sizes,
            needs_post_compute_finalization: false,
        };

        self.cancel_overlapping_transitions(start_index, end_index);
        self.active_transitions.push(transition);
    }

    pub fn update(&mut self, current_time: f32, particle_system: &mut ParticleSystem) {
        if self.active_transitions.is_empty() {
            return;
        }

        let mut completed_indices = Vec::new();

        for (index, transition) in self.active_transitions.iter().enumerate() {
            let elapsed = current_time - transition.start_time;
            let progress = (elapsed / transition.duration).min(1.0);

            let particle_count = (transition.end_index - transition.start_index) as usize;

            // Update particle sizes based on transition type and progress
            for i in 0..particle_count {
                let particle_index = (transition.start_index + i as u32) as usize;
                let target_size = transition.target_sizes.get(i).copied().unwrap_or(12.0);

                let new_size = match transition.transition_type {
                    TransitionType::Grow => {
                        // Grow from 0.001 to target size
                        0.001 + (target_size - 0.001) * progress
                    }
                    TransitionType::Shrink => {
                        // Shrink from target size to 0
                        target_size * (1.0 - progress)
                    }
                };

                particle_system.set_particle_size(particle_index, new_size);
            }

            // Check if transition is complete
            if progress >= 1.0 {
                console_log!(
                    "✅ Completed {:?} transition for {} particles",
                    transition.transition_type,
                    particle_count
                );

                match transition.transition_type {
                    TransitionType::Shrink => {
                        // Update active particle count for shrink transitions
                        particle_system.set_active_count(transition.start_index);
                    }
                    TransitionType::Grow => {
                        // For grow transitions, the active count should already be set
                        // but ensure it's consistent
                        particle_system.set_active_count(transition.end_index);
                    }
                }

                completed_indices.push(index);
            }
        }

        // Remove completed transitions (in reverse order to maintain indices)
        for &index in completed_indices.iter().rev() {
            self.active_transitions.remove(index);
        }

        if !completed_indices.is_empty() {
            console_log!(
                "🧹 Cleaned up {} completed transitions, {} remain active",
                completed_indices.len(),
                self.active_transitions.len()
            );
        }
    }

    fn cancel_overlapping_transitions(&mut self, start_index: u32, end_index: u32) {
        let mut indices_to_remove = Vec::new();

        for (index, transition) in self.active_transitions.iter().enumerate() {
            // Check if ranges overlap
            let overlap_start = transition.start_index.max(start_index);
            let overlap_end = transition.end_index.min(end_index);

            if overlap_start < overlap_end {
                console_log!(
                    "🚫 Cancelling overlapping {:?} transition for particles {}-{}",
                    transition.transition_type,
                    transition.start_index,
                    transition.end_index
                );
                indices_to_remove.push(index);
            }
        }

        // Remove overlapping transitions (in reverse order)
        for &index in indices_to_remove.iter().rev() {
            self.active_transitions.remove(index);
        }

        if !indices_to_remove.is_empty() {
            console_log!(
                "🧹 Cancelled {} overlapping transitions",
                indices_to_remove.len()
            );
        }
    }

    pub fn get_active_count(&self) -> usize {
        self.active_transitions.len()
    }

    pub fn has_active_transitions(&self) -> bool {
        !self.active_transitions.is_empty()
    }
}

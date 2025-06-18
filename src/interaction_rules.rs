use rand::prelude::*;
use rand::rngs::SmallRng;

#[derive(Debug, Clone)]
pub struct InteractionRule {
    pub attraction: f32,
    pub min_radius: f32,
    pub max_radius: f32,
}

#[derive(Debug, Clone)]
pub struct InteractionRules {
    rules: Vec<Vec<InteractionRule>>,
    num_types: usize,
}

impl InteractionRules {
    pub fn new_random(rng: &mut SmallRng) -> Self {
        let num_types = 5;
        let mut rules = Vec::with_capacity(num_types);

        for i in 0..num_types {
            let mut type_rules = Vec::with_capacity(num_types);
            for j in 0..num_types {
                let rule = if i == j {
                    // Self-interaction: stronger repulsive
                    InteractionRule {
                        attraction: rng.gen_range(-2.0..-0.5), // -2.0 to -0.5 (stronger repulsion)
                        min_radius: rng.gen_range(5.0..15.0),  // 5 to 15
                        max_radius: rng.gen_range(20.0..50.0), // min_radius + (15 to 35)
                    }
                } else {
                    // Inter-type interaction
                    let min_radius = rng.gen_range(10.0..30.0);
                    InteractionRule {
                        attraction: rng.gen_range(-0.5..1.5), // -0.5 to 1.5 (stronger forces)
                        min_radius,
                        max_radius: min_radius + rng.gen_range(20.0..80.0), // min + (20 to 80)
                    }
                };
                type_rules.push(rule);
            }
            rules.push(type_rules);
        }

        Self { rules, num_types }
    }

    pub fn get_rule(&self, type_a: usize, type_b: usize) -> &InteractionRule {
        &self.rules[type_a][type_b]
    }

    pub fn get_num_types(&self) -> usize {
        self.num_types
    }

    // Convert to buffer format for GPU upload
    pub fn to_buffer(&self) -> Vec<u8> {
        let mut buffer = Vec::new();

        for i in 0..self.num_types {
            for j in 0..self.num_types {
                let rule = &self.rules[i][j];
                buffer.extend_from_slice(&rule.attraction.to_le_bytes());
                buffer.extend_from_slice(&rule.min_radius.to_le_bytes());
                buffer.extend_from_slice(&rule.max_radius.to_le_bytes());
                buffer.extend_from_slice(&0.0f32.to_le_bytes()); // padding
            }
        }

        buffer
    }

    pub fn get_buffer_size(&self) -> usize {
        self.num_types * self.num_types * 16 // 4 floats * 4 bytes per rule
    }
}

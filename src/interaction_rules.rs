// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

use rand::prelude::*;
use rand::rngs::SmallRng;
use std::ops::Range;

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

// Number of particle types (see particle_system::NUM_TYPES, which this must stay
// in sync with — types 0-7 are HTV, 8-10 are WLP-exclusive "flavor" types).
const NUM_TYPES: usize = 11;

// Default inter-type attraction range, used for the original 8-type system.
pub const DEFAULT_INTER_TYPE_ATTRACTION: Range<f32> = -0.5..1.5;

// Types 8-10 (WLP's exclusive "flavor" particles) are meant to add visual
// variety, not extra chaos on top of the original 8-type baseline. Any pair
// involving one of them gets this much narrower, near-neutral range instead
// of the default — a fraction of the force/repulsion swing, so they mix in
// gently rather than adding a whole new layer of dynamic movement.
pub const WLP_EXTRA_ATTRACTION: Range<f32> = -0.05..0.15;

// First type index that's "extra" beyond the original 8-type system.
const FIRST_EXTRA_TYPE: usize = 8;

impl InteractionRules {
    fn random_rule(is_self: bool, attraction_range: Range<f32>, rng: &mut SmallRng) -> InteractionRule {
        if is_self {
            // Self-interaction: stronger repulsive (attraction range unaffected by bias —
            // biasing is only meaningful for how a type relates to *other* types).
            InteractionRule {
                attraction: rng.gen_range(-2.0..-0.5), // -2.0 to -0.5 (stronger repulsion)
                min_radius: rng.gen_range(5.0..15.0),  // 5 to 15
                max_radius: rng.gen_range(20.0..50.0), // min_radius + (15 to 35)
            }
        } else {
            let min_radius = rng.gen_range(10.0..30.0);
            InteractionRule {
                attraction: rng.gen_range(attraction_range),
                min_radius,
                max_radius: min_radius + rng.gen_range(20.0..80.0), // min + (20 to 80)
            }
        }
    }

    pub fn new_random(rng: &mut SmallRng) -> Self {
        let num_types = NUM_TYPES;
        let mut rules = Vec::with_capacity(num_types);

        for i in 0..num_types {
            let mut type_rules = Vec::with_capacity(num_types);
            for j in 0..num_types {
                let touches_extra = i >= FIRST_EXTRA_TYPE || j >= FIRST_EXTRA_TYPE;
                let range = if touches_extra { WLP_EXTRA_ATTRACTION } else { DEFAULT_INTER_TYPE_ATTRACTION };
                type_rules.push(Self::random_rule(i == j, range, rng));
            }
            rules.push(type_rules);
        }

        Self { rules, num_types }
    }

    /// Like new_random(), but keeps every cell of `base` unchanged except those
    /// where `i` or `j` is in `type_indices`, which are re-rolled using
    /// `attraction_range` instead of the default. Used to give a subset of
    /// particle types (e.g. WLP's exclusive 8/9/10) a distinct "personality"
    /// bias — cooperative/clustering vs. hostile/repulsive — without disturbing
    /// the shared types' (0-7) already-evolving matrix (see RuleEvolution::reshuffle_biased).
    pub fn new_random_partial(
        base: &InteractionRules,
        type_indices: &[usize],
        attraction_range: Range<f32>,
        rng: &mut SmallRng,
    ) -> Self {
        let num_types = base.num_types;
        let mut rules = Vec::with_capacity(num_types);
        for i in 0..num_types {
            let mut type_rules = Vec::with_capacity(num_types);
            for j in 0..num_types {
                let touches_special = type_indices.contains(&i) || type_indices.contains(&j);
                let rule = if touches_special {
                    Self::random_rule(i == j, attraction_range.clone(), rng)
                } else {
                    base.rules[i][j].clone()
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

    /// Linearly interpolate toward another rule set.
    /// t = 0.0 returns a copy of `self`, t = 1.0 returns a copy of `other`.
    pub fn lerp_toward(&self, other: &InteractionRules, t: f32) -> InteractionRules {
        let t = t.clamp(0.0, 1.0);
        let num_types = self.num_types;
        let mut rules = Vec::with_capacity(num_types);
        for i in 0..num_types {
            let mut type_rules = Vec::with_capacity(num_types);
            for j in 0..num_types {
                let a = &self.rules[i][j];
                let b = &other.rules[i][j];
                type_rules.push(InteractionRule {
                    attraction: a.attraction + t * (b.attraction - a.attraction),
                    min_radius: a.min_radius + t * (b.min_radius - a.min_radius),
                    max_radius: a.max_radius + t * (b.max_radius - a.max_radius),
                });
            }
            rules.push(type_rules);
        }
        InteractionRules { rules, num_types }
    }
}

/// Continuous lerp-based evolution between rule sets.
/// Platform-agnostic — shared by both the WASM and native builds.
pub struct RuleEvolution {
    source: InteractionRules,
    target: InteractionRules,
    current: InteractionRules,
    t: f32,
    duration: f32, // seconds for a full source→target cycle
}

impl RuleEvolution {
    /// Create a new evolution starting at `initial` and lerping toward a fresh random set.
    pub fn new(initial: InteractionRules, rng: &mut SmallRng) -> Self {
        let target = InteractionRules::new_random(rng);
        let current = initial.clone();
        Self {
            source: initial,
            target,
            current,
            t: 0.0,
            duration: 900.0, // default: 15 minutes
        }
    }

    /// Advance the lerp by `delta_time` seconds.
    /// Returns a reference to the current (interpolated) rules.
    pub fn tick(&mut self, delta_time: f32, rng: &mut SmallRng) -> &InteractionRules {
        if self.duration > 0.0 {
            self.t += delta_time / self.duration;
            if self.t >= 1.0 {
                self.source = self.target.clone();
                self.target = InteractionRules::new_random(rng);
                self.t = 0.0;
            }
            self.current = self.source.lerp_toward(&self.target, self.t);
        }
        &self.current
    }

    /// Immediately adopt the current target, then start lerping toward a new random set.
    /// Called when a super-lightning event is detected.
    pub fn snap_to_new(&mut self, rng: &mut SmallRng) {
        self.source = self.target.clone();
        self.current = self.source.clone();
        self.target = InteractionRules::new_random(rng);
        self.t = 0.0;
    }

    /// Same snap pattern as snap_to_new(), but only reshuffles the rules touching
    /// `type_indices` (using the given, possibly biased, attraction range) — every
    /// other type's evolving matrix is left completely untouched. Called on a
    /// HTV -> WLP hypothesis transition to give WLP's exclusive types (8/9/10) a
    /// fresh "personality" bias each time WLP is (re-)entered.
    pub fn reshuffle_biased(&mut self, type_indices: &[usize], attraction_range: Range<f32>, rng: &mut SmallRng) {
        self.source = self.target.clone();
        self.current = self.source.clone();
        self.target = InteractionRules::new_random_partial(&self.source, type_indices, attraction_range, rng);
        self.t = 0.0;
    }

    /// Update the cycle duration (in seconds).
    pub fn set_duration(&mut self, seconds: f32) {
        self.duration = seconds.max(1.0);
    }

    /// Current lerp progress in [0, 1].
    pub fn progress(&self) -> f32 {
        self.t
    }

    /// Current interpolated rules.
    pub fn current(&self) -> &InteractionRules {
        &self.current
    }
}

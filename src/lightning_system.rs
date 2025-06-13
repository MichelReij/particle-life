use crate::{console_log, SimulationParams};
use rand::prelude::*;
use rand::rngs::SmallRng;

#[derive(Debug, Clone)]
pub struct LightningSegment {
    pub start_pos: [f32; 2],
    pub end_pos: [f32; 2],
    pub thickness: f32,
    pub alpha: f32,
    pub generation: u32,
    pub appear_time: f32,
    pub is_visible: u32,
    pub padding: f32,
}

#[derive(Debug, Clone)]
pub struct LightningBolt {
    pub num_segments: u32,
    pub flash_id: u32,
    pub start_time: f32,
    pub padding: f32,
}

#[derive(Debug)]
pub struct LightningSystem {
    segments: Vec<LightningSegment>,
    bolts: Vec<LightningBolt>,
    max_segments: usize,
    max_bolts: usize,
    last_flash_time: f32,
    next_flash_time: f32,
    current_flash_id: u32,
    rng: SmallRng,
}

impl LightningSystem {
    pub fn new() -> Self {
        let max_segments = 30;
        let max_bolts = 4;

        let mut segments = Vec::with_capacity(max_segments);
        let mut bolts = Vec::with_capacity(max_bolts);

        // Initialize with empty segments and bolts
        for _ in 0..max_segments {
            segments.push(LightningSegment {
                start_pos: [0.0, 0.0],
                end_pos: [0.0, 0.0],
                thickness: 0.0,
                alpha: 0.0,
                generation: 0,
                appear_time: 0.0,
                is_visible: 0,
                padding: 0.0,
            });
        }

        for _ in 0..max_bolts {
            bolts.push(LightningBolt {
                num_segments: 0,
                flash_id: 0,
                start_time: 0.0,
                padding: 0.0,
            });
        }

        Self {
            segments,
            bolts,
            max_segments,
            max_bolts,
            last_flash_time: 0.0,
            next_flash_time: 2.0, // First flash after 2 seconds
            current_flash_id: 1,
            rng: unsafe { SmallRng::from_entropy() },
        }
    }

    pub fn update(&mut self, current_time: f32, params: &SimulationParams) {
        // Check if it's time for a new lightning flash
        if current_time >= self.next_flash_time {
            self.generate_lightning_flash(current_time, params);
            self.schedule_next_flash(current_time, params);
        }

        // Update segment visibility
        self.update_segment_visibility(current_time, params);
    }

    fn generate_lightning_flash(&mut self, current_time: f32, params: &SimulationParams) {
        console_log!("⚡ Generating lightning flash at time {:.2}s", current_time);

        // Create a new bolt
        let bolt_index = (self.current_flash_id % self.max_bolts as u32) as usize;

        // Generate lightning start position within a circle (radius 600px from center)
        let angle = self.rng.gen_range(0.0..std::f32::consts::TAU);
        let radius = self.rng.gen_range(0.0..600.0);
        let center_x = 1200.0; // Center of 2400px world
        let center_y = 1200.0;

        let start_x = center_x + radius * angle.cos();
        let start_y = center_y + radius * angle.sin();

        // Generate branching lightning segments
        let num_segments = self.rng.gen_range(8..15);
        let mut current_pos = [start_x, start_y];
        let mut segment_count = 0;

        for i in 0..num_segments.min(self.max_segments as u32) {
            if segment_count >= self.max_segments {
                break;
            }

            // Generate direction with some randomness
            let angle = self.rng.gen_range(0.0..std::f32::consts::TAU);
            let length = self.rng.gen_range(40.0..90.0); // 40-90px segments

            let end_pos = [
                current_pos[0] + length * angle.cos(),
                current_pos[1] + length * angle.sin(),
            ];

            // Create segment
            let appear_time = current_time + i as f32 * 0.02; // Stagger appearance
            let thickness = 3.0 * 0.7f32.powf(i as f32); // Decrease with generation
            let thickness = thickness.max(1.0).min(3.0);

            self.segments[segment_count] = LightningSegment {
                start_pos: current_pos,
                end_pos,
                thickness,
                alpha: 1.0 * 0.9f32.powf(i as f32),
                generation: i,
                appear_time,
                is_visible: 1,
                padding: 0.0,
            };

            current_pos = end_pos;
            segment_count += 1;
        }

        // Update bolt info
        self.bolts[bolt_index] = LightningBolt {
            num_segments: segment_count as u32,
            flash_id: self.current_flash_id,
            start_time: current_time,
            padding: 0.0,
        };

        self.current_flash_id += 1;
        self.last_flash_time = current_time;
    }

    fn schedule_next_flash(&mut self, current_time: f32, params: &SimulationParams) {
        // Calculate interval based on electrical activity (lightning frequency)
        // Higher frequency = shorter intervals
        let base_interval = 20.0; // 20 seconds at low activity
        let min_interval = 8.0; // 8 seconds at max activity

        let activity_factor = params.lightning_intensity.clamp(0.0, 1.0);
        let interval = base_interval - (base_interval - min_interval) * activity_factor;

        // Add ±25% randomization
        let randomization = self.rng.gen_range(-0.25..0.25);
        let randomized_interval = interval * (1.0 + randomization);

        self.next_flash_time = current_time + randomized_interval;

        console_log!(
            "⏰ Next lightning flash scheduled in {:.2}s (activity: {:.1}%)",
            randomized_interval,
            activity_factor * 100.0
        );
    }

    fn update_segment_visibility(&mut self, current_time: f32, params: &SimulationParams) {
        let segment_duration = params.lightning_duration;

        for segment in &mut self.segments {
            if segment.is_visible == 1 {
                let age = current_time - segment.appear_time;
                if age >= segment_duration {
                    segment.is_visible = 0;
                }
            }
        }
    }

    pub fn get_segments_buffer(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(self.max_segments * 32); // 32 bytes per segment

        for segment in &self.segments {
            buffer.extend_from_slice(&segment.start_pos[0].to_le_bytes());
            buffer.extend_from_slice(&segment.start_pos[1].to_le_bytes());
            buffer.extend_from_slice(&segment.end_pos[0].to_le_bytes());
            buffer.extend_from_slice(&segment.end_pos[1].to_le_bytes());
            buffer.extend_from_slice(&segment.thickness.to_le_bytes());
            buffer.extend_from_slice(&segment.alpha.to_le_bytes());
            buffer.extend_from_slice(&segment.generation.to_le_bytes());
            buffer.extend_from_slice(&segment.appear_time.to_le_bytes());
            buffer.extend_from_slice(&segment.is_visible.to_le_bytes());
            buffer.extend_from_slice(&segment.padding.to_le_bytes());
        }

        buffer
    }

    pub fn get_bolts_buffer(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(self.max_bolts * 16); // 16 bytes per bolt

        for bolt in &self.bolts {
            buffer.extend_from_slice(&bolt.num_segments.to_le_bytes());
            buffer.extend_from_slice(&bolt.flash_id.to_le_bytes());
            buffer.extend_from_slice(&bolt.start_time.to_le_bytes());
            buffer.extend_from_slice(&bolt.padding.to_le_bytes());
        }

        buffer
    }
}

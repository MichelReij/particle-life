// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// src/stats_reader.rs
// GPU-side statistieken per particle-type lezen voor sonificatie.
// Beheert de stats compute pipeline + staging buffer (CPU-readback).
//
// Gebruik:
//   1. StatsReader::new(device, sim_params_buffer)
//   2. stats_reader.make_bind_group(device, output_particle_buffer, sim_params_buffer)
//   3. In render loop: if stats_reader.maybe_dispatch(&mut encoder, &bind_group, count) { submit; start_readback() }
//   4. Elke frame: if let Some(stats) = stats_reader.poll_result() { sla op }

use crate::sonification::{GpuGlobalStats, GpuTypeStats};
use std::sync::{Arc, Mutex};

/// 8 vec4 (7 per-type + 1 globaal) × 16 bytes = 128 bytes
const STATS_BUFFER_SIZE: u64 = 8 * 4 * 4;

type StatsResult = ([GpuTypeStats; 7], GpuGlobalStats);

pub struct StatsReader {
    pub pipeline:          wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub stats_buffer:      wgpu::Buffer,
    pub staging_buffer:    wgpu::Buffer,
    frame_counter:         u32,
    pub poll_interval:     u32,  // elke N frames dispatchen (standaard 6 ≈ 10 Hz)
    pub last_stats:        Option<StatsResult>,
    /// Gezet door start_readback(), gewist door poll_result() zodra data uitgelezen.
    readback_ready:        Arc<Mutex<bool>>,
    readback_pending:      bool,
}

impl StatsReader {
    pub fn new(
        device:            &wgpu::Device,
        sim_params_buffer: &wgpu::Buffer,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("Stats Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/stats_compute.wgsl").into()
            ),
        });

        let bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("Stats BGL"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            }
        );

        let pipeline_layout = device.create_pipeline_layout(
            &wgpu::PipelineLayoutDescriptor {
                label:                Some("Stats Pipeline Layout"),
                bind_group_layouts:   &[&bind_group_layout],
                push_constant_ranges: &[],
            }
        );

        let pipeline = device.create_compute_pipeline(
            &wgpu::ComputePipelineDescriptor {
                label:               Some("Stats Compute Pipeline"),
                layout:              Some(&pipeline_layout),
                module:              &shader,
                entry_point:         Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache:               None,
            }
        );

        let stats_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("Stats Buffer"),
            size:               STATS_BUFFER_SIZE,
            usage:              wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("Stats Staging Buffer"),
            size:               STATS_BUFFER_SIZE,
            usage:              wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            stats_buffer,
            staging_buffer,
            frame_counter: 0,
            poll_interval: 6,
            last_stats: None,
            readback_ready: Arc::new(Mutex::new(false)),
            readback_pending: false,
        }
    }

    /// Maak een bind group aan voor het huidige output particle buffer.
    /// Opnieuw aanmaken bij elke ping-pong wissel.
    pub fn make_bind_group(
        &self,
        device:            &wgpu::Device,
        particle_buffer:   &wgpu::Buffer,
        sim_params_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label:  Some("Stats Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: particle_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: sim_params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: self.stats_buffer.as_entire_binding() },
            ],
        })
    }

    /// Voeg stats compute pass toe aan de encoder als het interval bereikt is.
    /// Geeft `true` terug als gedispatched (=> aanroeper moet daarna read_stats aanroepen).
    pub fn maybe_dispatch(
        &mut self,
        encoder:       &mut wgpu::CommandEncoder,
        bind_group:    &wgpu::BindGroup,
        _num_particles: u32,
    ) -> bool {
        self.frame_counter += 1;
        if self.frame_counter < self.poll_interval {
            return false;
        }
        self.frame_counter = 0;

        // Stats compute pass
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label:            Some("Stats Compute Pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(8, 1, 1); // 7 per type + 1 globaal
        }

        // Kopieer stats_buffer → staging_buffer voor CPU-readback
        encoder.copy_buffer_to_buffer(
            &self.stats_buffer, 0,
            &self.staging_buffer, 0,
            STATS_BUFFER_SIZE,
        );

        true
    }

    /// Start een niet-blokkerende GPU-readback. Aanroepen ná submit van de encoder
    /// die maybe_dispatch true teruggaf. Resultaat ophalen via poll_result().
    pub fn start_readback(&mut self) {
        // Reset vlag voor deze readback-cyclus
        if let Ok(mut ready) = self.readback_ready.lock() {
            *ready = false;
        }
        let flag = Arc::clone(&self.readback_ready);
        self.staging_buffer.slice(..).map_async(wgpu::MapMode::Read, move |result| {
            if result.is_ok() {
                if let Ok(mut ready) = flag.lock() {
                    *ready = true;
                }
            }
        });
        self.readback_pending = true;
    }

    /// Controleer of een lopende readback klaar is. Geeft Some(stats) terug als ja,
    /// leest de buffer uit, en wist de pending-staat. Elke frame aanroepen.
    pub fn poll_result(&mut self) -> Option<StatsResult> {
        if !self.readback_pending { return None; }

        let is_ready = self.readback_ready.lock().ok().map(|g| *g).unwrap_or(false);
        if !is_ready { return None; }

        self.readback_pending = false;

        let raw    = self.staging_buffer.slice(..).get_mapped_range();
        let floats: &[f32] = bytemuck::cast_slice(&raw[..]);

        let mut per_type = [GpuTypeStats::default(); 7];
        for i in 0..7 {
            let b = i * 4;
            per_type[i] = GpuTypeStats::from_floats(
                floats[b..b+4].try_into().unwrap()
            );
        }
        let global = GpuGlobalStats::from_floats(floats[28..32].try_into().unwrap());

        drop(raw);
        self.staging_buffer.unmap();

        Some((per_type, global))
    }
}

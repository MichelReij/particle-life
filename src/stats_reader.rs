// src/stats_reader.rs
// GPU-side statistieken per particle-type lezen voor sonificatie.
// Beheert de stats compute pipeline + staging buffer (CPU-readback).
//
// Gebruik:
//   1. StatsReader::new(device, sim_params_buffer)
//   2. stats_reader.make_bind_group(device, output_particle_buffer, sim_params_buffer)
//   3. In render loop: stats_reader.maybe_dispatch(&mut encoder, &bind_group, particle_count)
//   4. Na submit: if dispatched { pollster::block_on(stats_reader.read_stats(device)) }

use crate::sonification::{GpuGlobalStats, GpuTypeStats};

/// 8 vec4 (7 per-type + 1 globaal) × 16 bytes = 128 bytes
const STATS_BUFFER_SIZE: u64 = 8 * 4 * 4;

pub struct StatsReader {
    pub pipeline:          wgpu::ComputePipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub stats_buffer:      wgpu::Buffer,
    pub staging_buffer:    wgpu::Buffer,
    frame_counter:         u32,
    pub poll_interval:     u32,  // elke N frames dispatchen (standaard 6 ≈ 10 Hz)
    pub last_stats:        Option<([GpuTypeStats; 7], GpuGlobalStats)>,
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

    /// Lees de stats terug van GPU naar CPU.
    /// Moet worden aangeroepen ná het submit van de encoder die maybe_dispatch true teruggaf.
    /// Blokkeert tot de GPU klaar is (gebruik pollster::block_on of in een aparte thread).
    pub async fn read_stats(
        &self,
        device: &wgpu::Device,
    ) -> Result<([GpuTypeStats; 7], GpuGlobalStats), Box<dyn std::error::Error + Send + Sync>> {
        let slice = self.staging_buffer.slice(..);

        let (tx, rx) = futures::channel::oneshot::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| { let _ = tx.send(r); });

        device.poll(wgpu::MaintainBase::Wait)
            .map_err(|e| format!("Device poll: {:?}", e))?;

        rx.await
            .map_err(|_| "Channel gesloten".to_string())?
            .map_err(|e| format!("Buffer map: {:?}", e))?;

        let raw    = slice.get_mapped_range();
        let floats: &[f32] = bytemuck::cast_slice(&raw[..]);

        let mut per_type = [GpuTypeStats::default(); 7];
        for i in 0..7 {
            let b = i * 4;
            per_type[i] = GpuTypeStats::from_floats(
                floats[b..b+4].try_into().unwrap()
            );
        }
        let g = &floats[28..32];
        let global = GpuGlobalStats::from_floats(g.try_into().unwrap());

        drop(raw);
        self.staging_buffer.unmap();

        Ok((per_type, global))
    }
}

// Minimal native binary that uses shared core components
// This demonstrates the correct approach: shared codebase with minimal platform differences

use particle_life_wasm::*;
use rand::prelude::*;
use rand::rngs::SmallRng;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

// System monitoring utilities
#[cfg(target_os = "linux")]
fn get_cpu_usage() -> Option<f32> {
    use std::fs;

    // Read /proc/stat for CPU usage calculation
    static mut LAST_IDLE: u64 = 0;
    static mut LAST_TOTAL: u64 = 0;

    if let Ok(stat) = fs::read_to_string("/proc/stat") {
        if let Some(cpu_line) = stat.lines().next() {
            let values: Vec<u64> = cpu_line
                .split_whitespace()
                .skip(1)
                .take(8)
                .filter_map(|s| s.parse().ok())
                .collect();

            if values.len() >= 4 {
                let idle = values[3];
                let total: u64 = values.iter().sum();

                unsafe {
                    let idle_delta = idle.saturating_sub(LAST_IDLE);
                    let total_delta = total.saturating_sub(LAST_TOTAL);

                    let cpu_usage = if total_delta > 0 {
                        100.0 * (1.0 - idle_delta as f32 / total_delta as f32)
                    } else {
                        0.0
                    };

                    LAST_IDLE = idle;
                    LAST_TOTAL = total;

                    Some(cpu_usage.max(0.0).min(100.0))
                }
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn get_cpu_usage() -> Option<f32> {
    // macOS implementation using sysctl/host_statistics
    // For now, return None - can be implemented with system calls
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn get_cpu_usage() -> Option<f32> {
    None
}

#[cfg(target_os = "linux")]
fn get_memory_usage() -> Option<(f32, f32)> {
    use std::fs;

    if let Ok(meminfo) = fs::read_to_string("/proc/meminfo") {
        let mut total_kb = 0u64;
        let mut available_kb = 0u64;

        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                total_kb = line.split_whitespace().nth(1)?.parse().ok()?;
            } else if line.starts_with("MemAvailable:") {
                available_kb = line.split_whitespace().nth(1)?.parse().ok()?;
            }
        }

        if total_kb > 0 && available_kb > 0 {
            let used_kb = total_kb.saturating_sub(available_kb);
            let used_percent = (used_kb as f32 / total_kb as f32) * 100.0;
            let total_mb = total_kb as f32 / 1024.0;
            Some((used_percent, total_mb))
        } else {
            None
        }
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn get_memory_usage() -> Option<(f32, f32)> {
    // macOS implementation - can use vm_stat or system calls
    None
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn get_memory_usage() -> Option<(f32, f32)> {
    None
}

struct MinimalNativeApp {
    window: Option<Arc<Window>>,
    // Same shared components as WASM
    particle_system: ParticleSystem,
    simulation_params: SimulationParams,
    interaction_rules: InteractionRules,
    renderer: Option<WebGpuRenderer>,
    // Native-specific
    last_frame: std::time::Instant,
    current_time: f32,
    // FPS/statistics
    stats_last_log: std::time::Instant,
    stats_frame_count: u32,
    stats_render_time_accum: f32,
    // System load monitoring
    frame_times: Vec<f32>, // Rolling window of frame times for jitter analysis
    max_frame_time: f32,   // Peak frame time in current period
    min_frame_time: f32,   // Minimum frame time in current period
    // OS-level monitoring
    cpu_usage_samples: Vec<f32>,    // Rolling window for CPU usage
    memory_usage_samples: Vec<f32>, // Rolling window for memory usage
}

impl Default for MinimalNativeApp {
    fn default() -> Self {
        // Same initialization logic as ParticleLifeEngine::new()
        let mut rng = SmallRng::from_entropy();
        let mut simulation_params = SimulationParams::new();

        // Apply custom native defaults using central conversion functions
        simulation_params.apply_zoom(1.45, None, None); // zoom = 1.45
        simulation_params.apply_temperature(20.0); // temperature = 20.0°C
        simulation_params.apply_pressure(200.0); // pressure = 200.0
        simulation_params.apply_uv_light(40.0); // uv_light = 40.0
        simulation_params.apply_electrical_activity(2.0); // electrical_activity = 2.0

        console_log!("🎯 Applied native defaults via central conversion functions:");
        console_log!(
            "  🔍 Zoom: 1.45x (viewport: {:.0}×{:.0})",
            simulation_params.viewport_width,
            simulation_params.viewport_height
        );
        console_log!(
            "  🌡️ Temperature: 20.0°C → friction: {:.3}, drift: {:.1}, bg_color: ({:.3}, {:.3}, {:.3})",
            simulation_params.friction,
            simulation_params.drift_x_per_second,
            simulation_params.background_color_r,
            simulation_params.background_color_g,
            simulation_params.background_color_b
        );
        console_log!(
            "  🔧 Pressure: 200.0 → force_scale: {:.1}, r_smooth: {:.3}",
            simulation_params.force_scale,
            simulation_params.r_smooth
        );
        console_log!(
            "  ☀️ UV Light: 40.0 → inter_type_radius_scale: {:.3}",
            simulation_params.inter_type_radius_scale
        );
        console_log!(
            "  ⚡ Electrical: 2.0 → attraction_scale: {:.3}, lightning_freq: {:.3}",
            simulation_params.inter_type_attraction_scale,
            simulation_params.lightning_frequency
        );

        let interaction_rules = InteractionRules::new_random(&mut rng);
        let particle_system = ParticleSystem::new(&simulation_params, &interaction_rules, &mut rng);

        Self {
            window: None,
            particle_system,
            simulation_params,
            interaction_rules,
            renderer: None,
            last_frame: std::time::Instant::now(),
            current_time: 0.0,
            stats_last_log: std::time::Instant::now(),
            stats_frame_count: 0,
            stats_render_time_accum: 0.0,
            // System monitoring initialization
            frame_times: Vec::with_capacity(300), // 5 seconds at 60 FPS
            max_frame_time: 0.0,
            min_frame_time: f32::MAX,
            // OS monitoring initialization
            cpu_usage_samples: Vec::with_capacity(60), // 1 minute of samples
            memory_usage_samples: Vec::with_capacity(60),
        }
    }
}

impl ApplicationHandler for MinimalNativeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        console_log!("🚀 Native app using shared core components");

        // Only platform-specific part: window creation
        let window_attributes = Window::default_attributes()
            .with_title("Particle Life - Shared Components")
            .with_inner_size(winit::dpi::LogicalSize::new(800, 800))
            .with_resizable(false);

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        self.window = Some(window.clone());

        // Initialize shared renderer (only surface creation is platform-specific)
        pollster::block_on(async {
            match WebGpuRenderer::new(window.clone()).await {
                Ok(renderer) => {
                    console_log!("✅ Shared WebGPU renderer initialized for native");
                    self.renderer = Some(renderer);
                }
                Err(e) => {
                    console_log!("❌ Failed to initialize renderer: {:?}", e);
                }
            }
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                console_log!("👋 Closing native app");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                let delta_time = (now - self.last_frame).as_secs_f32();
                self.last_frame = now;

                // Update every frame (remove 60 FPS limiting to see if that was the issue)
                // Same update logic as ParticleLifeEngine::update_frame()
                self.current_time += delta_time;
                self.simulation_params.set_time(self.current_time);
                self.simulation_params.set_delta_time(delta_time);

                // --- FPS & render time statistics ---
                self.stats_frame_count += 1;
                let render_start = std::time::Instant::now();

                if let Some(renderer) = &mut self.renderer {
                    // Same render call as WASM - render is sync, not async
                    let lightning_segments_data = Vec::new(); // Empty - lightning generated by GPU
                    let lightning_bolts_data = Vec::new(); // Empty - lightning generated by GPU

                    match renderer.render(
                        &self.particle_system,
                        &self.simulation_params,
                        &self.interaction_rules,
                        &lightning_segments_data,
                        &lightning_bolts_data,
                    ) {
                        Ok(_) => {
                            // Render successful - measuring command submission time, not GPU completion
                            // Uncomment next line for true GPU timing (but this blocks and reduces FPS):
                            // let _ = renderer.get_device().poll(wgpu::MaintainBase::Wait);
                        }
                        Err(e) => {
                            console_log!("❌ Render error: {:?}", e);
                        }
                    }
                }

                let render_time = render_start.elapsed().as_secs_f32();
                self.stats_render_time_accum += render_time;

                // Track frame timing for system load analysis
                let total_frame_time = delta_time;

                // Update min/max frame times
                self.max_frame_time = self.max_frame_time.max(total_frame_time);
                self.min_frame_time = self.min_frame_time.min(total_frame_time);

                // Maintain rolling window of frame times (last 5 seconds)
                self.frame_times.push(total_frame_time);
                if self.frame_times.len() > 300 {
                    self.frame_times.remove(0);
                }

                // Log every 5 seconds
                let stats_now = std::time::Instant::now();
                let elapsed = (stats_now - self.stats_last_log).as_secs_f32();
                if elapsed >= 5.0 {
                    // Collect OS-level statistics
                    if let Some(cpu_usage) = get_cpu_usage() {
                        self.cpu_usage_samples.push(cpu_usage);
                        if self.cpu_usage_samples.len() > 60 {
                            self.cpu_usage_samples.remove(0);
                        }
                    }

                    if let Some((mem_usage_percent, _total_mem_mb)) = get_memory_usage() {
                        self.memory_usage_samples.push(mem_usage_percent);
                        if self.memory_usage_samples.len() > 60 {
                            self.memory_usage_samples.remove(0);
                        }
                    }

                    let avg_fps = self.stats_frame_count as f32 / elapsed;
                    let avg_render_ms =
                        (self.stats_render_time_accum / self.stats_frame_count as f32) * 1000.0;
                    let active_particles = self.particle_system.get_active_count();

                    // Calculate frame timing statistics
                    let max_frame_ms = self.max_frame_time * 1000.0;
                    let min_frame_ms = self.min_frame_time * 1000.0;
                    let min_theoretical_fps = if self.max_frame_time > 0.0 {
                        1.0 / self.max_frame_time
                    } else {
                        0.0
                    };

                    // Calculate frame time variance (jitter indicator)
                    let avg_frame_time = elapsed / self.stats_frame_count as f32;
                    let frame_variance: f32 = self
                        .frame_times
                        .iter()
                        .map(|&t| (t - avg_frame_time).powi(2))
                        .sum::<f32>()
                        / self.frame_times.len() as f32;
                    let frame_jitter_ms = frame_variance.sqrt() * 1000.0;

                    // Memory usage estimation (rough)
                    let particles_memory_kb = (active_particles * 48) / 1024; // 48 bytes per particle

                    // Basic performance stats
                    console_log!(
                        "\n📊 5s stats: avg_fps={:.1} | render_time={:.2}ms | particles={} ({:.1}KB)",
                        avg_fps,
                        avg_render_ms,
                        active_particles,
                        particles_memory_kb
                    );
                    console_log!(
                        "⏱️  frame_timing: min={:.2}ms max={:.2}ms jitter={:.2}ms | worst_fps={:.1}",
                        min_frame_ms, max_frame_ms, frame_jitter_ms, min_theoretical_fps
                    );

                    // OS-level system monitoring
                    if !self.cpu_usage_samples.is_empty() {
                        let current_cpu = self.cpu_usage_samples.last().copied().unwrap_or(0.0);
                        let avg_cpu = self.cpu_usage_samples.iter().sum::<f32>()
                            / self.cpu_usage_samples.len() as f32;
                        let max_cpu = self
                            .cpu_usage_samples
                            .iter()
                            .copied()
                            .fold(0.0f32, f32::max);

                        console_log!(
                            "💻 CPU: current={:.1}% avg={:.1}% peak={:.1}% (last {}s)",
                            current_cpu,
                            avg_cpu,
                            max_cpu,
                            self.cpu_usage_samples.len() * 5
                        );
                    }

                    if !self.memory_usage_samples.is_empty() {
                        let current_mem = self.memory_usage_samples.last().copied().unwrap_or(0.0);
                        let avg_mem = self.memory_usage_samples.iter().sum::<f32>()
                            / self.memory_usage_samples.len() as f32;
                        let max_mem = self
                            .memory_usage_samples
                            .iter()
                            .copied()
                            .fold(0.0f32, f32::max);

                        if let Some((_, total_mem_mb)) = get_memory_usage() {
                            console_log!(
                                "🧠 RAM: {:.1}% ({:.0}MB) | avg={:.1}% peak={:.1}% | total={:.0}MB",
                                current_mem,
                                current_mem * total_mem_mb / 100.0,
                                avg_mem,
                                max_mem,
                                total_mem_mb
                            );
                        } else {
                            console_log!(
                                "🧠 RAM: current={:.1}% avg={:.1}% peak={:.1}%",
                                current_mem,
                                avg_mem,
                                max_mem
                            );
                        }
                    }

                    // System load indicators with enhanced warnings
                    if avg_fps < 55.0 {
                        console_log!("⚠️  Performance warning: FPS below 55");
                    }
                    if max_frame_ms > 20.0 {
                        console_log!("⚠️  Frame spike warning: {}ms frame detected", max_frame_ms);
                    }
                    if frame_jitter_ms > 2.0 {
                        console_log!(
                            "⚠️  Frame jitter warning: {:.1}ms variance",
                            frame_jitter_ms
                        );
                    }

                    // OS-level warnings
                    if let Some(cpu) = self.cpu_usage_samples.last() {
                        if *cpu > 80.0 {
                            console_log!("⚠️  High CPU usage: {:.1}%", cpu);
                        }
                    }
                    if let Some(mem) = self.memory_usage_samples.last() {
                        if *mem > 85.0 {
                            console_log!("⚠️  High memory usage: {:.1}%", mem);
                        }
                    }

                    // Reset for next period
                    self.stats_last_log = stats_now;
                    self.stats_frame_count = 0;
                    self.stats_render_time_accum = 0.0;
                    self.max_frame_time = 0.0;
                    self.min_frame_time = f32::MAX;
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    console_log!("🎯 Starting native app with shared core components");

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = MinimalNativeApp::default();
    event_loop.run_app(&mut app)?;

    Ok(())
}

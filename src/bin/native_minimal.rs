// Minimal native binary that uses shared core components
// This demonstrates the correct approach: shared codebase with minimal platform differences

use particle_life_wasm::config::*;
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
    // FPS tracking for display with smoothing
    fps_last_update: std::time::Instant,
    fps_frame_count: u32,
    current_fps: f32,
    fps_samples: Vec<f32>,
    fps_sample_index: usize,
}

impl Default for MinimalNativeApp {
    fn default() -> Self {
        // Same initialization logic as ParticleLifeEngine::new()
        let mut rng = SmallRng::from_entropy();
        let mut simulation_params = SimulationParams::new();

        // Apply custom native defaults using central conversion functions
        simulation_params.apply_zoom(1.0, None, None); // zoom = 1.0
        simulation_params.apply_temperature(20.0); // temperature = 20.0°C
        simulation_params.apply_pressure(200.0); // pressure = 200.0
        simulation_params.apply_uv_light(40.0); // uv_light = 40.0
        simulation_params.apply_electrical_activity(2.0); // electrical_activity = 2.0

        console_log!("🎯 Applied native defaults via central conversion functions:");
        console_log!(
            "  🔍 Zoom: 1.0x (viewport: {:.0}×{:.0})",
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
            fps_last_update: std::time::Instant::now(),
            fps_frame_count: 0,
            current_fps: 0.0,
            fps_samples: vec![60.0; FPS_SAMPLE_COUNT], // Initialize with samples at 60 FPS
            fps_sample_index: 0,
        }
    }
}

impl ApplicationHandler for MinimalNativeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        console_log!("🚀 Native app using shared core components");

        // Only platform-specific part: window creation
        let window_attributes = Window::default_attributes()
            .with_title("Particle Life - Shared Components")
            .with_inner_size(winit::dpi::LogicalSize::new(
                CANVAS_WIDTH_U32,
                CANVAS_HEIGHT_U32,
            ))
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

                // Update simulation
                self.current_time += delta_time;
                self.simulation_params.set_time(self.current_time);
                self.simulation_params.set_delta_time(delta_time);

                // Render
                if let Some(renderer) = &mut self.renderer {
                    let lightning_segments_data = Vec::new();
                    let lightning_bolts_data = Vec::new();

                    match renderer.render(
                        &self.particle_system,
                        &self.simulation_params,
                        &self.interaction_rules,
                        &lightning_segments_data,
                        &lightning_bolts_data,
                    ) {
                        Ok(_) => {
                            // Render successful
                        }
                        Err(e) => {
                            console_log!("❌ Render error: {:?}", e);
                        }
                    }
                }

                // Update FPS display - more frequent on-screen update, less frequent console
                self.fps_frame_count += 1;
                let fps_now = std::time::Instant::now();
                let fps_elapsed = (fps_now - self.fps_last_update).as_secs_f32();

                // Update FPS data every FPS_UPDATE_INTERVAL seconds for responsive on-screen display
                if fps_elapsed >= FPS_UPDATE_INTERVAL {
                    let instantaneous_fps = self.fps_frame_count as f32 / fps_elapsed;

                    // Reset samples if there's a big performance stutter (more than 50% difference)
                    let current_avg =
                        self.fps_samples.iter().sum::<f32>() / self.fps_samples.len() as f32;
                    if (instantaneous_fps - current_avg).abs() > current_avg * 0.5 {
                        self.reset_fps_samples(instantaneous_fps);
                    }

                    // Add to circular buffer for moving average
                    self.fps_samples[self.fps_sample_index] = instantaneous_fps;
                    self.fps_sample_index = (self.fps_sample_index + 1) % self.fps_samples.len();

                    // Calculate moving average
                    self.current_fps =
                        self.fps_samples.iter().sum::<f32>() / self.fps_samples.len() as f32;

                    // Update FPS data in renderer for on-screen display
                    if let Some(renderer) = &mut self.renderer {
                        let active_particles = self.particle_system.get_active_count();
                        renderer.update_fps_data(
                            self.current_fps,
                            0, // frame_count reset
                            active_particles,
                            self.current_time,
                        );
                    }

                    self.fps_last_update = fps_now;
                    self.fps_frame_count = 0;
                }

                // Console output every FPS_CONSOLE_INTERVAL seconds now that on-screen overlay is active
                if fps_elapsed >= FPS_CONSOLE_INTERVAL {
                    // Clear console and show FPS prominently
                    print!("\x1B[2J\x1B[1;1H"); // Clear screen and move to top
                    let active_particles = self.particle_system.get_active_count();

                    println!("╔══════════════════════════════════════╗");
                    println!("║          PARTICLE LIFE NATIVE       ║");
                    println!("╠══════════════════════════════════════╣");
                    println!("║  FPS: {:<27.1} ║", self.current_fps);
                    println!("║  Particles: {:<22} ║", active_particles);
                    println!("║  Time: {:<25.1} ║", self.current_time);
                    println!("╚══════════════════════════════════════╝");
                    println!();
                    println!("✨ On-screen FPS overlay is now active!");
                    println!("   Check bottom-center of the round screen.");
                    println!("   (Console output less frequent now)");
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }

                // Target 60 FPS by requesting next frame
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    std::time::Instant::now() + std::time::Duration::from_millis(16), // ~60 FPS
                ));
            }
            _ => {}
        }
    }
}

impl MinimalNativeApp {
    /// Reset FPS samples to current value (useful after performance stutters)
    fn reset_fps_samples(&mut self, current_fps: f32) {
        self.fps_samples.fill(current_fps);
        self.fps_sample_index = 0;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    console_log!("🎯 Starting native app with shared core components");

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait); // Wait for events instead of polling continuously

    let mut app = MinimalNativeApp::default();
    event_loop.run_app(&mut app)?;

    Ok(())
}

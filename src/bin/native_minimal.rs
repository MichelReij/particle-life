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

                // Debug logging every 60 frames
                if (self.current_time * 60.0) as u32 % 60 == 0 {
                    console_log!(
                        "🔄 Frame update: time={:.2}s, delta={:.4}s, particles={}",
                        self.current_time,
                        delta_time,
                        self.particle_system.get_active_count()
                    );
                }

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
                            // Render successful
                        }
                        Err(e) => {
                            console_log!("❌ Render error: {:?}", e);
                        }
                    }
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

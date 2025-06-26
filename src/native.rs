//! Native implementation of the particle life simulation
//! This module provides a native window and event loop for desktop platforms

use crate::webgpu_renderer::SurfaceTarget;
use crate::{console_log, InteractionRules, ParticleSystem, SimulationParams, WebGpuRenderer};
use rand::rngs::SmallRng;
use rand::SeedableRng;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
};

pub async fn run_native() {
    console_log!("🚀 Starting native particle life simulation");

    // Create event loop and window
    let event_loop = EventLoop::new().expect("Failed to create event loop");

    // Use PhysicalSize for true pixel control - no DPR scaling!
    let window_size = winit::dpi::PhysicalSize::new(800, 800);
    let window_attributes = winit::window::Window::default_attributes()
        .with_title("Particle Life - Native")
        .with_inner_size(window_size);
    let window = std::sync::Arc::new(
        event_loop
            .create_window(window_attributes)
            .expect("Failed to create window"),
    );

    console_log!("🪟 Created window: 800x800 physical pixels (no DPR scaling)");

    // Initialize simulation components
    let mut rng = SmallRng::from_entropy();
    let mut simulation_params = SimulationParams::new();

    // Use fixed logical viewport size for consistency across platforms
    // Set viewport to 800x800 to match canvas for better visibility
    simulation_params.viewport_width = 800.0;
    simulation_params.viewport_height = 800.0;

    // Center the viewport in the virtual world to see particles
    simulation_params.virtual_world_offset_x = (2400.0 - 800.0) / 2.0; // Center horizontally
    simulation_params.virtual_world_offset_y = (2400.0 - 800.0) / 2.0; // Center vertically

    // Make particles larger for better visibility in native mode
    simulation_params.particle_render_size = 24.0; // Doubled from 12.0 for better visibility

    let interaction_rules = InteractionRules::new_random(&mut rng);
    let mut particle_system = ParticleSystem::new(&simulation_params, &interaction_rules, &mut rng);

    console_log!(
        "🎮 Initialized simulation components: {} particles, viewport: 800x800",
        particle_system.get_active_count()
    );

    // Initialize WebGPU renderer (optional for now)
    let mut renderer = match WebGpuRenderer::new(SurfaceTarget::Window(window.clone())).await {
        Ok(mut renderer) => {
            console_log!("✅ WebGPU renderer initialized successfully");
            // Initialize particle buffers with actual particle data
            renderer.initialize_particle_buffers(&particle_system);
            Some(renderer)
        }
        Err(e) => {
            console_log!("⚠️ WebGPU renderer not ready yet: {:?}", e);
            console_log!("💡 Continuing with window-only mode for now");
            None
        }
    };

    console_log!("🎨 Starting main loop (window will stay open)");

    // Track frame timing
    let mut last_frame_time = std::time::Instant::now();

    // Run the event loop
    let _ = event_loop.run(move |event, elwt| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => {
                    console_log!("🚪 Window close requested");
                    elwt.exit();
                }
                WindowEvent::Resized(physical_size) => {
                    console_log!(
                        "📏 Window resized to: {}x{} (ignoring - keeping fixed 800x800 viewport)",
                        physical_size.width,
                        physical_size.height
                    );
                    // Keep viewport fixed at 800x800 regardless of window size
                }
                _ => {}
            },
            Event::AboutToWait => {
                // Calculate delta time
                let now = std::time::Instant::now();
                let delta_time = now.duration_since(last_frame_time).as_secs_f32();
                last_frame_time = now;

                // Update simulation parameters with current time
                simulation_params.time += delta_time;

                // Run physics simulation
                particle_system.update_physics(&simulation_params, &interaction_rules);

                // Only render if we have a renderer
                if let Some(ref mut renderer) = renderer {
                    // Render frame
                    let lightning_segments_data = vec![]; // Empty for now
                    let lightning_bolts_data = vec![]; // Empty for now

                    match renderer.render(
                        &particle_system,
                        &simulation_params,
                        &interaction_rules,
                        &lightning_segments_data,
                        &lightning_bolts_data,
                    ) {
                        Ok(()) => {
                            // Render succeeded - only log occasionally to avoid spam
                            static mut FRAME_COUNT: u32 = 0;
                            unsafe {
                                FRAME_COUNT += 1;
                                if FRAME_COUNT % 60 == 0 {
                                    console_log!(
                                        "🎨 Rendered frame {} with {} particles",
                                        FRAME_COUNT,
                                        particle_system.get_active_count()
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            console_log!("❌ Render error: {:?}", e);
                        }
                    }
                }

                // Request a redraw for continuous animation
                window.request_redraw();
            }
            _ => {}
        }
    });
}

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
    // ESP32 communication
    esp32_manager: Option<ESP32Manager>,
    last_esp32_update: std::time::Instant,
    // Audio system
    audio_manager: Option<AudioManager>,
    // Native-specific
    last_frame: std::time::Instant,
    current_time: f32,
    // FPS tracking for display with smoothing
    fps_last_update: std::time::Instant,
    fps_frame_count: u32,
    current_fps: f32,
    fps_samples: Vec<f32>,
    fps_sample_index: usize,
    // Smart lightning detection
    lightning_polling_enabled: bool,
    last_lightning_poll: std::time::Instant,
    current_flash_id: u32,
    lightning_start_time: f32,
    lightning_communicated: bool,
    next_poll_time: std::time::Instant,
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
        simulation_params.apply_ph(10.0); // pH = 10.0 (optimal for life)
        simulation_params.apply_electrical_activity(2.0); // electrical_activity = 2.0 (default)

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
            esp32_manager: None, // Will be initialized when ESP32 is detected
            last_esp32_update: std::time::Instant::now(),
            audio_manager: None, // Will be initialized during setup
            last_frame: std::time::Instant::now(),
            current_time: 0.0,
            fps_last_update: std::time::Instant::now(),
            fps_frame_count: 0,
            current_fps: 0.0,
            fps_samples: vec![60.0; FPS_SAMPLE_COUNT], // Initialize with samples at 60 FPS
            fps_sample_index: 0,
            // Initialize smart lightning detection
            lightning_polling_enabled: true,
            last_lightning_poll: std::time::Instant::now(),
            current_flash_id: 0,
            lightning_start_time: 0.0,
            lightning_communicated: false,
            next_poll_time: std::time::Instant::now(),
        }
    }
}

impl ApplicationHandler for MinimalNativeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        console_log!("🚀 Native app using shared core components");

        // Initialize ESP32 communication
        console_log!("🔌 Starting ESP32 communication...");

        // Test ESP32 sensor data conversion functions
        test_esp32_sensor_data_conversion();

        self.esp32_manager = Some(ESP32Manager::new());

        // Only platform-specific part: window creation
        #[cfg(target_os = "linux")]
        let window_attributes = {
            use winit::window::Fullscreen;
            Window::default_attributes()
                .with_title("Particle Life - Round Screen")
                .with_inner_size(winit::dpi::LogicalSize::new(1080, 1080))
                .with_fullscreen(Some(Fullscreen::Borderless(None)))
                .with_resizable(false)
                .with_decorations(false)
        };

        #[cfg(not(target_os = "linux"))]
        let window_attributes = Window::default_attributes()
            .with_title("Particle Life - Shared Components")
            .with_inner_size(winit::dpi::LogicalSize::new(
                CANVAS_WIDTH_U32,
                CANVAS_HEIGHT_U32,
            ))
            .with_resizable(false);

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        self.window = Some(window.clone());

        // Display fullscreen instructions for Linux
        #[cfg(target_os = "linux")]
        console_log!("🖥️  FULLSCREEN MODE (1080x1080): Optimized for round display - Press [Escape] or [Q] to exit");
        console_log!(
            "🎵 AUDIO CONTROLS: [M] toggle music, [+/-] volume ±5 (0-100), volume 0=pause"
        );

        #[cfg(not(target_os = "linux"))]
        console_log!("🪟 WINDOWED MODE: Close window or press Alt+F4 to exit");
        console_log!(
            "🎵 AUDIO CONTROLS: [M] toggle music, [+/-] volume ±5 (0-100), volume 0=pause"
        );

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

        // Initialize audio system (with fallback to disable on ALSA issues)
        match AudioManager::new() {
            Ok(audio_manager) => {
                console_log!("🎵 Audio system initialized successfully");
                self.audio_manager = Some(audio_manager);
            }
            Err(e) => {
                console_log!("❌ Failed to initialize audio: {:?}", e);
                console_log!("   Audio disabled - continuing without background music...");
                console_log!("   This is normal on Linux systems with ALSA buffer underrun issues");
                console_log!("   The particle simulation will work perfectly without audio");
                // Continue without audio - this is non-critical for the particle simulation
                self.audio_manager = None;
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // Drop GPU resources BEFORE the window is destroyed to prevent segfault.
        // The wgpu Surface holds a reference to the window; if the window is freed
        // first the Vulkan driver crashes with a segmentation fault.
        console_log!("🧹 Cleaning up GPU resources before exit...");
        self.renderer = None;       // drops Surface + all wgpu resources
        self.audio_manager = None;  // stops audio stream cleanly
        self.esp32_manager = None;  // stops serial thread cleanly
        self.window = None;         // window can now be safely destroyed
        console_log!("✅ Cleanup complete, goodbye!");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                console_log!("👋 Closing native app");
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                // Handle keyboard input for fullscreen mode exit and audio controls
                if event.state == winit::event::ElementState::Pressed {
                    match event.logical_key {
                        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) => {
                            console_log!("🚪 Escape key pressed - exiting fullscreen app");
                            event_loop.exit();
                        }
                        winit::keyboard::Key::Character(ref c) if c == "q" => {
                            console_log!("🚪 Q key pressed - exiting app");
                            event_loop.exit();
                        }
                        winit::keyboard::Key::Character(ref c) if c == "m" => {
                            // Toggle background music
                            if let Some(audio) = &mut self.audio_manager {
                                audio.toggle_background();
                            } else {
                                console_log!("🔇 Audio system is disabled");
                            }
                        }
                        winit::keyboard::Key::Character(ref c) if c == "+" => {
                            // Increase volume by 5
                            if let Some(audio) = &mut self.audio_manager {
                                audio.volume_up();
                            } else {
                                console_log!("🔇 Audio system is disabled");
                            }
                        }
                        winit::keyboard::Key::Character(ref c) if c == "-" => {
                            // Decrease volume by 5
                            if let Some(audio) = &mut self.audio_manager {
                                audio.volume_down();
                            } else {
                                console_log!("🔇 Audio system is disabled");
                            }
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                let delta_time = (now - self.last_frame).as_secs_f32();
                self.last_frame = now;

                // Update ESP32 sensor data (non-blocking)
                if let Some(esp32_manager) = &self.esp32_manager {
                    // Check for updates every 16ms (~60 FPS)
                    if now.duration_since(self.last_esp32_update).as_millis() >= 16 {
                        self.last_esp32_update = now;

                        match esp32_manager.get_sensor_data() {
                            Ok(sensor_data) => {
                                // Apply ESP32 sensor data to simulation parameters
                                self.simulation_params
                                    .apply_esp32_sensor_data(&sensor_data, delta_time);

                                // Update audio volume from ESP32 potentiometer
                                if let Some(audio) = &mut self.audio_manager {
                                    let volume_percentage = sensor_data.to_volume_percentage();
                                    audio.set_volume(volume_percentage);
                                }

                                // Handle sleep mode
                                if sensor_data.sleep {
                                    // TODO: Implement sleep mode (screen saver, reduced processing, etc.)
                                    console_log!("😴 ESP32 sleep mode activated");
                                }
                            }
                            Err(ESP32Error::PortNotFound) => {
                                // ESP32 not connected yet, use default values
                            }
                            Err(ESP32Error::ConnectionLost) => {
                                console_log!("📡 ESP32 connection lost, using default values");
                            }
                            Err(err) => {
                                console_log!("❌ ESP32 error: {:?}", err);
                            }
                        }

                        // Log ESP32 status occasionally
                        if now.duration_since(self.last_esp32_update).as_secs() >= 5 {
                            let status = esp32_manager.get_status();
                            console_log!("📡 ESP32 Status: {:?}", status);
                        }
                    }
                }

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
                            // Render successful - perform smart lightning detection afterwards
                        }
                        Err(e) => {
                            console_log!("❌ Render error: {:?}", e);
                        }
                    }
                }

                // Smart lightning detection - only poll when needed (after render borrow ends)
                if self.renderer.is_some() {
                    self.update_smart_lightning_detection();
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

                    // FPS display completely disabled - no need to update FPS data at all
                    // This ensures no text rendering occurs on screen

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
                    println!("║  FPS: {:<27.0} ║", self.current_fps);
                    println!("║  Particles: {:<22} ║", active_particles);
                    println!("║  Time: {:<25.1} ║", self.current_time);
                    println!("╚══════════════════════════════════════╝");
                    println!();
                    println!("✨ FPS counter removed from screen!");
                    println!("   Only console stats are shown now.");
                    println!("   Screen should be clean without overlay text.");
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
    /// Smart lightning detection with timing-based polling optimization
    fn update_smart_lightning_detection(&mut self) {
        let renderer = match &mut self.renderer {
            Some(r) => r,
            None => return,
        };
        let now = std::time::Instant::now();

        // Only poll if polling is enabled and it's time to poll
        if !self.lightning_polling_enabled || now < self.next_poll_time {
            return;
        }

        // If we have communicated lightning and it's been more than 2 seconds since start_time,
        // re-enable polling for the next lightning
        if self.lightning_communicated {
            let time_since_lightning = self.current_time - self.lightning_start_time;
            if time_since_lightning >= 2.0 {
                console_log!("🔄 Lightning detection: Re-enabling polling after 2s post-lightning");
                self.lightning_polling_enabled = true;
                self.lightning_communicated = false;
                self.current_flash_id = 0; // Reset to detect new lightning
                self.next_poll_time = now; // Poll immediately
            }
            return;
        }

        // Rate limit polling to every 100ms to avoid overwhelming the GPU
        if now.duration_since(self.last_lightning_poll).as_millis() < 100 {
            return;
        }

        self.last_lightning_poll = now;

        // Read lightning data from GPU asynchronously (non-blocking)
        match pollster::block_on(renderer.read_lightning_bolt_data()) {
            Ok(lightning_bolt) => {
                // Removed verbose lightning buffer logging
                // console_log!("📡 Lightning buffer read: flash_id={}, start_time={:.3}, super={}, next_time={:.3}",
                //     lightning_data.flash_id, lightning_data.start_time, lightning_data.is_super_lightning, lightning_data.next_time);                // Check if we found a new lightning (flash_id changed and start_time is reasonable)
                if lightning_bolt.flash_id > self.current_flash_id
                    && lightning_bolt.start_time > 0.0
                    && lightning_bolt.start_time <= self.current_time + 10.0
                {
                    console_log!(
                        "⚡ NEW LIGHTNING DETECTED! Flash ID: {}, Type: {}, Start Time: {:.3}s",
                        lightning_bolt.flash_id,
                        if lightning_bolt.is_super() {
                            "Super"
                        } else {
                            "Normal"
                        },
                        lightning_bolt.start_time
                    );

                    // Update our state
                    self.current_flash_id = lightning_bolt.flash_id;
                    self.lightning_start_time = lightning_bolt.start_time;

                    // Send to ESP32 immediately when lightning starts
                    self.communicate_lightning_to_esp32(
                        lightning_bolt.flash_id,
                        lightning_bolt.is_super(),
                        lightning_bolt.start_time,
                    );

                    // Mark as communicated and disable polling until 2s after start_time
                    self.lightning_communicated = true;
                    self.lightning_polling_enabled = false;

                    console_log!(
                        "⏰ Lightning detection: Pausing polling until 2s after start_time"
                    );
                }

                // If we have lightning data and know when the next lightning will occur,
                // schedule next poll for just before that time
                if lightning_bolt.next_lightning_time > self.current_time {
                    let wait_time_secs =
                        lightning_bolt.next_lightning_time - self.current_time - 0.1; // Poll 100ms before
                    if wait_time_secs > 0.5 {
                        // Only pause if we have at least 500ms
                        let wait_duration = std::time::Duration::from_secs_f32(wait_time_secs);
                        self.next_poll_time = now + wait_duration;
                        console_log!("⏰ Smart polling: Next poll in {:.1}s (before expected lightning at {:.1}s)",
                            wait_time_secs, lightning_bolt.next_lightning_time);
                    }
                }
            }
            Err(e) => {
                console_log!("❌ Failed to read lightning data: {:?}", e);
                // Continue polling on error, but with longer delay
                self.next_poll_time = now + std::time::Duration::from_millis(500);
            }
        }
    }

    /// Communicate lightning event to ESP32
    fn communicate_lightning_to_esp32(
        &self,
        flash_id: u32,
        is_super_lightning: bool,
        start_time: f32,
    ) {
        if let Some(esp32_manager) = &self.esp32_manager {
            esp32_manager.send_lightning_event(
                flash_id,
                if is_super_lightning { 1 } else { 0 },
                start_time,
                if is_super_lightning { 1.0 } else { 0.7 }, // Intensity based on type
            );

            console_log!(
                "📤 ESP32: Lightning event sent! Flash ID: {}, Type: {}, Start: {:.3}s",
                flash_id,
                if is_super_lightning {
                    "Super"
                } else {
                    "Normal"
                },
                start_time
            );
        } else {
            console_log!("⚠️ ESP32 manager not available, lightning event not sent");
        }
    }

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

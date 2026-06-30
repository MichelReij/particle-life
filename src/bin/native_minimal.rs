// Copyright © 2025 - 2026 Michel Reij | Bewogen Kunst | Moving Art
// Licensed under CC BY-NC 4.0 — https://creativecommons.org/licenses/by-nc/4.0/

// Minimal native binary that uses shared core components

use particle_life_wasm::audio_engine::AudioEngine;
use particle_life_wasm::config::*;
use particle_life_wasm::sonification::{GpuGlobalStats, GpuTypeStats, SonificationState};
use particle_life_wasm::stats_reader::StatsReader;
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

fn temperature_to_lerp_duration(temp_celsius: f32) -> f32 {
    const MIN_TEMP: f32 = 3.0;
    const MAX_TEMP: f32 = 160.0;
    const MAX_DURATION: f32 = 1800.0;
    const MIN_DURATION: f32 = 180.0;
    let t = ((temp_celsius - MIN_TEMP) / (MAX_TEMP - MIN_TEMP)).clamp(0.0, 1.0);
    MAX_DURATION - t * (MAX_DURATION - MIN_DURATION)
}

struct MinimalNativeApp {
    window: Option<Arc<Window>>,
    particle_system: ParticleSystem,
    simulation_params: SimulationParams,
    interaction_rules: InteractionRules,
    rng: SmallRng,
    renderer: Option<WebGpuRenderer>,
    rule_evolution: RuleEvolution,
    esp32_manager: Option<ESP32Manager>,
    last_esp32_update: std::time::Instant,

    // Audio: nieuwe synthesizer
    audio_engine: Option<AudioEngine>,

    // Sonificatie
    sonification_state: SonificationState,
    last_gpu_stats: Option<([GpuTypeStats; 7], GpuGlobalStats)>,
    stats_reader: Option<StatsReader>,

    last_frame: std::time::Instant,
    current_time: f32,
    fps_last_update: std::time::Instant,
    fps_frame_count: u32,
    current_fps: f32,
    // Doelaantal deeltjes zoals zojuist uit de druksensor berekend — los van
    // particle_system.get_active_count(), dat tijdens een krimp-transitie bewust nog
    // het oude (hogere) aantal toont totdat de ~1.5s GPU-animatie is afgerond. Voor de
    // overlay willen we het werkelijk aangevraagde aantal, niet die vertraagde waarde.
    desired_particle_count: u32,
    fps_samples: Vec<f32>,
    fps_sample_index: usize,
    lightning_polling_enabled: bool,
    last_lightning_poll: std::time::Instant,
    current_flash_id: u32,
    lightning_start_time: f32,
    lightning_communicated: bool,
    next_poll_time: std::time::Instant,
    last_night_alpha_sent: f32,
    last_night_alpha_update: std::time::Instant,
}

impl Default for MinimalNativeApp {
    fn default() -> Self {
        let mut rng = SmallRng::from_entropy();
        let mut simulation_params = SimulationParams::new();

        simulation_params.apply_zoom(1.0, None, None);
        simulation_params.apply_temperature(20.0);
        simulation_params.apply_pressure(200.0);
        simulation_params.apply_ph(10.0);
        simulation_params.apply_electrical_activity(2.0);

        console_log!("🎯 Native defaults toegepast");

        let interaction_rules = InteractionRules::new_random(&mut rng);
        let particle_system = ParticleSystem::new(&simulation_params, &interaction_rules, &mut rng);
        let initial_particle_count = particle_system.get_active_count();

        let mut rule_evolution = RuleEvolution::new(interaction_rules.clone(), &mut rng);
        rule_evolution.set_duration(temperature_to_lerp_duration(20.0));

        Self {
            window: None,
            particle_system,
            simulation_params,
            interaction_rules,
            rng,
            renderer: None,
            rule_evolution,
            esp32_manager: None,
            last_esp32_update: std::time::Instant::now(),
            audio_engine: None,
            sonification_state: SonificationState::default(),
            last_gpu_stats: None,
            stats_reader: None,
            last_frame: std::time::Instant::now(),
            current_time: 0.0,
            fps_last_update: std::time::Instant::now(),
            fps_frame_count: 0,
            current_fps: 0.0,
            desired_particle_count: initial_particle_count,
            fps_samples: vec![60.0; FPS_SAMPLE_COUNT],
            fps_sample_index: 0,
            lightning_polling_enabled: true,
            last_lightning_poll: std::time::Instant::now(),
            current_flash_id: 0,
            lightning_start_time: 0.0,
            lightning_communicated: false,
            next_poll_time: std::time::Instant::now(),
            last_night_alpha_sent: -1.0,
            last_night_alpha_update: std::time::Instant::now(),
        }
    }
}

impl ApplicationHandler for MinimalNativeApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        console_log!("🚀 Origin of Life native gestart");

        test_esp32_sensor_data_conversion();
        self.esp32_manager = Some(ESP32Manager::new());

        // OS-fullscreen (Fullscreen::Borderless) vult het hele monitorvlak, ook als dat
        // niet vierkant is — de surface (altijd 1080×1080) wordt dan door de compositor
        // uitgerekt naar die verhouding. Op het ronde 1080×1080-productiescherm is het
        // monitorvlak zelf al vierkant, dus daar maakt dit niets uit; los daarvan vragen
        // we hier expliciet een vierkant venster aan (grootte = kortste monitorzijde,
        // gecentreerd), zodat de 1:1 aspect ratio altijd behouden blijft.
        #[cfg(target_os = "linux")]
        let window_attributes = {
            let (square, position) = event_loop.primary_monitor()
                .map(|monitor| {
                    let size = monitor.size();
                    let square = size.width.min(size.height).max(1);
                    let monitor_pos = monitor.position();
                    let x = monitor_pos.x + (size.width as i32 - square as i32) / 2;
                    let y = monitor_pos.y + (size.height as i32 - square as i32) / 2;
                    (square, winit::dpi::PhysicalPosition::new(x, y))
                })
                .unwrap_or((1080, winit::dpi::PhysicalPosition::new(0, 0)));

            Window::default_attributes()
                .with_title("Origin of Life")
                .with_inner_size(winit::dpi::PhysicalSize::new(square, square))
                .with_position(position)
                .with_resizable(false)
                .with_decorations(false)
        };

        #[cfg(not(target_os = "linux"))]
        let window_attributes = Window::default_attributes()
            .with_title("Origin of Life")
            .with_inner_size(winit::dpi::LogicalSize::new(CANVAS_WIDTH_U32, CANVAS_HEIGHT_U32))
            .with_resizable(false);

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        self.window = Some(window.clone());

        console_log!("🎵 Toetsen: [M] synthesizer aan, [S] pauzeer, [+]/[-] volume");

        pollster::block_on(async {
            match WebGpuRenderer::new(window.clone()).await {
                Ok(mut renderer) => {
                    console_log!("✅ WebGPU renderer geïnitialiseerd");

                    // Load copyright/dropshadow overlay (baked into binary at compile time)
                    let png_bytes = include_bytes!("../../assets/images/copyright_mask_native_1080.png");
                    match image::load_from_memory(png_bytes) {
                        Ok(img) => {
                            let rgba = img.to_rgba8();
                            renderer.set_overlay_from_rgba(rgba.as_raw(), rgba.width(), rgba.height());
                            console_log!("✅ Copyright overlay geladen ({}×{})", rgba.width(), rgba.height());
                        }
                        Err(e) => console_log!("⚠ Overlay laden mislukt: {:?}", e),
                    }

                    self.renderer = Some(renderer);
                }
                Err(e) => console_log!("❌ Renderer fout: {:?}", e),
            }
        });

        // StatsReader initialiseren
        if let Some(renderer) = &self.renderer {
            let device = renderer.get_device();
            let stats_reader = StatsReader::new(device, &renderer.sim_params_buffer);
            // Sla bind group referenties niet op — we maken ze elke dispatch opnieuw
            // (ping-pong buffer wisselt na elke render)
            self.stats_reader = Some(stats_reader);
            console_log!("✅ StatsReader geïnitialiseerd (niet-blokkerend, 10 Hz bij zoom > 3x)");
        }

        // AudioEngine initialiseren
        match AudioEngine::new() {
            Ok(engine) => {
                console_log!("✅ AudioEngine (supersaw) actief — 7 stemmen @ 44.1 kHz");
                self.audio_engine = Some(engine);
            }
            Err(e) => {
                console_log!("❌ AudioEngine fout: {:?}", e);
                console_log!("   Simulatie gaat door zonder audio");
            }
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        console_log!("🧹 GPU resources opruimen...");
        self.stats_reader  = None;
        self.renderer      = None;
        self.audio_engine  = None;
        self.esp32_manager = None;
        self.window        = None;
        console_log!("✅ Klaar!");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == winit::event::ElementState::Pressed {
                    match event.logical_key {
                        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) => event_loop.exit(),
                        winit::keyboard::Key::Character(ref c) if c == "q" => event_loop.exit(),
                        winit::keyboard::Key::Character(ref c) if c == "m" => {
                            if let Some(e) = &self.audio_engine { e.set_paused(false); console_log!("🎵 Synthesizer: aan"); }
                        }
                        winit::keyboard::Key::Character(ref c) if c == "s" => {
                            if let Some(e) = &self.audio_engine { e.set_paused(true); console_log!("🔇 Synthesizer: gepauzeerd"); }
                        }
                        winit::keyboard::Key::Character(ref c) if c == "+" => {
                            if let Some(e) = &self.audio_engine { e.set_master_volume(0.8); }
                        }
                        winit::keyboard::Key::Character(ref c) if c == "-" => {
                            if let Some(e) = &self.audio_engine { e.set_master_volume(0.3); }
                        }
                        _ => {}
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                let now = std::time::Instant::now();
                let delta_time = (now - self.last_frame).as_secs_f32().min(0.033);
                self.last_frame = now;

                // ESP32 update (~60 Hz)
                if let Some(esp32) = &self.esp32_manager {
                    if now.duration_since(self.last_esp32_update).as_millis() >= 16 {
                        self.last_esp32_update = now;
                        match esp32.get_sensor_data() {
                            Ok(sd) => {
                                self.simulation_params.apply_esp32_sensor_data(&sd, delta_time);
                                self.rule_evolution.set_duration(temperature_to_lerp_duration(sd.to_temperature_celsius()));
                                if let Some(e) = &self.audio_engine {
                                    e.set_master_volume(sd.to_volume_percentage() as f32 / 100.0);
                                }
                                // Diepte/druk stuurt ook het particle-aantal (4800-6400),
                                // net als de web-UI via set_particle_count_from_pressure.
                                let target_count = self.simulation_params.pressure_to_particle_count(
                                    sd.to_pressure(),
                                    self.particle_system.get_max_particles(),
                                    self.particle_system.get_min_particles(),
                                );
                                self.desired_particle_count = target_count;
                                self.set_particle_count(target_count);
                            }
                            Err(ESP32Error::PortNotFound) => {}
                            Err(e) => console_log!("❌ ESP32: {:?}", e),
                        }
                    }
                }

                // Simulatie update
                self.current_time += delta_time;
                self.simulation_params.set_time(self.current_time);
                self.simulation_params.set_delta_time(delta_time);
                self.simulation_params.update_night_alpha();
                self.interaction_rules = self.rule_evolution.tick(delta_time, &mut self.rng).clone();

                // Stuur night_alpha naar ESP32 via UART
                self.communicate_night_alpha_to_esp32();

                // Sonificatie update
                let (_gpu_type_ref, _gpu_global_ref) = if self.simulation_params.current_zoom_level > 3.0 {
                    match &self.last_gpu_stats {
                        Some((per_type, global)) => (
                            Some(per_type as &[GpuTypeStats; 7]),
                            Some(global as &GpuGlobalStats),
                        ),
                        None => (None, None),
                    }
                } else {
                    (None, None)
                };

                // Standalone audio patch: sonificatie-koppeling tijdelijk uitgeschakeld

                // Poll vorige readback — altijd, vóór render
                if let Some(stats_reader) = &mut self.stats_reader {
                    if let Some(stats) = stats_reader.poll_result() {
                        self.last_gpu_stats = Some(stats);
                    }
                }

                // Krimp-transities houden active_count bewust hoog zolang de GPU de
                // verdwijnende deeltjes nog animeert (anders worden ze niet meer gedispatcht
                // en stopt de fade-out halverwege). Pas ná afloop verlagen we active_count
                // alsnog — zelfde check als lib.rs's (web-only) render(). Zonder dit blijft
                // het geboekte aantal voor altijd op het historische maximum staan.
                if self.simulation_params.is_transition_complete(self.current_time)
                    && self.simulation_params.transition_active
                {
                    let target_count = self.simulation_params.transition_end_count;
                    if !self.simulation_params.transition_is_grow {
                        self.particle_system.set_active_count(target_count);
                        self.simulation_params.set_num_particles(target_count);
                    }
                    self.simulation_params.stop_particle_transition();
                }

                // Render
                if let Some(renderer) = &mut self.renderer {
                    renderer.update_fps_data(
                        self.current_fps,
                        self.fps_frame_count,
                        self.desired_particle_count,
                        self.current_time,
                    );
                    match renderer.render(&self.particle_system, &self.simulation_params,
                                          &self.interaction_rules, &[], &[]) {
                        Ok(_) => {}
                        Err(e) => console_log!("❌ Render fout: {:?}", e),
                    }
                }

                // Stats dispatch (na render, bij zoom > 3)
                if self.simulation_params.current_zoom_level > 3.0 {
                    if let (Some(stats_reader), Some(renderer)) =
                        (&mut self.stats_reader, &mut self.renderer)
                    {
                        // Na render() is current_buffer_index al gewisseld.
                        // De output particles zitten in 1 - current_buffer_index.
                        let output_idx = 1 - renderer.current_buffer_index;

                        let bg = stats_reader.make_bind_group(
                            renderer.get_device(),
                            &renderer.particle_buffers[output_idx],
                            &renderer.sim_params_buffer,
                        );

                        let mut encoder = renderer.get_device().create_command_encoder(
                            &wgpu::CommandEncoderDescriptor { label: Some("Stats Encoder") });

                        if stats_reader.maybe_dispatch(
                            &mut encoder, &bg, self.particle_system.get_active_count())
                        {
                            renderer.queue().submit(std::iter::once(encoder.finish()));
                            stats_reader.start_readback();
                        }
                    }
                }

                // Lightning detectie
                if self.renderer.is_some() {
                    self.update_smart_lightning_detection();
                }

                // FPS
                self.fps_frame_count += 1;
                let fps_elapsed = (now - self.fps_last_update).as_secs_f32();
                if fps_elapsed >= FPS_UPDATE_INTERVAL {
                    let inst = self.fps_frame_count as f32 / fps_elapsed;
                    self.fps_samples[self.fps_sample_index] = inst;
                    self.fps_sample_index = (self.fps_sample_index + 1) % self.fps_samples.len();
                    self.current_fps = self.fps_samples.iter().sum::<f32>() / self.fps_samples.len() as f32;
                    self.fps_last_update = now;
                    self.fps_frame_count = 0;
                }

                if let Some(w) = &self.window { w.request_redraw(); }
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    std::time::Instant::now() + std::time::Duration::from_millis(16)));
            }
            _ => {}
        }
    }
}

impl MinimalNativeApp {
    /// Native variant van `ParticleLifeEngine::set_particle_count` (lib.rs, web-only via
    /// wasm_bindgen) — start een vloeiende groei/krimp-transitie naar `count` deeltjes.
    /// MinimalNativeApp deelt geen struct met ParticleLifeEngine, dus dezelfde logica
    /// wordt hier gerepliceerd op de native velden.
    fn set_particle_count(&mut self, count: u32) {
        if count > self.particle_system.get_max_particles() {
            return;
        }

        // Deze functie wordt elke ESP32-tick (~60Hz) aangeroepen, niet alleen bij een
        // echte sliderwijziging. Tijdens een krimp-transitie blijft active_count bewust
        // nog op het oude (hoge) aantal staan totdat de 1.5s-animatie afloopt — als we
        // dan tegen active_count zouden vergelijken, ziet elke volgende tick nog steeds
        // "count != current_count" en herstart de transitie eindeloos, zodat hij nooit
        // afrondt. Dus: al onderweg naar hetzelfde doel? Niets doen.
        if self.simulation_params.transition_active && self.simulation_params.transition_end_count == count {
            return;
        }

        let current_count = self.particle_system.get_active_count();
        if count == current_count {
            self.simulation_params.set_num_particles(count);
            return;
        }

        // Vóór het overschrijven vastleggen: nodig om in de krimp-tak te bepalen welk
        // bereik al gemarkeerd was door een eerdere (nog lopende) krimp-transitie.
        let prev_transition_active = self.simulation_params.transition_active;
        let prev_transition_is_grow = self.simulation_params.transition_is_grow;
        let prev_transition_end_count = self.simulation_params.transition_end_count;

        self.simulation_params.start_particle_transition(current_count, count, self.current_time);

        if count > current_count {
            self.particle_system.set_active_count(count);
            self.simulation_params.set_num_particles(count);
            for i in current_count..count {
                if let Some(particle) = self.particle_system.get_particle_mut(i as usize) {
                    particle.position = [
                        self.rng.gen_range(0.0..self.simulation_params.virtual_world_width),
                        self.rng.gen_range(0.0..self.simulation_params.virtual_world_height),
                    ];
                    particle.velocity = [self.rng.gen_range(-2.0..2.0), self.rng.gen_range(-2.0..2.0)];
                    particle.size = 0.1;
                    particle.transition_start = self.current_time;
                    particle.transition_type = 0;
                    particle.is_active = false;
                }
            }
        } else {
            // Bij een EMA-gesmoothde, geleidelijk dalende druk komt hier elke tick een
            // iets lager doel binnen terwijl active_count (bewust) pas ná afloop daalt.
            // Zonder correctie zou elke stap opnieuw het hele bereik [count, current_count)
            // markeren — inclusief particles die al middenin hun fade-out zitten — en hun
            // transition_start resetten naar nu, wat ze laat flikkeren/terugspringen.
            // Als er al een krimp-transitie loopt, alleen het NIEUWE surplus-bereik
            // markeren; particles die al verdwijnen blijven met rust.
            let already_shrinking_from = if prev_transition_active && !prev_transition_is_grow {
                prev_transition_end_count.min(current_count)
            } else {
                current_count
            };
            let mark_end = already_shrinking_from.max(count);

            for i in count..mark_end {
                if let Some(particle) = self.particle_system.get_particle_mut(i as usize) {
                    particle.transition_start = self.current_time;
                    particle.transition_type = 1;
                }
            }
        }

        if let Some(renderer) = &mut self.renderer {
            renderer.update_particle_transitions(&self.particle_system);
        }
    }

    fn update_smart_lightning_detection(&mut self) {
        let renderer = match &mut self.renderer { Some(r) => r, None => return };
        let now = std::time::Instant::now();

        if !self.lightning_polling_enabled || now < self.next_poll_time { return; }

        if self.lightning_communicated {
            if self.current_time - self.lightning_start_time >= 2.0 {
                self.lightning_polling_enabled = true;
                self.lightning_communicated    = false;
                self.current_flash_id          = 0;
                self.next_poll_time            = now;
            }
            return;
        }

        if now.duration_since(self.last_lightning_poll).as_millis() < 100 { return; }
        self.last_lightning_poll = now;

        match pollster::block_on(renderer.read_lightning_bolt_data()) {
            Ok(bolt) => {
                if bolt.flash_id > self.current_flash_id
                    && bolt.start_time > 0.0
                    && bolt.start_time <= self.current_time + 10.0
                {
                    self.current_flash_id     = bolt.flash_id;
                    self.lightning_start_time = bolt.start_time;

                    if bolt.is_super() {
                        self.rule_evolution.snap_to_new(&mut self.rng);
                        console_log!("⚡ Super-lightning: regels gesnapt");
                    }

                    self.communicate_lightning_to_esp32(bolt.flash_id, bolt.is_super(), bolt.start_time);
                    self.lightning_communicated    = true;
                    self.lightning_polling_enabled = false;
                }

                if bolt.next_lightning_time > self.current_time {
                    let wait = bolt.next_lightning_time - self.current_time - 0.1;
                    if wait > 0.5 {
                        self.next_poll_time = now + std::time::Duration::from_secs_f32(wait);
                    }
                }
            }
            Err(e) => {
                self.next_poll_time = now + std::time::Duration::from_millis(500);
                console_log!("❌ Lightning lezen mislukt: {:?}", e);
            }
        }
    }

    fn communicate_lightning_to_esp32(&self, flash_id: u32, is_super: bool, start_time: f32) {
        if let Some(esp32) = &self.esp32_manager {
            esp32.send_lightning_event(
                flash_id, if is_super { 1 } else { 0 }, start_time,
                if is_super { 1.0 } else { 0.7 },
            );
        }
    }

    fn communicate_night_alpha_to_esp32(&mut self) {
        let now = std::time::Instant::now();
        if now.duration_since(self.last_night_alpha_update).as_millis() < 100 { return; }
        self.last_night_alpha_update = now;

        let alpha = self.simulation_params.night_alpha;
        if let Some(esp32) = &self.esp32_manager {
            esp32.update_night_alpha(alpha);
        }
        self.last_night_alpha_sent = alpha;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    console_log!("🎯 Origin of Life — native binary gestart");

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = MinimalNativeApp::default();
    event_loop.run_app(&mut app)?;

    Ok(())
}

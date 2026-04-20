use crate::config::*;
use crate::{InteractionRules, ParticleSystem, SimulationParams};
use rand::Rng;
use wgpu::util::DeviceExt;

/// Cross-platform current time in milliseconds
#[cfg(target_arch = "wasm32")]
fn current_time_ms() -> f64 {
    js_sys::Date::now()
}

#[cfg(not(target_arch = "wasm32"))]
fn current_time_ms() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64
}

// Lightning bolt struct that matches WGSL layout
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightningBolt {
    pub num_segments: u32,
    pub flash_id: u32,
    pub start_time: f32,
    pub next_lightning_time: f32,
    pub is_super_lightning: u32,
    pub needs_rules_reset: u32,
    pub _padding1: u32,
    pub _padding2: u32,
}

impl LightningBolt {
    pub fn is_visible(&self) -> bool { self.num_segments > 0 }
    pub fn is_super(&self) -> bool { self.is_super_lightning != 0 }
    pub fn lightning_type(&self) -> &'static str {
        if self.is_super() { "SUPER LIGHTNING" } else { "normal lightning" }
    }
}

#[derive(Debug, Clone)]
pub struct LightningEvent {
    pub flash_id: u32,
    pub start_time: f32,
    pub is_super: bool,
    pub detected_at: f64,
}

#[derive(Debug)]
pub struct LightningDetector {
    last_flash_id: u32,
    pending_events: Vec<LightningEvent>,
    cached_lightning_bolt: Option<LightningBolt>,
    last_gpu_read_time: f64,
    pub frame_counter: u32,
    next_poll_time: Option<f64>,
    polling_paused: bool,
}

impl Default for LightningDetector {
    fn default() -> Self {
        Self {
            last_flash_id: 0,
            pending_events: Vec::new(),
            cached_lightning_bolt: None,
            last_gpu_read_time: current_time_ms(),
            frame_counter: 0,
            next_poll_time: None,
            polling_paused: false,
        }
    }
}

impl LightningDetector {
    pub fn new() -> Self { Self::default() }

    pub fn poll_events(&mut self) -> Vec<LightningEvent> {
        let events = self.pending_events.clone();
        self.pending_events.clear();
        events
    }

    pub fn process_lightning_bolt(&mut self, bolt: &LightningBolt) {
        self.cached_lightning_bolt = Some(*bolt);
        if bolt.flash_id > self.last_flash_id {
            if bolt.is_visible() {
                let event = LightningEvent {
                    flash_id: bolt.flash_id,
                    start_time: bolt.start_time,
                    is_super: bolt.is_super(),
                    detected_at: current_time_ms(),
                };
                self.pending_events.push(event);
                crate::console_log!(
                    "⚡ Lightning detector: New flash ID {} detected ({})",
                    bolt.flash_id,
                    if bolt.is_super() { "SUPER" } else { "normal" }
                );
            }
            self.last_flash_id = bolt.flash_id;
            if bolt.next_lightning_time > 0.0 {
                let seconds_until_next = bolt.next_lightning_time - bolt.start_time;
                if seconds_until_next > 0.0 {
                    self.next_poll_time = Some(current_time_ms() + seconds_until_next as f64 * 1000.0);
                    self.polling_paused = true;
                } else {
                    self.polling_paused = false;
                    self.next_poll_time = None;
                }
            }
        }
    }

    pub fn should_read_gpu_buffer(&mut self) -> bool {
        self.frame_counter += 1;
        if self.polling_paused {
            if let Some(next_poll_time) = self.next_poll_time {
                if current_time_ms() >= next_poll_time {
                    self.polling_paused = false;
                    self.next_poll_time = None;
                    return true;
                } else {
                    return false;
                }
            } else {
                self.polling_paused = false;
            }
        }
        if self.frame_counter % 5 == 0 {
            let elapsed = current_time_ms() - self.last_gpu_read_time;
            if elapsed >= 50.0 {
                self.last_gpu_read_time = current_time_ms();
                return true;
            }
        }
        false
    }

    pub fn get_cached_lightning_bolt(&self) -> Option<LightningBolt> { self.cached_lightning_bolt }
    pub fn is_polling_paused(&self) -> bool { self.polling_paused }
    pub fn time_until_next_poll(&self) -> Option<f64> {
        if let Some(npt) = self.next_poll_time {
            let now = current_time_ms();
            if npt > now { Some(npt - now) } else { None }
        } else { None }
    }
}

#[cfg(target_arch = "wasm32")]
pub type RendererError = wasm_bindgen::JsValue;

#[cfg(not(target_arch = "wasm32"))]
pub type RendererError = Box<dyn std::error::Error + Send + Sync>;

pub fn renderer_error(message: &str, source: impl std::fmt::Debug) -> RendererError {
    let msg = format!("{}: {:?}", message, source);
    #[cfg(target_arch = "wasm32")]
    { wasm_bindgen::JsValue::from_str(&msg) }
    #[cfg(not(target_arch = "wasm32"))]
    { Box::new(std::io::Error::new(std::io::ErrorKind::Other, msg)) }
}

/// WebGPU renderer that handles all GPU operations using pure wgpu
pub struct WebGpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,

    // Simulation buffers — pub voor toegang door stats_reader en native_minimal
    pub sim_params_buffer: wgpu::Buffer,
    pub particle_buffers: [wgpu::Buffer; 2],
    pub current_buffer_index: usize,
    interaction_rules_buffer: wgpu::Buffer,
    lightning_segments_buffer: wgpu::Buffer,
    lightning_bolt_buffer: wgpu::Buffer,
    particle_colors_buffer: wgpu::Buffer,
    quad_vertex_buffer: wgpu::Buffer,

    lightning_detector: LightningDetector,

    scene_texture: wgpu::Texture,
    scene_texture_view: wgpu::TextureView,
    intermediate_texture: wgpu::Texture,
    intermediate_texture_view: wgpu::TextureView,
    #[allow(dead_code)]
    scene_sampler: wgpu::Sampler,

    background_render_pipeline: wgpu::RenderPipeline,
    grid_render_pipeline: wgpu::RenderPipeline,
    render_pipeline: wgpu::RenderPipeline,
    glow_render_pipeline: wgpu::RenderPipeline,
    compute_pipeline: wgpu::ComputePipeline,
    lightning_compute_pipeline: wgpu::ComputePipeline,
    lightning_render_pipeline: wgpu::RenderPipeline,
    fisheye_render_pipeline: wgpu::RenderPipeline,
    zoom_render_pipeline: wgpu::RenderPipeline,

    compute_bind_groups: [wgpu::BindGroup; 2],
    render_bind_groups: [wgpu::BindGroup; 2],
    lightning_compute_bind_group: wgpu::BindGroup,
    lightning_render_bind_group: wgpu::BindGroup,
    background_render_bind_group: wgpu::BindGroup,
    grid_render_bind_group: wgpu::BindGroup,
    fisheye_render_bind_group: wgpu::BindGroup,
    zoom_render_bind_group: wgpu::BindGroup,

    zoom_uniforms_buffer: wgpu::Buffer,

    text_overlay_pipeline: Option<wgpu::RenderPipeline>,
    text_overlay_bind_group: Option<wgpu::BindGroup>,
    fps_data_buffer: Option<wgpu::Buffer>,
}

impl WebGpuRenderer {
    /// Initialize WebGPU renderer - WASM version with canvas
    #[cfg(target_arch = "wasm32")]
    pub async fn new(canvas: &web_sys::HtmlCanvasElement) -> Result<WebGpuRenderer, RendererError> {
        let canvas_width = canvas.width();
        let canvas_height = canvas.height();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| renderer_error("Failed to create surface", e))?;
        Self::initialize_common(instance, surface, canvas_width, canvas_height).await
    }

    /// Initialize WebGPU renderer - Native version with window
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn new(
        window: std::sync::Arc<winit::window::Window>,
    ) -> Result<WebGpuRenderer, RendererError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| renderer_error("Failed to create surface", e))?;
        Self::initialize_common(instance, surface, CANVAS_WIDTH_U32, CANVAS_HEIGHT_U32).await
    }

    async fn initialize_common(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<WebGpuRenderer, RendererError> {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| renderer_error("Failed to request adapter", e))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Particle Life Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| renderer_error("Failed to create device", e))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter().copied()
            .find(|f| !f.is_srgb())
            .or_else(|| surface_caps.formats.iter().copied().find(|f| f.is_srgb()))
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let sim_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Simulation Parameters Buffer"),
            size: 256,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let quad_vertices: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0];
        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(&quad_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let max_types = 16;
        let particle_colors_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Colors Buffer"),
            size: (max_types * 16) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let max_particles = 6400usize;
        let active_particles = 6400usize;
        let num_types = 7usize;
        let mut initial_particle_data = Vec::with_capacity(max_particles * 48);
        let mut rng = rand::thread_rng();

        for i in 0..max_particles {
            let particle_type = (i % num_types) as u32;
            let pos_x = rng.gen::<f32>() * VIRTUAL_WORLD_WIDTH;
            let pos_y = rng.gen::<f32>() * VIRTUAL_WORLD_HEIGHT;
            let vel_x = (rng.gen::<f32>() - 0.5) * 4.0;
            let vel_y = (rng.gen::<f32>() - 0.5) * 4.0;
            let base_multiplier = match particle_type { 0=>1.2, 1=>1.5, 2=>0.7, 3=>0.9, _=>1.0 };
            let target_size = PARTICLE_SIZE * base_multiplier * (1.0 + (rng.gen::<f32>() - 0.5) * 0.4);
            let is_active = if i < active_particles { 1u32 } else { 0u32 };

            initial_particle_data.extend_from_slice(&pos_x.to_le_bytes());
            initial_particle_data.extend_from_slice(&pos_y.to_le_bytes());
            initial_particle_data.extend_from_slice(&vel_x.to_le_bytes());
            initial_particle_data.extend_from_slice(&vel_y.to_le_bytes());
            initial_particle_data.extend_from_slice(&particle_type.to_le_bytes());
            initial_particle_data.extend_from_slice(&target_size.to_le_bytes());
            initial_particle_data.extend_from_slice(&target_size.to_le_bytes());
            initial_particle_data.extend_from_slice(&0.0f32.to_le_bytes());
            initial_particle_data.extend_from_slice(&0u32.to_le_bytes());
            initial_particle_data.extend_from_slice(&is_active.to_le_bytes());
            initial_particle_data.extend_from_slice(&0.0f32.to_le_bytes());
            initial_particle_data.extend_from_slice(&0.0f32.to_le_bytes());
        }

        let particle_buffer_size = (max_particles * 48) as u64;
        let particle_buffers = [
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Particle Buffer A"),
                size: particle_buffer_size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Particle Buffer B"),
                size: particle_buffer_size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        ];
        queue.write_buffer(&particle_buffers[0], 0, &initial_particle_data);
        queue.write_buffer(&particle_buffers[1], 0, &initial_particle_data);

        let interaction_rules_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Interaction Rules Buffer"),
            size: (max_types * max_types * 16) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let max_lightning_segments = 1024usize;
        let lightning_segments_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lightning Segments Buffer"),
            size: (max_lightning_segments * 48) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let lightning_bolt_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lightning Bolt Buffer"),
            size: 32,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let zoom_uniforms_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Zoom Uniforms Buffer"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let scene_texture_size = wgpu::Extent3d {
            width: FISHEYE_BUFFER_WIDTH_U32, height: FISHEYE_BUFFER_HEIGHT_U32, depth_or_array_layers: 1,
        };
        let intermediate_texture_size = scene_texture_size;

        let scene_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Scene Texture"), size: scene_texture_size,
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let scene_texture_view = scene_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let intermediate_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Intermediate Texture"), size: intermediate_texture_size,
            mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT
                 | wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let intermediate_texture_view = intermediate_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let scene_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Scene Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // ── Bind group layouts ──────────────────────────────────────────────────
        let compute_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compute BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 3, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 4, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 5, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
            ],
        });

        let render_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Render BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
            ],
        });

        let post_processing_uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PP Uniform BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
            ],
        });

        let post_processing_texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("PP Texture BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
            ],
        });

        let zoom_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Zoom BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
            ],
        });

        let lightning_compute_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lightning Compute BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: false }, has_dynamic_offset: false, min_binding_size: None }, count: None },
            ],
        });

        let lightning_render_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lightning Render BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 9, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 10, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 11, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
            ],
        });

        // ── Bind groups ─────────────────────────────────────────────────────────
        let compute_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Compute BG A"), layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: particle_buffers[0].as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: interaction_rules_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: sim_params_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: particle_buffers[1].as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: lightning_segments_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 5, resource: lightning_bolt_buffer.as_entire_binding() },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Compute BG B"), layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: particle_buffers[1].as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: interaction_rules_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: sim_params_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 3, resource: particle_buffers[0].as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 4, resource: lightning_segments_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 5, resource: lightning_bolt_buffer.as_entire_binding() },
                ],
            }),
        ];

        let render_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Render BG A"), layout: &render_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: particle_colors_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: sim_params_buffer.as_entire_binding() },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Render BG B"), layout: &render_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: particle_colors_buffer.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 2, resource: sim_params_buffer.as_entire_binding() },
                ],
            }),
        ];

        let background_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Background BG"), layout: &post_processing_uniform_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: sim_params_buffer.as_entire_binding() }],
        });

        let grid_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Grid BG"), layout: &post_processing_uniform_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: sim_params_buffer.as_entire_binding() }],
        });

        let fisheye_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Fisheye BG"), layout: &post_processing_texture_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: sim_params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&scene_sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&scene_texture_view) },
            ],
        });

        let lightning_compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lightning Compute BG"), layout: &lightning_compute_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: sim_params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: lightning_segments_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: lightning_bolt_buffer.as_entire_binding() },
            ],
        });

        let lightning_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lightning Render BG"), layout: &lightning_render_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: sim_params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 9, resource: lightning_segments_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 10, resource: lightning_bolt_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 11, resource: sim_params_buffer.as_entire_binding() },
            ],
        });

        #[cfg(not(target_arch = "wasm32"))]
        let initial_gamma = 1.0f32;
        #[cfg(target_arch = "wasm32")]
        let initial_gamma = 0.0f32;

        let initial_zoom_uniforms = [1.0f32, VIRTUAL_WORLD_CENTER_X, VIRTUAL_WORLD_CENTER_Y, initial_gamma];
        queue.write_buffer(&zoom_uniforms_buffer, 0, bytemuck::cast_slice(&initial_zoom_uniforms));

        let zoom_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Zoom BG"), layout: &zoom_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Sampler(&scene_sampler) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&intermediate_texture_view) },
                wgpu::BindGroupEntry { binding: 2, resource: zoom_uniforms_buffer.as_entire_binding() },
            ],
        });

        // ── Shaders ─────────────────────────────────────────────────────────────
        let vertex_shader   = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Vertex"),   source: wgpu::ShaderSource::Wgsl(include_str!("shaders/vert.wgsl").into()) });
        let fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Fragment"), source: wgpu::ShaderSource::Wgsl(include_str!("shaders/frag.wgsl").into()) });
        let glow_shader     = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Glow"),     source: wgpu::ShaderSource::Wgsl(include_str!("shaders/glow_frag.wgsl").into()) });
        let compute_shader  = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Compute"),  source: wgpu::ShaderSource::Wgsl(include_str!("shaders/compute.wgsl").into()) });
        let bg_vert         = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("BG Vert"),  source: wgpu::ShaderSource::Wgsl(include_str!("shaders/background_vert.wgsl").into()) });
        let bg_frag         = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("BG Frag"),  source: wgpu::ShaderSource::Wgsl(include_str!("shaders/background_frag.wgsl").into()) });
        let grid_frag       = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Grid"),     source: wgpu::ShaderSource::Wgsl(include_str!("shaders/grid_frag.wgsl").into()) });
        let fisheye_frag    = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Fisheye"),  source: wgpu::ShaderSource::Wgsl(include_str!("shaders/fisheye_frag.wgsl").into()) });
        let zoom_frag       = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Zoom"),     source: wgpu::ShaderSource::Wgsl(include_str!("shaders/zoom_frag.wgsl").into()) });
        let lc_shader       = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("LC"),       source: wgpu::ShaderSource::Wgsl(include_str!("shaders/lightning_compute.wgsl").into()) });
        let lv_shader       = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("LV"),       source: wgpu::ShaderSource::Wgsl(include_str!("shaders/lightning_vert.wgsl").into()) });
        let lf_shader       = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("LF"),       source: wgpu::ShaderSource::Wgsl(include_str!("shaders/lightning_frag_buffer.wgsl").into()) });

        #[cfg(not(target_arch = "wasm32"))]
        let text_vert = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Text Vert"), source: wgpu::ShaderSource::Wgsl(include_str!("shaders/text_vert.wgsl").into()) });
        #[cfg(not(target_arch = "wasm32"))]
        let text_frag = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Text Frag"), source: wgpu::ShaderSource::Wgsl(include_str!("shaders/text_overlay.wgsl").into()) });

        // ── Pipelines ────────────────────────────────────────────────────────────
        // Helper: particle vertex buffer layouts
        let particle_vbl = wgpu::VertexBufferLayout {
            array_stride: 48,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { shader_location: 0, offset: 0,  format: wgpu::VertexFormat::Float32x2 },
                wgpu::VertexAttribute { shader_location: 1, offset: 8,  format: wgpu::VertexFormat::Float32x2 },
                wgpu::VertexAttribute { shader_location: 2, offset: 16, format: wgpu::VertexFormat::Uint32 },
                wgpu::VertexAttribute { shader_location: 3, offset: 20, format: wgpu::VertexFormat::Float32 },
                wgpu::VertexAttribute { shader_location: 5, offset: 24, format: wgpu::VertexFormat::Float32 },
                wgpu::VertexAttribute { shader_location: 6, offset: 28, format: wgpu::VertexFormat::Float32 },
                wgpu::VertexAttribute { shader_location: 7, offset: 32, format: wgpu::VertexFormat::Uint32 },
                wgpu::VertexAttribute { shader_location: 8, offset: 36, format: wgpu::VertexFormat::Uint32 },
            ],
        };
        let quad_vbl = wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute { shader_location: 4, offset: 0, format: wgpu::VertexFormat::Float32x2 }],
        };

        let render_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("Render PL"), bind_group_layouts: &[&render_bind_group_layout], push_constant_ranges: &[] });
        let compute_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("Compute PL"), bind_group_layouts: &[&compute_bind_group_layout], push_constant_ranges: &[] });
        let pp_uniform_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("PP Uniform PL"), bind_group_layouts: &[&post_processing_uniform_bgl], push_constant_ranges: &[] });
        let pp_texture_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("PP Texture PL"), bind_group_layouts: &[&post_processing_texture_bgl], push_constant_ranges: &[] });
        let zoom_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("Zoom PL"), bind_group_layouts: &[&zoom_bgl], push_constant_ranges: &[] });
        let lc_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("LC PL"), bind_group_layouts: &[&lightning_compute_bgl], push_constant_ranges: &[] });
        let lr_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("LR PL"), bind_group_layouts: &[&lightning_render_bgl], push_constant_ranges: &[] });

        let alpha_blend = wgpu::BlendState::ALPHA_BLENDING;

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Render"), layout: Some(&render_pl),
            vertex: wgpu::VertexState { module: &vertex_shader, entry_point: Some("main"), buffers: &[particle_vbl.clone(), quad_vbl.clone()], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &fragment_shader, entry_point: Some("main"), targets: &[Some(wgpu::ColorTargetState { format: surface_format, blend: Some(alpha_blend), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None, multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false }, multiview: None, cache: None,
        });

        let glow_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Glow Render"), layout: Some(&render_pl),
            vertex: wgpu::VertexState { module: &vertex_shader, entry_point: Some("main"), buffers: &[particle_vbl.clone(), quad_vbl.clone()], compilation_options: wgpu::PipelineCompilationOptions { constants: &[("wobble_margin", 3.0)], ..Default::default() } },
            fragment: Some(wgpu::FragmentState { module: &glow_shader, entry_point: Some("main"), targets: &[Some(wgpu::ColorTargetState { format: surface_format, blend: Some(alpha_blend), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() },
            depth_stencil: None, multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false }, multiview: None, cache: None,
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("Particle Compute"), layout: Some(&compute_pl), module: &compute_shader, entry_point: Some("main"), compilation_options: Default::default(), cache: None });
        let lightning_compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("Lightning Compute"), layout: Some(&lc_pl), module: &lc_shader, entry_point: Some("main"), compilation_options: Default::default(), cache: None });

        let make_fullscreen_pipeline = |label, layout: &wgpu::PipelineLayout, frag, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label), layout: Some(layout),
                vertex: wgpu::VertexState { module: &bg_vert, entry_point: Some("main"), buffers: &[], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState { module: frag, entry_point: Some("main"), targets: &[Some(wgpu::ColorTargetState { format: surface_format, blend, write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
                primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
                depth_stencil: None, multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false }, multiview: None, cache: None,
            })
        };

        let background_render_pipeline = make_fullscreen_pipeline("Background", &pp_uniform_pl, &bg_frag, None);
        let grid_render_pipeline       = make_fullscreen_pipeline("Grid",       &pp_uniform_pl, &grid_frag, Some(alpha_blend));
        let fisheye_render_pipeline    = make_fullscreen_pipeline("Fisheye",    &pp_texture_pl, &fisheye_frag, None);
        let zoom_render_pipeline       = make_fullscreen_pipeline("Zoom",       &zoom_pl,       &zoom_frag, None);

        let lightning_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Lightning Render"), layout: Some(&lr_pl),
            vertex: wgpu::VertexState { module: &lv_shader, entry_point: Some("main"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState { module: &lf_shader, entry_point: Some("main"), targets: &[Some(wgpu::ColorTargetState { format: surface_format, blend: Some(alpha_blend), write_mask: wgpu::ColorWrites::ALL })], compilation_options: Default::default() }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
            depth_stencil: None, multisample: wgpu::MultisampleState { count: 1, mask: !0, alpha_to_coverage_enabled: false }, multiview: None, cache: None,
        });

        // Text overlay (native only — currently disabled)
        #[cfg(not(target_arch = "wasm32"))]
        let fps_data_buffer_raw = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("FPS Buffer"), size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        #[cfg(not(target_arch = "wasm32"))]
        let fps_data_buffer = Some(fps_data_buffer_raw);
        #[cfg(target_arch = "wasm32")]
        let fps_data_buffer: Option<wgpu::Buffer> = None;

        let text_overlay_pipeline: Option<wgpu::RenderPipeline> = None;
        let text_overlay_bind_group: Option<wgpu::BindGroup> = None;

        Ok(WebGpuRenderer {
            device,
            queue,
            surface,
            sim_params_buffer,
            particle_buffers,
            current_buffer_index: 0,
            interaction_rules_buffer,
            lightning_segments_buffer,
            lightning_bolt_buffer,
            particle_colors_buffer,
            quad_vertex_buffer,
            lightning_detector: LightningDetector::new(),
            scene_texture,
            scene_texture_view,
            intermediate_texture,
            intermediate_texture_view,
            scene_sampler,
            background_render_pipeline,
            grid_render_pipeline,
            render_pipeline,
            glow_render_pipeline,
            compute_pipeline,
            lightning_compute_pipeline,
            lightning_render_pipeline,
            fisheye_render_pipeline,
            zoom_render_pipeline,
            compute_bind_groups,
            render_bind_groups,
            lightning_compute_bind_group,
            lightning_render_bind_group,
            background_render_bind_group,
            grid_render_bind_group,
            fisheye_render_bind_group,
            zoom_render_bind_group,
            zoom_uniforms_buffer,
            fps_data_buffer,
            text_overlay_bind_group,
            text_overlay_pipeline,
        })
    }

    /// Render a frame using WebGPU
    pub fn render(
        &mut self,
        particle_system: &ParticleSystem,
        simulation_params: &SimulationParams,
        interaction_rules: &InteractionRules,
        lightning_segments_data: &[u8],
        lightning_bolts_data: &[u8],
    ) -> Result<(), RendererError> {
        let actual_particle_count = particle_system.get_active_count();
        let sim_params_data = simulation_params.to_buffer_with_particle_count_and_zoom(
            actual_particle_count, simulation_params.current_zoom_level);
        self.queue.write_buffer(&self.sim_params_buffer, 0, &sim_params_data);
        self.queue.write_buffer(&self.interaction_rules_buffer, 0, &interaction_rules.to_buffer());
        self.queue.write_buffer(&self.particle_colors_buffer, 0, &particle_system.get_colors_buffer());

        if !lightning_segments_data.is_empty() {
            self.queue.write_buffer(&self.lightning_segments_buffer, 0, lightning_segments_data);
        }
        if !lightning_bolts_data.is_empty() {
            self.queue.write_buffer(&self.lightning_bolt_buffer, 0, lightning_bolts_data);
        }

        self.update_zoom_uniforms(simulation_params);

        let output = self.surface.get_current_texture()
            .map_err(|e| renderer_error("Failed to get surface texture", e))?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor { label: Some("Frame Encoder") });

        // Lightning compute
        { let mut p = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("Lightning Compute"), timestamp_writes: None });
          p.set_pipeline(&self.lightning_compute_pipeline);
          p.set_bind_group(0, &self.lightning_compute_bind_group, &[]);
          p.dispatch_workgroups(1, 1, 1); }

        // Physics compute
        { let mut p = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("Particle Compute"), timestamp_writes: None });
          p.set_pipeline(&self.compute_pipeline);
          p.set_bind_group(0, &self.compute_bind_groups[self.current_buffer_index], &[]);
          let n = particle_system.get_active_count();
          p.dispatch_workgroups((n + 63) / 64, 1, 1); }

        let out_idx = 1 - self.current_buffer_index;

        // Background
        { let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("Background"), color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &self.scene_texture_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })], depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None });
          p.set_pipeline(&self.background_render_pipeline); p.set_bind_group(0, &self.background_render_bind_group, &[]); p.draw(0..6, 0..1); }

        // Grid
        { let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("Grid"), color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &self.scene_texture_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })], depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None });
          p.set_pipeline(&self.grid_render_pipeline); p.set_bind_group(0, &self.grid_render_bind_group, &[]); p.draw(0..6, 0..1); }

        // Glow
        { let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("Glow"), color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &self.scene_texture_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })], depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None });
          p.set_pipeline(&self.glow_render_pipeline); p.set_bind_group(0, &self.render_bind_groups[out_idx], &[]);
          p.set_vertex_buffer(0, self.particle_buffers[out_idx].slice(..)); p.set_vertex_buffer(1, self.quad_vertex_buffer.slice(..));
          p.draw(0..4, 0..particle_system.get_active_count()); }

        // Particles
        { let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("Particles"), color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &self.scene_texture_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })], depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None });
          p.set_pipeline(&self.render_pipeline); p.set_bind_group(0, &self.render_bind_groups[out_idx], &[]);
          p.set_vertex_buffer(0, self.particle_buffers[out_idx].slice(..)); p.set_vertex_buffer(1, self.quad_vertex_buffer.slice(..));
          p.draw(0..4, 0..particle_system.get_active_count()); }

        // Lightning
        { let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("Lightning"), color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &self.scene_texture_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })], depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None });
          p.set_pipeline(&self.lightning_render_pipeline); p.set_bind_group(0, &self.lightning_render_bind_group, &[]); p.draw(0..6, 0..1); }

        // Fisheye
        if simulation_params.fisheye_strength != 0.0 {
            let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("Fisheye"), color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &self.intermediate_texture_view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })], depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None });
            p.set_pipeline(&self.fisheye_render_pipeline); p.set_bind_group(0, &self.fisheye_render_bind_group, &[]); p.draw(0..6, 0..1);
        } else {
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo { texture: &self.scene_texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                wgpu::TexelCopyTextureInfo { texture: &self.intermediate_texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                self.scene_texture.size(),
            );
        }

        // Zoom
        { let mut p = encoder.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("Zoom"), color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })], depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None });
          p.set_pipeline(&self.zoom_render_pipeline); p.set_bind_group(0, &self.zoom_render_bind_group, &[]); p.draw(0..6, 0..1); }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        self.current_buffer_index = 1 - self.current_buffer_index;
        Ok(())
    }

    pub fn update_zoom_uniforms(&mut self, simulation_params: &SimulationParams) {
        let zoom_level = simulation_params.virtual_world_width / simulation_params.viewport_width;
        let center_x = simulation_params.virtual_world_offset_x + simulation_params.viewport_width / 2.0;
        let center_y = simulation_params.virtual_world_offset_y + simulation_params.viewport_height / 2.0;

        #[cfg(target_arch = "wasm32")]
        let gamma = 0.0f32;
        #[cfg(not(target_arch = "wasm32"))]
        let gamma = 1.0f32;

        let zoom_uniforms = [zoom_level, center_x, center_y, gamma,
            simulation_params.virtual_world_width, simulation_params.virtual_world_height,
            simulation_params.canvas_render_width, simulation_params.canvas_render_height];
        self.queue.write_buffer(&self.zoom_uniforms_buffer, 0, bytemuck::cast_slice(&zoom_uniforms));
    }

    pub fn initialize_particle_buffers(&mut self, particle_system: &ParticleSystem) {
        let data = particle_system.to_buffer();
        self.queue.write_buffer(&self.particle_buffers[0], 0, &data);
        self.queue.write_buffer(&self.particle_buffers[1], 0, &data);
    }

    pub fn update_particle_active_states(&mut self, particle_system: &ParticleSystem) {
        for i in 0..particle_system.get_max_particles() as usize {
            let is_active = if i < particle_system.get_active_count() as usize { 1u32 } else { 0u32 };
            self.queue.write_buffer(&self.particle_buffers[self.current_buffer_index], (i * 48 + 36) as u64, &is_active.to_le_bytes());
        }
    }

    pub fn update_particle_sizes(&mut self, particle_system: &ParticleSystem) {
        for i in 0..particle_system.get_max_particles() as usize {
            if let Some(p) = particle_system.get_particle(i) {
                let base = i * 48;
                self.queue.write_buffer(&self.particle_buffers[self.current_buffer_index], (base + 20) as u64, &p.size.to_le_bytes());
                self.queue.write_buffer(&self.particle_buffers[self.current_buffer_index], (base + 24) as u64, &p.target_size.to_le_bytes());
            }
        }
    }

    pub fn update_particle_transitions(&mut self, particle_system: &ParticleSystem) {
        for i in 0..particle_system.get_max_particles() as usize {
            if let Some(p) = particle_system.get_particle(i) {
                if p.transition_start > 0.0 {
                    let base = i * 48;
                    self.queue.write_buffer(&self.particle_buffers[self.current_buffer_index], (base + 28) as u64, &p.transition_start.to_le_bytes());
                    self.queue.write_buffer(&self.particle_buffers[self.current_buffer_index], (base + 32) as u64, &p.transition_type.to_le_bytes());
                }
            }
        }
    }

    pub async fn get_lightning_status(&self) -> Result<LightningBolt, RendererError> {
        self.read_lightning_bolt_data().await
    }

    pub async fn read_lightning_bolt_data(&self) -> Result<LightningBolt, RendererError> {
        let size = std::mem::size_of::<LightningBolt>() as u64;
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lightning Staging"), size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Lightning Copy") });
        encoder.copy_buffer_to_buffer(&self.lightning_bolt_buffer, 0, &staging, 0, size);
        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = staging.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).ok(); });
        self.device.poll(wgpu::MaintainBase::Wait).map_err(|e| renderer_error("Poll failed", e))?;
        rx.await.map_err(|_| renderer_error("Channel closed", ""))?.map_err(|e| renderer_error("Map failed", e))?;

        let data = slice.get_mapped_range();
        let bolt = *bytemuck::from_bytes::<LightningBolt>(&data[..std::mem::size_of::<LightningBolt>()]);
        drop(data);
        staging.unmap();
        Ok(bolt)
    }

    pub fn poll_lightning_events(&mut self) -> Vec<LightningEvent> { self.lightning_detector.poll_events() }
    pub fn get_cached_lightning_bolt(&self) -> Option<LightningBolt> { self.lightning_detector.get_cached_lightning_bolt() }

    pub fn update_lightning_detection(&mut self, _sim_params: &SimulationParams) {
        if self.lightning_detector.frame_counter % 300 == 0 {
            if let Some(t) = self.lightning_detector.time_until_next_poll() {
                crate::console_log!("⏰ Lightning: volgende poll over {:.1}s", t / 1000.0);
            }
        }
    }

    pub async fn update_lightning_cache(&mut self) -> Result<(), RendererError> {
        let bolt = self.get_lightning_status().await?;
        self.lightning_detector.process_lightning_bolt(&bolt);
        Ok(())
    }

    pub fn get_device(&self) -> &wgpu::Device { &self.device }

    /// Geeft een referentie naar de queue (nodig voor stats dispatch in native_minimal)
    pub fn queue(&self) -> &wgpu::Queue { &self.queue }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn update_fps_data(&mut self, fps: f32, frame_count: u32, particle_count: u32, time: f32) {
        if let Some(b) = &self.fps_data_buffer {
            self.queue.write_buffer(b, 0, bytemuck::cast_slice(&[fps, frame_count as f32, particle_count as f32, time]));
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn update_fps_data(&mut self, _fps: f32, _frame_count: u32, _particle_count: u32, _time: f32) {}
}

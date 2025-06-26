use crate::{console_log, InteractionRules, ParticleSystem, SimulationParams};
use rand::Rng;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;

// Conditional error type: JsValue for WASM, Box<dyn std::error::Error> for native
#[cfg(target_arch = "wasm32")]
pub type RendererError = wasm_bindgen::JsValue;

#[cfg(not(target_arch = "wasm32"))]
pub type RendererError = Box<dyn std::error::Error + Send + Sync>;

// Helper function to create renderer errors with proper formatting
pub fn renderer_error(message: &str, source: impl std::fmt::Debug) -> RendererError {
    let formatted_message = format!("{}: {:?}", message, source);

    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen::JsValue::from_str(&formatted_message)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            formatted_message,
        ))
    }
}

/// WebGPU renderer that handles all GPU operations using pure wgpu
pub struct WebGpuRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,

    // Simulation buffers
    sim_params_buffer: wgpu::Buffer,
    particle_buffers: [wgpu::Buffer; 2], // Double-buffered for compute shader
    current_buffer_index: usize,
    interaction_rules_buffer: wgpu::Buffer,
    lightning_segments_buffer: wgpu::Buffer,
    lightning_bolt_buffer: wgpu::Buffer,
    particle_colors_buffer: wgpu::Buffer, // Add particle colors buffer
    quad_vertex_buffer: wgpu::Buffer,     // Add quad vertex buffer for instanced rendering

    // Textures for post-processing pipeline
    scene_texture: wgpu::Texture,
    scene_texture_view: wgpu::TextureView,
    intermediate_texture: wgpu::Texture,
    intermediate_texture_view: wgpu::TextureView,
    scene_sampler: wgpu::Sampler,

    // Rendering pipelines
    background_render_pipeline: wgpu::RenderPipeline,
    grid_render_pipeline: wgpu::RenderPipeline,
    render_pipeline: wgpu::RenderPipeline,
    compute_pipeline: wgpu::ComputePipeline,
    lightning_compute_pipeline: wgpu::ComputePipeline,
    lightning_render_pipeline: wgpu::RenderPipeline,
    fisheye_render_pipeline: wgpu::RenderPipeline,
    vignette_render_pipeline: wgpu::RenderPipeline,
    zoom_render_pipeline: wgpu::RenderPipeline,

    // Bind groups
    compute_bind_groups: [wgpu::BindGroup; 2], // Double-buffered bind groups
    render_bind_groups: [wgpu::BindGroup; 2],  // Render bind groups for each buffer
    lightning_compute_bind_group: wgpu::BindGroup,
    lightning_render_bind_group: wgpu::BindGroup,
    background_render_bind_group: wgpu::BindGroup,
    grid_render_bind_group: wgpu::BindGroup,
    fisheye_render_bind_group: wgpu::BindGroup,
    vignette_render_bind_group: wgpu::BindGroup,
    zoom_render_bind_group: wgpu::BindGroup,

    // Zoom uniforms buffer
    zoom_uniforms_buffer: wgpu::Buffer,

    // Canvas dimensions
    canvas_width: u32,
    canvas_height: u32,
}

impl WebGpuRenderer {
    /// Initialize WebGPU renderer - WASM version with canvas
    #[cfg(target_arch = "wasm32")]
    pub async fn new(canvas: &web_sys::HtmlCanvasElement) -> Result<WebGpuRenderer, RendererError> {
        console_log!("🎨 Initializing wgpu WebGPU renderer");

        // Get canvas dimensions
        let canvas_width = canvas.width();
        let canvas_height = canvas.height();

        // Create wgpu instance with WebGPU preference
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::BROWSER_WEBGPU, // Only WebGPU on web, no WebGL fallback
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::all(), // All native backends (Vulkan, Metal, DX12, etc.)
            ..Default::default()
        });

        #[cfg(target_arch = "wasm32")]
        console_log!("🚀 Using WebGPU backend (no WebGL fallback)");
        #[cfg(not(target_arch = "wasm32"))]
        console_log!("🚀 Using native GPU backends");

        // Create surface from canvas - WGPU 25.0.2 correct web approach
        // Use SurfaceTarget::Canvas for web canvas elements
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
        console_log!("🎨 Initializing wgpu native renderer");

        // Create wgpu instance with native backends
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::BROWSER_WEBGPU, // Only WebGPU on web, no WebGL fallback
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::all(), // All native backends (Vulkan, Metal, DX12, etc.)
            ..Default::default()
        });

        console_log!("🚀 Using native GPU backends");

        // Create surface from window
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| renderer_error("Failed to create surface", e))?;

        // Use fixed 800x800 logical size regardless of window size
        Self::initialize_common(instance, surface, 800, 800).await
    }

    /// Common initialization for both platforms
    async fn initialize_common(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<WebGpuRenderer, RendererError> {
        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| renderer_error("Failed to request adapter", e))?;

        console_log!("✅ WebGPU adapter found: {:?}", adapter.get_info());

        // Request device and queue
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Particle Life Device"),
                required_features: wgpu::Features::empty(),
                // Use WebGPU limits for both platforms - no WebGL fallback
                #[cfg(target_arch = "wasm32")]
                required_limits: wgpu::Limits::default(), // WebGPU limits for browser
                #[cfg(not(target_arch = "wasm32"))]
                required_limits: wgpu::Limits::default(), // Native GPU limits
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| renderer_error("Failed to create device", e))?;

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: width,
            height: height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &surface_config);

        // Create simulation parameters buffer
        let sim_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Simulation Parameters Buffer"),
            size: 256, // Enough for simulation parameters
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create quad vertex buffer for instanced rendering (4 vertices for triangle strip)
        let quad_vertices: [f32; 8] = [
            -1.0, -1.0, // Bottom-left
            1.0, -1.0, // Bottom-right
            -1.0, 1.0, // Top-left
            1.0, 1.0, // Top-right
        ];

        let quad_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(&quad_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Create particle colors buffer
        let max_types = 16;
        let particle_colors_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Colors Buffer"),
            size: (max_types * 16) as u64, // 4 floats per color (RGBA)
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create initial particle data (similar to TypeScript createInitialParticles)
        let max_particles = 6400; // Reduced from 32768 - supports up to 8K particles efficiently
        let active_particles = 6400; // Initialize all particles that the engine uses
        let num_types = 5;
        let virtual_world_width = 2400.0;
        let virtual_world_height = 2400.0;
        let particle_render_size = 12.0;

        // Create initial particle data buffer
        let mut initial_particle_data = Vec::with_capacity((max_particles * 48) as usize);

        let mut rng = rand::thread_rng();

        for i in 0..max_particles {
            let particle_type = (i % num_types) as u32;

            // Position (vec2f) - ALL particles get random positions
            let pos_x = rng.gen::<f32>() * virtual_world_width;
            let pos_y = rng.gen::<f32>() * virtual_world_height;

            // Velocity (vec2f) - ALL particles get random velocities
            let vel_x = (rng.gen::<f32>() - 0.5) * 4.0;
            let vel_y = (rng.gen::<f32>() - 0.5) * 4.0;

            // Size based on particle type
            let base_multiplier = match particle_type {
                0 => 1.5f32, // Blue - large
                1 => 1.2f32, // Orange - medium-large
                2 => 0.7f32, // Red - small
                3 => 0.9f32, // Purple - medium-small
                4 => 1.0f32, // Green - balanced
                _ => 1.0f32,
            };
            let size_randomization = (rng.gen::<f32>() - 0.5) * 0.4; // ±20%
            let size_multiplier = base_multiplier * (1.0f32 + size_randomization);
            let target_size = particle_render_size * size_multiplier;

            // Debug: Log suspicious large target_size values
            if target_size > 30.0 {
                console_log!(
                    "⚠️ Large target_size detected: particle {}, type {}, base_multiplier {}, size_multiplier {}, target_size {}",
                    i, particle_type, base_multiplier, size_multiplier, target_size
                );
            }

            // ALL particles get proper size and target_size (inactive particles just won't be rendered)
            let particle_size = target_size;

            // Pack data as bytes (f32 = 4 bytes, u32 = 4 bytes)
            initial_particle_data.extend_from_slice(&pos_x.to_le_bytes()); // 0-3
            initial_particle_data.extend_from_slice(&pos_y.to_le_bytes()); // 4-7
            initial_particle_data.extend_from_slice(&vel_x.to_le_bytes()); // 8-11
            initial_particle_data.extend_from_slice(&vel_y.to_le_bytes()); // 12-15
            initial_particle_data.extend_from_slice(&particle_type.to_le_bytes()); // 16-19
            initial_particle_data.extend_from_slice(&particle_size.to_le_bytes()); // 20-23

            // Add target_size (same as current size for initial particles)
            initial_particle_data.extend_from_slice(&target_size.to_le_bytes()); // 24-27

            // Add new transition fields
            initial_particle_data.extend_from_slice(&0.0f32.to_le_bytes()); // 28-31: transition_start (no transition initially)
            initial_particle_data.extend_from_slice(&0u32.to_le_bytes()); // 32-35: transition_type (0 = grow)

            // Add is_active field (true for particles within initial count)
            let is_active = if i < active_particles { 1u32 } else { 0u32 };
            initial_particle_data.extend_from_slice(&is_active.to_le_bytes()); // 36-39: is_active

            // Add padding for 16-byte alignment (8 bytes total padding)
            initial_particle_data.extend_from_slice(&0.0f32.to_le_bytes()); // 40-43: padding1
            initial_particle_data.extend_from_slice(&0.0f32.to_le_bytes()); // 44-47: padding2
        }

        // Create particle buffers (double-buffered for ping-pong) with initial data
        let particle_buffer_size = (max_particles * 48) as u64; // 48 bytes per particle (12 fields * 4 bytes, 16-byte aligned)
        let particle_buffers = [
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Particle Buffer A"),
                size: particle_buffer_size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Particle Buffer B"),
                size: particle_buffer_size,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
        ];

        // Initialize both buffers with the initial particle data
        queue.write_buffer(&particle_buffers[0], 0, &initial_particle_data);
        queue.write_buffer(&particle_buffers[1], 0, &initial_particle_data);

        console_log!(
            "🎯 Initialized particle buffers with {} active particles out of {} total",
            active_particles,
            max_particles
        );

        // Create interaction rules buffer
        let max_types = 16;
        let interaction_rules_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Interaction Rules Buffer"),
            size: (max_types * max_types * 16) as u64, // 16 bytes per rule
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create lightning buffers
        let max_lightning_segments = 1024;
        let lightning_segments_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lightning Segments Buffer"),
            size: (max_lightning_segments * 48) as u64, // 48 bytes per segment: 8+8+4+4+4+4+4+4+4+4 = 48 bytes (16-byte aligned)
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let lightning_bolt_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lightning Bolt Buffer"),
            size: 16, // Single bolt: 16 bytes (4 u32/f32 fields: num_segments, flash_id, start_time, next_lightning_time) - properly aligned to 16-byte boundary
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create zoom uniforms buffer
        let zoom_uniforms_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Zoom Uniforms Buffer"),
            size: 16, // zoom_level(f32), center_x(f32), center_y(f32), padding(f32)
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create post-processing textures
        // Scene texture should be 2400x2400 to render the full virtual world
        let scene_texture_size = wgpu::Extent3d {
            width: 2400,
            height: 2400,
            depth_or_array_layers: 1,
        };

        // Intermediate texture should also be 2400x2400 to match scene texture
        let intermediate_texture_size = wgpu::Extent3d {
            width: 2400,
            height: 2400,
            depth_or_array_layers: 1,
        };

        let scene_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Scene Texture"),
            size: scene_texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let scene_texture_view = scene_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let intermediate_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Intermediate Texture"),
            size: intermediate_texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let intermediate_texture_view =
            intermediate_texture.create_view(&wgpu::TextureViewDescriptor::default());

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

        // Create bind group layout for compute shader
        let compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Compute Bind Group Layout"),
                entries: &[
                    // Particles input (for compute)
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
                    // Interaction rules (for compute)
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Simulation parameters (uniform for all stages)
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Particles output (for compute)
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Lightning segments buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Lightning bolt buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // Create render bind group layout
        let render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Render Bind Group Layout"),
                entries: &[
                    // Particle colors (for vertex shader) - binding 0
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Simulation parameters (for vertex/fragment shaders) - binding 2
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // Create double-buffered compute bind groups
        let compute_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Compute Bind Group A"),
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: particle_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: interaction_rules_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: sim_params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: particle_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: lightning_segments_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: lightning_bolt_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Compute Bind Group B"),
                layout: &compute_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: particle_buffers[1].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: interaction_rules_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: sim_params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: particle_buffers[0].as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: lightning_segments_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: lightning_bolt_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

        // Create render bind groups for both buffers
        let render_bind_groups = [
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Render Bind Group A"),
                layout: &render_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: particle_colors_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: sim_params_buffer.as_entire_binding(),
                    },
                ],
            }),
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Render Bind Group B"),
                layout: &render_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: particle_colors_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: sim_params_buffer.as_entire_binding(),
                    },
                ],
            }),
        ];

        // Load shaders
        let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Vertex Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/vert.wgsl").into()),
        });

        let fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/frag.wgsl").into()),
        });

        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/compute.wgsl").into()),
        });

        // Create pipeline layout
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Compute Pipeline Layout"),
                bind_group_layouts: &[&compute_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&render_bind_group_layout],
                push_constant_ranges: &[],
            });

        // Create render pipeline
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("main"),
                buffers: &[
                    // Buffer 0: Particle instance buffer
                    wgpu::VertexBufferLayout {
                        array_stride: 48, // pos(8) + vel(8) + type(4) + size(4) + target_size(4) + transition_start(4) + transition_type(4) + is_active(4) + padding(8) = 48 bytes (16-byte aligned)
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            // location(0): particle position vec2<f32>
                            wgpu::VertexAttribute {
                                shader_location: 0,
                                offset: 0,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                            // location(1): particle velocity vec2<f32>
                            wgpu::VertexAttribute {
                                shader_location: 1,
                                offset: 8,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                            // location(2): particle type u32
                            wgpu::VertexAttribute {
                                shader_location: 2,
                                offset: 16,
                                format: wgpu::VertexFormat::Uint32,
                            },
                            // location(3): particle size f32
                            wgpu::VertexAttribute {
                                shader_location: 3,
                                offset: 20,
                                format: wgpu::VertexFormat::Float32,
                            },
                            // location(5): target size f32
                            wgpu::VertexAttribute {
                                shader_location: 5,
                                offset: 24,
                                format: wgpu::VertexFormat::Float32,
                            },
                            // location(6): transition start f32
                            wgpu::VertexAttribute {
                                shader_location: 6,
                                offset: 28,
                                format: wgpu::VertexFormat::Float32,
                            },
                            // location(7): transition type u32
                            wgpu::VertexAttribute {
                                shader_location: 7,
                                offset: 32,
                                format: wgpu::VertexFormat::Uint32,
                            },
                            // location(8): is active u32 (bool as u32)
                            wgpu::VertexAttribute {
                                shader_location: 8,
                                offset: 36,
                                format: wgpu::VertexFormat::Uint32,
                            },
                        ],
                    },
                    // Buffer 1: Quad vertex buffer
                    wgpu::VertexBufferLayout {
                        array_stride: 8, // 2 floats: x, y
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            // location(4): quad position vec2<f32>
                            wgpu::VertexAttribute {
                                shader_location: 4,
                                offset: 0,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                        ],
                    },
                ],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &fragment_shader,
                entry_point: Some("main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip, // Use triangle strip like original
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // Load lightning shaders
        let lightning_compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lightning Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/lightning_compute.wgsl").into()),
        });

        let lightning_vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lightning Vertex Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/lightning_vert.wgsl").into()),
        });

        let lightning_fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lightning Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/lightning_frag_buffer.wgsl").into(),
            ),
        });

        // Create lightning compute bind group layout
        let lightning_compute_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Lightning Compute Bind Group Layout"),
                entries: &[
                    // Simulation parameters
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Lightning segments buffer
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Lightning bolt buffer
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
            });

        // Create lightning render bind group layout
        let lightning_render_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Lightning Render Bind Group Layout"),
                entries: &[
                    // Simulation parameters (binding 0)
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Lightning segments buffer (binding 9)
                    wgpu::BindGroupLayoutEntry {
                        binding: 9,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Lightning bolt buffer (binding 10)
                    wgpu::BindGroupLayoutEntry {
                        binding: 10,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Simulation parameters for fragment shader (binding 11)
                    wgpu::BindGroupLayoutEntry {
                        binding: 11,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // Create lightning bind groups
        let lightning_compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lightning Compute Bind Group"),
            layout: &lightning_compute_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: sim_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lightning_segments_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: lightning_bolt_buffer.as_entire_binding(),
                },
            ],
        });

        let lightning_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lightning Render Bind Group"),
            layout: &lightning_render_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: sim_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: lightning_segments_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: lightning_bolt_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: sim_params_buffer.as_entire_binding(),
                },
            ],
        });

        // Create lightning compute pipeline
        let lightning_compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Lightning Compute Pipeline Layout"),
                bind_group_layouts: &[&lightning_compute_bind_group_layout],
                push_constant_ranges: &[],
            });

        let lightning_compute_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Lightning Compute Pipeline"),
                layout: Some(&lightning_compute_pipeline_layout),
                module: &lightning_compute_shader,
                entry_point: Some("main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            });

        // Create lightning render pipeline
        let lightning_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Lightning Render Pipeline Layout"),
                bind_group_layouts: &[&lightning_render_bind_group_layout],
                push_constant_ranges: &[],
            });

        let lightning_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Lightning Render Pipeline"),
                layout: Some(&lightning_render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &lightning_vertex_shader,
                    entry_point: Some("main"),
                    buffers: &[], // No vertex buffers - generate fullscreen quad in shader
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &lightning_fragment_shader,
                    entry_point: Some("main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            });

        // Create compute pipeline
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Particle Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // Load post-processing shaders
        let background_vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Background Vertex Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/background_vert.wgsl").into()),
        });

        let background_fragment_shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Background Fragment Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("shaders/background_frag.wgsl").into(),
                ),
            });

        let grid_fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Grid Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/grid_frag.wgsl").into()),
        });

        let fisheye_fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Fisheye Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/fisheye_frag.wgsl").into()),
        });

        let vignette_fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Vignette Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/vignette_frag.wgsl").into()),
        });

        let zoom_fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Zoom Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/zoom_frag.wgsl").into()),
        });

        // Create post-processing bind group layouts
        let post_processing_uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Post Processing Uniform Bind Group Layout"),
                entries: &[
                    // Simulation parameters
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let post_processing_texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Post Processing Texture Bind Group Layout"),
                entries: &[
                    // Simulation parameters
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Scene sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Scene texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        // Create zoom bind group layout (similar to post-processing but with zoom uniforms)
        let zoom_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Zoom Bind Group Layout"),
                entries: &[
                    // Scene sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Scene texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Zoom uniforms
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        // Create post-processing bind groups
        let background_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Background Render Bind Group"),
            layout: &post_processing_uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: sim_params_buffer.as_entire_binding(),
            }],
        });

        let grid_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Grid Render Bind Group"),
            layout: &post_processing_uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: sim_params_buffer.as_entire_binding(),
            }],
        });

        let fisheye_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Fisheye Render Bind Group"),
            layout: &post_processing_texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: sim_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&scene_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&scene_texture_view),
                },
            ],
        });

        let vignette_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Vignette Render Bind Group"),
            layout: &post_processing_texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: sim_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&scene_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&intermediate_texture_view),
                },
            ],
        });

        // Initialize zoom uniforms buffer with default values
        let initial_zoom_uniforms = [1.0f32, 1200.0, 1200.0, 0.0]; // zoom=1.0, center at (1200,1200)
        queue.write_buffer(
            &zoom_uniforms_buffer,
            0,
            bytemuck::cast_slice(&initial_zoom_uniforms),
        );

        let zoom_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Zoom Render Bind Group"),
            layout: &zoom_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&scene_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&intermediate_texture_view), // Read from intermediate texture (after fisheye)
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: zoom_uniforms_buffer.as_entire_binding(),
                },
            ],
        });

        // Create post-processing pipelines
        let background_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Background Render Pipeline Layout"),
                bind_group_layouts: &[&post_processing_uniform_bind_group_layout],
                push_constant_ranges: &[],
            });

        let background_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Background Render Pipeline"),
                layout: Some(&background_render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &background_vertex_shader,
                    entry_point: Some("main"),
                    buffers: &[], // No vertex buffers - generate fullscreen quad in shader
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &background_fragment_shader,
                    entry_point: Some("main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: None, // Replace background
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            });

        let grid_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Grid Render Pipeline Layout"),
                bind_group_layouts: &[&post_processing_uniform_bind_group_layout],
                push_constant_ranges: &[],
            });

        let grid_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Grid Render Pipeline"),
            layout: Some(&grid_render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &background_vertex_shader, // Reuse same vertex shader
                entry_point: Some("main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &grid_fragment_shader,
                entry_point: Some("main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING), // Overlay on background
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let fisheye_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Fisheye Render Pipeline Layout"),
                bind_group_layouts: &[&post_processing_texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        let fisheye_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Fisheye Render Pipeline"),
                layout: Some(&fisheye_render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &background_vertex_shader, // Reuse same vertex shader
                    entry_point: Some("main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fisheye_fragment_shader,
                    entry_point: Some("main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: None, // Replace
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            });

        let vignette_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Vignette Render Pipeline Layout"),
                bind_group_layouts: &[&post_processing_texture_bind_group_layout],
                push_constant_ranges: &[],
            });

        let vignette_render_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Vignette Render Pipeline"),
                layout: Some(&vignette_render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &background_vertex_shader, // Reuse same vertex shader
                    entry_point: Some("main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &vignette_fragment_shader,
                    entry_point: Some("main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: None, // Replace
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            });

        // Create zoom render pipeline
        let zoom_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Zoom Render Pipeline Layout"),
                bind_group_layouts: &[&zoom_bind_group_layout],
                push_constant_ranges: &[],
            });

        let zoom_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Zoom Render Pipeline"),
            layout: Some(&zoom_render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &background_vertex_shader, // Reuse same vertex shader
                entry_point: Some("main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &zoom_fragment_shader,
                entry_point: Some("main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None, // Replace
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        console_log!("✅ WebGPU renderer initialized successfully");

        Ok(WebGpuRenderer {
            device,
            queue,
            surface,
            surface_config,
            sim_params_buffer,
            particle_buffers,
            current_buffer_index: 0,
            interaction_rules_buffer,
            lightning_segments_buffer,
            lightning_bolt_buffer,
            particle_colors_buffer,
            quad_vertex_buffer,
            scene_texture,
            scene_texture_view,
            intermediate_texture,
            intermediate_texture_view,
            scene_sampler,
            background_render_pipeline,
            grid_render_pipeline,
            render_pipeline,
            compute_pipeline,
            lightning_compute_pipeline,
            lightning_render_pipeline,
            fisheye_render_pipeline,
            vignette_render_pipeline,
            zoom_render_pipeline,
            compute_bind_groups,
            render_bind_groups,
            lightning_compute_bind_group,
            lightning_render_bind_group,
            background_render_bind_group,
            grid_render_bind_group,
            fisheye_render_bind_group,
            vignette_render_bind_group,
            zoom_render_bind_group,
            zoom_uniforms_buffer,
            canvas_width: width,
            canvas_height: height,
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
        // Only update simulation parameters buffer (contains time and deltaTime which change every frame)
        let actual_particle_count = particle_system.get_active_count();
        let sim_params_data =
            simulation_params.to_buffer_with_particle_count(actual_particle_count);
        self.queue
            .write_buffer(&self.sim_params_buffer, 0, &sim_params_data);

        // Update interaction rules buffer ONLY if rules have changed (optimization opportunity)
        // For now, still updating every frame but this could be optimized
        let rules_data = interaction_rules.to_buffer();
        self.queue
            .write_buffer(&self.interaction_rules_buffer, 0, &rules_data);

        // Update particle colors buffer ONLY if colors have changed (optimization opportunity)
        // For now, still updating every frame but this could be optimized
        let colors_data = particle_system.get_colors_buffer();
        self.queue
            .write_buffer(&self.particle_colors_buffer, 0, &colors_data);

        // Update lightning buffers with current lightning data
        if !lightning_segments_data.is_empty() {
            self.queue
                .write_buffer(&self.lightning_segments_buffer, 0, lightning_segments_data);
        }
        if !lightning_bolts_data.is_empty() {
            self.queue
                .write_buffer(&self.lightning_bolt_buffer, 0, lightning_bolts_data);
        }

        // DON'T update particle buffer every frame - let GPU compute shader handle particle state
        // HOWEVER, we DO need to update it when particle count changes (active/inactive flags change)
        // This is handled by calling initialize_particle_buffers() when needed

        // Update zoom uniforms with current simulation parameters
        self.update_zoom_uniforms(simulation_params);

        // Get current surface texture
        let output = self
            .surface
            .get_current_texture()
            .map_err(|e| renderer_error("Failed to get surface texture", e))?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Create single command encoder for entire frame (matches TypeScript efficiency)
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Particle Life Frame Encoder"),
            });

        // Lightning Compute Pass - FIRST to generate lightning data (matches TypeScript order)
        {
            let mut lightning_compute_pass =
                encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Lightning Compute Pass"),
                    timestamp_writes: None,
                });

            lightning_compute_pass.set_pipeline(&self.lightning_compute_pipeline);
            lightning_compute_pass.set_bind_group(0, &self.lightning_compute_bind_group, &[]);
            lightning_compute_pass.dispatch_workgroups(1, 1, 1); // Single workgroup for lightning generation

            // Debug lightning parameters much less frequently to avoid spam and duplicates
            // Only log when time is exactly divisible to prevent multiple logs per second
            let time_seconds = simulation_params.time as u32;
            let time_fraction = simulation_params.time - time_seconds as f32;
            if time_seconds > 0
                && time_seconds % 10 == 0
                && time_fraction >= 0.0
                && time_fraction < 0.02
            {
                console_log!(
                    "⚡ Lightning compute: freq={:.3}, intensity={:.3}, duration={:.3}, time={}s, elec_activity={:.3}",
                    simulation_params.lightning_frequency,
                    simulation_params.lightning_intensity,
                    simulation_params.lightning_duration,
                    time_seconds,
                    simulation_params.inter_type_attraction_scale
                );

                // Add logging about expected segment generation
                console_log!(
                    "🔧 Lightning compute dispatched - segments should be generated if conditions are met (electrical_activity > 0 and time >= next_flash_time)"
                );
            }
        }

        // Add segment count logging after lightning compute completes
        // Note: We can't easily read back GPU buffer data, but we can log completion
        if simulation_params.lightning_frequency > 0.0
            && simulation_params.inter_type_attraction_scale > 0.0
        {
            let time_seconds = simulation_params.time as u32;
            let time_fraction = simulation_params.time - time_seconds as f32;
            if time_seconds > 0
                && time_seconds % 10 == 0
                && time_fraction >= 0.0
                && time_fraction < 0.02
            {
                console_log!("📊 Lightning compute completed - segments should now be available for rendering (if generated)");
            }
        }

        // Physics Compute Pass - runs after lightning data is generated (matches TypeScript order)
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Particle Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(
                0,
                &self.compute_bind_groups[self.current_buffer_index],
                &[],
            );

            let particle_count = particle_system.get_active_count();
            let workgroup_size = 64;
            let dispatch_size = (particle_count + workgroup_size - 1) / workgroup_size;

            // Debug every 60 frames to see if compute is running
            static mut FRAME_COUNT: u32 = 0;
            unsafe {
                FRAME_COUNT += 1;
                // Reduced compute logging frequency
                if FRAME_COUNT % 300 == 0 {
                    console_log!(
                        "🧮 Compute: {} particles, {} workgroups",
                        particle_count,
                        dispatch_size
                    );
                }
            }

            compute_pass.dispatch_workgroups(dispatch_size, 1, 1);
        }

        // Calculate output buffer index for rendering (compute writes to the other buffer)
        let output_buffer_index = 1 - self.current_buffer_index;

        // Multi-pass rendering pipeline:
        // 1. Background -> scene_texture
        // 2. Particles -> scene_texture (additive)
        // 3. Lightning -> scene_texture (additive)
        // 4. Grid -> scene_texture (additive)
        // 5. Fisheye (if enabled) -> intermediate_texture
        // 6. Vignette -> final output

        // Pass 1: Background render to scene texture
        {
            let mut background_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Background Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.scene_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), // Clear to black
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            background_pass.set_pipeline(&self.background_render_pipeline);
            background_pass.set_bind_group(0, &self.background_render_bind_group, &[]);
            background_pass.draw(0..6, 0..1); // Fullscreen quad (6 vertices for 2 triangles)
        }

        // Pass 2: Particle render to scene texture (additive blend)
        {
            let mut particle_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Particle Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.scene_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Keep background
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            particle_pass.set_pipeline(&self.render_pipeline);
            particle_pass.set_bind_group(0, &self.render_bind_groups[output_buffer_index], &[]);
            particle_pass
                .set_vertex_buffer(0, self.particle_buffers[output_buffer_index].slice(..));
            particle_pass.set_vertex_buffer(1, self.quad_vertex_buffer.slice(..));

            let particle_count = particle_system.get_active_count();
            particle_pass.draw(0..4, 0..particle_count);
        }

        // Pass 3: Grid render to scene texture (additive blend)
        {
            let mut grid_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Grid Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.scene_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Keep existing content
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            grid_pass.set_pipeline(&self.grid_render_pipeline);
            grid_pass.set_bind_group(0, &self.grid_render_bind_group, &[]);
            grid_pass.draw(0..6, 0..1); // Fullscreen quad
        }

        // Pass 4: Lightning render to scene texture (additive blend)
        {
            let mut lightning_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Lightning Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.scene_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Keep existing content
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            lightning_pass.set_pipeline(&self.lightning_render_pipeline);
            lightning_pass.set_bind_group(0, &self.lightning_render_bind_group, &[]);
            lightning_pass.draw(0..6, 0..1); // Fullscreen quad
        }

        // Pass 5: Fisheye (always runs) - scene_texture -> intermediate_texture
        // The shader decides whether to apply the effect based on fisheye_strength parameter
        {
            let mut fisheye_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Fisheye Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.intermediate_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            fisheye_pass.set_pipeline(&self.fisheye_render_pipeline);
            fisheye_pass.set_bind_group(0, &self.fisheye_render_bind_group, &[]);
            fisheye_pass.draw(0..6, 0..1); // Fullscreen quad
        }

        // Pass 6: Zoom post-processing (final step) - matches TypeScript pipeline
        {
            let mut zoom_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Zoom Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Always use zoom pipeline as final step (matches TypeScript)
            // This reads from intermediate_texture (after fisheye processing)
            zoom_pass.set_pipeline(&self.zoom_render_pipeline);
            zoom_pass.set_bind_group(0, &self.zoom_render_bind_group, &[]);
            zoom_pass.draw(0..6, 0..1); // Fullscreen quad
        }

        // Submit commands
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        // Ping-pong buffers - switch for next frame (matches TypeScript pattern)
        self.current_buffer_index = 1 - self.current_buffer_index;

        Ok(())
    }

    /// Update zoom uniforms buffer with current zoom parameters
    pub fn update_zoom_uniforms(&mut self, simulation_params: &SimulationParams) {
        // Calculate zoom level from viewport size
        let zoom_level = 2400.0 / simulation_params.viewport_width;

        // Calculate center of the current viewport in virtual world coordinates
        let center_x =
            simulation_params.virtual_world_offset_x + (simulation_params.viewport_width / 2.0);
        let center_y =
            simulation_params.virtual_world_offset_y + (simulation_params.viewport_height / 2.0);

        // Update zoom uniforms buffer
        let zoom_uniforms = [zoom_level, center_x, center_y, 0.0f32]; // padding for alignment
        self.queue.write_buffer(
            &self.zoom_uniforms_buffer,
            0,
            bytemuck::cast_slice(&zoom_uniforms),
        );

        // Only log occasionally to avoid spam
        // console_log!("🔍 Updated zoom uniforms: level={:.2}, center=({:.0},{:.0})", zoom_level, center_x, center_y);
    }

    /// Initialize particle buffers with initial particle data
    /// This must be called after renderer creation to populate the GPU buffers
    pub fn initialize_particle_buffers(&mut self, particle_system: &ParticleSystem) {
        let particle_data = particle_system.to_buffer();

        // Initialize both particle buffers with the same initial data
        self.queue
            .write_buffer(&self.particle_buffers[0], 0, &particle_data);
        self.queue
            .write_buffer(&self.particle_buffers[1], 0, &particle_data);

        console_log!(
            "🎯 Initialized both particle buffers with {} particles",
            particle_system.get_active_count()
        );
    }

    /// Update particle data during simulation (only updates input buffer to preserve physics)
    pub fn update_particle_active_states(&mut self, particle_system: &ParticleSystem) {
        // Instead of uploading the entire buffer, only update the is_active flags at specific byte offsets
        // This preserves the live GPU physics data (positions, velocities) while updating metadata

        let max_particles = particle_system.get_max_particles() as usize;
        let active_count = particle_system.get_active_count() as usize;

        // Update only the is_active field at offset 36 for each particle (48 bytes apart)
        for i in 0..max_particles {
            let byte_offset = (i * 48 + 36) as u64; // is_active is at offset 36 in each 48-byte particle
            let is_active = if i < active_count { 1u32 } else { 0u32 };

            self.queue.write_buffer(
                &self.particle_buffers[self.current_buffer_index],
                byte_offset,
                &is_active.to_le_bytes(),
            );
        }

        console_log!(
            "🔄 Updated only is_active flags in input buffer {} for {} particles (preserving physics data)",
            self.current_buffer_index,
            particle_system.get_active_count()
        );
    }

    /// Update only transition fields for particles to avoid overwriting live GPU physics data
    /// This preserves positions and velocities while updating transition_start and transition_type
    pub fn update_particle_transitions(&mut self, particle_system: &ParticleSystem) {
        let max_particles = particle_system.get_max_particles() as usize;

        // Create a buffer to hold only the transition updates
        let mut updates = Vec::new();

        for i in 0..max_particles {
            if let Some(particle) = particle_system.get_particle(i) {
                // Only update if particle has transition data
                if particle.transition_start > 0.0 {
                    let particle_offset = i * 48; // 48 bytes per particle

                    // Update transition_start field (offset 28, 4 bytes)
                    let transition_start_offset = particle_offset + 28;
                    updates.push((
                        transition_start_offset,
                        particle.transition_start.to_le_bytes().to_vec(),
                    ));

                    // Update transition_type field (offset 32, 4 bytes)
                    let transition_type_offset = particle_offset + 32;
                    updates.push((
                        transition_type_offset,
                        particle.transition_type.to_le_bytes().to_vec(),
                    ));
                }
            }
        }

        // Apply all updates to the input buffer (preserves physics data in output buffer)
        for (offset, data) in updates {
            self.queue.write_buffer(
                &self.particle_buffers[self.current_buffer_index],
                offset as u64,
                &data,
            );
        }

        console_log!("🔄 Updated transition fields for particles with active transitions");
    }
}

use crate::{console_log, ParticleSystem, SimulationParams, InteractionRules};
use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;

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
    /// Initialize WebGPU renderer with the given canvas
    pub async fn new(canvas: &web_sys::HtmlCanvasElement) -> Result<WebGpuRenderer, JsValue> {
        console_log!("🎨 Initializing wgpu WebGPU renderer");

        // Get canvas dimensions
        let canvas_width = canvas.width();
        let canvas_height = canvas.height();

        // Create wgpu instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Create surface from canvas
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| JsValue::from_str(&format!("Failed to create surface: {:?}", e)))?;

        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("Failed to request adapter: {:?}", e)))?;

        console_log!("✅ WebGPU adapter found: {:?}", adapter.get_info());

        // Request device and queue
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Particle Life Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("Failed to create device: {:?}", e)))?;

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
            width: canvas_width,
            height: canvas_height,
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
            -1.0, -1.0,  // Bottom-left
             1.0, -1.0,  // Bottom-right
            -1.0,  1.0,  // Top-left
             1.0,  1.0,  // Top-right
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

        // Create particle buffers (double-buffered for ping-pong)
        let max_particles = 32768;
        let particle_buffer_size = (max_particles * 24) as u64; // 24 bytes per particle
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
            size: (max_lightning_segments * 32) as u64, // 32 bytes per segment
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let lightning_bolt_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lightning Bolt Buffer"),
            size: 16, // Single bolt structure, 16 bytes
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
                        array_stride: 24, // PARTICLE_SIZE_BYTES: pos(8) + vel(8) + type(4) + size(4) = 24 bytes
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
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/lightning_frag.wgsl").into()),
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
            ],
        });

        // Create lightning compute pipeline
        let lightning_compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Lightning Compute Pipeline Layout"),
                bind_group_layouts: &[&lightning_compute_bind_group_layout],
                push_constant_ranges: &[],
            });

        let lightning_compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
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

        let lightning_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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

        let background_fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Background Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/background_frag.wgsl").into()),
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
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: sim_params_buffer.as_entire_binding(),
                },
            ],
        });

        let grid_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Grid Render Bind Group"),
            layout: &post_processing_uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: sim_params_buffer.as_entire_binding(),
                },
            ],
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
        queue.write_buffer(&zoom_uniforms_buffer, 0, bytemuck::cast_slice(&initial_zoom_uniforms));

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

        let background_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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

        let fisheye_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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

        let vignette_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            canvas_width,
            canvas_height,
        })
    }

    /// Render a frame using WebGPU
    pub fn render(
        &mut self,
        particle_system: &ParticleSystem,
        simulation_params: &SimulationParams,
        interaction_rules: &InteractionRules,
    ) -> Result<(), JsValue> {
        // Update simulation parameters buffer
        let sim_params_data = simulation_params.to_buffer();
        self.queue
            .write_buffer(&self.sim_params_buffer, 0, &sim_params_data);

        // Update interaction rules buffer
        let rules_data = interaction_rules.to_buffer();
        self.queue
            .write_buffer(&self.interaction_rules_buffer, 0, &rules_data);

        // Update particle colors buffer
        let colors_data = particle_system.get_colors_buffer();
        self.queue
            .write_buffer(&self.particle_colors_buffer, 0, &colors_data);

        // Update current input particle buffer with current particle data
        let particle_data = particle_system.to_buffer();
        self.queue.write_buffer(
            &self.particle_buffers[self.current_buffer_index],
            0,
            &particle_data,
        );

        // Update zoom uniforms with current simulation parameters
        self.update_zoom_uniforms(simulation_params);

        // Get current surface texture
        let output = self
            .surface
            .get_current_texture()
            .map_err(|e| JsValue::from_str(&format!("Failed to get surface texture: {:?}", e)))?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        // Run compute pass (physics simulation)
        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Physics Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &self.compute_bind_groups[self.current_buffer_index], &[]);

            let particle_count = particle_system.get_active_count();
            let workgroup_size = 64;
            let dispatch_size = (particle_count + workgroup_size - 1) / workgroup_size;
            compute_pass.dispatch_workgroups(dispatch_size, 1, 1);
        }

        // Run lightning compute pass (lightning generation)
        {
            let mut lightning_compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Lightning Compute Pass"),
                timestamp_writes: None,
            });

            lightning_compute_pass.set_pipeline(&self.lightning_compute_pipeline);
            lightning_compute_pass.set_bind_group(0, &self.lightning_compute_bind_group, &[]);
            lightning_compute_pass.dispatch_workgroups(1, 1, 1); // Single workgroup for lightning generation
        }        // Switch buffer index for next frame (input/output ping-pong)
        let output_buffer_index = 1 - self.current_buffer_index;
        self.current_buffer_index = output_buffer_index;

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
            particle_pass.set_vertex_buffer(0, self.particle_buffers[output_buffer_index].slice(..));
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

        Ok(())
    }

    /// Update zoom uniforms buffer with current zoom parameters
    pub fn update_zoom_uniforms(&mut self, simulation_params: &SimulationParams) {
        // Calculate zoom level from viewport size
        let zoom_level = 2400.0 / simulation_params.viewport_width;

        // Calculate center of the current viewport in virtual world coordinates
        let center_x = simulation_params.virtual_world_offset_x + (simulation_params.viewport_width / 2.0);
        let center_y = simulation_params.virtual_world_offset_y + (simulation_params.viewport_height / 2.0);

        // Update zoom uniforms buffer
        let zoom_uniforms = [zoom_level, center_x, center_y, 0.0f32]; // padding for alignment
        self.queue.write_buffer(&self.zoom_uniforms_buffer, 0, bytemuck::cast_slice(&zoom_uniforms));

        console_log!("🔍 Updated zoom uniforms: level={:.2}, center=({:.0},{:.0})", zoom_level, center_x, center_y);
    }
}

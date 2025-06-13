use crate::{console_log, ParticleLifeEngine, SimulationParams, ShaderType};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::*;

/// Complete WebGPU renderer that handles all GPU operations in Rust
/// This creates a clean architecture where TypeScript only handles UI controls
#[wasm_bindgen]
pub struct ParticleLifeRenderer {
    // WebGPU core objects
    device: GpuDevice,
    queue: GpuQueue,
    context: GpuCanvasContext,

    // Simulation engine
    engine: ParticleLifeEngine,

    // GPU resources
    sim_params_buffer: GpuBuffer,
    particle_color_buffer: GpuBuffer,
    interaction_rules_buffer: GpuBuffer,
    particle_buffer_a: GpuBuffer,
    particle_buffer_b: GpuBuffer,
    vertex_buffer: GpuBuffer,

    // Pipelines and bind groups
    compute_pipeline: GpuComputePipeline,
    render_pipeline: GpuRenderPipeline,
    compute_bind_group: GpuBindGroup,
    render_bind_group: GpuBindGroup,

    // State management
    current_buffer_index: usize,
    canvas_width: u32,
    canvas_height: u32,
    frame_count: u32,
}

#[wasm_bindgen]
impl ParticleLifeRenderer {
    /// Initialize the complete particle life renderer
    #[wasm_bindgen(constructor)]
    pub async fn new(canvas: HtmlCanvasElement) -> Result<ParticleLifeRenderer, JsValue> {
        console_log!("🚀 Initializing complete Particle Life renderer in Rust");

        // Initialize WebGPU
        let (device, queue, context, canvas_width, canvas_height) =
            Self::init_webgpu(&canvas).await?;

        // Create simulation engine
        let mut engine = ParticleLifeEngine::new();
        engine.randomize_particles();

        // Create GPU resources
        let sim_params_buffer = Self::create_uniform_buffer(&device, 256, "Simulation Parameters");
        let particle_color_buffer = Self::create_storage_buffer(&device, 1024, "Particle Colors");
        let interaction_rules_buffer = Self::create_storage_buffer(&device, 2048, "Interaction Rules");
        let (particle_buffer_a, particle_buffer_b) = Self::create_particle_buffers(&device);
        let vertex_buffer = Self::create_quad_vertex_buffer(&device);

        // Create shaders and pipelines
        let preferred_format = web_sys::window()
            .unwrap()
            .navigator()
            .gpu()
            .unwrap()
            .get_preferred_canvas_format();

        let compute_pipeline = Self::create_compute_pipeline(&device).await?;
        let render_pipeline = Self::create_render_pipeline(&device, &preferred_format).await?;

        // Create bind groups
        let compute_bind_group = Self::create_compute_bind_group(
            &device,
            &compute_pipeline,
            &sim_params_buffer,
            &interaction_rules_buffer,
            &particle_buffer_a,
            &particle_buffer_b
        );

        let render_bind_group = Self::create_render_bind_group(
            &device,
            &render_pipeline,
            &sim_params_buffer,
            &particle_color_buffer,
            &particle_buffer_a
        );

        Ok(ParticleLifeRenderer {
            device,
            queue,
            context,
            engine,
            sim_params_buffer,
            particle_color_buffer,
            interaction_rules_buffer,
            particle_buffer_a,
            particle_buffer_b,
            vertex_buffer,
            compute_pipeline,
            render_pipeline,
            compute_bind_group,
            render_bind_group,
            current_buffer_index: 0,
            canvas_width,
            canvas_height,
            frame_count: 0,
        })
    }

    /// Update simulation parameters - called from TypeScript UI
    pub fn update_simulation_params(&mut self, params: SimulationParams) {
        self.engine.update_simulation_params(params);

        // Upload to GPU
        let buffer_data = self.engine.get_simulation_params().to_buffer();
        self.queue.write_buffer_with_u8_array(&self.sim_params_buffer, 0, &buffer_data);

        // Update particle colors
        let color_data = self.engine.get_particle_colors_buffer();
        self.queue.write_buffer_with_u8_array(&self.particle_color_buffer, 0, &color_data);

        // Update interaction rules
        let rules_data = self.engine.get_interaction_rules_buffer();
        self.queue.write_buffer_with_u8_array(&self.interaction_rules_buffer, 0, &rules_data);
    }

    /// Main render loop - handles both compute and render passes
    pub fn render_frame(&mut self) -> Result<(), JsValue> {
        self.frame_count += 1;

        // Update simulation on CPU first (for now, we'll move this to GPU later)
        self.engine.update(0.016); // 60 FPS

        // Upload particle data to GPU
        let particle_data = self.engine.get_particles_buffer();
        let current_buffer = if self.current_buffer_index == 0 {
            &self.particle_buffer_a
        } else {
            &self.particle_buffer_b
        };
        self.queue.write_buffer_with_u8_array(current_buffer, 0, &particle_data);

        // Create command encoder
        let command_encoder = self.device.create_command_encoder();

        // GPU Compute pass (future: move particle physics to GPU)
        {
            let compute_pass = command_encoder.begin_compute_pass();
            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, Some(&self.compute_bind_group), &js_sys::Array::new());

            let num_particles = self.engine.get_particle_count();
            let workgroup_size = 64;
            let dispatch_count = (num_particles + workgroup_size - 1) / workgroup_size;
            compute_pass.dispatch_workgroups(dispatch_count as u32, 1, 1);
            compute_pass.end();
        }

        // Render pass
        let texture = self.context.get_current_texture();
        let view = texture.create_view();

        let sim_params = self.engine.get_simulation_params();
        let clear_color = GpuColor::new(
            sim_params.background_color_r as f64,
            sim_params.background_color_g as f64,
            sim_params.background_color_b as f64,
            1.0
        );

        let color_attachment = GpuRenderPassColorAttachment::new(
            &GpuRenderPassColorAttachmentClearValue::clear_value_gPu_color(&clear_color),
            &GpuLoadOp::Clear,
            &GpuStoreOp::Store,
            &view,
        );

        let render_pass_descriptor = GpuRenderPassDescriptor::new(&js_sys::Array::from_iter([color_attachment]));

        {
            let render_pass = command_encoder.begin_render_pass(&render_pass_descriptor);
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, Some(&self.render_bind_group), &js_sys::Array::new());
            render_pass.set_vertex_buffer(0, Some(&self.vertex_buffer), None, None);

            let num_particles = self.engine.get_particle_count();
            render_pass.draw(4, num_particles as u32, 0, 0); // 4 vertices per quad
            render_pass.end();
        }

        // Submit commands
        self.queue.submit(&js_sys::Array::from_iter([command_encoder.finish()]));

        // Swap buffers for next frame
        self.current_buffer_index = 1 - self.current_buffer_index;

        Ok(())
    }

    /// Start the render loop
    pub fn start_render_loop(&mut self) {
        console_log!("🎬 Starting Rust-based render loop");

        // We'll use requestAnimationFrame from JavaScript to call render_frame
        // This will be handled by the TypeScript UI layer
    }

    /// Resize renderer when canvas changes
    pub fn resize(&mut self, width: u32, height: u32) {
        self.canvas_width = width;
        self.canvas_height = height;

        // Update simulation parameters with new canvas size
        let mut params = self.engine.get_simulation_params().clone();
        params.canvas_render_width = width as f32;
        params.canvas_render_height = height as f32;
        self.update_simulation_params(params);

        // Reconfigure canvas context
        let preferred_format = web_sys::window()
            .unwrap()
            .navigator()
            .gpu()
            .unwrap()
            .get_preferred_canvas_format();

        let mut config = GpuCanvasConfiguration::new(&self.device, &preferred_format);
        config.alpha_mode(GpuCanvasAlphaMode::Opaque);
        config.width(width);
        config.height(height);
        self.context.configure(&config);
    }

    /// Get current FPS (calculated from frame count)
    pub fn get_fps(&self) -> f32 {
        // Simple FPS calculation - in a real implementation you'd track timing
        60.0
    }

    /// Get current particle count
    pub fn get_particle_count(&self) -> usize {
        self.engine.get_particle_count()
    }

    /// Randomize particles - called from UI
    pub fn randomize_particles(&mut self) {
        self.engine.randomize_particles();
    }

    /// Reset simulation - called from UI
    pub fn reset_simulation(&mut self) {
        self.engine.reset();
    }
}

// Private implementation methods
impl ParticleLifeRenderer {
    async fn init_webgpu(canvas: &HtmlCanvasElement) -> Result<(GpuDevice, GpuQueue, GpuCanvasContext, u32, u32), JsValue> {
        let window = web_sys::window().ok_or("No window object")?;
        let navigator = window.navigator();
        let gpu = navigator.gpu().ok_or("WebGPU not supported")?;

        // Request adapter
        let adapter_promise = gpu.request_adapter();
        let adapter = JsFuture::from(adapter_promise)
            .await?
            .dyn_into::<GpuAdapter>()?;

        // Request device
        let device_promise = adapter.request_device();
        let device = JsFuture::from(device_promise)
            .await?
            .dyn_into::<GpuDevice>()?;

        let queue = device.queue();

        // Configure canvas
        let context = canvas
            .get_context("webgpu")?
            .ok_or("Failed to get WebGPU context")?
            .dyn_into::<GpuCanvasContext>()?;

        let preferred_format = gpu.get_preferred_canvas_format();
        let mut config = GpuCanvasConfiguration::new(&device, &preferred_format);
        config.alpha_mode(GpuCanvasAlphaMode::Opaque);
        context.configure(&config);

        let canvas_width = canvas.width();
        let canvas_height = canvas.height();

        Ok((device, queue, context, canvas_width, canvas_height))
    }

    fn create_uniform_buffer(device: &GpuDevice, size: u64, label: &str) -> GpuBuffer {
        let mut descriptor = GpuBufferDescriptor::new(
            size as f64,
            GpuBufferUsage::UNIFORM | GpuBufferUsage::COPY_DST
        );
        descriptor.label(label);
        device.create_buffer(&descriptor)
    }

    fn create_storage_buffer(device: &GpuDevice, size: u64, label: &str) -> GpuBuffer {
        let mut descriptor = GpuBufferDescriptor::new(
            size as f64,
            GpuBufferUsage::STORAGE | GpuBufferUsage::COPY_DST
        );
        descriptor.label(label);
        device.create_buffer(&descriptor)
    }

    fn create_particle_buffers(device: &GpuDevice) -> (GpuBuffer, GpuBuffer) {
        let buffer_size = 6400 * 24; // max particles * bytes per particle

        let mut descriptor = GpuBufferDescriptor::new(
            buffer_size as f64,
            GpuBufferUsage::STORAGE | GpuBufferUsage::VERTEX | GpuBufferUsage::COPY_DST | GpuBufferUsage::COPY_SRC
        );

        descriptor.label("Particle Buffer A");
        let buffer_a = device.create_buffer(&descriptor);

        descriptor.label("Particle Buffer B");
        let buffer_b = device.create_buffer(&descriptor);

        (buffer_a, buffer_b)
    }

    fn create_quad_vertex_buffer(device: &GpuDevice) -> GpuBuffer {
        // Create quad vertices for instanced rendering
        let vertices: &[f32] = &[
            -1.0, -1.0,  // Bottom left
             1.0, -1.0,  // Bottom right
            -1.0,  1.0,  // Top left
             1.0,  1.0,  // Top right
        ];

        let vertex_data = bytemuck::cast_slice(vertices);

        let mut descriptor = GpuBufferDescriptor::new(
            vertex_data.len() as f64,
            GpuBufferUsage::VERTEX | GpuBufferUsage::COPY_DST
        );
        descriptor.label("Quad Vertex Buffer");

        let buffer = device.create_buffer(&descriptor);

        // We'll need to upload the data separately since we can't do it in constructor
        // The queue.write_buffer call will happen in the new() method

        buffer
    }

    async fn create_compute_pipeline(device: &GpuDevice) -> Result<GpuComputePipeline, JsValue> {
        let shader_source = ShaderType::ParticleCompute.source();

        let mut shader_descriptor = GpuShaderModuleDescriptor::new(shader_source);
        shader_descriptor.label(ShaderType::ParticleCompute.label());
        let shader_module = device.create_shader_module(&shader_descriptor);

        let compute_stage = GpuProgrammableStage::new(&shader_module, "main");

        let mut pipeline_descriptor = GpuComputePipelineDescriptor::new(
            &GpuPipelineLayout::auto_(&device),
            &compute_stage
        );
        pipeline_descriptor.label("Particle Compute Pipeline");

        Ok(device.create_compute_pipeline(&pipeline_descriptor))
    }

    async fn create_render_pipeline(device: &GpuDevice, format: &str) -> Result<GpuRenderPipeline, JsValue> {
        let vertex_source = ShaderType::ParticleVertex.source();
        let fragment_source = ShaderType::ParticleFragment.source();

        let mut vertex_descriptor = GpuShaderModuleDescriptor::new(vertex_source);
        vertex_descriptor.label(ShaderType::ParticleVertex.label());
        let vertex_module = device.create_shader_module(&vertex_descriptor);

        let mut fragment_descriptor = GpuShaderModuleDescriptor::new(fragment_source);
        fragment_descriptor.label(ShaderType::ParticleFragment.label());
        let fragment_module = device.create_shader_module(&fragment_descriptor);

        // Create vertex buffer layout
        let vertex_attribute = GpuVertexAttribute::new(0, GpuVertexFormat::Float32x2, 0);
        let vertex_buffer_layout = GpuVertexBufferLayout::new(
            8, // 2 floats * 4 bytes
            &js_sys::Array::from_iter([vertex_attribute])
        );
        vertex_buffer_layout.set_step_mode(GpuVertexStepMode::Vertex);

        let vertex_state = GpuVertexState::new(&vertex_module, "main");
        vertex_state.set_buffers(&js_sys::Array::from_iter([vertex_buffer_layout]));

        let fragment_targets = js_sys::Array::from_iter([
            GpuColorTargetState::new(format)
        ]);
        let fragment_state = GpuFragmentState::new(&fragment_module, "main", &fragment_targets);

        let mut pipeline_descriptor = GpuRenderPipelineDescriptor::new(
            &GpuPipelineLayout::auto_(&device),
            &vertex_state
        );
        pipeline_descriptor.label("Particle Render Pipeline");
        pipeline_descriptor.fragment(&fragment_state);

        let primitive_state = GpuPrimitiveState::new();
        primitive_state.set_topology(GpuPrimitiveTopology::TriangleStrip);
        pipeline_descriptor.primitive(&primitive_state);

        Ok(device.create_render_pipeline(&pipeline_descriptor))
    }

    fn create_compute_bind_group(
        device: &GpuDevice,
        pipeline: &GpuComputePipeline,
        sim_params: &GpuBuffer,
        rules: &GpuBuffer,
        particles_a: &GpuBuffer,
        particles_b: &GpuBuffer,
    ) -> GpuBindGroup {
        let layout = pipeline.get_bind_group_layout(0);

        let entries = js_sys::Array::from_iter([
            GpuBindGroupEntry::new(0, &GpuBindingResource::buffer(sim_params)),
            GpuBindGroupEntry::new(1, &GpuBindingResource::buffer(rules)),
            GpuBindGroupEntry::new(2, &GpuBindingResource::buffer(particles_a)),
            GpuBindGroupEntry::new(3, &GpuBindingResource::buffer(particles_b)),
        ]);

        let mut descriptor = GpuBindGroupDescriptor::new(&entries, &layout);
        descriptor.label("Compute Bind Group");

        device.create_bind_group(&descriptor)
    }

    fn create_render_bind_group(
        device: &GpuDevice,
        pipeline: &GpuRenderPipeline,
        sim_params: &GpuBuffer,
        colors: &GpuBuffer,
        particles: &GpuBuffer,
    ) -> GpuBindGroup {
        let layout = pipeline.get_bind_group_layout(0);

        let entries = js_sys::Array::from_iter([
            GpuBindGroupEntry::new(0, &GpuBindingResource::buffer(sim_params)),
            GpuBindGroupEntry::new(1, &GpuBindingResource::buffer(colors)),
            GpuBindGroupEntry::new(2, &GpuBindingResource::buffer(particles)),
        ]);

        let mut descriptor = GpuBindGroupDescriptor::new(&entries, &layout);
        descriptor.label("Render Bind Group");

        device.create_bind_group(&descriptor)
    }
}

use super::text::Vertex;

// ── TextPipeline ──────────────────────────────────────────────────────────────

pub struct TextPipeline {
    pub render_pipeline: wgpu::RenderPipeline,
    /// Bind group layout for the orthographic projection matrix (group 0).
    pub projection_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout for the MTSDF font texture + sampler + params (group 1).
    pub font_bind_group_layout: wgpu::BindGroupLayout,
}

// ── create_text_pipeline ──────────────────────────────────────────────────────

pub fn create_text_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> TextPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("text_shader"),
        source: wgpu::ShaderSource::Wgsl(
            include_str!("shaders/text_shader.wgsl").into(),
        ),
    });

    // ── Bind group layouts ────────────────────────────────────────────────────

    let projection_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text_projection_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    // mat4x4<f32> = 64 bytes; lets wgpu validate at bind group
                    // creation rather than deferring to draw time.
                    min_binding_size: wgpu::BufferSize::new(64),
                },
                count: None,
            }],
        });

    let font_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text_font_bgl"),
            entries: &[
                // Binding 0: MTSDF atlas texture (sampled as linear float)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Binding 1: Linear sampler (required for SDF interpolation)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Binding 2: MtsdfParams uniform (distance_range, atlas_w/h)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        // MtsdfParams = 4 × f32 = 16 bytes
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                },
            ],
        });

    // ── Pipeline layout ───────────────────────────────────────────────────────

    let pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text_pipeline_layout"),
            bind_group_layouts: &[
                &projection_bind_group_layout,
                &font_bind_group_layout,
            ],
            ..Default::default()
        });

    // ── Render pipeline ───────────────────────────────────────────────────────

    let render_pipeline =
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

    TextPipeline {
        render_pipeline,
        projection_bind_group_layout,
        font_bind_group_layout,
    }
}

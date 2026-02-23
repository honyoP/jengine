#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub fg_color: [f32; 4],
    pub bg_color: [f32; 4],
    /// ECS entity ID for looking up visual offsets in the shader (animation juice).
    /// NO_ENTITY (u32::MAX) if not animated.
    pub entity_id: u32,
    /// 0.0 = Background layer (static), 0.5 = Sprite layer, 1.0 = Foreground layer.
    pub layer_id: f32,
}

impl TileVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
        0 => Float32x2,  // position
        1 => Float32x2,  // uv
        2 => Float32x4,  // fg_color
        3 => Float32x4,  // bg_color
        4 => Uint32,     // entity_id
        5 => Float32,    // layer_id
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TileVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub struct TilePipeline {
    pub render_pipeline: wgpu::RenderPipeline,
    pub projection_bind_group_layout: wgpu::BindGroupLayout,
    pub atlas_bind_group_layout: wgpu::BindGroupLayout,
    pub entity_offsets_bind_group_layout: wgpu::BindGroupLayout,
}

pub fn create_tile_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> TilePipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("tile_shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
    });

    let projection_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("projection_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

    let atlas_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("atlas_bgl"),
            entries: &[
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

    let entity_offsets_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("entity_offsets_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("tile_pipeline_layout"),
        bind_group_layouts: &[
            &projection_bind_group_layout,
            &atlas_bind_group_layout,
            &entity_offsets_bind_group_layout,
        ],
        ..Default::default()
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("tile_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[TileVertex::layout()],
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

    TilePipeline {
        render_pipeline,
        projection_bind_group_layout,
        atlas_bind_group_layout,
        entity_offsets_bind_group_layout,
    }
}

/// Orthographic projection matrix (column-major) mapping pixel coords to clip space.
pub fn orthographic_projection(width: f32, height: f32) -> [f32; 16] {
    [
        2.0 / width, 0.0,           0.0, 0.0,
        0.0,         -2.0 / height, 0.0, 0.0,
        0.0,         0.0,           1.0, 0.0,
        -1.0,        1.0,           0.0, 1.0,
    ]
}

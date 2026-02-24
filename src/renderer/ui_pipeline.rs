use wgpu;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UIVertex {
    /// Screen-space position in pixels (x, y, z).
    pub position: [f32; 3],
    /// Total width and height of the UI element in pixels.
    pub rect_size: [f32; 2],
    /// Local coordinate within the quad [0, 1].
    pub rect_coord: [f32; 2],
    /// Main background color.
    pub color: [f32; 4],
    /// Border color.
    pub border_color: [f32; 4],
    /// Corner radii: [top-left, top-right, bottom-right, bottom-left].
    pub radius: [f32; 4],
    /// Thickness of the border in pixels.
    pub border_thickness: f32,
    /// Softness of the outer shadow/glow.
    pub shadow_blur: f32,
    /// Pattern mode: 0=solid, 1=crosshatch, 2=dotted
    pub mode: u32,
    /// Pattern parameter (e.g. scale or intensity)
    pub mode_param: f32,
    /// Scissor clip rect in screen pixels: [min_x, min_y, max_x, max_y].
    /// Use [-1e6, -1e6, 1e6, 1e6] to disable clipping.
    pub clip_rect: [f32; 4],
}

impl UIVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 11] = wgpu::vertex_attr_array![
        0  => Float32x3, // position (x, y, z)
        1  => Float32x2, // rect_size
        2  => Float32x2, // rect_coord
        3  => Float32x4, // color
        4  => Float32x4, // border_color
        5  => Float32x4, // radius
        6  => Float32,   // border_thickness
        7  => Float32,   // shadow_blur
        8  => Uint32,    // mode
        9  => Float32,   // mode_param
        10 => Float32x4, // clip_rect (min_x, min_y, max_x, max_y) in screen pixels
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<UIVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub struct UIPipeline {
    pub render_pipeline: wgpu::RenderPipeline,
    pub projection_bind_group_layout: wgpu::BindGroupLayout,
}

pub fn create_ui_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> UIPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("modern_ui_shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("shaders/ui_shader.wgsl").into()),
    });

    let projection_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ui_projection_bgl"),
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

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("ui_pipeline_layout"),
        bind_group_layouts: &[&projection_bind_group_layout],
        ..Default::default()
    });

    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("ui_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[UIVertex::layout()],
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
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    UIPipeline {
        render_pipeline,
        projection_bind_group_layout,
    }
}

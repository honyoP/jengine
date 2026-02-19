use std::sync::Arc;
use winit::window::Window;
use wgpu::util::DeviceExt;

pub struct ScanlinePipeline {
    pub pipeline:     wgpu::RenderPipeline,
    pub scene_bgl:    wgpu::BindGroupLayout, // group 0: texture + sampler
    pub uniforms_bgl: wgpu::BindGroupLayout, // group 1: scale_factor uniform
}

pub struct ScanlinePass {
    pub pipeline:             ScanlinePipeline,
    pub intermediate_texture: wgpu::Texture,
    /// sRGB view — used as render attachment for the main pass.
    pub render_view:          wgpu::TextureView,
    /// Linear (non-sRGB) view — sampled by the scanline shader to avoid
    /// applying gamma correction twice.
    pub sample_view:          wgpu::TextureView,
    pub scene_bind_group:     wgpu::BindGroup,  // texture + nearest sampler
    pub uniforms_buffer:      wgpu::Buffer,     // 16 bytes [scale_factor, 0, 0, 0]
    pub uniforms_bind_group:  wgpu::BindGroup,
}

fn make_pipeline(
    device: &wgpu::Device,
    output_format: wgpu::TextureFormat,
) -> ScanlinePipeline {
    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/scanline_shader.wgsl"));

    // group 0: scene texture + sampler
    let scene_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("scanline_scene_bgl"),
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

    // group 1: scale_factor uniform (16 bytes)
    let uniforms_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("scanline_uniforms_bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("scanline_pipeline_layout"),
        bind_group_layouts: &[&scene_bgl, &uniforms_bgl],
        ..Default::default()
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("scanline_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[], // positions generated from vertex_index
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: output_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
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

    ScanlinePipeline { pipeline, scene_bgl, uniforms_bgl }
}

/// Create a `ScanlinePass` for the given surface configuration.
pub fn create_scanline_pass(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    scale_factor: f32,
) -> ScanlinePass {
    let pipeline = make_pipeline(device, config.format);

    // The intermediate texture is rendered into by the main pass, then read
    // by the scanline blit pass.
    let non_srgb_fmt = config.format.remove_srgb_suffix();

    let intermediate_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scanline_intermediate"),
        size: wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        // Expose the non-sRGB view so the shader can sample linear values.
        view_formats: &[non_srgb_fmt],
    });

    // sRGB view → render attachment for the main pass.
    let render_view = intermediate_texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Linear view → shader input, avoids double gamma.
    let sample_view = intermediate_texture.create_view(&wgpu::TextureViewDescriptor {
        format: Some(non_srgb_fmt),
        ..Default::default()
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("scanline_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    let scene_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("scanline_scene_bg"),
        layout: &pipeline.scene_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&sample_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    let uniforms_data: [f32; 4] = [scale_factor, 0.0, 0.0, 0.0];
    let uniforms_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("scanline_uniforms"),
        contents: bytemuck::cast_slice(&uniforms_data),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let uniforms_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("scanline_uniforms_bg"),
        layout: &pipeline.uniforms_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniforms_buffer.as_entire_binding(),
        }],
    });

    ScanlinePass {
        pipeline,
        intermediate_texture,
        render_view,
        sample_view,
        scene_bind_group,
        uniforms_buffer,
        uniforms_bind_group,
    }
}

/// Recreate size-dependent resources after a window resize.
pub fn resize_scanline_pass(
    pass: &mut ScanlinePass,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    window: &Arc<Window>,
    config: &wgpu::SurfaceConfiguration,
) {
    let non_srgb_fmt = config.format.remove_srgb_suffix();

    pass.intermediate_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scanline_intermediate"),
        size: wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[non_srgb_fmt],
    });

    pass.render_view = pass.intermediate_texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    pass.sample_view = pass.intermediate_texture.create_view(&wgpu::TextureViewDescriptor {
        format: Some(non_srgb_fmt),
        ..Default::default()
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("scanline_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });

    pass.scene_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("scanline_scene_bg"),
        layout: &pass.pipeline.scene_bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&pass.sample_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });

    // Update scale_factor in case DPI changed.
    let uniforms_data: [f32; 4] = [window.scale_factor() as f32, 0.0, 0.0, 0.0];
    queue.write_buffer(&pass.uniforms_buffer, 0, bytemuck::cast_slice(&uniforms_data));
}

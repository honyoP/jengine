use wgpu::util::DeviceExt;

// ── PostProcessEffect ─────────────────────────────────────────────────────────

pub trait PostProcessEffect {
    /// Unique name used to identify the effect type for toggling (e.g. "scanline").
    fn effect_name(&self) -> &'static str;

    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source_view: &wgpu::TextureView,
        target_view: &wgpu::TextureView,
    );

    fn resize(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
        scale_factor: f32,
    );
}

// ── PostProcessStack ──────────────────────────────────────────────────────────

pub struct PostProcessStack {
    effects: Vec<Box<dyn PostProcessEffect>>,
    /// Ping-pong textures for chaining multiple effects.
    texture_a: wgpu::Texture,
    texture_b: wgpu::Texture,
    view_a: wgpu::TextureView,
    view_b: wgpu::TextureView,
    width: u32,
    height: u32,
}

impl PostProcessStack {
    pub fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> Self {
        let (texture_a, view_a) = create_intermediate_texture(device, config, "post_process_a");
        let (texture_b, view_b) = create_intermediate_texture(device, config, "post_process_b");

        Self {
            effects: Vec::new(),
            texture_a,
            texture_b,
            view_a,
            view_b,
            width: config.width,
            height: config.height,
        }
    }

    /// Add an effect, replacing any existing effect with the same name.
    /// This prevents duplicate effects when toggled on repeatedly.
    pub fn add_effect(&mut self, effect: Box<dyn PostProcessEffect>) {
        let name = effect.effect_name();
        self.effects.retain(|e| e.effect_name() != name);
        self.effects.push(effect);
    }

    /// Remove the effect with the given name, if present.
    pub fn remove_effect(&mut self, name: &str) {
        self.effects.retain(|e| e.effect_name() != name);
    }

    pub fn clear_effects(&mut self) {
        self.effects.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &wgpu::SurfaceConfiguration,
        scale_factor: f32,
    ) {
        if config.width == 0 || config.height == 0 { return; }
        
        self.width = config.width;
        self.height = config.height;
        
        let (texture_a, view_a) = create_intermediate_texture(device, config, "post_process_a");
        let (texture_b, view_b) = create_intermediate_texture(device, config, "post_process_b");
        
        self.texture_a = texture_a;
        self.view_a = view_a;
        self.texture_b = texture_b;
        self.view_b = view_b;

        for effect in &mut self.effects {
            effect.resize(device, queue, config, scale_factor);
        }
    }

    /// The view that the main scene should render into.
    pub fn main_render_target(&self) -> &wgpu::TextureView {
        &self.view_a
    }

    pub fn run(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        final_target_view: &wgpu::TextureView,
    ) {
        if self.effects.is_empty() {
            return;
        }

        let n = self.effects.len();
        // Use raw pointers to work around the borrow checker when ping-ponging
        // between view_a and view_b inside the same struct.
        let view_a_ptr: *const wgpu::TextureView = &self.view_a;
        let view_b_ptr: *const wgpu::TextureView = &self.view_b;

        let mut use_a_as_source = true;

        for (i, effect) in self.effects.iter_mut().enumerate() {
            let is_last = i == n - 1;
            // SAFETY: view_a and view_b are stored in self and live as long as
            // self does. We only read through these pointers (never alias &mut),
            // and the mutable borrow is on self.effects, a separate field.
            let source: &wgpu::TextureView = unsafe {
                &*(if use_a_as_source { view_a_ptr } else { view_b_ptr })
            };
            let target: &wgpu::TextureView = if is_last {
                final_target_view
            } else {
                unsafe { &*(if use_a_as_source { view_b_ptr } else { view_a_ptr }) }
            };

            effect.render(device, queue, encoder, source, target);

            // Only flip the ping-pong for the next (non-last) effect.
            if !is_last {
                use_a_as_source = !use_a_as_source;
            }
        }
    }
}

fn create_intermediate_texture(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
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
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

// ── Fullscreen Quad Helper ──────────────────────────────────────────────────

pub fn create_fullscreen_pipeline(
    device: &wgpu::Device,
    label: &str,
    shader_source: &str,
    bind_group_layouts: &[&wgpu::BindGroupLayout],
    output_format: wgpu::TextureFormat,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("{}_layout", label)),
        bind_group_layouts,
        ..Default::default()
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: output_format,
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
    })
}

// ── Common Shader Utils ─────────────────────────────────────────────────────

const FULLSCREEN_VS: &str = "
struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOut {
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
    );
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
    );
    var out: VertexOut;
    out.pos = vec4<f32>(positions[vi], 0.0, 1.0);
    out.uv  = uvs[vi];
    return out;
}
";

// ── ScanlineEffect ──────────────────────────────────────────────────────────

pub struct ScanlineEffect {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    cached_bg: Option<wgpu::BindGroup>,
    last_source: usize,
}

impl ScanlineEffect {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, scale_factor: f32) -> Self {
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scanline_bgl"),
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

        let u_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scanline_uniform_bgl"),
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

        let shader = format!("{}
        @group(0) @binding(0) var t_diffuse: texture_2d<f32>;
        @group(0) @binding(1) var s_diffuse: sampler;
        struct Uniforms {{ scale: f32, _pad0: f32, _pad1: f32, _pad2: f32 }};
        @group(1) @binding(0) var<uniform> uniforms: Uniforms;

        @fragment
        fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {{
            let color = textureSample(t_diffuse, s_diffuse, in.uv);
            let logical_y = floor(in.pos.y / uniforms.scale);
            let factor = select(1.0, 0.82, (u32(logical_y) % 2u) == 0u);
            return vec4<f32>(color.rgb * factor, color.a);
        }}", FULLSCREEN_VS);

        let pipeline = create_fullscreen_pipeline(device, "scanline", &shader, &[&bgl, &u_bgl], format);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("scanline_uniform"),
            contents: bytemuck::cast_slice(&[scale_factor, 0.0, 0.0, 0.0]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scanline_uniform_bg"),
            layout: &u_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        Self { pipeline, bind_group_layout: bgl, sampler, uniform_buffer, uniform_bind_group, cached_bg: None, last_source: 0 }
    }
}

impl PostProcessEffect for ScanlineEffect {
    fn effect_name(&self) -> &'static str { "scanline" }

    fn render(&mut self, device: &wgpu::Device, _queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, source_view: &wgpu::TextureView, target_view: &wgpu::TextureView) {
        let src_addr = source_view as *const _ as usize;
        if self.cached_bg.is_none() || self.last_source != src_addr {
            self.cached_bg = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(source_view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
                label: None,
            }));
            self.last_source = src_addr;
        }
        let bind_group = self.cached_bg.as_ref().unwrap();

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scanline_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.set_bind_group(1, &self.uniform_bind_group, &[]);
        pass.draw(0..6, 0..1);
    }

    fn resize(&mut self, _device: &wgpu::Device, queue: &wgpu::Queue, _config: &wgpu::SurfaceConfiguration, scale_factor: f32) {
        self.cached_bg = None; // Invalidate: texture views are recreated on resize.
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[scale_factor, 0.0, 0.0, 0.0]));
    }
}

// ── VignetteEffect ──────────────────────────────────────────────────────────

pub struct VignetteEffect {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    cached_bg: Option<wgpu::BindGroup>,
    last_source: usize,
}

impl VignetteEffect {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vignette_bgl"),
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

        let shader = format!("{}
        @group(0) @binding(0) var t_diffuse: texture_2d<f32>;
        @group(0) @binding(1) var s_diffuse: sampler;

        @fragment
        fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {{
            let color = textureSample(t_diffuse, s_diffuse, in.uv);
            let dist = distance(in.uv, vec2<f32>(0.5, 0.5));
            let vignette = smoothstep(0.8, 0.4, dist);
            return vec4<f32>(color.rgb * vignette, color.a);
        }}", FULLSCREEN_VS);

        let pipeline = create_fullscreen_pipeline(device, "vignette", &shader, &[&bgl], format);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self { pipeline, bind_group_layout: bgl, sampler, cached_bg: None, last_source: 0 }
    }
}

impl PostProcessEffect for VignetteEffect {
    fn effect_name(&self) -> &'static str { "vignette" }

    fn render(&mut self, device: &wgpu::Device, _queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, source_view: &wgpu::TextureView, target_view: &wgpu::TextureView) {
        let src_addr = source_view as *const _ as usize;
        if self.cached_bg.is_none() || self.last_source != src_addr {
            self.cached_bg = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(source_view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
                label: None,
            }));
            self.last_source = src_addr;
        }
        let bind_group = self.cached_bg.as_ref().unwrap();

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("vignette_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..6, 0..1);
    }

    fn resize(&mut self, _device: &wgpu::Device, _queue: &wgpu::Queue, _config: &wgpu::SurfaceConfiguration, _scale_factor: f32) {
        self.cached_bg = None;
    }
}

// ── ChromaticAberrationEffect ───────────────────────────────────────────────

pub struct ChromaticAberrationEffect {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    cached_bg: Option<wgpu::BindGroup>,
    last_source: usize,
}

impl ChromaticAberrationEffect {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("chromatic_bgl"),
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

        let shader = format!("{}
        @group(0) @binding(0) var t_diffuse: texture_2d<f32>;
        @group(0) @binding(1) var s_diffuse: sampler;

        @fragment
        fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {{
            let offset = 0.003;
            
            // Sample with bounds checking to avoid edge-clamping streaks
            var r = textureSample(t_diffuse, s_diffuse, in.uv).r;
            if (in.uv.x + offset <= 1.0) {{
                r = textureSample(t_diffuse, s_diffuse, in.uv + vec2<f32>(offset, 0.0)).r;
            }}
            
            let g = textureSample(t_diffuse, s_diffuse, in.uv).g;
            
            var b = textureSample(t_diffuse, s_diffuse, in.uv).b;
            if (in.uv.x - offset >= 0.0) {{
                b = textureSample(t_diffuse, s_diffuse, in.uv - vec2<f32>(offset, 0.0)).b;
            }}
            
            let a = textureSample(t_diffuse, s_diffuse, in.uv).a;
            return vec4<f32>(r, g, b, a);
        }}", FULLSCREEN_VS);

        let pipeline = create_fullscreen_pipeline(device, "chromatic", &shader, &[&bgl], format);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self { pipeline, bind_group_layout: bgl, sampler, cached_bg: None, last_source: 0 }
    }
}

impl PostProcessEffect for ChromaticAberrationEffect {
    fn effect_name(&self) -> &'static str { "chromatic_aberration" }

    fn render(&mut self, device: &wgpu::Device, _queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, source_view: &wgpu::TextureView, target_view: &wgpu::TextureView) {
        let src_addr = source_view as *const _ as usize;
        if self.cached_bg.is_none() || self.last_source != src_addr {
            self.cached_bg = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(source_view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
                label: None,
            }));
            self.last_source = src_addr;
        }
        let bind_group = self.cached_bg.as_ref().unwrap();

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("chromatic_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..6, 0..1);
    }

    fn resize(&mut self, _device: &wgpu::Device, _queue: &wgpu::Queue, _config: &wgpu::SurfaceConfiguration, _scale_factor: f32) {
        self.cached_bg = None;
    }
}

// ── BloomEffect (Simplified Glow) ───────────────────────────────────────────
//
// Note: this is a single-pass threshold + 3×3 blur approximation, not a true
// multi-pass Gaussian bloom. The UV sample offset (0.002) is in UV space and
// therefore not resolution-aware — it will appear coarser at lower resolutions.
// A proper bloom would downsample, blur, and upsample in separate passes.

pub struct BloomEffect {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    cached_bg: Option<wgpu::BindGroup>,
    last_source: usize,
}

impl BloomEffect {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bloom_bgl"),
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

        // Simple threshold + 9-tap blur shader
        let shader = format!("{}
        @group(0) @binding(0) var t_diffuse: texture_2d<f32>;
        @group(0) @binding(1) var s_diffuse: sampler;

        @fragment
        fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {{
            let color = textureSample(t_diffuse, s_diffuse, in.uv);
            
            // Extract brightness
            let brightness = dot(color.rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
            var bloom = vec3<f32>(0.0);
            
            if brightness > 0.7 {{
                bloom = color.rgb;
            }}

            // Basic 3x3 blur
            let offset = 0.002;
            var blur = vec3<f32>(0.0);
            var weight = 0.0;
            for (var y = -1.0; y <= 1.0; y += 1.0) {{
                for (var x = -1.0; x <= 1.0; x += 1.0) {{
                    let sample_uv = in.uv + vec2<f32>(x * offset, y * offset);
                    // Only contribute samples that are within texture bounds
                    if (sample_uv.x >= 0.0 && sample_uv.x <= 1.0 && sample_uv.y >= 0.0 && sample_uv.y <= 1.0) {{
                        blur += textureSample(t_diffuse, s_diffuse, sample_uv).rgb;
                        weight += 1.0;
                    }}
                }}
            }}
            if (weight > 0.0) {{
                blur /= weight;
            }} else {{
                blur = color.rgb;
            }}

            let final_bloom = max(bloom, blur * 0.5);
            return vec4<f32>(color.rgb + final_bloom * 0.4, color.a);
        }}", FULLSCREEN_VS);

        let pipeline = create_fullscreen_pipeline(device, "bloom", &shader, &[&bgl], format);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self { pipeline, bind_group_layout: bgl, sampler, cached_bg: None, last_source: 0 }
    }
}

impl PostProcessEffect for BloomEffect {
    fn effect_name(&self) -> &'static str { "bloom" }

    fn render(&mut self, device: &wgpu::Device, _queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, source_view: &wgpu::TextureView, target_view: &wgpu::TextureView) {
        let src_addr = source_view as *const _ as usize;
        if self.cached_bg.is_none() || self.last_source != src_addr {
            self.cached_bg = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(source_view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
                label: None,
            }));
            self.last_source = src_addr;
        }
        let bind_group = self.cached_bg.as_ref().unwrap();

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("bloom_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..6, 0..1);
    }

    fn resize(&mut self, _device: &wgpu::Device, _queue: &wgpu::Queue, _config: &wgpu::SurfaceConfiguration, _scale_factor: f32) {
        self.cached_bg = None;
    }
}

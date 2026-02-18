pub mod atlas;
pub mod particle_pipeline;
pub mod pipeline;
pub mod sprite_atlas;

use std::sync::Arc;

use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use atlas::Atlas;
use particle_pipeline::{ParticlePipeline, ParticleVertex, create_particle_pipeline};
use pipeline::{TilePipeline, TileVertex, create_tile_pipeline, orthographic_projection};
use sprite_atlas::SpriteAtlas;

pub struct Renderer {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    tile_pipeline: TilePipeline,
    particle_pipeline: ParticlePipeline,
    projection_buffer: wgpu::Buffer,
    projection_bind_group: wgpu::BindGroup,
    /// Bind group for the character/glyph atlas (always present).
    atlas_bind_group: wgpu::BindGroup,
    /// Bind group for the optional sprite atlas (None until load_sprite_folder is called).
    sprite_atlas_bind_group: Option<wgpu::BindGroup>,
    pub(crate) atlas: Atlas,
    /// Loaded sprite atlas metadata (UVs, tile spans, etc.).
    pub(crate) sprite_atlas: Option<SpriteAtlas>,
}

impl Renderer {
    pub async fn new(window: Arc<Window>, png_bytes: &[u8], tile_w: u32, tile_h: u32) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(Arc::clone(&window)).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("no suitable GPU adapter found");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .expect("failed to create device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let atlas = Atlas::from_png(&device, &queue, png_bytes, tile_w, tile_h);
        let tile_pipeline = create_tile_pipeline(&device, format);

        let particle_pipeline = create_particle_pipeline(
            &device,
            format,
            &tile_pipeline.projection_bind_group_layout,
        );

        let proj = orthographic_projection(config.width as f32, config.height as f32);
        let projection_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("projection_buffer"),
            contents: bytemuck::cast_slice(&proj),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let projection_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("projection_bg"),
            layout: &tile_pipeline.projection_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: projection_buffer.as_entire_binding(),
            }],
        });

        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas_bg"),
            layout: &tile_pipeline.atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas.texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas.sampler),
                },
            ],
        });

        Self {
            window,
            surface,
            device,
            queue,
            config,
            tile_pipeline,
            particle_pipeline,
            projection_buffer,
            projection_bind_group,
            atlas_bind_group,
            sprite_atlas_bind_group: None,
            atlas,
            sprite_atlas: None,
        }
    }

    /// Load all `.png` files from `path` (recursively) into the sprite atlas.
    /// Must be called once during initialisation, before the game loop starts.
    pub fn load_sprite_folder(&mut self, path: &str, tile_w: u32, tile_h: u32) {
        let atlas = SpriteAtlas::load_folder(&self.device, &self.queue, path, tile_w, tile_h);

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sprite_atlas_bg"),
            layout: &self.tile_pipeline.atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas.texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas.sampler),
                },
            ],
        });

        self.sprite_atlas_bind_group = Some(bind_group);
        self.sprite_atlas = Some(atlas);
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        self.surface.configure(&self.device, &self.config);

        let proj = orthographic_projection(new_size.width as f32, new_size.height as f32);
        self.queue
            .write_buffer(&self.projection_buffer, 0, bytemuck::cast_slice(&proj));
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Render one frame.
    ///
    /// Draw order within the single render pass:
    /// 1. `char_verts`   — character atlas (bg solid fills + char glyphs)
    /// 2. `sprite_verts` — sprite atlas (static and animated sprites)
    /// 3. `particle_verts` — particle pipeline
    pub fn render(
        &mut self,
        char_verts: &[TileVertex],
        sprite_verts: &[TileVertex],
        particle_verts: &[ParticleVertex],
    ) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.08,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // ── Pass 1: character atlas (bg grid + char glyph grid) ───────
            if !char_verts.is_empty() {
                let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("char_vertex_buffer"),
                    contents: bytemuck::cast_slice(char_verts),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                pass.set_pipeline(&self.tile_pipeline.render_pipeline);
                pass.set_bind_group(0, &self.projection_bind_group, &[]);
                pass.set_bind_group(1, &self.atlas_bind_group, &[]);
                pass.set_vertex_buffer(0, vbuf.slice(..));
                pass.draw(0..char_verts.len() as u32, 0..1);
            }

            // ── Pass 2: sprite atlas (static + animated sprites) ──────────
            if !sprite_verts.is_empty() {
                if let Some(sprite_bg) = &self.sprite_atlas_bind_group {
                    let vbuf =
                        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("sprite_vertex_buffer"),
                            contents: bytemuck::cast_slice(sprite_verts),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                    pass.set_pipeline(&self.tile_pipeline.render_pipeline);
                    pass.set_bind_group(0, &self.projection_bind_group, &[]);
                    pass.set_bind_group(1, sprite_bg, &[]);
                    pass.set_vertex_buffer(0, vbuf.slice(..));
                    pass.draw(0..sprite_verts.len() as u32, 0..1);
                }
            }

            // ── Pass 3: particles ─────────────────────────────────────────
            if !particle_verts.is_empty() {
                let pbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("particle_vertex_buffer"),
                    contents: bytemuck::cast_slice(particle_verts),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                pass.set_pipeline(&self.particle_pipeline.render_pipeline);
                pass.set_bind_group(0, &self.projection_bind_group, &[]);
                pass.set_vertex_buffer(0, pbuf.slice(..));
                pass.draw(0..particle_verts.len() as u32, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}
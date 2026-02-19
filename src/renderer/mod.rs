pub mod atlas;
pub mod particle_pipeline;
pub mod pipeline;
pub mod scanline_pipeline;
pub mod sprite_atlas;
pub mod text;
pub mod utils;

use std::sync::Arc;

use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use atlas::Atlas;
use particle_pipeline::{ParticlePipeline, ParticleVertex, create_particle_pipeline};
use pipeline::{TilePipeline, TileVertex, create_tile_pipeline, orthographic_projection};
use scanline_pipeline::{ScanlinePass, create_scanline_pass, resize_scanline_pass};
use sprite_atlas::SpriteAtlas;

use crate::camera::CameraUniform;

pub struct Renderer {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    tile_pipeline: TilePipeline,
    particle_pipeline: ParticlePipeline,
    /// Static orthographic projection (no camera) — used exclusively by the UI pass.
    projection_buffer: wgpu::Buffer,
    projection_bind_group: wgpu::BindGroup,
    /// Camera view-projection buffer — used by world passes (char, sprite, particle).
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    /// Bind group for the character/glyph atlas (always present).
    atlas_bind_group: wgpu::BindGroup,
    /// Bind group for the optional sprite atlas (None until load_sprite_folder is called).
    sprite_atlas_bind_group: Option<wgpu::BindGroup>,
    pub(crate) atlas: Atlas,
    /// Loaded sprite atlas metadata (UVs, tile spans, etc.).
    pub(crate) sprite_atlas: Option<SpriteAtlas>,
    // ── UI overlay vertex buffer (persistent, invalidated by FNV hash) ─────
    /// Persistent GPU vertex buffer for UI overlay; reallocated only when
    /// vertex count exceeds current capacity (not every frame).
    ui_vertex_buffer: Option<wgpu::Buffer>,
    /// Number of TileVertex slots the current ui_vertex_buffer can hold.
    ui_vertex_buffer_capacity: u32,
    /// FNV-1a hash of the last uploaded UI vertex bytes; used to skip
    /// redundant write_buffer calls when UI is unchanged.
    ui_vertex_hash: u64,
    /// Optional CRT scanline post-process pass.
    scanline_pass: Option<ScanlinePass>,
}

/// FNV-1a 64-bit hash — used to detect unchanged UI vertex data.
fn fnv1a_64(data: &[u8]) -> u64 {
    data.iter().fold(14695981039346656037u64, |h, &b| {
        h.wrapping_mul(1099511628211) ^ b as u64
    })
}

impl Renderer {
    pub async fn new(window: Arc<Window>, png_bytes: &[u8], tile_w: u32, tile_h: u32, use_scanlines: bool) -> Self {
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
            .request_device(&wgpu::DeviceDescriptor::default())
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

        // ── Static UI projection buffer (no camera transform) ─────────────
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

        // ── Camera view-projection buffer (world passes) ──────────────────
        // Initialised to the identity ortho so the first frame looks correct
        // even before Camera::build_view_proj is called.
        let cam_uniform = CameraUniform::identity_ortho(
            config.width as f32,
            config.height as f32,
        );
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera_buffer"),
            contents: bytemuck::cast_slice(&[cam_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera_bg"),
            layout: &tile_pipeline.projection_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
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

        let scanline_pass = if use_scanlines {
            Some(create_scanline_pass(&device, &config, window.scale_factor() as f32))
        } else {
            None
        };

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
            camera_buffer,
            camera_bind_group,
            atlas_bind_group,
            sprite_atlas_bind_group: None,
            atlas,
            sprite_atlas: None,
            ui_vertex_buffer: None,
            ui_vertex_buffer_capacity: 0,
            ui_vertex_hash: 0,
            scanline_pass,
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

        // Keep the UI projection up-to-date with the window size.
        let proj = orthographic_projection(new_size.width as f32, new_size.height as f32);
        self.queue
            .write_buffer(&self.projection_buffer, 0, bytemuck::cast_slice(&proj));

        if let Some(ref mut sp) = self.scanline_pass {
            resize_scanline_pass(sp, &self.device, &self.queue, &self.window, &self.config);
        }
    }

    /// Upload a new camera view-projection matrix to the GPU.
    /// Call this once per frame (after `Camera::tick`, before `render`).
    pub fn update_camera(&mut self, uniform: &CameraUniform) {
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(std::slice::from_ref(uniform)),
        );
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Render one frame.
    ///
    /// Draw order within the single render pass:
    /// 1. `char_verts`     — character atlas (bg solid fills + char glyphs) [camera]
    /// 2. `sprite_verts`   — sprite atlas (static and animated sprites)     [camera]
    /// 3. `particle_verts` — particle pipeline                               [camera]
    /// 4. `ui_verts`       — UI overlay (char atlas, always on top, Layer 2) [screen]
    ///
    /// Passes 1–3 use the camera view-projection (`camera_bind_group`) so they
    /// scroll/zoom with the camera.  Pass 4 uses the plain orthographic projection
    /// (`projection_bind_group`) so UI stays fixed on screen regardless of camera.
    ///
    /// The UI vertex buffer is persistent and only uploaded to the GPU when
    /// the vertex content changes (FNV-1a hash-based invalidation).
    pub fn render(
        &mut self,
        char_verts: &[TileVertex],
        sprite_verts: &[TileVertex],
        particle_verts: &[ParticleVertex],
        ui_verts: &[TileVertex],
    ) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // ── UI vertex buffer invalidation ─────────────────────────────────
        // Only reallocate / re-upload when the UI vertex data actually changes.
        if !ui_verts.is_empty() {
            let ui_bytes: &[u8] = bytemuck::cast_slice(ui_verts);
            let new_hash = fnv1a_64(ui_bytes);
            let new_count = ui_verts.len() as u32;

            if new_count > self.ui_vertex_buffer_capacity || self.ui_vertex_buffer.is_none() {
                // Grow the buffer (next power-of-two, min 256 vertices).
                let capacity = new_count.next_power_of_two().max(256);
                self.ui_vertex_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ui_vertex_buffer"),
                    size: capacity as u64 * std::mem::size_of::<TileVertex>() as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                self.ui_vertex_buffer_capacity = capacity;
                self.ui_vertex_hash = !new_hash; // Force upload on resize.
            }

            if new_hash != self.ui_vertex_hash {
                self.queue.write_buffer(
                    self.ui_vertex_buffer.as_ref().unwrap(),
                    0,
                    ui_bytes,
                );
                self.ui_vertex_hash = new_hash;
            }
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        // Pick render target: intermediate texture (scanlines) or swapchain directly.
        let target_view: &wgpu::TextureView = match &self.scanline_pass {
            Some(sp) => &sp.render_view,
            None     => &view,
        };

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // ── Pass 1: character atlas (bg grid + char glyph grid) [camera] ─
            if !char_verts.is_empty() {
                let vbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("char_vertex_buffer"),
                    contents: bytemuck::cast_slice(char_verts),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                pass.set_pipeline(&self.tile_pipeline.render_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_bind_group(1, &self.atlas_bind_group, &[]);
                pass.set_vertex_buffer(0, vbuf.slice(..));
                pass.draw(0..char_verts.len() as u32, 0..1);
            }

            // ── Pass 2: sprite atlas (static + animated sprites) [camera] ────
            if !sprite_verts.is_empty() {
                if let Some(sprite_bg) = &self.sprite_atlas_bind_group {
                    let vbuf =
                        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("sprite_vertex_buffer"),
                            contents: bytemuck::cast_slice(sprite_verts),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                    pass.set_pipeline(&self.tile_pipeline.render_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_bind_group(1, sprite_bg, &[]);
                    pass.set_vertex_buffer(0, vbuf.slice(..));
                    pass.draw(0..sprite_verts.len() as u32, 0..1);
                }
            }

            // ── Pass 3: particles [camera] ────────────────────────────────────
            if !particle_verts.is_empty() {
                let pbuf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("particle_vertex_buffer"),
                    contents: bytemuck::cast_slice(particle_verts),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                pass.set_pipeline(&self.particle_pipeline.render_pipeline);
                pass.set_bind_group(0, &self.camera_bind_group, &[]);
                pass.set_vertex_buffer(0, pbuf.slice(..));
                pass.draw(0..particle_verts.len() as u32, 0..1);
            }

            // ── Pass 4: UI overlay (char atlas, always on top) [screen] ──────
            // Uses the plain projection_bind_group so UI ignores the camera.
            if !ui_verts.is_empty() {
                if let Some(ui_buf) = &self.ui_vertex_buffer {
                    let count = ui_verts.len() as u32;
                    let byte_len = (count as usize * std::mem::size_of::<TileVertex>()) as u64;
                    pass.set_pipeline(&self.tile_pipeline.render_pipeline);
                    pass.set_bind_group(0, &self.projection_bind_group, &[]);
                    pass.set_bind_group(1, &self.atlas_bind_group, &[]);
                    pass.set_vertex_buffer(0, ui_buf.slice(..byte_len));
                    pass.draw(0..count, 0..1);
                }
            }
        }

        // ── Scanline blit pass (only when enabled) ───────────────────────
        if let Some(ref sp) = self.scanline_pass {
            let mut blit = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scanline_blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            blit.set_pipeline(&sp.pipeline.pipeline);
            blit.set_bind_group(0, &sp.scene_bind_group, &[]);
            blit.set_bind_group(1, &sp.uniforms_bind_group, &[]);
            blit.draw(0..6, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}
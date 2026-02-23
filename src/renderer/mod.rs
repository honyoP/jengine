pub mod atlas;
pub mod particle_pipeline;
pub mod pipeline;
pub mod post_process;
pub mod sprite_atlas;
pub mod text;
pub mod text_pipeline;
pub mod utils;

use std::sync::Arc;

use wgpu::util::DeviceExt;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use atlas::Atlas;
use particle_pipeline::{ParticlePipeline, ParticleVertex, create_particle_pipeline};
use pipeline::{TilePipeline, TileVertex, create_tile_pipeline, orthographic_projection};
use post_process::PostProcessStack;
use sprite_atlas::SpriteAtlas;
use text::Vertex as TextVertex;
use text_pipeline::{TextPipeline, create_text_pipeline};

use crate::camera::CameraUniform;

// ── MtsdfParams ───────────────────────────────────────────────────────────────

/// Per-font parameters uploaded to the text shader's group(1) binding(2).
///
/// `distance_range` comes from the msdf-atlas-gen JSON (`atlas.distanceRange`).
/// `atlas_width` / `atlas_height` are read directly from the loaded PNG texture.
/// The shader uses these to compute the correct screen-pixel AA band at any scale.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct MtsdfParams {
    distance_range: f32,
    atlas_width:    f32,
    atlas_height:   f32,
    _pad:           f32,
}

// The MTSDF font atlas is baked in at compile time.
// It is loaded as a *separate* texture from the tile atlas so it can use
// Linear filtering (required for SDF reconstruction) and Rgba8Unorm format
// (no gamma conversion — distance values are linear).
static MTSDF_FONT_PNG: &[u8] = include_bytes!("../../resources/font_atlas.png");

pub struct Renderer {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    tile_pipeline: TilePipeline,
    particle_pipeline: ParticlePipeline,
    /// MTSDF text pipeline (separate shader, vertex format, and sampler).
    text_pipeline: TextPipeline,
    /// Static orthographic projection (no camera) — used by UI and text passes.
    projection_buffer: wgpu::Buffer,
    projection_bind_group: wgpu::BindGroup,
    /// Text pipeline's own projection bind group (same buffer, compatible layout).
    text_projection_bind_group: wgpu::BindGroup,
    /// Camera view-projection buffer — used by world passes (char, sprite, particle).
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    /// Storage buffer for entity animation offsets [f32; 2], indexed by entity_id.
    entity_offsets_buffer: wgpu::Buffer,
    entity_offsets_bind_group: wgpu::BindGroup,
    /// Bind group for the character/glyph tile atlas (always present).
    atlas_bind_group: wgpu::BindGroup,
    /// Bind group for the optional sprite atlas (None until load_sprite_folder is called).
    sprite_atlas_bind_group: Option<wgpu::BindGroup>,
    /// Keeps the MTSDF font GPU texture alive (TextureView holds a ref-count
    /// internally, but storing the Texture here makes ownership unambiguous).
    #[allow(dead_code)]
    font_texture: wgpu::Texture,
    /// Bind group for the MTSDF font atlas (Linear sampler, Rgba8Unorm, + params).
    font_bind_group: wgpu::BindGroup,
    /// Cached MTSDF parameters (distance_range, atlas size) mirrored on the CPU
    /// so `set_mtsdf_distance_range` can patch only the range without re-reading
    /// the buffer from the GPU.
    mtsdf_params: MtsdfParams,
    /// GPU buffer for [`MtsdfParams`]; written via `queue.write_buffer`.
    mtsdf_params_buffer: wgpu::Buffer,
    // ── Persistent geometry buffers (capacity-doubling) ──────────────────
    // All six buffers below use the same strategy: reallocate only when the
    // current capacity is exceeded; write new data each frame via write_buffer.
    // This avoids per-frame GPU allocations on the hot rendering path.
    char_vertex_buffer: Option<wgpu::Buffer>,
    char_vertex_buffer_capacity: u32,
    sprite_vertex_buffer: Option<wgpu::Buffer>,
    sprite_vertex_buffer_capacity: u32,
    particle_vertex_buffer: Option<wgpu::Buffer>,
    particle_vertex_buffer_capacity: u32,
    text_vertex_buffer: Option<wgpu::Buffer>,
    text_vertex_buffer_capacity: u32,
    text_index_buffer: Option<wgpu::Buffer>,
    text_index_buffer_capacity: u32,
    pub(crate) atlas: Atlas,
    /// Loaded sprite atlas metadata (UVs, tile spans, etc.).
    pub(crate) sprite_atlas: Option<SpriteAtlas>,
    // ── UI overlay vertex buffer (persistent, invalidated by FNV hash) ─────
    ui_vertex_buffer: Option<wgpu::Buffer>,
    ui_vertex_buffer_capacity: u32,
    ui_vertex_hash: u64,
    /// Post-processing stack for "juice" effects.
    pub post_process: PostProcessStack,
}

/// FNV-1a 64-bit hash — used to detect unchanged UI vertex data.
/// Algorithm: XOR byte into accumulator, then multiply by the FNV prime.
fn fnv1a_64(data: &[u8]) -> u64 {
    data.iter().fold(14695981039346656037u64, |h, &b| {
        (h ^ b as u64).wrapping_mul(1099511628211)
    })
}

/// Load a PNG from raw bytes as an `Rgba8Unorm` texture (no gamma conversion).
/// Used for the MTSDF atlas where channel values are linear distance fields.
fn load_rgba8_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    png_bytes: &[u8],
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let img = image::load_from_memory(png_bytes)
        .expect("failed to load MTSDF font PNG")
        .to_rgba8();
    let (w, h) = img.dimensions();

    let texture = device.create_texture_with_data(
        queue,
        &wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Rgba8Unorm (not sRGB) — SDF values must not be gamma-corrected.
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        },
        wgpu::util::TextureDataOrder::LayerMajor,
        &img,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
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
        
        // ── Entity Offsets Storage Buffer ──
        // Pre-allocated for MAX_ANIMATED_ENTITIES entries (16 bytes each = [f32;4] for alignment).
        let initial_offsets = vec![[0.0f32, 0.0, 0.0, 0.0]; crate::engine::MAX_ANIMATED_ENTITIES];
        let entity_offsets_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("entity_offsets_buffer"),
            contents: bytemuck::cast_slice(&initial_offsets),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let tile_pipeline = create_tile_pipeline(&device, format);
        
        let entity_offsets_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("entity_offsets_bg"),
            layout: &tile_pipeline.entity_offsets_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: entity_offsets_buffer.as_entire_binding(),
            }],
        });

        let text_pipeline = create_text_pipeline(&device, format);

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

        // Text pipeline uses its own layout but the same buffer.
        let text_projection_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text_projection_bg"),
            layout: &text_pipeline.projection_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: projection_buffer.as_entire_binding(),
            }],
        });

        // ── Camera view-projection buffer (world passes) ──────────────────
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

        // ── Tile atlas bind group (Nearest sampler) ───────────────────────
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

        // ── MTSDF font atlas (Linear sampler, Rgba8Unorm) ─────────────────
        let (font_texture, font_view) =
            load_rgba8_texture(&device, &queue, MTSDF_FONT_PNG, "mtsdf_font_atlas");
        let font_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mtsdf_font_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Initialise MTSDF params from the loaded texture dimensions.
        // distance_range defaults to 4.0 (common msdf-atlas-gen default).
        // Callers should invoke `set_mtsdf_distance_range` after loading the
        // font JSON to supply the exact value for their atlas.
        let mtsdf_params = MtsdfParams {
            distance_range: 4.0,
            atlas_width:    font_texture.width() as f32,
            atlas_height:   font_texture.height() as f32,
            _pad: 0.0,
        };
        let mtsdf_params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mtsdf_params_buffer"),
            contents: bytemuck::cast_slice(&[mtsdf_params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let font_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("font_bg"),
            layout: &text_pipeline.font_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&font_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&font_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: mtsdf_params_buffer.as_entire_binding(),
                },
            ],
        });

        let mut post_process = PostProcessStack::new(&device, &config);
        if use_scanlines {
            post_process.add_effect(Box::new(post_process::ScanlineEffect::new(
                &device,
                config.format,
                window.scale_factor() as f32,
            )));
        }

        Self {
            window,
            surface,
            device,
            queue,
            config,
            tile_pipeline,
            particle_pipeline,
            text_pipeline,
            projection_buffer,
            projection_bind_group,
            text_projection_bind_group,
            camera_buffer,
            camera_bind_group,
            entity_offsets_buffer,
            entity_offsets_bind_group,
            atlas_bind_group,
            sprite_atlas_bind_group: None,
            font_texture,
            font_bind_group,
            mtsdf_params,
            mtsdf_params_buffer,
            char_vertex_buffer: None,
            char_vertex_buffer_capacity: 0,
            sprite_vertex_buffer: None,
            sprite_vertex_buffer_capacity: 0,
            particle_vertex_buffer: None,
            particle_vertex_buffer_capacity: 0,
            text_vertex_buffer: None,
            text_vertex_buffer_capacity: 0,
            text_index_buffer: None,
            text_index_buffer_capacity: 0,
            atlas,
            sprite_atlas: None,
            ui_vertex_buffer: None,
            ui_vertex_buffer_capacity: 0,
            ui_vertex_hash: 0,
            post_process,
        }
    }

    /// Load all `.png` files from `path` (recursively) into the sprite atlas.
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

    /// Returns the metadata (UVs, spans) for a named sprite if it exists.
    pub fn get_sprite_data(&self, name: &str) -> Option<crate::renderer::sprite_atlas::SpriteData> {
        self.sprite_atlas.as_ref()?.get_data(name).cloned()
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

        self.post_process.resize(&self.device, &self.queue, &self.config, self.window.scale_factor() as f32);
    }

    /// Upload a new camera view-projection matrix to the GPU.
    pub fn update_camera(&mut self, uniform: &CameraUniform) {
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(std::slice::from_ref(uniform)),
        );
    }

    /// Upload an array of [f32; 4] offsets for entity animation.
    pub fn update_entity_offsets(&mut self, offsets: &[[f32; 4]]) {
        if !offsets.is_empty() {
            self.queue.write_buffer(
                &self.entity_offsets_buffer,
                0,
                bytemuck::cast_slice(offsets),
            );
        }
    }

    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Update the MTSDF distance range used by the text shader.
    ///
    /// Call this once after registering a font via `TextLayer::set_font`, passing
    /// `font.distance_range` from the loaded [`Font`].  The atlas dimensions are
    /// taken from the actual GPU texture and do not need to be supplied again.
    ///
    /// [`Font`]: super::text::Font
    pub fn set_mtsdf_distance_range(&mut self, range: f32) {
        self.mtsdf_params.distance_range = range;
        self.queue.write_buffer(
            &self.mtsdf_params_buffer,
            0,
            bytemuck::cast_slice(&[self.mtsdf_params]),
        );
    }

    /// Render one frame.
    ///
    /// Draw order within the single render pass:
    /// 1. `char_verts`     — character tile atlas (bg fills + char glyphs)   [camera]
    /// 2. `sprite_verts`   — sprite atlas (static and animated sprites)       [camera]
    /// 3. `particle_verts` — particle pipeline                                [camera]
    /// 4. `ui_verts`       — UI solid fills (TileVertex, Layer 2)             [screen]
    /// 5. `text_verts`     — MTSDF text (Labels + ui_char_at)                 [screen]
    ///
    /// Passes 1–3 use the camera bind group (scroll/zoom).
    /// Passes 4–5 use the plain projection bind group (screen-fixed).
    pub fn render(
        &mut self,
        char_verts: &[TileVertex],
        sprite_verts: &[TileVertex],
        particle_verts: &[ParticleVertex],
        ui_verts: &[TileVertex],
        text_verts: &[TextVertex],
        text_indices: &[u16],
    ) -> Result<(), wgpu::SurfaceError> {
        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // ── Persistent buffer helpers ─────────────────────────────────────
        //
        // Shared capacity-doubling logic used by all six geometry buffers.
        // Reallocates only when the current capacity is exceeded, then writes
        // new data each frame via `write_buffer` (no map/unmap overhead).
        macro_rules! upload_vertex_buf {
            ($buf:expr, $cap:expr, $data:expr, $ty:ty, $label:literal) => {
                if !$data.is_empty() {
                    let count = $data.len() as u32;
                    if count > $cap || $buf.is_none() {
                        let new_cap = count.next_power_of_two().max(256);
                        $buf = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some($label),
                            size: new_cap as u64 * std::mem::size_of::<$ty>() as u64,
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        }));
                        $cap = new_cap;
                    }
                    self.queue.write_buffer(
                        $buf.as_ref().unwrap(), 0, bytemuck::cast_slice($data));
                }
            };
        }

        upload_vertex_buf!(self.char_vertex_buffer,     self.char_vertex_buffer_capacity,     char_verts,     TileVertex,     "char_vertex_buffer");
        upload_vertex_buf!(self.sprite_vertex_buffer,   self.sprite_vertex_buffer_capacity,   sprite_verts,   TileVertex,     "sprite_vertex_buffer");
        upload_vertex_buf!(self.particle_vertex_buffer, self.particle_vertex_buffer_capacity, particle_verts, ParticleVertex, "particle_vertex_buffer");

        // ── Text vertex/index buffer management ───────────────────────────
        if !text_indices.is_empty() {
            let vert_count = text_verts.len() as u32;
            if vert_count > self.text_vertex_buffer_capacity || self.text_vertex_buffer.is_none() {
                let cap = vert_count.next_power_of_two().max(256);
                self.text_vertex_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("text_vertex_buffer"),
                    size: cap as u64 * std::mem::size_of::<TextVertex>() as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                self.text_vertex_buffer_capacity = cap;
            }
            self.queue.write_buffer(
                self.text_vertex_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(text_verts),
            );

            let idx_count = text_indices.len() as u32;
            if idx_count > self.text_index_buffer_capacity || self.text_index_buffer.is_none() {
                let cap = idx_count.next_power_of_two().max(256);
                self.text_index_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("text_index_buffer"),
                    size: cap as u64 * std::mem::size_of::<u16>() as u64,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                self.text_index_buffer_capacity = cap;
            }
            self.queue.write_buffer(
                self.text_index_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(text_indices),
            );
        }

        // ── UI vertex buffer invalidation ─────────────────────────────────
        if !ui_verts.is_empty() {
            let ui_bytes: &[u8] = bytemuck::cast_slice(ui_verts);
            let new_hash = fnv1a_64(ui_bytes);
            let new_count = ui_verts.len() as u32;

            if new_count > self.ui_vertex_buffer_capacity || self.ui_vertex_buffer.is_none() {
                let capacity = new_count.next_power_of_two().max(256);
                self.ui_vertex_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("ui_vertex_buffer"),
                    size: capacity as u64 * std::mem::size_of::<TileVertex>() as u64,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                self.ui_vertex_buffer_capacity = capacity;
                self.ui_vertex_hash = !new_hash;
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

        let target_view: &wgpu::TextureView = if self.post_process.is_empty() {
            &view
        } else {
            self.post_process.main_render_target()
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
                            r: 0.0, g: 0.0, b: 0.0, a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // ── Pass 1: character tile atlas [camera] ─────────────────────
            if !char_verts.is_empty() {
                if let Some(vbuf) = &self.char_vertex_buffer {
                    let byte_len = char_verts.len() as u64 * std::mem::size_of::<TileVertex>() as u64;
                    pass.set_pipeline(&self.tile_pipeline.render_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_bind_group(1, &self.atlas_bind_group, &[]);
                    pass.set_bind_group(2, &self.entity_offsets_bind_group, &[]);
                    pass.set_vertex_buffer(0, vbuf.slice(..byte_len));
                    pass.draw(0..char_verts.len() as u32, 0..1);
                }
            }

            // ── Pass 2: sprite atlas [camera] ─────────────────────────────
            if !sprite_verts.is_empty() {
                if let (Some(vbuf), Some(sprite_bg)) =
                    (&self.sprite_vertex_buffer, &self.sprite_atlas_bind_group)
                {
                    let byte_len = sprite_verts.len() as u64 * std::mem::size_of::<TileVertex>() as u64;
                    pass.set_pipeline(&self.tile_pipeline.render_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_bind_group(1, sprite_bg, &[]);
                    pass.set_bind_group(2, &self.entity_offsets_bind_group, &[]);
                    pass.set_vertex_buffer(0, vbuf.slice(..byte_len));
                    pass.draw(0..sprite_verts.len() as u32, 0..1);
                }
            }

            // ── Pass 3: particles [camera] ────────────────────────────────
            if !particle_verts.is_empty() {
                if let Some(pbuf) = &self.particle_vertex_buffer {
                    let byte_len = particle_verts.len() as u64 * std::mem::size_of::<ParticleVertex>() as u64;
                    pass.set_pipeline(&self.particle_pipeline.render_pipeline);
                    pass.set_bind_group(0, &self.camera_bind_group, &[]);
                    pass.set_vertex_buffer(0, pbuf.slice(..byte_len));
                    pass.draw(0..particle_verts.len() as u32, 0..1);
                }
            }

            // ── Pass 4: UI solid fills (TileVertex, screen-fixed) ─────────
            if !ui_verts.is_empty() {
                if let Some(ui_buf) = &self.ui_vertex_buffer {
                    let count = ui_verts.len() as u32;
                    let byte_len =
                        (count as usize * std::mem::size_of::<TileVertex>()) as u64;
                    pass.set_pipeline(&self.tile_pipeline.render_pipeline);
                    pass.set_bind_group(0, &self.projection_bind_group, &[]);
                    pass.set_bind_group(1, &self.atlas_bind_group, &[]);
                    pass.set_bind_group(2, &self.entity_offsets_bind_group, &[]);
                    pass.set_vertex_buffer(0, ui_buf.slice(..byte_len));
                    pass.draw(0..count, 0..1);
                }
            }

            // ── Pass 5: MTSDF text (Labels + ui_char_at, screen-fixed) ───
            if !text_indices.is_empty() {
                // Buffers were uploaded in the pre-pass section above.
                let vbyte_len =
                    text_verts.len() as u64 * std::mem::size_of::<TextVertex>() as u64;
                let ibyte_len =
                    text_indices.len() as u64 * std::mem::size_of::<u16>() as u64;
                pass.set_pipeline(&self.text_pipeline.render_pipeline);
                pass.set_bind_group(0, &self.text_projection_bind_group, &[]);
                pass.set_bind_group(1, &self.font_bind_group, &[]);
                pass.set_vertex_buffer(
                    0,
                    self.text_vertex_buffer.as_ref().unwrap().slice(..vbyte_len),
                );
                pass.set_index_buffer(
                    self.text_index_buffer.as_ref().unwrap().slice(..ibyte_len),
                    wgpu::IndexFormat::Uint16,
                );
                pass.draw_indexed(0..text_indices.len() as u32, 0, 0..1);
            }
        }

        // ── Post-processing stack ─────────────────────────────────────────
        if !self.post_process.is_empty() {
            self.post_process.run(&self.device, &self.queue, &mut encoder, &view);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(())
    }
}

use std::collections::{HashMap, HashSet};

use image::RgbaImage;
use wgpu::util::DeviceExt;

// ── SpriteData ───────────────────────────────────────────────────────────────

/// UV coordinates and tile-span metadata for a single named sprite.
#[derive(Clone, Debug)]
pub struct SpriteData {
    pub uv_min: [f32; 2],
    pub uv_max: [f32; 2],
    /// How many tile-unit columns this sprite spans.
    pub tile_w_span: u32,
    /// How many tile-unit rows this sprite spans.
    pub tile_h_span: u32,
}

// ── Shelf packing (pure, GPU-free) ───────────────────────────────────────────

/// One sprite's position inside the packed atlas.
#[derive(Debug, PartialEq)]
pub struct PlacedSprite {
    pub name: String,
    /// Top-left pixel coordinate inside the atlas.
    pub atlas_x: u32,
    pub atlas_y: u32,
    /// Pixel dimensions of this sprite.
    pub pixel_w: u32,
    pub pixel_h: u32,
}

/// Pure shelf-packing algorithm — no I/O, no GPU.
///
/// `items` is a slice of `(name, pixel_w, pixel_h)`.  Duplicate names are
/// skipped (only the first occurrence is packed).  Sprites wider than
/// `max_width` are skipped with a warning.
///
/// Returns `(placements, atlas_pixel_width, atlas_pixel_height)`.  Both
/// atlas dimensions are rounded up to the next power of two.
pub fn pack(items: &[(String, u32, u32)], max_width: u32) -> (Vec<PlacedSprite>, u32, u32) {
    // Sort by height descending for better shelf utilisation.
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| items[b].2.cmp(&items[a].2));

    let mut placed_names: HashSet<&str> = HashSet::new();
    let mut placements: Vec<PlacedSprite> = Vec::new();
    let mut cur_x = 0u32;
    let mut cur_y = 0u32;
    let mut row_h = 0u32;

    for &i in &order {
        let (ref name, w, h) = items[i];

        // Skip duplicates — only the first (tallest-sorted) occurrence is placed.
        if !placed_names.insert(name.as_str()) {
            continue;
        }

        if w > max_width {
            eprintln!("sprite_atlas: '{name}' is wider ({w}px) than ATLAS_WIDTH ({max_width}); skipping");
            continue;
        }

        if cur_x + w > max_width {
            // Start a new shelf.
            cur_y += row_h;
            cur_x = 0;
            row_h = 0;
        }

        placements.push(PlacedSprite {
            name: name.clone(),
            atlas_x: cur_x,
            atlas_y: cur_y,
            pixel_w: w,
            pixel_h: h,
        });
        cur_x += w;
        row_h = row_h.max(h);
    }

    let used_h = cur_y + row_h;
    let atlas_h = used_h.next_power_of_two().max(1);
    let atlas_w = max_width.next_power_of_two();
    (placements, atlas_w, atlas_h)
}

// ── SpriteAtlas ──────────────────────────────────────────────────────────────

pub struct SpriteAtlas {
    pub sprites: HashMap<String, SpriteData>,
    pub texture_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl SpriteAtlas {
    /// Maximum row width of the packed atlas texture in pixels.
    const ATLAS_WIDTH: u32 = 512;

    /// Scan `path` recursively for `.png` files, pack them with a shelf
    /// algorithm, upload to the GPU, and return a ready-to-use atlas.
    ///
    /// Duplicate file-stem names are deduplicated at load time so that the
    /// pixel dimensions stored in each placement always match the image
    /// that will be copied at bake time.
    pub fn load_folder(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &str,
        tile_w: u32,
        tile_h: u32,
    ) -> Self {
        // ── 1. Discover and load PNG files ───────────────────────────────
        let mut loaded: Vec<(String, image::DynamicImage)> = Vec::new();
        let mut seen_names: HashSet<String> = HashSet::new();

        for entry in walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let file_path = entry.path();
            if file_path.extension().and_then(|s| s.to_str()) != Some("png") {
                continue;
            }
            let name = match file_path.file_stem().and_then(|s| s.to_str()) {
                Some(n) if !n.is_empty() => n.to_string(),
                _ => continue,
            };

            // Deduplicate: only the first file with a given stem name is used.
            if !seen_names.insert(name.clone()) {
                eprintln!(
                    "sprite_atlas: duplicate name '{}' from {:?}; skipping",
                    name, file_path
                );
                continue;
            }

            match image::open(file_path) {
                Ok(img) => loaded.push((name, img)),
                Err(e) => eprintln!("sprite_atlas: failed to load {:?}: {e}", file_path),
            }
        }

        if loaded.is_empty() {
            return Self::empty(device, queue);
        }

        // ── 2. Pack (pure, no GPU) ─────────────────────────────────────
        // Each (name, w, h) tuple is built from the same DynamicImage that
        // will be copied below, so pw/ph can never diverge from the actual
        // image dimensions.
        let dims: Vec<(String, u32, u32)> = loaded
            .iter()
            .map(|(name, img)| (name.clone(), img.width(), img.height()))
            .collect();

        let (placements, atlas_w, atlas_h) = pack(&dims, Self::ATLAS_WIDTH);

        // ── 3. Composite into a single RGBA image ─────────────────────
        let mut atlas_img = RgbaImage::new(atlas_w, atlas_h);

        let img_lookup: HashMap<&str, &image::DynamicImage> =
            loaded.iter().map(|(n, i)| (n.as_str(), i)).collect();

        let mut sprites = HashMap::new();

        for p in &placements {
            // img_lookup always matches p.name because loaded is deduplicated.
            let Some(img) = img_lookup.get(p.name.as_str()) else { continue };
            let rgba = img.to_rgba8();

            // Use the pixel dimensions from PlacedSprite (== rgba.width/height)
            // as both loop bounds and as the basis for UV computation.
            for dy in 0..p.pixel_h {
                for dx in 0..p.pixel_w {
                    atlas_img.put_pixel(
                        p.atlas_x + dx,
                        p.atlas_y + dy,
                        *rgba.get_pixel(dx, dy),
                    );
                }
            }

            let uv_min = [
                p.atlas_x as f32 / atlas_w as f32,
                p.atlas_y as f32 / atlas_h as f32,
            ];
            let uv_max = [
                (p.atlas_x + p.pixel_w) as f32 / atlas_w as f32,
                (p.atlas_y + p.pixel_h) as f32 / atlas_h as f32,
            ];
            let tile_w_span = ((p.pixel_w as f32 / tile_w as f32).round() as u32).max(1);
            let tile_h_span = ((p.pixel_h as f32 / tile_h as f32).round() as u32).max(1);

            sprites.insert(p.name.clone(), SpriteData { uv_min, uv_max, tile_w_span, tile_h_span });
        }

        // ── 4. Upload to GPU ──────────────────────────────────────────
        let (texture_view, sampler) = Self::upload(device, queue, &atlas_img);
        Self { sprites, texture_view, sampler }
    }

    /// Returns the metadata for a named sprite.
    pub fn get_data(&self, name: &str) -> Option<&SpriteData> {
        self.sprites.get(name)
    }

    /// Create a 1×1 transparent atlas when no sprites are available.
    fn empty(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let img = RgbaImage::new(1, 1);
        let (texture_view, sampler) = Self::upload(device, queue, &img);
        Self { sprites: HashMap::new(), texture_view, sampler }
    }

    fn upload(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        img: &RgbaImage,
    ) -> (wgpu::TextureView, wgpu::Sampler) {
        let (w, h) = img.dimensions();
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("sprite_atlas_tex"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            img.as_raw(),
        );
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        (texture_view, sampler)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────


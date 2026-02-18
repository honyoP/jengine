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
pub(crate) struct PlacedSprite {
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
pub(crate) fn pack(items: &[(String, u32, u32)], max_width: u32) -> (Vec<PlacedSprite>, u32, u32) {
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

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: build an item tuple.
    fn item(name: &str, w: u32, h: u32) -> (String, u32, u32) {
        (name.to_string(), w, h)
    }

    // ── pack() correctness ────────────────────────────────────────────────

    #[test]
    fn pack_empty_input_returns_no_placements() {
        let (placements, atlas_w, atlas_h) = pack(&[], 512);
        assert!(placements.is_empty());
        // Atlas height rounds 0 → next_power_of_two(0).max(1) = 1.
        assert_eq!(atlas_h, 1);
        assert_eq!(atlas_w, 512);
    }

    #[test]
    fn pack_single_sprite_placed_at_origin() {
        let items = [item("hero", 16, 24)];
        let (pl, _, _) = pack(&items, 512);
        assert_eq!(pl.len(), 1);
        assert_eq!(pl[0].atlas_x, 0);
        assert_eq!(pl[0].atlas_y, 0);
        assert_eq!(pl[0].pixel_w, 16);
        assert_eq!(pl[0].pixel_h, 24);
    }

    #[test]
    fn pack_two_sprites_on_same_shelf() {
        // Both fit side-by-side within ATLAS_WIDTH.
        let items = [item("a", 16, 24), item("b", 16, 24)];
        let (pl, _, _) = pack(&items, 512);
        assert_eq!(pl.len(), 2);
        // One is at x=0, the other at x=16 (order may differ due to height sort).
        let xs: Vec<u32> = pl.iter().map(|p| p.atlas_x).collect();
        assert!(xs.contains(&0) && xs.contains(&16));
        // Both on the same row (y=0).
        assert!(pl.iter().all(|p| p.atlas_y == 0));
    }

    #[test]
    fn pack_wraps_to_next_shelf_when_row_full() {
        // Three 200px-wide sprites; the third won't fit in a 512px row.
        let items = [item("a", 200, 32), item("b", 200, 32), item("c", 200, 32)];
        let (pl, _, _) = pack(&items, 512);
        assert_eq!(pl.len(), 3);
        let row0: Vec<_> = pl.iter().filter(|p| p.atlas_y == 0).collect();
        let row1: Vec<_> = pl.iter().filter(|p| p.atlas_y > 0).collect();
        assert_eq!(row0.len(), 2, "first two sprites fit on row 0");
        assert_eq!(row1.len(), 1, "third sprite wraps to row 1");
        assert_eq!(row1[0].atlas_y, 32, "row 1 starts at y = row-0 height");
    }

    #[test]
    fn pack_sorts_taller_sprites_first() {
        // The tall sprite should appear on row 0 even though it was listed last.
        let items = [item("small", 32, 16), item("tall", 32, 64)];
        let (pl, _, _) = pack(&items, 512);
        let tall = pl.iter().find(|p| p.name == "tall").unwrap();
        let small = pl.iter().find(|p| p.name == "small").unwrap();
        assert_eq!(tall.atlas_y, 0, "tallest sprite always placed first");
        assert_eq!(small.atlas_y, 0, "shorter sprite shares the same shelf");
        // Tall is to the left of small (placed first).
        assert!(tall.atlas_x < small.atlas_x);
    }

    #[test]
    fn pack_skips_sprite_wider_than_atlas() {
        let items = [item("giant", 600, 48), item("normal", 16, 24)];
        let (pl, _, _) = pack(&items, 512);
        assert_eq!(pl.len(), 1, "oversized sprite is excluded");
        assert_eq!(pl[0].name, "normal");
    }

    #[test]
    fn pack_dedup_only_places_first_occurrence_of_name() {
        // Two entries share the name "hero"; only the first (tallest after sort)
        // should appear in the placements.
        let items = [item("hero", 16, 24), item("hero", 64, 64)];
        let (pl, _, _) = pack(&items, 512);
        assert_eq!(pl.len(), 1, "duplicate name produces only one placement");
        // The 64x64 is taller so it will be sorted first and placed; the 16x24 is
        // the duplicate and should be dropped.
        assert_eq!(pl[0].pixel_w, 64);
        assert_eq!(pl[0].pixel_h, 64);
    }

    #[test]
    fn pack_atlas_width_is_power_of_two() {
        let items = [item("a", 10, 10)];
        let (_, atlas_w, _) = pack(&items, 100);
        assert!(atlas_w.is_power_of_two(), "atlas_w={atlas_w} must be a power of two");
    }

    #[test]
    fn pack_atlas_height_is_power_of_two() {
        let items = [item("a", 16, 24), item("b", 16, 24)];
        let (_, _, atlas_h) = pack(&items, 512);
        assert!(atlas_h.is_power_of_two(), "atlas_h={atlas_h} must be a power of two");
    }

    #[test]
    fn pack_no_placement_overflows_atlas_x() {
        let items: Vec<_> = (0..10).map(|i| item(&format!("s{i}"), 40, 20)).collect();
        let (pl, atlas_w, _) = pack(&items, 256);
        for p in &pl {
            assert!(
                p.atlas_x + p.pixel_w <= atlas_w,
                "sprite '{}' overflows atlas x: {}+{} > {atlas_w}",
                p.name, p.atlas_x, p.pixel_w
            );
        }
    }

    #[test]
    fn pack_no_placement_overflows_atlas_y() {
        let items: Vec<_> = (0..6).map(|i| item(&format!("s{i}"), 100, 30)).collect();
        let (pl, _, atlas_h) = pack(&items, 256);
        for p in &pl {
            assert!(
                p.atlas_y + p.pixel_h <= atlas_h,
                "sprite '{}' overflows atlas y: {}+{} > {atlas_h}",
                p.name, p.atlas_y, p.pixel_h
            );
        }
    }

    // ── Tile-span computation ─────────────────────────────────────────────

    fn tile_span(pixel_w: u32, pixel_h: u32, tile_w: u32, tile_h: u32) -> (u32, u32) {
        let tw = ((pixel_w as f32 / tile_w as f32).round() as u32).max(1);
        let th = ((pixel_h as f32 / tile_h as f32).round() as u32).max(1);
        (tw, th)
    }

    #[test]
    fn tile_span_1x1_for_exact_tile_size() {
        assert_eq!(tile_span(16, 24, 16, 24), (1, 1));
    }

    #[test]
    fn tile_span_2x2_for_double_tile_size() {
        // big_enemy: 32x48 on a 16x24 grid = 2×2.
        assert_eq!(tile_span(32, 48, 16, 24), (2, 2));
    }

    #[test]
    fn tile_span_rounds_to_nearest_tile() {
        // 24px wide on a 16px grid → 24/16 = 1.5 → rounds to 2.
        let (tw, _) = tile_span(24, 24, 16, 24);
        assert_eq!(tw, 2);
    }

    #[test]
    fn tile_span_minimum_is_one_even_for_tiny_sprites() {
        // A 1×1 pixel sprite on a 16×24 grid must still report span (1, 1).
        assert_eq!(tile_span(1, 1, 16, 24), (1, 1));
    }

    // ── UV value constraints ──────────────────────────────────────────────

    #[test]
    fn pack_uvs_within_zero_one_range() {
        let items = [item("a", 32, 48), item("b", 16, 24)];
        let (pl, atlas_w, atlas_h) = pack(&items, 512);
        for p in &pl {
            let uv_min = [p.atlas_x as f32 / atlas_w as f32, p.atlas_y as f32 / atlas_h as f32];
            let uv_max = [
                (p.atlas_x + p.pixel_w) as f32 / atlas_w as f32,
                (p.atlas_y + p.pixel_h) as f32 / atlas_h as f32,
            ];
            for v in uv_min.iter().chain(uv_max.iter()) {
                assert!(*v >= 0.0 && *v <= 1.0, "UV {v} out of [0,1] for '{}'", p.name);
            }
        }
    }

    #[test]
    fn pack_uv_min_strictly_less_than_uv_max() {
        let items = [item("a", 16, 24)];
        let (pl, atlas_w, atlas_h) = pack(&items, 512);
        let p = &pl[0];
        let uv_min_x = p.atlas_x as f32 / atlas_w as f32;
        let uv_max_x = (p.atlas_x + p.pixel_w) as f32 / atlas_w as f32;
        let uv_min_y = p.atlas_y as f32 / atlas_h as f32;
        let uv_max_y = (p.atlas_y + p.pixel_h) as f32 / atlas_h as f32;
        assert!(uv_min_x < uv_max_x, "uv_min_x must be < uv_max_x");
        assert!(uv_min_y < uv_max_y, "uv_min_y must be < uv_max_y");
    }

    // ── Regression: the duplicate-name crash ─────────────────────────────

    #[test]
    fn pack_duplicate_name_does_not_cause_dimension_mismatch() {
        // Simulates the original crash: two different-sized images share a name.
        // The smaller image's pw/ph (from its placement) must not be applied
        // to the larger image (which img_map would return after overwrite).
        // With the dedup fix, only ONE placement is created, so no mismatch.
        let items = [
            item("big_enemy", 32, 48),   // first seen
            item("big_enemy", 64, 96),   // duplicate — must be dropped
        ];
        let (pl, _, _) = pack(&items, 512);
        assert_eq!(pl.len(), 1);
        // Only one placement; its dimensions are those of the first occurrence
        // (which is 64x96 since the packer sorts tallest-first, making 64x96
        //  the first-seen after sorting).
        assert_eq!(pl[0].pixel_w, 64);
        assert_eq!(pl[0].pixel_h, 96);
    }
}
use jengine::renderer::sprite_atlas::{pack, PlacedSprite};
use jengine::renderer::utils::{letterbox_viewport, Viewport};
use jengine::window::{WindowConfig, WindowMode};

// ── Sprite Atlas Packing Tests ────────────────────────────────────────────

// Helper: build an item tuple.
fn item(name: &str, w: u32, h: u32) -> (String, u32, u32) {
    (name.to_string(), w, h)
}

#[test]
fn pack_empty_input_returns_no_placements() {
    let (placements, _atlas_w, atlas_h): (Vec<PlacedSprite>, u32, u32) = pack(&[], 512);
    assert!(placements.is_empty());
    assert_eq!(atlas_h, 1);
}

#[test]
fn pack_single_sprite_placed_at_origin() {
    let items = [item("hero", 16, 24)];
    let (pl, _, _): (Vec<PlacedSprite>, u32, u32) = pack(&items, 512);
    assert_eq!(pl.len(), 1);
    assert_eq!(pl[0].atlas_x, 0);
    assert_eq!(pl[0].atlas_y, 0);
    assert_eq!(pl[0].pixel_w, 16);
    assert_eq!(pl[0].pixel_h, 24);
}

#[test]
fn pack_two_sprites_on_same_shelf() {
    let items = [item("a", 16, 24), item("b", 16, 24)];
    let (pl, _, _): (Vec<PlacedSprite>, u32, u32) = pack(&items, 512);
    assert_eq!(pl.len(), 2);
    let xs: Vec<u32> = pl.iter().map(|p| p.atlas_x).collect();
    assert!(xs.contains(&0) && xs.contains(&16));
    assert!(pl.iter().all(|p| p.atlas_y == 0));
}

#[test]
fn pack_wraps_to_next_shelf_when_row_full() {
    let items = [item("a", 200, 32), item("b", 200, 32), item("c", 200, 32)];
    let (pl, _, _): (Vec<PlacedSprite>, u32, u32) = pack(&items, 512);
    assert_eq!(pl.len(), 3);
    let row0: Vec<_> = pl.iter().filter(|p| p.atlas_y == 0).collect();
    let row1: Vec<_> = pl.iter().filter(|p| p.atlas_y > 0).collect();
    assert_eq!(row0.len(), 2);
    assert_eq!(row1.len(), 1);
    assert_eq!(row1[0].atlas_y, 32);
}

#[test]
fn pack_sorts_taller_sprites_first() {
    let items = [item("small", 32, 16), item("tall", 32, 64)];
    let (pl, _, _): (Vec<PlacedSprite>, u32, u32) = pack(&items, 512);
    let tall = pl.iter().find(|p| p.name == "tall").unwrap();
    let small = pl.iter().find(|p| p.name == "small").unwrap();
    assert_eq!(tall.atlas_y, 0);
    assert_eq!(small.atlas_y, 0);
    assert!(tall.atlas_x < small.atlas_x);
}

#[test]
fn pack_skips_sprite_wider_than_atlas() {
    let items = [item("giant", 600, 48), item("normal", 16, 24)];
    let (pl, _, _): (Vec<PlacedSprite>, u32, u32) = pack(&items, 512);
    assert_eq!(pl.len(), 1);
    assert_eq!(pl[0].name, "normal");
}

#[test]
fn pack_dedup_only_places_first_occurrence_of_name() {
    let items = [item("hero", 16, 24), item("hero", 64, 64)];
    let (pl, _, _): (Vec<PlacedSprite>, u32, u32) = pack(&items, 512);
    assert_eq!(pl.len(), 1);
    assert_eq!(pl[0].pixel_w, 64);
    assert_eq!(pl[0].pixel_h, 64);
}

#[test]
fn pack_atlas_width_is_power_of_two() {
    let items = [item("a", 10, 10)];
    let (_, atlas_w, _): (Vec<PlacedSprite>, u32, u32) = pack(&items, 100);
    assert!(atlas_w.is_power_of_two());
}

// ── Viewport Utils Tests ──────────────────────────────────────────────────

fn cfg(pw: u32, ph: u32, lw: u32, lh: u32) -> WindowConfig {
    WindowConfig { physical_width: pw, physical_height: ph,
                   logical_width: lw,  logical_height: lh,
                   mode: WindowMode::Windowed }
}

#[test]
fn exact_fit_no_offset() {
    let v = letterbox_viewport(&cfg(1280, 720, 1280, 720));
    assert_eq!(v.x, 0.0);
    assert_eq!(v.y, 0.0);
    assert_eq!(v.width,  1280.0);
    assert_eq!(v.height, 720.0);
}

#[test]
fn integer_scale_2x_no_offset() {
    let v = letterbox_viewport(&cfg(2560, 1440, 1280, 720));
    assert_eq!(v.x, 0.0);
    assert_eq!(v.y, 0.0);
    assert_eq!(v.width,  2560.0);
    assert_eq!(v.height, 1440.0);
}

#[test]
fn letterbox_4x3_in_16x9() {
    let v = letterbox_viewport(&cfg(1280, 720, 800, 600));
    assert!((v.x - 160.0).abs() < 1e-3);
    assert!((v.y -   0.0).abs() < 1e-3);
    assert!((v.width  - 960.0).abs() < 1e-3);
    assert!((v.height - 720.0).abs() < 1e-3);
}

#[test]
fn pillarbox_16x9_in_4x3() {
    let v = letterbox_viewport(&cfg(800, 600, 1280, 720));
    assert!((v.x -   0.0).abs() < 1e-3);
    assert!((v.y -  75.0).abs() < 1e-3);
    assert!((v.width  - 800.0).abs() < 1e-3);
    assert!((v.height - 450.0).abs() < 1e-3);
}

#[test]
fn square_logical_in_wide_physical() {
    let v = letterbox_viewport(&cfg(1024, 600, 512, 512));
    let expected_w = 512.0 * (600.0f32 / 512.0);
    let expected_x = (1024.0 - expected_w) / 2.0;
    assert!((v.x - expected_x).abs() < 1e-3);
    assert!((v.y - 0.0).abs() < 1e-3);
    assert!((v.width  - expected_w).abs() < 1e-3);
    assert!((v.height - 600.0).abs() < 1e-3);
}

#[test]
fn zero_logical_width_returns_zero_viewport() {
    let v = letterbox_viewport(&cfg(1280, 720, 0, 720));
    assert_eq!(v, Viewport { x: 0.0, y: 0.0, width: 0.0, height: 0.0 });
}

#[test]
fn zero_logical_height_returns_zero_viewport() {
    let v = letterbox_viewport(&cfg(1280, 720, 1280, 0));
    assert_eq!(v, Viewport { x: 0.0, y: 0.0, width: 0.0, height: 0.0 });
}

#[test]
fn zero_physical_size_gives_zero_sized_viewport() {
    let v = letterbox_viewport(&cfg(0, 0, 1280, 720));
    assert_eq!(v.x, 0.0);
    assert_eq!(v.y, 0.0);
    assert_eq!(v.width,  0.0);
    assert_eq!(v.height, 0.0);
}

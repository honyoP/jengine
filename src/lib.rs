pub mod ecs;
pub mod engine;
pub mod geometry;
pub mod pathfinding;
pub mod renderer;
pub mod ui;

/// Built-in 16×24 bitmap font atlas embedded at compile time.
pub const DEFAULT_TILESET: &[u8] = include_bytes!("../resources/font_atlas_16x24.png");
pub const DEFAULT_TILE_W: u32 = 16;
pub const DEFAULT_TILE_H: u32 = 24;

/// Glyph map for the built-in font atlas (char-keyed JSON, see `Font::from_atlas_json`).
pub const DEFAULT_FONT_GLYPHS: &str = include_str!("../resources/font_glyph_map.json");

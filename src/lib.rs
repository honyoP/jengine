pub mod camera;
pub mod ecs;
pub mod engine;
pub mod geometry;
pub mod input;
pub mod audio;
pub mod pathfinding;
pub mod renderer;
pub mod scene;
pub mod ui;
pub mod window;

/// Built-in MTSDF font atlas embedded at compile time (used as tile fallback).
pub const DEFAULT_TILESET: &[u8] = include_bytes!("../resources/font_atlas.png");
pub const DEFAULT_TILE_W: u32 = 16;
pub const DEFAULT_TILE_H: u32 = 24;

/// msdf-atlas-gen JSON descriptor for the built-in MTSDF font (see `Font::from_mtsdf_json`).
pub const DEFAULT_FONT_METADATA: &str = include_str!("../resources/font_metadata.json");

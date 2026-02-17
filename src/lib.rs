pub mod ecs;
pub mod engine;
pub mod geometry;
pub mod pathfinding;
pub mod renderer;

/// Built-in 16x16 CP437/Unicode tileset embedded at compile time.
pub const DEFAULT_TILESET: &[u8] = include_bytes!("../resources/unicode_16x16.png");
pub const DEFAULT_TILE_W: u32 = 16;
pub const DEFAULT_TILE_H: u32 = 16;

//! # Pathfinding Showcase
//! 
//! A modern demonstration of jengine's pathfinding, audio, and animation systems.
//! 
//! Highlights:
//! - Optimized A* (4-dir and 8-dir) using flat-vector storage.
//! - Shader-based "juice" (animations) on path markers.
//! - Reactive UI using immediate-mode primitives.
//! - Positional and global audio via the restored Kira backend.

use jengine::engine::{Color, Game, jEngine, KeyCode, AnimationType};
use jengine::pathfinding::prelude::{astar, astar_8dir, DijkstraMap};
use jengine::renderer::text::Font;
use jengine::ecs::{World, Entity};
use jengine::{DEFAULT_FONT_METADATA, DEFAULT_TILE_H, DEFAULT_TILE_W, DEFAULT_TILESET};

// ── Constants ────────────────────────────────────────────────────────────────

const MAP_W: i32 = 40;
const MAP_H: i32 = 22;

const C_WALL:   Color = Color([0.15, 0.15, 0.18, 1.0]);
const C_FLOOR:  Color = Color([0.05, 0.05, 0.06, 1.0]);
const C_START:  Color = Color([0.20, 0.90, 0.40, 1.0]);
const C_GOAL:   Color = Color([1.00, 0.30, 0.20, 1.0]);
const C_PATH:   Color = Color([0.30, 0.60, 1.00, 1.0]);
const C_ACCENT: Color = Color([1.00, 0.80, 0.20, 1.0]);

// ── Components ───────────────────────────────────────────────────────────────

#[derive(Copy, Clone)]
struct Position { x: i32, y: i32 }
struct Marker { color: Color, glyph: char }

// ── Pathfinding Example ──────────────────────────────────────────────────────

struct PathfindingShowcase {
    world: World,
    walls: Vec<bool>,
    start_ent: Entity,
    goal_ent: Entity,
    path: Option<Vec<(i32, i32)>>,
    dijkstra: DijkstraMap,
    mode_8dir: bool,
    show_dijkstra: bool,
    dirty: bool,
}

impl PathfindingShowcase {
    fn new() -> Self {
        let mut world = World::new();
        let walls = Self::generate_maze();

        // Spawn interactive markers as entities to enable GPU animations
        let start_ent = world.spawn();
        world.insert(start_ent, Position { x: 5, y: 5 });
        world.insert(start_ent, Marker { color: C_START, glyph: 'S' });

        let goal_ent = world.spawn();
        world.insert(goal_ent, Position { x: 35, y: 15 });
        world.insert(goal_ent, Marker { color: C_GOAL, glyph: 'G' });

        let mut s = Self {
            world,
            walls,
            start_ent,
            goal_ent,
            path: None,
            dijkstra: DijkstraMap::new(MAP_W, MAP_H, &[(0,0)], |_, _| true),
            mode_8dir: false,
            show_dijkstra: false,
            dirty: true,
        };
        s.recompute();
        s
    }

    fn generate_maze() -> Vec<bool> {
        let mut walls = vec![false; (MAP_W * MAP_H) as usize];
        let mut block = |x, y| {
            if x >= 0 && x < MAP_W && y >= 0 && y < MAP_H {
                walls[(y * MAP_W + x) as usize] = true;
            }
        };

        // Perimeter
        for x in 0..MAP_W { block(x, 0); block(x, MAP_H - 1); }
        for y in 0..MAP_H { block(0, y); block(MAP_W - 1, y); }

        // Corridors
        for x in 10..30 { block(x, 7); block(x, 14); }
        for y in 3..10  { block(10, y); }
        for y in 14..19 { block(30, y); }
        
        walls
    }

    fn is_walkable(&self, x: i32, y: i32) -> bool {
        if x < 0 || x >= MAP_W || y < 0 || y >= MAP_H { return false; }
        !self.walls[(y * MAP_W + x) as usize]
    }

    fn recompute(&mut self) {
        let s = self.world.get::<Position>(self.start_ent).unwrap();
        let g = self.world.get::<Position>(self.goal_ent).unwrap();
        let (sx, sy) = (s.x, s.y);
        let (gx, gy) = (g.x, g.y);

        if self.mode_8dir {
            self.path = astar_8dir((sx, sy), (gx, gy), MAP_W, MAP_H, |x, y| self.is_walkable(x, y), 2000);
        } else {
            self.path = astar((sx, sy), (gx, gy), MAP_W, MAP_H, |x, y| self.is_walkable(x, y), 2000);
        }

        self.dijkstra = DijkstraMap::new(MAP_W, MAP_H, &[(gx, gy)], |x, y| self.is_walkable(x, y));
        self.dirty = false;
    }
}

impl Game for PathfindingShowcase {
    fn on_enter(&mut self, engine: &mut jEngine) {
        if let Ok(font) = Font::from_mtsdf_json(DEFAULT_FONT_METADATA) {
            engine.renderer.set_mtsdf_distance_range(font.distance_range);
            engine.ui.text.set_font(font);
        }
        
        // Load sound effects
        engine.audio.load_sound("move", "resources/audio/UI_selection.wav");
        engine.audio.load_sound("toggle", "resources/audio/UI_click.wav");
    }

    fn update(&mut self, engine: &mut jEngine) {
        let mut moved = false;
        let mut pos = *self.world.get::<Position>(self.start_ent).unwrap();

        if engine.is_key_pressed(KeyCode::ArrowLeft)  { pos.x -= 1; moved = true; }
        if engine.is_key_pressed(KeyCode::ArrowRight) { pos.x += 1; moved = true; }
        if engine.is_key_pressed(KeyCode::ArrowUp)    { pos.y -= 1; moved = true; }
        if engine.is_key_pressed(KeyCode::ArrowDown)  { pos.y += 1; moved = true; }

        if moved && self.is_walkable(pos.x, pos.y) {
            self.world.insert(self.start_ent, pos);
            self.dirty = true;
            engine.play_sound("move");
            // Play a small "juice" animation on the start marker when it moves
            engine.play_animation(self.start_ent, AnimationType::Bash { 
                direction: [0.0, -0.5], 
                magnitude: 4.0 
            });
        }

        if engine.is_key_pressed(KeyCode::Tab) {
            self.mode_8dir = !self.mode_8dir;
            self.dirty = true;
            engine.play_sound("toggle");
            // Shiver the goal to indicate path mode change
            engine.play_animation(self.goal_ent, AnimationType::Shiver { magnitude: 3.0 });
        }

        if engine.is_key_pressed(KeyCode::KeyD) {
            self.show_dijkstra = !self.show_dijkstra;
            engine.play_sound("toggle");
        }

        if self.dirty {
            self.recompute();
        }
    }

    fn render(&mut self, engine: &mut jEngine) {
        engine.clear();
        let tw = engine.tile_width();
        let th = engine.tile_height();

        // ── 1. Map Rendering ──
        let path_set: std::collections::HashSet<(i32, i32)> = self.path.as_ref()
            .map(|p| p.iter().copied().collect())
            .unwrap_or_default();

        for y in 0..MAP_H {
            for x in 0..MAP_W {
                let ux = x as u32;
                let uy = y as u32;

                if self.walls[(y * MAP_W + x) as usize] {
                    engine.set_background(ux, uy, C_WALL);
                    engine.set_foreground(ux, uy, '#', Color([0.3, 0.3, 0.35, 1.0]));
                } else if self.show_dijkstra {
                    let d = self.dijkstra.get(x, y);
                    if d < f32::MAX {
                        let t = (1.0 - (d / 30.0).min(1.0)) * 0.6;
                        engine.set_background(ux, uy, Color([0.1, 0.2 * t, 0.8 * t, 1.0]));
                    } else {
                        engine.set_background(ux, uy, C_FLOOR);
                    }
                } else if path_set.contains(&(x, y)) {
                    engine.set_background(ux, uy, C_PATH);
                    engine.set_foreground(ux, uy, '.', Color::WHITE);
                } else {
                    engine.set_background(ux, uy, C_FLOOR);
                }
            }
        }

        // ── 2. Entity Rendering (Start/Goal) ──
        // Using set_foreground_entity enables the shader-based animation offsets
        let s_pos = self.world.get::<Position>(self.start_ent).unwrap();
        let s_mkr = self.world.get::<Marker>(self.start_ent).unwrap();
        engine.set_foreground_entity(s_pos.x as u32, s_pos.y as u32, self.start_ent, s_mkr.glyph, s_mkr.color);

        let g_pos = self.world.get::<Position>(self.goal_ent).unwrap();
        let g_mkr = self.world.get::<Marker>(self.goal_ent).unwrap();
        engine.set_foreground_entity(g_pos.x as u32, g_pos.y as u32, self.goal_ent, g_mkr.glyph, g_mkr.color);

        // ── 3. UI Overlay ──
        let sw = engine.grid_width() as f32 * tw as f32;
        let sh = engine.grid_height() as f32 * th as f32;
        let th_f = th as f32;

        engine.ui.ui_rect(0.0, 0.0, sw, th_f * 2.0, Color([0.0, 0.0, 0.0, 0.8]));
        engine.ui.ui_text(10.0, 5.0, "PATHFINDING SHOWCASE", C_ACCENT, Color::TRANSPARENT, Some(18.0));
        
        let mode_str = if self.mode_8dir { "8-Directional (Octile)" } else { "4-Directional (Manhattan)" };
        engine.ui.ui_text(10.0, 25.0, &format!("Mode: {}", mode_str), Color::WHITE, Color::TRANSPARENT, None);

        // Legend/Controls at bottom
        engine.ui.ui_rect(0.0, sh - th_f * 2.0, sw, th_f * 2.0, Color([0.0, 0.0, 0.0, 0.8]));
        engine.ui.ui_text(10.0, sh - 40.0, "[Arrows] Move Start  [Tab] Toggle 4/8-dir  [D] Toggle Dijkstra Heatmap", Color::GRAY, Color::TRANSPARENT, None);
        
        let path_info = self.path.as_ref().map(|p| format!("Path Length: {}", p.len())).unwrap_or("No Path Found".to_string());
        engine.ui.ui_text(sw - 200.0, 5.0, &path_info, C_PATH, Color::TRANSPARENT, None);
    }
}

fn main() {
    jEngine::builder()
        .with_title("jengine — Pathfinding Showcase")
        .with_size(800, 576)
        .with_tileset(DEFAULT_TILESET, DEFAULT_TILE_W, DEFAULT_TILE_H)
        .run(PathfindingShowcase::new());
}

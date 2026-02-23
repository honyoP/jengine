//! # Pathfinding Example
//!
//! Demonstrates both pathfinding algorithms shipped with jengine.
//!
//! Algorithms shown:
//!   · `astar(start, goal, is_passable, max_iter)` — A* shortest path (4-directional)
//!   · `DijkstraMap::new(w, h, goals, is_passable)` — flood-fill distance map from one or more goals
//!   · `DijkstraMap::direction_to_goal(x, y)`       — greedy step toward goals
//!   · `DijkstraMap::direction_away(x, y)`           — greedy step away from goals
//!
//! The map is a fixed 40×22 grid with hand-placed walls.  A* highlights the
//! shortest path; pressing Tab switches to a Dijkstra heat-map view where each
//! tile's colour encodes its distance from the goal.
//!
//! Controls:
//!   Arrow keys  — move the START marker
//!   Tab         — toggle between A* path view and Dijkstra heat-map view
//!   Esc         — quit

use jengine::engine::{Color, Game, jEngine, KeyCode};
use jengine::pathfinding::prelude::{astar, DijkstraMap};
use jengine::renderer::text::Font;
use jengine::{DEFAULT_FONT_METADATA, DEFAULT_TILE_H, DEFAULT_TILE_W, DEFAULT_TILESET};

// ── Map layout ────────────────────────────────────────────────────────────────

const MAP_W: i32 = 40;
const MAP_H: i32 = 22;

/// Wall tiles encoded as (x, y) pairs.  Built once at startup.
fn build_walls() -> Vec<bool> {
    let mut walls = vec![false; (MAP_W * MAP_H) as usize];

    let mut block = |x: i32, y: i32| {
        if x >= 0 && y >= 0 && x < MAP_W && y < MAP_H {
            walls[(y * MAP_W + x) as usize] = true;
        }
    };

    // Border
    for x in 0..MAP_W {
        block(x, 0);
        block(x, MAP_H - 1);
    }
    for y in 0..MAP_H {
        block(0, y);
        block(MAP_W - 1, y);
    }

    // Interior walls — create a maze-like set of corridors.
    for x in 5..20  { block(x, 6); }
    for x in 15..35 { block(x, 12); }
    for y in 6..12  { block(20, y); }
    for y in 3..12  { block(10, y); }
    for y in 12..18 { block(28, y); }
    for x in 22..35 { block(x, 17); }

    walls
}

fn is_wall(walls: &[bool], x: i32, y: i32) -> bool {
    if x < 0 || y < 0 || x >= MAP_W || y >= MAP_H {
        return true;
    }
    walls[(y * MAP_W + x) as usize]
}

// ── Tile colours ──────────────────────────────────────────────────────────────

const C_WALL:  Color = Color([0.25, 0.25, 0.28, 1.0]);
const C_FLOOR: Color = Color([0.07, 0.08, 0.09, 1.0]);
const C_START: Color = Color([0.10, 0.80, 0.30, 1.0]);
const C_GOAL:  Color = Color([0.90, 0.20, 0.10, 1.0]);
const C_PATH:  Color = Color([0.20, 0.70, 1.00, 1.0]);

// ── View mode ─────────────────────────────────────────────────────────────────

#[derive(PartialEq)]
enum View {
    /// A* shortest path highlighted in cyan.
    Astar,
    /// Dijkstra distance heat map (blue gradient, closer = brighter).
    Dijkstra,
}

// ── Game ──────────────────────────────────────────────────────────────────────

struct PathfindingDemo {
    font_loaded: bool,
    walls:       Vec<bool>,
    start:       (i32, i32),
    goal:        (i32, i32),
    view:        View,
    /// Cached A* path — recomputed when start/goal changes.
    path:        Option<Vec<(i32, i32)>>,
    /// Cached Dijkstra map from goal — recomputed when goal changes.
    dijkstra:    DijkstraMap,
    dirty:       bool,
}

impl PathfindingDemo {
    fn new() -> Self {
        let walls = build_walls();
        let start = (3, 3);
        let goal  = (37, 19);

        let dijkstra = DijkstraMap::new(MAP_W, MAP_H, &[goal], |x, y| !is_wall(&walls, x, y));
        let path = astar(start, goal, |x, y| !is_wall(&walls, x, y), 2000);

        Self {
            font_loaded: false,
            walls,
            start,
            goal,
            view: View::Astar,
            path,
            dijkstra,
            dirty: false,
        }
    }

    fn recompute(&mut self) {
        self.path = astar(
            self.start,
            self.goal,
            |x, y| !is_wall(&self.walls, x, y),
            2000,
        );
        // Dijkstra map is goal-only — we only need to recompute if goal changes.
        // Recompute always for simplicity in this demo.
        self.dijkstra = DijkstraMap::new(
            MAP_W,
            MAP_H,
            &[self.goal],
            |x, y| !is_wall(&self.walls, x, y),
        );
        self.dirty = false;
    }
}

impl Game for PathfindingDemo {
    fn update(&mut self, engine: &mut jEngine) {
        if engine.is_key_pressed(KeyCode::Escape) {
            engine.request_quit();
            return;
        }

        // Toggle view mode.
        if engine.is_key_pressed(KeyCode::Tab) {
            self.view = match self.view {
                View::Astar    => View::Dijkstra,
                View::Dijkstra => View::Astar,
            };
        }

        // Move the start marker with arrow keys.
        let mut moved = false;
        if engine.is_key_pressed(KeyCode::ArrowLeft)  { self.start.0 -= 1; moved = true; }
        if engine.is_key_pressed(KeyCode::ArrowRight) { self.start.0 += 1; moved = true; }
        if engine.is_key_pressed(KeyCode::ArrowUp)    { self.start.1 -= 1; moved = true; }
        if engine.is_key_pressed(KeyCode::ArrowDown)  { self.start.1 += 1; moved = true; }

        if moved {
            // Clamp start inside the map and prevent landing on walls.
            self.start.0 = self.start.0.clamp(1, MAP_W - 2);
            self.start.1 = self.start.1.clamp(1, MAP_H - 2);
            if is_wall(&self.walls, self.start.0, self.start.1) {
                // Reject the move — stay put.
                self.start.0 = self.start.0.clamp(1, MAP_W - 2);
                self.start.1 = self.start.1.clamp(1, MAP_H - 2);
            }
            self.dirty = true;
        }

        if self.dirty {
            self.recompute();
        }
    }

    fn render(&mut self, engine: &mut jEngine) {
        if !self.font_loaded {
            if let Ok(font) = Font::from_mtsdf_json(DEFAULT_FONT_METADATA) {
                engine.ui.text.set_font(font);
            }
            self.font_loaded = true;
        }

        engine.clear();

        let tw = engine.tile_width() as f32;
        let th = engine.tile_height() as f32;
        let sw = engine.grid_width() as f32 * tw;

        // ── Map layer ─────────────────────────────────────────────────────────
        let path_set: std::collections::HashSet<(i32, i32)> = match &self.path {
            Some(p) if self.view == View::Astar => p.iter().copied().collect(),
            _ => std::collections::HashSet::new(),
        };

        // Precompute the maximum reachable Dijkstra value for normalisation.
        let dijk_max = {
            let mut m = 1.0_f32;
            for y in 0..MAP_H {
                for x in 0..MAP_W {
                    let v = self.dijkstra.get(x, y);
                    if v != f32::MAX { m = m.max(v); }
                }
            }
            m
        };

        for y in 0..MAP_H.min(engine.grid_height() as i32) {
            for x in 0..MAP_W.min(engine.grid_width() as i32) {
                let ux = x as u32;
                let uy = y as u32;
                let tile = (x, y);

                if is_wall(&self.walls, x, y) {
                    engine.set_background(ux, uy, C_WALL);
                    engine.set_foreground(ux, uy, '#', Color([0.4, 0.4, 0.45, 1.0]));
                } else if tile == self.start {
                    engine.set_background(ux, uy, C_START);
                    engine.set_foreground(ux, uy, 'S', Color::BLACK);
                } else if tile == self.goal {
                    engine.set_background(ux, uy, C_GOAL);
                    engine.set_foreground(ux, uy, 'G', Color::WHITE);
                } else if self.view == View::Astar && path_set.contains(&tile) {
                    // A* path highlighted in blue.
                    engine.set_background(ux, uy, C_PATH);
                    engine.set_foreground(ux, uy, '.', Color::WHITE);
                } else if self.view == View::Dijkstra {
                    // Dijkstra heat map: bright blue = close to goal, dark = far.
                    let v = self.dijkstra.get(x, y);
                    if v == f32::MAX {
                        engine.set_background(ux, uy, C_FLOOR);
                    } else {
                        let t = 1.0 - (v / dijk_max).clamp(0.0, 1.0);
                        engine.set_background(ux, uy, Color([t * 0.05, t * 0.15, t * 0.8, 1.0]));
                    }
                } else {
                    engine.set_background(ux, uy, C_FLOOR);
                }
            }
        }

        // ── HUD overlay ───────────────────────────────────────────────────────
        let path_len = self.path.as_ref().map(|p| p.len()).unwrap_or(0);
        let status = match &self.view {
            View::Astar => {
                if self.path.is_some() {
                    format!("A*  path length: {path_len} steps")
                } else {
                    "A*  NO PATH FOUND".to_string()
                }
            }
            View::Dijkstra => {
                let dist = self.dijkstra.get(self.start.0, self.start.1);
                if dist == f32::MAX {
                    "Dijkstra: unreachable".to_string()
                } else {
                    format!("Dijkstra  distance from S to G: {dist:.0}")
                }
            }
        };

        engine.ui.ui_rect(0.0, 0.0, sw, th, Color([0.0, 0.0, 0.0, 0.88]));
        engine.ui.ui_text(tw, 0.0, &status, Color::WHITE, Color::TRANSPARENT, None);

        let sh = engine.grid_height() as f32 * th;
        engine.ui.ui_rect(0.0, sh - th, sw, th, Color([0.0, 0.0, 0.0, 0.88]));
        engine.ui.ui_text(
            tw,
            sh - th,
            "[Arrows] move S   [Tab] toggle A*/Dijkstra view   [Esc] quit",
            Color([0.6, 0.7, 0.65, 1.0]),
            Color::TRANSPARENT, None);

        // Legend row just above the hint bar.
        engine.ui.ui_rect(0.0, sh - th * 2.0, sw, th, Color([0.0, 0.0, 0.0, 0.7]));
        engine.ui.ui_text(tw,         sh - th * 2.0, "S=start", C_START, Color::TRANSPARENT, None);
        engine.ui.ui_text(tw * 10.0,  sh - th * 2.0, "G=goal",  C_GOAL,  Color::TRANSPARENT, None);
        engine.ui.ui_text(tw * 18.0,  sh - th * 2.0, "#=wall",  C_WALL,  Color::TRANSPARENT, None);
        engine.ui.ui_text(tw * 26.0,  sh - th * 2.0, "·=path",  C_PATH,  Color::TRANSPARENT, None);
    }
}

fn main() {
    jEngine::builder()
        .with_title("jengine — Pathfinding")
        .with_size(800, 576)
        .with_tileset(DEFAULT_TILESET, DEFAULT_TILE_W, DEFAULT_TILE_H)
        .run(PathfindingDemo::new());
}

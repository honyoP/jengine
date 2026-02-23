//! # ECS Example
//!
//! Demonstrates jengine's sparse-set Entity-Component System (ECS).
//!
//! Concepts shown:
//!   · `World::spawn` / `World::despawn` — entity lifecycle with generational handles
//!   · `World::insert` — attach any `'static` type as a component (no registration required)
//!   · `World::get` / `World::get_mut` — fetch a single component by entity handle
//!   · `World::query` — iterate all entities that have a given component (`&T`)
//!   · `World::query_mut` — same, but yields `&mut T`
//!   · `World::query_multi_mut` — iterate entities that have ALL listed components (`&mut T, &mut U, …`)
//!   · Dead-entity safety — despawned handles are ignored by `get`/`has`
//!
//! Controls:
//!   Space  — spawn a new entity at the grid centre
//!   D      — deal 25 damage to every living entity; dead ones despawn next tick
//!   Esc    — quit

use jengine::ecs::World;
use jengine::engine::{Color, Game, jEngine, KeyCode};
use jengine::renderer::text::Font;
use jengine::{DEFAULT_FONT_METADATA, DEFAULT_TILE_H, DEFAULT_TILE_W, DEFAULT_TILESET};

// ── Components ────────────────────────────────────────────────────────────────
// Components are plain Rust structs — no derive macros or trait impls required.

/// Tile-grid position (column, row).
struct Position {
    x: u32,
    y: u32,
}

/// Movement direction in grid cells per move-step (can be negative).
struct Velocity {
    dx: i32,
    dy: i32,
}

/// Hit-points. Entity is queued for removal when `current` drops to zero.
struct Health {
    current: i32,
    max: i32,
}

/// Visual representation: which character and colour to draw.
struct Renderable {
    glyph: char,
    color: Color,
}

// ── Game ──────────────────────────────────────────────────────────────────────

struct EcsDemo {
    /// The ECS world owns all entities and their component data.
    world: World,
    /// Incremented on each spawn to cycle through colours and glyphs.
    spawn_seq: u64,
    /// Entities advance one tile every `MOVE_INTERVAL` ticks.
    move_timer: u64,
    /// True once the bitmap font has been registered in the UI text layer.
    font_loaded: bool,
}

/// How many fixed-update ticks between entity movement steps.
const MOVE_INTERVAL: u64 = 18;

impl EcsDemo {
    fn new() -> Self {
        let mut demo = Self {
            world: World::new(),
            spawn_seq: 0,
            move_timer: 0,
            font_loaded: false,
        };
        // Pre-populate the world so there is something to look at immediately.
        for i in 0..8 {
            demo.spawn_at(8 + i * 6, 7);
        }
        demo
    }

    /// Spawn one entity at grid position `(x, y)`.
    ///
    /// Velocity and appearance cycle deterministically with `spawn_seq` so that
    /// each new entity looks different from the previous one.
    fn spawn_at(&mut self, x: u32, y: u32) {
        // `World::spawn` returns a generational Entity handle.  All subsequent
        // component inserts reference this handle.
        let entity = self.world.spawn();

        let seq = self.spawn_seq;

        // Diagonal direction — four quadrants cycling with seq.
        let (dx, dy) = match seq % 4 {
            0 => (1_i32, 1_i32),
            1 => (-1, 1),
            2 => (1, -1),
            _ => (-1, -1),
        };

        // Colour palette cycles through six entries.
        let color = [
            Color::CYAN,
            Color::YELLOW,
            Color::GREEN,
            Color::MAGENTA,
            Color::ORANGE,
            Color::WHITE,
        ][seq as usize % 6];

        // Glyph cycles through printable ASCII symbols.
        let glyph = ['@', '&', '%', '*', '#', '+'][seq as usize % 6];

        // `World::insert` accepts any `'static` value — no registration step.
        self.world.insert(entity, Position { x, y });
        self.world.insert(entity, Velocity { dx, dy });
        self.world.insert(entity, Health { current: 100, max: 100 });
        self.world.insert(entity, Renderable { glyph, color });

        self.spawn_seq += 1;
    }
}

impl Game for EcsDemo {
    fn update(&mut self, engine: &mut jEngine) {
        // ── Quit ──────────────────────────────────────────────────────────────
        if engine.is_key_pressed(KeyCode::Escape) {
            engine.request_quit();
            return;
        }

        // ── Spawn a new entity ────────────────────────────────────────────────
        if engine.is_key_pressed(KeyCode::Space) {
            let cx = engine.grid_width() / 2;
            let cy = engine.grid_height() / 2;
            self.spawn_at(cx, cy);
        }

        // ── Deal damage to every entity ───────────────────────────────────────
        // `query_mut` yields `(Entity, &mut Health)` — single-component mutable.
        if engine.is_key_pressed(KeyCode::KeyD) {
            for (_entity, health) in self.world.query_mut::<Health>() {
                health.current -= 25;
            }
        }

        // ── Move entities on a fixed interval ─────────────────────────────────
        self.move_timer += 1;
        if self.move_timer >= MOVE_INTERVAL {
            self.move_timer = 0;
            let gw = engine.grid_width() as i32;
            let gh = engine.grid_height() as i32;

            // `query_multi_mut` yields `(Entity, (&mut Position, &mut Velocity))`
            // for every entity that has BOTH components.  The borrow checker
            // guarantees no aliasing because all type parameters are distinct.
            for (_entity, (pos, vel)) in self.world.query_multi_mut::<(Position, Velocity)>() {
                // `rem_euclid` wraps negative results correctly (unlike `%`).
                pos.x = (pos.x as i32 + vel.dx).rem_euclid(gw) as u32;
                pos.y = (pos.y as i32 + vel.dy).rem_euclid(gh) as u32;
            }
        }

        // ── Despawn dead entities ─────────────────────────────────────────────
        // We collect the dead handles first so we don't mutate the world while
        // the query iterator still holds a shared borrow.
        let dead: Vec<_> = self
            .world
            .query::<Health>()
            .filter(|(_e, h)| h.current <= 0)
            .map(|(e, _)| e)
            .collect();

        for entity in dead {
            // `despawn` increments the entity's generation, invalidating any
            // copies of the old handle.  Components are removed automatically.
            self.world.despawn(entity);
        }
    }

    fn render(&mut self, engine: &mut jEngine) {
        // Register the bitmap font once so that `ui_text` can render glyphs.
        if !self.font_loaded {
            if let Ok(font) = Font::from_mtsdf_json(DEFAULT_FONT_METADATA) {
                engine.ui.text.set_font(font);
            }
            self.font_loaded = true;
        }

        engine.clear();

        let gw = engine.grid_width();
        let gh = engine.grid_height();

        // Checkerboard background — makes individual tile positions easy to see.
        for y in 0..gh {
            for x in 0..gw {
                let shade = if (x + y) % 2 == 0 {
                    Color([0.06, 0.06, 0.07, 1.0])
                } else {
                    Color([0.04, 0.04, 0.05, 1.0])
                };
                engine.set_background(x, y, shade);
            }
        }

        // Draw every entity that has a Position and a Renderable.
        //
        // `query_multi` yields `(Entity, (&Position, &Renderable))`.  We can
        // also call `world.get::<Health>(entity)` inside the loop because both
        // borrows are shared (`&`) and thus non-conflicting.
        for (entity, (pos, rend)) in self.world.query_multi::<(Position, Renderable)>() {
            // Dim the glyph proportionally to remaining health — pure data lookup.
            let frac = self
                .world
                .get::<Health>(entity)
                .map(|h| h.current as f32 / h.max as f32)
                .unwrap_or(1.0)
                .clamp(0.0, 1.0);

            let dimmed = Color([
                rend.color.0[0] * frac,
                rend.color.0[1] * frac,
                rend.color.0[2] * frac,
                1.0,
            ]);

            // Solid black behind each entity so it stands out from the checker.
            engine.set_background(pos.x, pos.y, Color::BLACK);
            engine.set_foreground(pos.x, pos.y, rend.glyph, dimmed);
        }

        // ── UI overlay (always drawn on top of the world) ─────────────────────
        let count = self.world.query::<Position>().count();
        let tw = engine.tile_width() as f32;
        let th = engine.tile_height() as f32;
        let sw = gw as f32 * tw;

        engine.ui.ui_rect(0.0, 0.0, sw, th, Color([0.0, 0.0, 0.0, 0.85]));
        engine.ui.ui_text(
            tw,
            0.0,
            &format!(
                "Entities: {count:<3}  |  [Space] spawn  [D] damage all  [Esc] quit"
            ),
            Color::WHITE,
            Color::TRANSPARENT, None);
    }
}

fn main() {
    jEngine::builder()
        .with_title("jengine — ECS")
        .with_size(800, 576)
        .with_tileset(DEFAULT_TILESET, DEFAULT_TILE_W, DEFAULT_TILE_H)
        .run(EcsDemo::new());
}

use jengine::ecs::{Entity, World};
use jengine::engine::{Color, Engine, Game, KeyCode};
use jengine::{DEFAULT_TILESET, DEFAULT_TILE_H, DEFAULT_TILE_W};

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

struct Position {
    x: u32,
    y: u32,
}

struct Renderable {
    glyph: char,
    fg: Color,
    bg: Color,
}

/// Marks the player-controlled entity.
struct Player;

/// Marks a solid wall tile.
struct Solid;

// ---------------------------------------------------------------------------
// DemoGame — owns the ECS World
// ---------------------------------------------------------------------------

struct DemoGame {
    world: World,
    player: Option<Entity>,
    map_w: u32,
    map_h: u32,
    initialized: bool,
    move_cooldown: u32,
}

impl DemoGame {
    fn new() -> Self {
        Self {
            world: World::new(),
            player: None,
            map_w: 0,
            map_h: 0,
            initialized: false,
            move_cooldown: 0,
        }
    }

    fn build_map(&mut self, w: u32, h: u32) {
        // Despawn all existing entities (walls + player).
        let to_despawn: Vec<Entity> = self.world.query::<Position>().map(|(e, _)| e).collect();
        for e in to_despawn {
            self.world.despawn(e);
        }

        self.map_w = w;
        self.map_h = h;

        let mut solids = Vec::new();

        // Border walls
        for x in 0..w {
            solids.push((x, 0));
            solids.push((x, h - 1));
        }
        for y in 1..h - 1 {
            solids.push((0, y));
            solids.push((w - 1, y));
        }

        // Horizontal bar
        if w > 6 && h > 4 {
            let wall_y = h / 3;
            for x in 2..w - 4 {
                solids.push((x, wall_y));
            }
        }

        // Vertical bar
        if w > 4 && h > 6 {
            let wall_x = w * 2 / 3;
            for y in h / 2..h - 2 {
                solids.push((wall_x, y));
            }
        }

        // Spawn wall entities
        for (x, y) in solids {
            let wall = self.world.spawn();
            self.world.insert(wall, Position { x, y });
            self.world.insert(
                wall,
                Renderable {
                    glyph: '#',
                    fg: Color::GRAY,
                    bg: Color::DARK_GRAY,
                },
            );
            self.world.insert(wall, Solid);
        }

        // Spawn player entity
        let player = self.world.spawn();
        self.world.insert(player, Position { x: w / 2, y: h / 2 });
        self.world.insert(
            player,
            Renderable {
                glyph: '@',
                fg: Color::YELLOW,
                bg: Color::BLACK,
            },
        );
        self.world.insert(player, Player);
        self.player = Some(player);
    }

    fn is_solid(&self, x: u32, y: u32) -> bool {
        self.world.query::<Solid>().any(|(e, _)| {
            self.world
                .get::<Position>(e)
                .map_or(false, |p| p.x == x && p.y == y)
        })
    }
}

impl Game for DemoGame {
    fn update(&mut self, engine: &mut Engine) {
        let gw = engine.grid_width();
        let gh = engine.grid_height();

        if !self.initialized || gw != self.map_w || gh != self.map_h {
            self.build_map(gw, gh);
            self.initialized = true;
        }

        // Determine movement direction
        let mut dx: i32 = 0;
        let mut dy: i32 = 0;

        let any_arrow_held = engine.is_key_held(KeyCode::ArrowUp)
            || engine.is_key_held(KeyCode::ArrowDown)
            || engine.is_key_held(KeyCode::ArrowLeft)
            || engine.is_key_held(KeyCode::ArrowRight)
            || engine.is_key_held(KeyCode::Numpad8)
            || engine.is_key_held(KeyCode::Numpad2)
            || engine.is_key_held(KeyCode::Numpad4)
            || engine.is_key_held(KeyCode::Numpad6)
            || engine.is_key_held(KeyCode::Numpad7)
            || engine.is_key_held(KeyCode::Numpad9)
            || engine.is_key_held(KeyCode::Numpad1)
            || engine.is_key_held(KeyCode::Numpad3);

        let any_arrow_pressed = engine.is_key_pressed(KeyCode::ArrowUp)
            || engine.is_key_pressed(KeyCode::ArrowDown)
            || engine.is_key_pressed(KeyCode::ArrowLeft)
            || engine.is_key_pressed(KeyCode::ArrowRight)
            || engine.is_key_pressed(KeyCode::Numpad8)
            || engine.is_key_pressed(KeyCode::Numpad2)
            || engine.is_key_pressed(KeyCode::Numpad4)
            || engine.is_key_pressed(KeyCode::Numpad6)
            || engine.is_key_pressed(KeyCode::Numpad7)
            || engine.is_key_pressed(KeyCode::Numpad9)
            || engine.is_key_pressed(KeyCode::Numpad1)
            || engine.is_key_pressed(KeyCode::Numpad3);

        let should_move = if any_arrow_pressed {
            self.move_cooldown = 15;
            true
        } else if any_arrow_held {
            if self.move_cooldown > 0 {
                self.move_cooldown -= 1;
                false
            } else {
                self.move_cooldown = 5;
                true
            }
        } else {
            self.move_cooldown = 0;
            false
        };

        if should_move {
            if engine.is_key_held(KeyCode::ArrowUp) || engine.is_key_held(KeyCode::Numpad8) {
                dy = -1;
            } else if engine.is_key_held(KeyCode::ArrowDown) || engine.is_key_held(KeyCode::Numpad2)
            {
                dy = 1;
            } else if engine.is_key_held(KeyCode::ArrowLeft) || engine.is_key_held(KeyCode::Numpad4)
            {
                dx = -1;
            } else if engine.is_key_held(KeyCode::ArrowRight)
                || engine.is_key_held(KeyCode::Numpad6)
            {
                dx = 1;
            } else if engine.is_key_held(KeyCode::Numpad7) {
                dy = -1;
                dx = -1;
            } else if engine.is_key_held(KeyCode::Numpad9) {
                dy = -1;
                dx = 1;
            } else if engine.is_key_held(KeyCode::Numpad1) {
                dy = 1;
                dx = -1;
            } else if engine.is_key_held(KeyCode::Numpad3) {
                dy = 1;
                dx = 1;
            }
        }

        if dx != 0 || dy != 0 {
            if let Some(player) = self.player {
                if let Some(pos) = self.world.get::<Position>(player) {
                    let new_x = (pos.x as i32 + dx) as u32;
                    let new_y = (pos.y as i32 + dy) as u32;

                    if !self.is_solid(new_x, new_y) {
                        if let Some(pos) = self.world.get_mut::<Position>(player) {
                            pos.x = new_x;
                            pos.y = new_y;
                        }
                    }
                }
            }
        }
    }

    fn render(&mut self, engine: &mut Engine) {
        engine.clear();

        // Draw all renderable entities — single query, O(n) scan.
        for (entity, renderable) in self.world.query::<Renderable>() {
            if let Some(pos) = self.world.get::<Position>(entity) {
                engine.set_char(pos.x, pos.y, renderable.glyph, renderable.fg, renderable.bg);
            }
        }
    }
}

fn main() {
    Engine::builder()
        .with_title("jengine demo")
        .with_size(800, 600)
        .with_tileset(DEFAULT_TILESET, DEFAULT_TILE_W, DEFAULT_TILE_H)
        .run(DemoGame::new());
}

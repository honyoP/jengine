use std::f32::consts::TAU;

use jengine::ecs::{Entity, World};
use jengine::engine::{AnimationType, Color, Engine, Game, KeyCode};
use jengine::DEFAULT_TILESET;

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

struct Position { x: u32, y: u32 }

/// Used only for char-atlas entities (torches, glitch tiles, fire tiles).
struct Renderable { glyph: char, fg: Color, bg: Color }

struct Player;
struct Solid;

/// Marker: drawn as the "wall" sprite.
struct Wall;

/// Small enemy drawn as "small_enemy" sprite; reacts with Shiver when bumped.
struct Enemy;

/// Large 2×2-tile enemy drawn as "big_enemy" sprite.
struct BigEnemy;

/// Multi-tile footprint for collision / rendering. Defaults to 1×1 if absent.
struct Size { w: u32, h: u32 }

/// Sub-pixel floating position for a particle entity.
struct ParticlePosition { x: f32, y: f32 }

/// Physics and lifetime for a particle.
struct Particle {
    velocity: [f32; 2],
    lifetime: f32,
    max_lifetime: f32,
    drag: f32,
    color: [f32; 4],
    color_end: [f32; 4],
}

/// Smoke emitter (torch).
struct Torch;

/// A floor tile that triggers a glitch-line Shiver effect when the player steps on it.
struct GlitchTile;

/// A floor tile that triggers a fire-explosion Bash effect when the player steps on it.
struct FireTile;

// ---------------------------------------------------------------------------
// Particle helpers
// ---------------------------------------------------------------------------

fn pseudo_rand(seed: u64) -> f32 {
    let x = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (x >> 33) as f32 / u32::MAX as f32
}

fn update_particles(world: &mut World, dt: f32) {
    let dead: Vec<Entity> = world
        .query_multi_mut::<(Particle, ParticlePosition)>()
        .filter_map(|(entity, (p, pos))| {
            pos.x += p.velocity[0] * dt;
            pos.y += p.velocity[1] * dt;
            let drag = (1.0 - p.drag * dt).max(0.0);
            p.velocity[0] *= drag;
            p.velocity[1] *= drag;
            p.lifetime -= dt;
            if p.lifetime <= 0.0 { Some(entity) } else { None }
        })
        .collect();
    for e in dead { world.despawn(e); }
}

fn render_particles(world: &World, engine: &mut Engine) {
    for (e, p) in world.query::<Particle>() {
        if let Some(pos) = world.get::<ParticlePosition>(e) {
            let t = (p.lifetime / p.max_lifetime).clamp(0.0, 1.0);
            let c = p.color;
            let e2 = p.color_end;
            let r = e2[0] + (c[0] - e2[0]) * t;
            let g = e2[1] + (c[1] - e2[1]) * t;
            let b = e2[2] + (c[2] - e2[2]) * t;
            let a = e2[3] + (c[3] - e2[3]) * t;
            engine.draw_particle(pos.x, pos.y, Color([r, g, b, a]), 2.0);
        }
    }
}

// ── Emitters ────────────────────────────────────────────────────────────────

fn spawn_smoke(world: &mut World, tick: u64, tile_w: u32, tile_h: u32) {
    if tick % 10 != 0 { return; }

    let torches: Vec<(u32, u32)> = world
        .query::<Torch>()
        .filter_map(|(e, _)| world.get::<Position>(e).map(|p| (p.x, p.y)))
        .collect();

    for (tx, ty) in torches {
        let cx = tx as f32 * tile_w as f32 + tile_w as f32 * 0.5;
        let cy = ty as f32 * tile_h as f32;

        for i in 0..2u64 {
            let seed = tick.wrapping_mul(73)
                .wrapping_add(tx as u64 * 97)
                .wrapping_add(ty as u64 * 53)
                .wrapping_add(i * 11);

            let drift = (pseudo_rand(seed) - 0.5) * 24.0;
            let rise = -(20.0 + pseudo_rand(seed.wrapping_add(7)) * 40.0);
            let max_life = 1.0 + pseudo_rand(seed.wrapping_add(17)) * 0.8;

            let amber = [1.0_f32, 0.55 + pseudo_rand(seed.wrapping_add(23)) * 0.15, 0.05, 0.9];
            let grey = 0.4 + pseudo_rand(seed.wrapping_add(29)) * 0.2;
            let smoke = [grey, grey, grey * 0.95, 0.0];

            let e = world.spawn();
            world.insert(e, ParticlePosition { x: cx + drift * 0.08, y: cy });
            world.insert(e, Particle {
                velocity: [drift, rise],
                lifetime: max_life,
                max_lifetime: max_life,
                drag: 1.8,
                color: amber,
                color_end: smoke,
            });
        }
    }
}

fn spawn_fire_ambient(world: &mut World, tick: u64, tile_w: u32, tile_h: u32) {
    if tick % 6 != 0 { return; }

    let fires: Vec<(u32, u32)> = world
        .query::<FireTile>()
        .filter_map(|(e, _)| world.get::<Position>(e).map(|p| (p.x, p.y)))
        .collect();

    for (fx, fy) in fires {
        let cx = fx as f32 * tile_w as f32 + tile_w as f32 * 0.5;
        let cy = fy as f32 * tile_h as f32 + tile_h as f32 * 0.5;

        for i in 0..3u64 {
            let seed = tick.wrapping_mul(59)
                .wrapping_add(fx as u64 * 43)
                .wrapping_add(fy as u64 * 37)
                .wrapping_add(i * 29);

            let dx = (pseudo_rand(seed) - 0.5) * 10.0;
            let dy = -(15.0 + pseudo_rand(seed.wrapping_add(3)) * 30.0);
            let r = 0.9 + pseudo_rand(seed.wrapping_add(5)) * 0.1;
            let g = pseudo_rand(seed.wrapping_add(7)) * 0.55;
            let max_life = 0.35 + pseudo_rand(seed.wrapping_add(9)) * 0.3;

            let e = world.spawn();
            world.insert(e, ParticlePosition { x: cx + dx * 0.2, y: cy });
            world.insert(e, Particle {
                velocity: [dx, dy],
                lifetime: max_life,
                max_lifetime: max_life,
                drag: 2.5,
                color: [r, g, 0.0, 1.0],
                color_end: [r * 0.2, 0.0, 0.0, 0.0],
            });
        }
    }
}

fn spawn_glitch_ambient(world: &mut World, tick: u64, tile_w: u32, tile_h: u32) {
    if tick % 22 != 0 { return; }

    let glitches: Vec<(u32, u32)> = world
        .query::<GlitchTile>()
        .filter_map(|(e, _)| world.get::<Position>(e).map(|p| (p.x, p.y)))
        .collect();

    for (gx, gy) in glitches {
        let cx = gx as f32 * tile_w as f32 + tile_w as f32 * 0.5;
        let cy = gy as f32 * tile_h as f32 + tile_h as f32 * 0.5;
        let seed = tick.wrapping_mul(67)
            .wrapping_add(gx as u64 * 53)
            .wrapping_add(gy as u64 * 41);

        let vx = (pseudo_rand(seed) - 0.5) * 80.0;
        let offset_y = (pseudo_rand(seed.wrapping_add(1)) - 0.5) * tile_h as f32 * 0.8;

        let e = world.spawn();
        world.insert(e, ParticlePosition { x: cx, y: cy + offset_y });
        world.insert(e, Particle {
            velocity: [vx, (pseudo_rand(seed.wrapping_add(2)) - 0.5) * 6.0],
            lifetime: 0.09,
            max_lifetime: 0.09,
            drag: 1.0,
            color: [0.0, 1.0, 1.0, 1.0],
            color_end: [0.0, 1.0, 1.0, 0.0],
        });
    }
}

// ── Burst effects ────────────────────────────────────────────────────────────

fn spawn_blood_burst(world: &mut World, px: f32, py: f32, tick: u64) {
    for i in 0..12u64 {
        let seed = tick.wrapping_mul(31)
            .wrapping_add(i * 97)
            .wrapping_add(px as u64 * 13)
            .wrapping_add(py as u64 * 7);
        let angle = pseudo_rand(seed) * TAU;
        let speed = 50.0 + pseudo_rand(seed.wrapping_add(3)) * 90.0;
        let max_life = 0.25 + pseudo_rand(seed.wrapping_add(7)) * 0.3;
        let red = 0.75 + pseudo_rand(seed.wrapping_add(11)) * 0.25;

        let e = world.spawn();
        world.insert(e, ParticlePosition { x: px, y: py });
        world.insert(e, Particle {
            velocity: [angle.cos() * speed, angle.sin() * speed],
            lifetime: max_life,
            max_lifetime: max_life,
            drag: 7.0,
            color: [red, 0.0, 0.0, 1.0],
            color_end: [red * 0.3, 0.0, 0.0, 0.0],
        });
    }
}

fn spawn_glitch_burst(world: &mut World, px: f32, py: f32, tile_h: u32, tick: u64) {
    for i in 0..14u64 {
        let seed = tick.wrapping_mul(61).wrapping_add(i * 17);
        let offset_y = (pseudo_rand(seed) - 0.5) * tile_h as f32 * 1.6;
        let vx = (pseudo_rand(seed.wrapping_add(1)) - 0.5) * 140.0;
        let choice = pseudo_rand(seed.wrapping_add(2));
        let color = if choice < 0.45 {
            [0.0_f32, 1.0, 1.0, 1.0]
        } else if choice < 0.80 {
            [1.0, 1.0, 1.0, 0.9]
        } else {
            [0.8, 0.0, 1.0, 0.9]
        };
        let max_life = 0.1 + pseudo_rand(seed.wrapping_add(3)) * 0.18;

        let e = world.spawn();
        world.insert(e, ParticlePosition { x: px, y: py + offset_y });
        world.insert(e, Particle {
            velocity: [vx, (pseudo_rand(seed.wrapping_add(4)) - 0.5) * 12.0],
            lifetime: max_life,
            max_lifetime: max_life,
            drag: 2.5,
            color,
            color_end: [color[0], color[1], color[2], 0.0],
        });
    }
}

fn spawn_fire_burst(world: &mut World, px: f32, py: f32, tick: u64) {
    for i in 0..22u64 {
        let seed = tick.wrapping_mul(71).wrapping_add(i * 19);
        let angle = pseudo_rand(seed) * TAU;
        let speed = 35.0 + pseudo_rand(seed.wrapping_add(1)) * 95.0;
        let r = 0.85 + pseudo_rand(seed.wrapping_add(2)) * 0.15;
        let g = pseudo_rand(seed.wrapping_add(3)) * 0.6;
        let max_life = 0.3 + pseudo_rand(seed.wrapping_add(4)) * 0.35;

        let e = world.spawn();
        world.insert(e, ParticlePosition { x: px, y: py });
        world.insert(e, Particle {
            velocity: [angle.cos() * speed, angle.sin() * speed],
            lifetime: max_life,
            max_lifetime: max_life,
            drag: 4.0,
            color: [r, g, 0.0, 1.0],
            color_end: [r * 0.15, 0.0, 0.0, 0.0],
        });
    }
}

fn spawn_slash(world: &mut World, px: f32, py: f32, direction: [f32; 2], tick: u64) {
    let perp = [-direction[1], direction[0]];
    for i in 0..11u64 {
        let t = i as f32 / 10.0;
        let along = t - 0.5;
        let seed = tick.wrapping_mul(83).wrapping_add(i * 23);

        let ex = px + direction[0] * 10.0 + perp[0] * along * 18.0;
        let ey = py + direction[1] * 10.0 + perp[1] * along * 18.0;
        let speed = 25.0 + pseudo_rand(seed) * 30.0;
        let vx = direction[0] * speed + perp[0] * along * 12.0;
        let vy = direction[1] * speed + perp[1] * along * 12.0;
        let max_life = 0.12 + pseudo_rand(seed.wrapping_add(1)) * 0.08;

        let e = world.spawn();
        world.insert(e, ParticlePosition { x: ex, y: ey });
        world.insert(e, Particle {
            velocity: [vx, vy],
            lifetime: max_life,
            max_lifetime: max_life,
            drag: 9.0,
            color: [1.0, 0.95, 0.7, 1.0],
            color_end: [1.0, 0.95, 0.7, 0.0],
        });
    }
}

// ---------------------------------------------------------------------------
// DemoGame
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
        let to_despawn: Vec<Entity> =
            self.world.query::<Position>().map(|(e, _)| e).collect();
        for e in to_despawn { self.world.despawn(e); }

        let particles: Vec<Entity> =
            self.world.query::<ParticlePosition>().map(|(e, _)| e).collect();
        for e in particles { self.world.despawn(e); }

        self.map_w = w;
        self.map_h = h;

        let mut solids: Vec<(u32, u32)> = Vec::new();

        // Border walls
        for x in 0..w { solids.push((x, 0)); solids.push((x, h - 1)); }
        for y in 1..h - 1 { solids.push((0, y)); solids.push((w - 1, y)); }

        // Horizontal bar
        if w > 6 && h > 4 {
            let wall_y = h / 3;
            for x in 2..w - 4 { solids.push((x, wall_y)); }
        }

        // Vertical bar
        if w > 4 && h > 6 {
            let wall_x = w * 2 / 3;
            for y in h / 2..h - 2 { solids.push((wall_x, y)); }
        }

        for &(x, y) in &solids {
            let wall = self.world.spawn();
            self.world.insert(wall, Position { x, y });
            self.world.insert(wall, Solid);
            self.world.insert(wall, Wall);
        }

        // Torches — char-based (with Renderable)
        let torch_spots: [(u32, u32); 3] = [
            (2, 2),
            (w.saturating_sub(3), 2),
            (2, h.saturating_sub(3)),
        ];
        for &(tx, ty) in &torch_spots {
            if !solids.contains(&(tx, ty)) {
                let t = self.world.spawn();
                self.world.insert(t, Position { x: tx, y: ty });
                self.world.insert(t, Renderable { glyph: '\u{2020}', fg: Color::YELLOW, bg: Color::BLACK });
                self.world.insert(t, Torch);
            }
        }

        // Glitch tile — char-based (with Renderable)
        let gx = w / 4;
        let gy = h * 2 / 3;
        if !solids.contains(&(gx, gy)) {
            let gt = self.world.spawn();
            self.world.insert(gt, Position { x: gx, y: gy });
            self.world.insert(gt, Renderable { glyph: '\u{2248}', fg: Color::CYAN, bg: Color::BLACK });
            self.world.insert(gt, GlitchTile);
        }

        // Fire tile — char-based (with Renderable)
        let fx = w * 3 / 4;
        let fy = h * 2 / 3;
        if !solids.contains(&(fx, fy)) {
            let ft = self.world.spawn();
            self.world.insert(ft, Position { x: fx, y: fy });
            self.world.insert(ft, Renderable { glyph: '\u{25C6}', fg: Color([1.0, 0.45, 0.0, 1.0]), bg: Color::BLACK });
            self.world.insert(ft, FireTile);
        }

        // Small enemy — sprite-based (no Renderable)
        let ex = w / 2 + 3;
        let ey = h / 2;
        if !solids.contains(&(ex, ey)) {
            let en = self.world.spawn();
            self.world.insert(en, Position { x: ex, y: ey });
            self.world.insert(en, Solid);
            self.world.insert(en, Enemy);
        }

        // Big enemy (2×2 tile sprite) — sprite-based (no Renderable)
        // Placed in the lower-left quadrant, away from player spawn.
        let bx = w / 3;
        let by = h * 2 / 3 - 2;
        if !solids.contains(&(bx, by)) {
            let be = self.world.spawn();
            self.world.insert(be, Position { x: bx, y: by });
            self.world.insert(be, Size { w: 3, h: 3 });
            self.world.insert(be, Solid);
            self.world.insert(be, BigEnemy);
        }

        // Player — sprite-based (no Renderable)
        let player = self.world.spawn();
        self.world.insert(player, Position { x: w / 2, y: h / 2 });
        self.world.insert(player, Player);
        self.player = Some(player);
    }

    fn is_solid(&self, x: u32, y: u32) -> bool {
        self.world.query::<Solid>().any(|(e, _)| {
            self.world.get::<Position>(e).map_or(false, |p| {
                let (w, h) = self.world.get::<Size>(e).map_or((1, 1), |s| (s.w, s.h));
                x >= p.x && x < p.x + w && y >= p.y && y < p.y + h
            })
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

        let tw = engine.tile_width();
        let th = engine.tile_height();

        update_particles(&mut self.world, engine.dt());
        spawn_smoke(&mut self.world, engine.tick(), tw, th);
        spawn_fire_ambient(&mut self.world, engine.tick(), tw, th);
        spawn_glitch_ambient(&mut self.world, engine.tick(), tw, th);

        // ── Movement ──────────────────────────────────────────────────────

        let mut dx: i32 = 0;
        let mut dy: i32 = 0;

        let any_held = engine.is_key_held(KeyCode::ArrowUp)
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

        let any_pressed = engine.is_key_pressed(KeyCode::ArrowUp)
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

        let should_move = if any_pressed {
            self.move_cooldown = 15;
            true
        } else if any_held {
            if self.move_cooldown > 0 { self.move_cooldown -= 1; false }
            else { self.move_cooldown = 5; true }
        } else {
            self.move_cooldown = 0;
            false
        };

        if should_move {
            if engine.is_key_held(KeyCode::ArrowUp) || engine.is_key_held(KeyCode::Numpad8) { dy = -1; }
            else if engine.is_key_held(KeyCode::ArrowDown) || engine.is_key_held(KeyCode::Numpad2) { dy = 1; }
            else if engine.is_key_held(KeyCode::ArrowLeft) || engine.is_key_held(KeyCode::Numpad4) { dx = -1; }
            else if engine.is_key_held(KeyCode::ArrowRight) || engine.is_key_held(KeyCode::Numpad6) { dx = 1; }
            else if engine.is_key_held(KeyCode::Numpad7) { dx = -1; dy = -1; }
            else if engine.is_key_held(KeyCode::Numpad9) { dx = 1; dy = -1; }
            else if engine.is_key_held(KeyCode::Numpad1) { dx = -1; dy = 1; }
            else if engine.is_key_held(KeyCode::Numpad3) { dx = 1; dy = 1; }
        }

        if dx != 0 || dy != 0 {
            if let Some(player) = self.player {
                if let Some(pos) = self.world.get::<Position>(player) {
                    let new_x = (pos.x as i32 + dx) as u32;
                    let new_y = (pos.y as i32 + dy) as u32;
                    let px = (pos.x as f32 + 0.5) * tw as f32;
                    let py = (pos.y as f32 + 0.5) * th as f32;

                    if !self.is_solid(new_x, new_y) {
                        if let Some(pos) = self.world.get_mut::<Position>(player) {
                            pos.x = new_x;
                            pos.y = new_y;
                        }

                        let npx = (new_x as f32 + 0.5) * tw as f32;
                        let npy = (new_y as f32 + 0.5) * th as f32;

                        let on_glitch = self.world.query::<GlitchTile>().any(|(e, _)| {
                            self.world.get::<Position>(e).map_or(false, |p| p.x == new_x && p.y == new_y)
                        });
                        if on_glitch {
                            engine.play_animation(player, AnimationType::Shiver { magnitude: 3.5 });
                            spawn_glitch_burst(&mut self.world, npx, npy, th, engine.tick());
                        }

                        let on_fire = self.world.query::<FireTile>().any(|(e, _)| {
                            self.world.get::<Position>(e).map_or(false, |p| p.x == new_x && p.y == new_y)
                        });
                        if on_fire {
                            engine.play_animation(player, AnimationType::Bash {
                                direction: [0.0, -1.0],
                                magnitude: 4.0,
                            });
                            spawn_fire_burst(&mut self.world, npx, npy, engine.tick());
                        }
                    } else {
                        // Bumped a solid — check if it's a small or big enemy.
                        let enemy_ent: Option<Entity> = self.world
                            .query::<Enemy>()
                            .find_map(|(e, _)| {
                                if self.world.get::<Position>(e).map_or(false, |p| p.x == new_x && p.y == new_y) {
                                    Some(e)
                                } else {
                                    None
                                }
                            })
                            .or_else(|| {
                                self.world.query::<BigEnemy>().find_map(|(e, _)| {
                                    if self.world.get::<Position>(e).map_or(false, |p| {
                                        let (w, h) = self.world.get::<Size>(e).map_or((1, 1), |s| (s.w, s.h));
                                        new_x >= p.x && new_x < p.x + w && new_y >= p.y && new_y < p.y + h
                                    }) {
                                        Some(e)
                                    } else {
                                        None
                                    }
                                })
                            });

                        if let Some(enemy) = enemy_ent {
                            engine.play_animation(player, AnimationType::Bash {
                                direction: [dx as f32, dy as f32],
                                magnitude: 4.0,
                            });
                            engine.play_animation(enemy, AnimationType::Shiver { magnitude: 2.5 });
                            spawn_slash(&mut self.world, px, py, [dx as f32, dy as f32], engine.tick());
                        } else {
                            spawn_blood_burst(&mut self.world, px, py, engine.tick());
                        }
                    }
                }
            }
        }
    }

    fn render(&mut self, engine: &mut Engine) {
        engine.clear();

        // ── Char-atlas entities (torches, glitch tiles, fire tiles) ─────────
        // These use set_background + set_foreground_entity via the char atlas.
        for (entity, renderable) in self.world.query::<Renderable>() {
            if let Some(pos) = self.world.get::<Position>(entity) {
                engine.set_background(pos.x, pos.y, renderable.bg);
                engine.set_foreground_entity(pos.x, pos.y, entity, renderable.glyph, renderable.fg);
            }
        }

        // ── Sprite-atlas entities ────────────────────────────────────────────
        // Walls (layer 1, no animation)
        let walls: Vec<(u32, u32)> = self.world
            .query::<Wall>()
            .filter_map(|(e, _)| self.world.get::<Position>(e).map(|p| (p.x, p.y)))
            .collect();
        for (wx, wy) in walls {
            engine.draw_sprite(wx, wy, "wall", 1, Color::WHITE);
        }

        // Small enemies (sprite, animated)
        let small_enemies: Vec<(Entity, u32, u32)> = self.world
            .query::<Enemy>()
            .filter_map(|(e, _)| self.world.get::<Position>(e).map(|p| (e, p.x, p.y)))
            .collect();
        for (entity, ex, ey) in small_enemies {
            engine.draw_sprite_entity(ex, ey, "small_enemy", entity, Color::WHITE);
        }

        // Big enemies (2×2 tile sprite, animated)
        let big_enemies: Vec<(Entity, u32, u32)> = self.world
            .query::<BigEnemy>()
            .filter_map(|(e, _)| self.world.get::<Position>(e).map(|p| (e, p.x, p.y)))
            .collect();
        for (entity, bx, by) in big_enemies {
            engine.draw_sprite_entity(bx, by, "big_enemy", entity, Color::WHITE);
        }

        // Player (sprite, animated) — drawn last so it renders on top
        if let Some(player) = self.player {
            if let Some(pos) = self.world.get::<Position>(player) {
                engine.draw_sprite_entity(pos.x, pos.y, "player", player, Color::WHITE);
            }
        }

        // ── Sub-pixel particles ──────────────────────────────────────────────
        render_particles(&self.world, engine);
    }
}

fn main() {
    Engine::builder()
        .with_title("jengine demo")
        .with_size(800, 600)
        // 16×24 tile grid — char tileset tiles are stretched to match.
        .with_tileset(DEFAULT_TILESET, 16, 24)
        // Sprite folder: player, small_enemy, big_enemy, wall PNGs.
        .with_sprite_folder("resources/sprites")
        .run(DemoGame::new());
}
use std::f32::consts::TAU;
use jengine::engine::{Color, Game, jEngine, KeyCode};
use jengine::ui::{Padding, BorderStyle};
use jengine::ui::widgets::Widget;
use jengine::ecs::{Entity, World};
use jengine::{DEFAULT_TILESET, DEFAULT_FONT_METADATA, DEFAULT_TILE_W, DEFAULT_TILE_H};

// ── Components ───────────────────────────────────────────────────────────────

struct Position { x: f32, y: f32 }
struct Velocity { vx: f32, vy: f32 }
struct Life { current: f32, max: f32 }
struct Particle {
    color_start: [f32; 4],
    color_end: [f32; 4],
    size_start: f32,
    size_end: f32,
    drag: f32,
}

// ── Helper ───────────────────────────────────────────────────────────────────

fn pseudo_rand(seed: u64) -> f32 {
    let x = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (x >> 33) as f32 / u32::MAX as f32
}

// ── Demo State ───────────────────────────────────────────────────────────────

struct PostProcessState {
    scanlines: bool,
    vignette: bool,
    chromatic: bool,
    bloom: bool,
}

struct ParticleDemo {
    world: World,
    font_loaded: bool,
    tick: u64,
    pp: PostProcessState,
}

impl ParticleDemo {
    fn new() -> Self {
        Self {
            world: World::new(),
            font_loaded: false,
            tick: 0,
            pp: PostProcessState {
                scanlines: false,
                vignette: false,
                chromatic: false,
                bloom: false,
            },
        }
    }

    fn spawn_particle(&mut self, x: f32, y: f32, vx: f32, vy: f32, life: f32, 
                      color_start: [f32; 4], color_end: [f32; 4], 
                      size_start: f32, size_end: f32, drag: f32) {
        let e = self.world.spawn();
        self.world.insert(e, Position { x, y });
        self.world.insert(e, Velocity { vx, vy });
        self.world.insert(e, Life { current: life, max: life });
        self.world.insert(e, Particle { color_start, color_end, size_start, size_end, drag });
    }

    // 1. Fire with Smoke
    fn spawn_fire_and_smoke(&mut self, x: f32, y: f32) {
        // Fire particles: fast, orange/yellow, short life
        for i in 0..2 {
            let seed = self.tick.wrapping_add(i as u64 * 137);
            let vx = (pseudo_rand(seed) - 0.5) * 60.0;
            let vy = -100.0 - pseudo_rand(seed.wrapping_add(1)) * 50.0;
            let life = 0.2 + pseudo_rand(seed.wrapping_add(2)) * 0.3;
            
            let color_start = [1.0, 0.9, 0.2, 1.0]; // Bright yellow
            let color_end = [1.0, 0.2, 0.0, 0.0];   // Fades to red/transparent
            
            self.spawn_particle(x, y, vx, vy, life, color_start, color_end, 8.0, 2.0, 1.5);
        }

        // Smoke: slow, grey, long life, rises higher
        if self.tick % 4 == 0 {
            let seed = self.tick.wrapping_add(999);
            let vx = (pseudo_rand(seed) - 0.5) * 40.0;
            let vy = -60.0 - pseudo_rand(seed.wrapping_add(1)) * 30.0;
            let life = 1.5 + pseudo_rand(seed.wrapping_add(2)) * 1.0;
            
            let g = 0.4 + pseudo_rand(seed.wrapping_add(3)) * 0.2;
            let color_start = [g, g, g, 0.5];
            let color_end = [g * 0.5, g * 0.5, g * 0.5, 0.0];
            
            self.spawn_particle(x, y - 10.0, vx, vy, life, color_start, color_end, 4.0, 12.0, 0.8);
        }
    }

    // 2. Explosion: grandiose multi-layered burst
    fn spawn_explosion(&mut self, engine: &mut jEngine, x: f32, y: f32) {
        // Trigger a camera shake for impact
        engine.camera_shake(15.0);

        // --- Layer A: The Core Flash (Bright, fast, very short) ---
        for i in 0..30 {
            let seed = self.tick.wrapping_add(i as u64 * 71);
            let angle = pseudo_rand(seed) * TAU;
            let speed = 200.0 + pseudo_rand(seed.wrapping_add(1)) * 400.0;
            let vx = angle.cos() * speed;
            let vy = angle.sin() * speed;
            let life = 0.1 + pseudo_rand(seed.wrapping_add(2)) * 0.15;
            self.spawn_particle(x, y, vx, vy, life, [1.0, 1.0, 1.0, 1.0], [1.0, 0.9, 0.5, 0.0], 12.0, 4.0, 5.0);
        }

        // --- Layer B: High-velocity Sparks (Fast, leave trails) ---
        for i in 0..80 {
            let seed = self.tick.wrapping_add(i as u64 * 113 + 500);
            let angle = pseudo_rand(seed) * TAU;
            let speed = 100.0 + pseudo_rand(seed.wrapping_add(1)) * 600.0;
            let vx = angle.cos() * speed;
            let vy = angle.sin() * speed;
            let life = 0.3 + pseudo_rand(seed.wrapping_add(2)) * 0.5;
            self.spawn_particle(x, y, vx, vy, life, [1.0, 0.6, 0.1, 1.0], [0.8, 0.1, 0.0, 0.0], 4.0, 1.0, 3.0);
        }

        // --- Layer C: Hot Embers / Smoke (Slow, lingering, rise slightly) ---
        for i in 0..40 {
            let seed = self.tick.wrapping_add(i as u64 * 197 + 1000);
            let angle = pseudo_rand(seed) * TAU;
            let speed = 20.0 + pseudo_rand(seed.wrapping_add(1)) * 80.0;
            let vx = angle.cos() * speed;
            let vy = angle.sin() * speed - 20.0; // Bias upward
            let life = 0.8 + pseudo_rand(seed.wrapping_add(2)) * 1.2;
            let grey = 0.3 + pseudo_rand(seed.wrapping_add(3)) * 0.2;
            self.spawn_particle(x, y, vx, vy, life, [1.0, 0.3, 0.0, 0.8], [grey, grey, grey, 0.0], 6.0, 16.0, 1.0);
        }
    }

    // 3. Glitch: horizontal jittery streaks
    fn spawn_glitch(&mut self, x: f32, y: f32) {
        let count = 12;
        for i in 0..count {
            let seed = self.tick.wrapping_add(i as u64 * 31);
            let vx = (pseudo_rand(seed) - 0.5) * 600.0;
            let vy = (pseudo_rand(seed.wrapping_add(1)) - 0.5) * 10.0;
            let life = 0.05 + pseudo_rand(seed.wrapping_add(2)) * 0.15;
            
            let r = pseudo_rand(seed.wrapping_add(3));
            let color = if r < 0.33 {
                [0.0, 1.0, 1.0, 1.0] // Cyan
            } else if r < 0.66 {
                [1.0, 0.0, 1.0, 1.0] // Magenta
            } else {
                [1.0, 1.0, 1.0, 1.0] // White
            };
            
            // Glitch particles are wide but thin
            self.spawn_particle(x, y + (pseudo_rand(seed.wrapping_add(4)) - 0.5) * 40.0, 
                               vx, vy, life, color, color, 12.0, 2.0, 0.0);
        }
    }

    // 4. Slash: arc of light
    fn spawn_slash(&mut self, x: f32, y: f32) {
        let count = 20;
        let base_angle = pseudo_rand(self.tick) * TAU;
        for i in 0..count {
            let seed = self.tick.wrapping_add(i as u64 * 17);
            let angle = base_angle + (i as f32 / count as f32 - 0.5) * 0.8;
            let speed = 300.0 + pseudo_rand(seed) * 150.0;
            
            let vx = angle.cos() * speed;
            let vy = angle.sin() * speed;
            let life = 0.1 + pseudo_rand(seed.wrapping_add(1)) * 0.1;
            
            let color_start = [0.8, 0.9, 1.0, 1.0]; // Pale blue
            let color_end = [1.0, 1.0, 1.0, 0.0];
            
            self.spawn_particle(x, y, vx, vy, life, color_start, color_end, 2.0, 6.0, 8.0);
        }
    }
}

impl Game for ParticleDemo {
    fn on_enter(&mut self, engine: &mut jEngine) {
        let sw = engine.renderer.window.inner_size().width as f32;
        let sh = engine.renderer.window.inner_size().height as f32;
        // Center camera so world coordinates match screen coordinates (0,0 is top-left)
        engine.set_camera_pos(sw * 0.5, sh * 0.5);
    }

    fn update(&mut self, engine: &mut jEngine) {
        self.tick += 1;
        let dt = engine.dt();

        if engine.is_key_pressed(KeyCode::Escape) {
            engine.request_quit();
        }

        // --- Continuous Fire at bottom ---
        let sw = engine.renderer.window.inner_size().width as f32;
        let sh = engine.renderer.window.inner_size().height as f32;
        self.spawn_fire_and_smoke(sw * 0.5, sh * 0.85);

        // --- Triggered effects ---
        if engine.input.is_mouse_pressed(jengine::input::MouseButton::Left) && !engine.input.mouse_consumed {
            let [mx, my] = engine.input.mouse_pos;
            self.spawn_explosion(engine, mx, my);
        }

        if engine.is_key_pressed(KeyCode::KeyG) {
            let [mx, my] = engine.input.mouse_pos;
            self.spawn_glitch(mx, my);
        }

        if engine.is_key_pressed(KeyCode::KeyS) {
            let [mx, my] = engine.input.mouse_pos;
            self.spawn_slash(mx, my);
        }

        // --- Post-processing Toggles ---
        let mut pp_changed = false;
        if engine.is_key_pressed(KeyCode::Digit1) { self.pp.scanlines = !self.pp.scanlines; pp_changed = true; }
        if engine.is_key_pressed(KeyCode::Digit2) { self.pp.vignette = !self.pp.vignette; pp_changed = true; }
        if engine.is_key_pressed(KeyCode::Digit3) { self.pp.chromatic = !self.pp.chromatic; pp_changed = true; }
        if engine.is_key_pressed(KeyCode::Digit4) { self.pp.bloom = !self.pp.bloom; pp_changed = true; }

        if pp_changed {
            engine.renderer.post_process.clear_effects();
            if self.pp.scanlines { engine.set_scanlines(true); }
            if self.pp.vignette { engine.set_vignette(true); }
            if self.pp.chromatic { engine.set_chromatic_aberration(true); }
            if self.pp.bloom { engine.set_bloom(true); }
        }

        // --- Movement & Lifetime System ---
        let dead: Vec<Entity> = self.world.query_multi_mut::<(Position, Velocity, Life, Particle)>()
            .filter_map(|(e, (pos, vel, life, p))| {
                life.current -= dt;
                if life.current <= 0.0 {
                    return Some(e);
                }

                // Drag: v = v * (1 - drag * dt)
                let drag_factor = (1.0 - p.drag * dt).max(0.0);
                vel.vx *= drag_factor;
                vel.vy *= drag_factor;

                pos.x += vel.vx * dt;
                pos.y += vel.vy * dt;

                None
            })
            .collect();

        for e in dead {
            self.world.despawn(e);
        }
    }

    fn render(&mut self, engine: &mut jEngine) {
        if !self.font_loaded {
            if let Ok(font) = jengine::renderer::text::Font::from_mtsdf_json(DEFAULT_FONT_METADATA) {
                engine.renderer.set_mtsdf_distance_range(font.distance_range);
                engine.ui.text.set_font(font);
            }
            self.font_loaded = true;
        }

        engine.clear();

        let sw = engine.renderer.window.inner_size().width as f32;
        let sh = engine.renderer.window.inner_size().height as f32;

        // Set dark navy background using the tile grid (Pass 1)
        // This ensures particles (Pass 3) are drawn ON TOP of the background.
        for y in 0..engine.grid_height() {
            for x in 0..engine.grid_width() {
                engine.set_background(x, y, Color([0.01, 0.01, 0.02, 1.0]));
            }
        }

        // --- Render System ---
        for (_e, (pos, life, p)) in self.world.query_multi::<(Position, Life, Particle)>() {
            let t = (life.current / life.max).clamp(0.0, 1.0);
            
            // Lerp color
            let mut c = [0.0; 4];
            for i in 0..4 {
                c[i] = p.color_end[i] + (p.color_start[i] - p.color_end[i]) * t;
            }
            
            // Lerp size
            let size = p.size_end + (p.size_start - p.size_end) * t;
            
            engine.draw_particle(pos.x, pos.y, Color(c), size);
        }

        // --- UI ---
        let tw = engine.tile_width() as f32;
        let th = engine.tile_height() as f32;
        
        engine.ui.ui_text(tw, th, "PARTICLE SHOWCASE", Color::WHITE, Color::TRANSPARENT, Some(48.0));
        
        let mut y = th * 4.0;
        let help = [
            "MOUSE LCLICK : Spawn Explosion",
            "[S] KEY      : Spawn Slash (at mouse)",
            "[G] KEY      : Spawn Glitch (at mouse)",
            "CONTINUOUS   : Fire & Smoke (bottom)",
            "[ESC]        : Quit demo"
        ];
        
        for line in help {
            engine.ui.ui_text(tw, y, line, Color([0.6, 0.7, 0.7, 1.0]), Color::TRANSPARENT, None);
            y += th * 1.2;
        }

        y += th;
        let pp_help = [
            format!("[1] Scanlines: {}", if self.pp.scanlines { "ON" } else { "OFF" }),
            format!("[2] Vignette:  {}", if self.pp.vignette  { "ON" } else { "OFF" }),
            format!("[3] Chromatic: {}", if self.pp.chromatic { "ON" } else { "OFF" }),
            format!("[4] Bloom:     {}", if self.pp.bloom     { "ON" } else { "OFF" }),
        ];
        for line in pp_help {
            engine.ui.ui_text(tw, y, &line, Color([0.4, 0.9, 0.6, 1.0]), Color::TRANSPARENT, None);
            y += th * 1.2;
        }
        
        let count = self.world.entity_count();
        engine.ui.ui_text(sw - tw * 18.0, sh - th * 2.5, &format!("Active Entities: {}", count), Color::CYAN, Color::TRANSPARENT, Some(20.0));
    }

    fn debug_render(&mut self, engine: &mut jEngine) -> Option<Box<dyn jengine::ui::widgets::Widget>> {
        use jengine::ui::widgets::{VStack, TextWidget};
        use jengine::ui::Alignment;

        let fs = 12.0;
        let tw = engine.tile_width() as f32;
        let th = engine.tile_height() as f32;

        // ── 1. World-Space Highlight & Standalone Hover Popup ──
        let [mx, my] = engine.input.mouse_pos;
        let [wx, wy] = engine.screen_to_world(mx, my);
        let gx = (wx / tw).floor();
        let gy = (wy / th).floor();
        
        // Highlight tile
        let [s_x, s_y] = engine.world_to_screen(gx * tw, gy * th);
        let z = engine.camera_zoom();
        engine.ui.debug_box(s_x, s_y, tw * z, th * z, Color::CYAN);

        // List entities at hovered tile (pos is in pixel space; gx/gy are tile indices).
        let mut hovered_entities = Vec::new();
        for (entity, pos) in self.world.query::<Position>() {
            if pos.x >= gx * tw && pos.x < (gx + 1.0) * tw
                && pos.y >= gy * th && pos.y < (gy + 1.0) * th {
                let components = self.world.components_for_entity(entity);
                let short_names: Vec<String> = components.iter()
                    .map(|&full_name| full_name.split("::").last().unwrap_or(full_name).to_string())
                    .collect();
                hovered_entities.push((entity, short_names));
            }
        }

        // Draw standalone popup next to cursor using a styled VStack
        if !hovered_entities.is_empty() {
            let h_x = mx + 15.0;
            let h_y = my + 15.0;
            let panel_w = 220.0;
            
            let mut popup = VStack::new(Alignment::Start)
                .with_padding(Padding::all(5.0))
                .with_bg(Color([0.05, 0.05, 0.1, 0.8]))
                .with_border(BorderStyle::Thin, Color::CYAN);
            
            for (entity, comps) in hovered_entities {
                popup = popup.add(TextWidget {
                    text: format!("E{}: {}", entity.id(), comps.join(", ")),
                    size: Some(fs),
                    color: Some(Color::WHITE),
                });
            }
            Widget::draw(&mut popup, engine, h_x, h_y, panel_w, None);
        }

        // ── 2. Build Draggable Content ──
        let total_entities = self.world.entity_count();
        let mut stack = VStack::new(Alignment::Start).with_spacing(2.0);

        stack = stack.add(TextWidget {
            text: format!("Entities (Total): {}", total_entities),
            size: Some(fs),
            color: Some(Color::DARK_GRAY),
        });

        stack = stack.add(TextWidget {
            text: "--- ENTITY LIST ---".to_string(),
            size: Some(fs),
            color: Some(Color::CYAN),
        });

        // Fetch first 100 entities to ensure we have enough content to scroll
        let entities = self.world.entities_debug_info_paginated(0, 100);
        for (entity, components) in entities {
            let mut short_comps = Vec::new();
            for c in components {
                short_comps.push(c.split("::").last().unwrap_or(c));
            }
            stack = stack.add(TextWidget {
                text: format!("E{}: {}", entity.id(), short_comps.join(", ")),
                size: Some(fs),
                color: Some(Color::BLACK),
            });
        }

        if total_entities > 10 {
            stack = stack.add(TextWidget {
                text: "... and more".to_string(),
                size: Some(fs),
                color: Some(Color([0.4, 0.4, 0.4, 1.0])),
            });
        }

        Some(Box::new(stack))
    }
}

fn main() {
    jEngine::builder()
        .with_title("jengine — Particle Showcase")
        .with_size(1280, 720)
        .with_tileset(DEFAULT_TILESET, DEFAULT_TILE_W, DEFAULT_TILE_H)
        .run(ParticleDemo::new());
}

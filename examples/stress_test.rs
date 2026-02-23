use jengine::engine::{Color, Game, jEngine, KeyCode};
use jengine::ecs::{Entity, World};
use jengine::input::{ActionMap, InputSource};
use jengine::{DEFAULT_TILESET, DEFAULT_FONT_METADATA, DEFAULT_TILE_W, DEFAULT_TILE_H};

// ── Components ───────────────────────────────────────────────────────────────

struct Position { x: f32, y: f32 }
struct Velocity { vx: f32, vy: f32 }
#[allow(dead_code)]
struct Life { current: f32, max: f32 }
struct EntityMarker; // Marker for the "crowd" entities

// ── Stress Test State ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum StressAction {
    IncEntities,
    DecEntities,
    IncParticles,
    DecParticles,
    Nova,
    Quit,
}

struct StressTest {
    world: World,
    actions: ActionMap<StressAction>,
    font_loaded: bool,
    particle_spawn_rate: usize, // particles per frame
    entity_target_count: usize,
}

impl StressTest {
    fn new() -> Self {
        let mut actions = ActionMap::new();
        actions.bind(StressAction::IncEntities,  InputSource::Key(KeyCode::KeyW));
        actions.bind(StressAction::DecEntities,  InputSource::Key(KeyCode::KeyQ));
        actions.bind(StressAction::IncParticles, InputSource::Key(KeyCode::KeyS));
        actions.bind(StressAction::DecParticles, InputSource::Key(KeyCode::KeyA));
        actions.bind(StressAction::Nova,         InputSource::Key(KeyCode::Space));
        actions.bind(StressAction::Quit,         InputSource::Key(KeyCode::Escape));

        Self {
            world: World::new(),
            actions,
            font_loaded: false,
            particle_spawn_rate: 10,
            entity_target_count: 100,
        }
    }

    fn spawn_particle(&mut self, x: f32, y: f32, vx: f32, vy: f32) {
        let e = self.world.spawn();
        self.world.insert(e, Position { x, y });
        self.world.insert(e, Velocity { vx, vy });
        self.world.insert(e, Life { current: 1.5, max: 1.5 });
    }

    fn spawn_entity(&mut self, sw: f32, sh: f32) {
        let e = self.world.spawn();
        let x = (pseudo_rand(self.world.entity_count() as u64) * sw) as u32;
        let y = (pseudo_rand(self.world.entity_count() as u64 + 7) * sh) as u32;
        
        // We use the grid path for entities to stress the grid vertex building
        // But since we want them to move smoothly, we use ParticlePosition/Velocity 
        // and just draw them as sprites or particles for this test.
        self.world.insert(e, EntityMarker);
        self.world.insert(e, Position { x: x as f32, y: y as f32 });
        let vx = (pseudo_rand(e.id() as u64) - 0.5) * 100.0;
        let vy = (pseudo_rand(e.id() as u64 + 1) - 0.5) * 100.0;
        self.world.insert(e, Velocity { vx, vy });
    }
}

fn pseudo_rand(seed: u64) -> f32 {
    let x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (x >> 33) as f32 / u32::MAX as f32
}

impl Game for StressTest {
    fn on_enter(&mut self, engine: &mut jEngine) {
        engine.audio.load_sound("UI_click", "resources/audio/UI_click.wav");
    }

    fn update(&mut self, engine: &mut jEngine) {
        let dt = engine.dt();
        let sw = engine.renderer.window.inner_size().width as f32;
        let sh = engine.renderer.window.inner_size().height as f32;

        if self.actions.is_pressed(StressAction::Quit, &engine.input) {
            engine.play_sound("UI_click");
            engine.request_quit();
        }

        // ── Adjust Load ──
        if self.actions.is_held(StressAction::IncEntities, &engine.input) { self.entity_target_count += 5; }
        if self.actions.is_held(StressAction::DecEntities, &engine.input) { self.entity_target_count = self.entity_target_count.saturating_sub(5); }
        if self.actions.is_held(StressAction::IncParticles, &engine.input) { self.particle_spawn_rate += 2; }
        if self.actions.is_held(StressAction::DecParticles, &engine.input) { self.particle_spawn_rate = self.particle_spawn_rate.saturating_sub(2); }

        // ── Maintain Entity Count ──
        let current_entities = self.world.query::<EntityMarker>().count();
        if current_entities < self.entity_target_count {
            for _ in 0..10 { self.spawn_entity(sw, sh); }
        } else if current_entities > self.entity_target_count {
            let to_kill: Vec<Entity> = self.world.query::<EntityMarker>().take(10).map(|(e, _)| e).collect();
            for e in to_kill { self.world.despawn(e); }
        }

        // ── Spawn Particles ──
        for _ in 0..self.particle_spawn_rate {
            let vx = (pseudo_rand(engine.tick() + self.world.entity_count() as u64) - 0.5) * 300.0;
            let vy = (pseudo_rand(engine.tick() + self.world.entity_count() as u64 + 1) - 0.5) * 300.0;
            self.spawn_particle(sw * 0.5, sh * 0.5, vx, vy);
        }

        if self.actions.is_pressed(StressAction::Nova, &engine.input) {
            for i in 0..2000 {
                let angle = (i as f32 / 2000.0) * std::f32::consts::TAU;
                let speed = 200.0 + pseudo_rand(i as u64) * 400.0;
                self.spawn_particle(sw * 0.5, sh * 0.5, angle.cos() * speed, angle.sin() * speed);
            }
            engine.camera_shake(15.0);
        }

        // ── System: Movement (Entities) ──
        for (_e, (pos, vel, _marker)) in self.world.query_multi_mut::<(Position, Velocity, EntityMarker)>() {
            pos.x += vel.vx * dt;
            pos.y += vel.vy * dt;

            // Bounce entities off walls
            if pos.x < 0.0 || pos.x > sw { vel.vx *= -1.0; pos.x = pos.x.clamp(0.0, sw); }
            if pos.y < 0.0 || pos.y > sh { vel.vy *= -1.0; pos.y = pos.y.clamp(0.0, sh); }
        }

        // ── System: Movement & Life (Particles) ──
        let mut dead = Vec::new();
        for (e, (pos, vel, life)) in self.world.query_multi_mut::<(Position, Velocity, Life)>() {
            pos.x += vel.vx * dt;
            pos.y += vel.vy * dt;

            life.current -= dt;
            if life.current <= 0.0 { dead.push(e); }
        }
        for e in dead { self.world.despawn(e); }
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

        // ── Draw Entities (as small circles for speed) ──
        for (_e, (_m, pos)) in self.world.query_multi::<(EntityMarker, Position)>() {
            engine.draw_particle(pos.x, pos.y, Color([0.4, 0.7, 1.0, 1.0]), 4.0);
        }

        // ── Draw Particles ──
        for (_e, (_l, pos)) in self.world.query_multi::<(Life, Position)>() {
            engine.draw_particle(pos.x, pos.y, Color([1.0, 0.5, 0.2, 0.8]), 2.0);
        }

        // ── Stats UI ──
        let th = engine.tile_height() as f32;
        
        engine.ui.ui_rect(0.0, 0.0, 350.0, th * 8.0, Color([0.0, 0.0, 0.0, 0.7]));
        
        let mut y = 10.0;
        let lines = [
            format!("STRESS TEST - [Space] for Nova"),
            format!("Entities:  {} (Q/W to adj)", self.entity_target_count),
            format!("P-Rate:    {} / frame (A/S to adj)", self.particle_spawn_rate),
            format!("Total ECS: {}", self.world.entity_count()),
            format!("FPS:       {:.1}", 1.0 / engine.dt().max(0.001)),
        ];

        for line in lines {
            engine.ui.ui_text(10.0, y, &line, Color::WHITE, Color::TRANSPARENT, None);
            y += th;
        }
    }
}

fn main() {
    jEngine::builder()
        .with_title("jengine — Performance Stress Test")
        .with_size(1280, 720)
        .with_tileset(DEFAULT_TILESET, DEFAULT_TILE_W, DEFAULT_TILE_H)
        .run(StressTest::new());
}

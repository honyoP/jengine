// I am a very lazy person, so this demo wasn't actually built by me, but by Claude Opus 4.6. If
// you stumble upon something extremely nasty, don't blame me. Thanks.
use std::collections::VecDeque;
use std::f32::consts::TAU;

use jengine::ecs::{Entity, World};
use jengine::ui::{BorderStyle, Label};
use jengine::ui::widgets::{Dropdown, InputBox, ToggleSelector};
use jengine::engine::{
    AnimationType, Color, Engine, Game, KeyCode,
};
use jengine::renderer::text::Font;
use jengine::window::{WindowConfig, WindowMode, apply_window_settings};
use jengine::{DEFAULT_TILESET, DEFAULT_FONT_GLYPHS, DEFAULT_TILE_W, DEFAULT_TILE_H};

// ── Palette ─────────────────────────────────────────────────────────────────
const PANEL_BG:     Color = Color([0.06, 0.09, 0.09, 1.0]);
const PANEL_BORDER: Color = Color([0.25, 0.65, 0.50, 1.0]);
const UI_TEXT:      Color = Color([0.75, 0.85, 0.80, 1.0]);
const UI_DIM:       Color = Color([0.40, 0.50, 0.48, 1.0]);
const UI_BRIGHT:    Color = Color([1.00, 0.95, 0.20, 1.0]);
const UI_ACCENT:    Color = Color([0.20, 0.90, 0.70, 1.0]);
const UI_RED:       Color = Color([0.95, 0.25, 0.15, 1.0]);
const HP_FILL:      Color = Color([0.05, 0.75, 0.15, 1.0]);
const HP_EMPTY:     Color = Color([0.05, 0.18, 0.08, 1.0]);
const XP_FILL:      Color = Color([0.10, 0.45, 1.00, 1.0]);
const XP_EMPTY:     Color = Color([0.02, 0.05, 0.18, 1.0]);
const LOG_BG:       Color = Color([0.03, 0.06, 0.07, 1.0]);

// ── ECS Components ───────────────────────────────────────────────────────────

struct Position { x: u32, y: u32 }

struct Renderable { glyph: char, fg: Color, bg: Color }

struct Player;
struct Solid;
struct Wall;
struct Enemy;
struct BigEnemy;
struct Size { w: u32, h: u32 }
struct Sprite { sprite_name: String }

/// NPC that opens a dialogue when bumped.
struct DialogueNpc;

struct ParticlePosition { x: f32, y: f32 }
struct Particle {
    velocity: [f32; 2],
    lifetime: f32,
    max_lifetime: f32,
    drag: f32,
    color: [f32; 4],
    color_end: [f32; 4],
}
struct Torch;
struct GlitchTile;
struct FireTile;

// ── UI State ─────────────────────────────────────────────────────────────────

#[derive(Copy, Clone, PartialEq, Debug)]
enum Tab { Inventory, SkillTree, Relationship }

struct LogEntry { text: String, color: Color }

struct DialogueState {
    npc_name: String,
    body:     String,
    options:  Vec<String>,
    selected: usize,
}

struct UiState {
    log_open:       bool,
    inventory_open: bool,
    active_tab:     Tab,
    log_messages:   VecDeque<LogEntry>,
    dialogue:       Option<DialogueState>,
    player_hp:      f32,
    player_max_hp:  f32,
    player_xp:      f32,
    player_max_xp:  f32,
}

impl UiState {
    fn new() -> Self {
        let mut s = Self {
            log_open:       false,
            inventory_open: false,
            active_tab:     Tab::Inventory,
            log_messages:   VecDeque::new(),
            dialogue:       None,
            player_hp:      80.0,
            player_max_hp:  100.0,
            player_xp:      450.0,
            player_max_xp:  1000.0,
        };
        s.log("Welcome to jengine. Press [i] inventory, [l] log.", UI_ACCENT);
        s.log("The ancient ruins feel uneasy today.", UI_TEXT);
        s.log("A chill runs through the air.", UI_DIM);
        s.log("You equip the rusty blade.", UI_TEXT);
        s.log("You sense hostility nearby.", UI_RED);
        s
    }

    fn log(&mut self, text: &str, color: Color) {
        if self.log_messages.len() >= 60 {
            self.log_messages.pop_front();
        }
        self.log_messages.push_back(LogEntry { text: text.to_string(), color });
    }
}

// ── Particle helpers ─────────────────────────────────────────────────────────

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
            let drift   = (pseudo_rand(seed) - 0.5) * 24.0;
            let rise    = -(20.0 + pseudo_rand(seed.wrapping_add(7)) * 40.0);
            let max_life = 1.0 + pseudo_rand(seed.wrapping_add(17)) * 0.8;
            let amber   = [1.0_f32, 0.55 + pseudo_rand(seed.wrapping_add(23)) * 0.15, 0.05, 0.9];
            let grey    = 0.4 + pseudo_rand(seed.wrapping_add(29)) * 0.2;
            let smoke   = [grey, grey, grey * 0.95, 0.0];
            let e = world.spawn();
            world.insert(e, ParticlePosition { x: cx + drift * 0.08, y: cy });
            world.insert(e, Particle { velocity: [drift, rise], lifetime: max_life,
                max_lifetime: max_life, drag: 1.8, color: amber, color_end: smoke });
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
            let r  = 0.9 + pseudo_rand(seed.wrapping_add(5)) * 0.1;
            let g  = pseudo_rand(seed.wrapping_add(7)) * 0.55;
            let max_life = 0.35 + pseudo_rand(seed.wrapping_add(9)) * 0.3;
            let e = world.spawn();
            world.insert(e, ParticlePosition { x: cx + dx * 0.2, y: cy });
            world.insert(e, Particle { velocity: [dx, dy], lifetime: max_life,
                max_lifetime: max_life, drag: 2.5,
                color: [r, g, 0.0, 1.0], color_end: [r * 0.2, 0.0, 0.0, 0.0] });
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
        world.insert(e, Particle { velocity: [vx, (pseudo_rand(seed.wrapping_add(2)) - 0.5) * 6.0],
            lifetime: 0.09, max_lifetime: 0.09, drag: 1.0,
            color: [0.0, 1.0, 1.0, 1.0], color_end: [0.0, 1.0, 1.0, 0.0] });
    }
}

fn spawn_blood_burst(world: &mut World, px: f32, py: f32, tick: u64) {
    for i in 0..12u64 {
        let seed = tick.wrapping_mul(31).wrapping_add(i * 97)
            .wrapping_add(px as u64 * 13).wrapping_add(py as u64 * 7);
        let angle = pseudo_rand(seed) * TAU;
        let speed = 50.0 + pseudo_rand(seed.wrapping_add(3)) * 90.0;
        let max_life = 0.25 + pseudo_rand(seed.wrapping_add(7)) * 0.3;
        let red = 0.75 + pseudo_rand(seed.wrapping_add(11)) * 0.25;
        let e = world.spawn();
        world.insert(e, ParticlePosition { x: px, y: py });
        world.insert(e, Particle { velocity: [angle.cos() * speed, angle.sin() * speed],
            lifetime: max_life, max_lifetime: max_life, drag: 7.0,
            color: [red, 0.0, 0.0, 1.0], color_end: [red * 0.3, 0.0, 0.0, 0.0] });
    }
}

fn spawn_glitch_burst(world: &mut World, px: f32, py: f32, tile_h: u32, tick: u64) {
    for i in 0..14u64 {
        let seed = tick.wrapping_mul(61).wrapping_add(i * 17);
        let offset_y = (pseudo_rand(seed) - 0.5) * tile_h as f32 * 1.6;
        let vx = (pseudo_rand(seed.wrapping_add(1)) - 0.5) * 140.0;
        let choice = pseudo_rand(seed.wrapping_add(2));
        let color = if choice < 0.45 { [0.0_f32, 1.0, 1.0, 1.0] }
                    else if choice < 0.80 { [1.0, 1.0, 1.0, 0.9] }
                    else { [0.8, 0.0, 1.0, 0.9] };
        let max_life = 0.1 + pseudo_rand(seed.wrapping_add(3)) * 0.18;
        let e = world.spawn();
        world.insert(e, ParticlePosition { x: px, y: py + offset_y });
        world.insert(e, Particle { velocity: [vx, (pseudo_rand(seed.wrapping_add(4)) - 0.5) * 12.0],
            lifetime: max_life, max_lifetime: max_life, drag: 2.5,
            color, color_end: [color[0], color[1], color[2], 0.0] });
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
        world.insert(e, Particle { velocity: [angle.cos() * speed, angle.sin() * speed],
            lifetime: max_life, max_lifetime: max_life, drag: 4.0,
            color: [r, g, 0.0, 1.0], color_end: [r * 0.15, 0.0, 0.0, 0.0] });
    }
}

fn spawn_slash(world: &mut World, px: f32, py: f32, direction: [f32; 2], tick: u64) {
    let perp = [-direction[1], direction[0]];
    for i in 0..11u64 {
        let t    = i as f32 / 10.0;
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
        world.insert(e, Particle { velocity: [vx, vy], lifetime: max_life,
            max_lifetime: max_life, drag: 9.0,
            color: [1.0, 0.95, 0.7, 1.0], color_end: [1.0, 0.95, 0.7, 0.0] });
    }
}

// ── DemoGame ─────────────────────────────────────────────────────────────────

struct DemoGame {
    world:         World,
    player:        Option<Entity>,
    map_w:         u32,
    map_h:         u32,
    initialized:   bool,
    move_cooldown: u32,
    ui:            UiState,
    /// Persistent label for the top status bar, rendered via the bitmap font.
    status_label:  Label,
    /// Active window configuration; updated by F11 / 1-2-3 key bindings.
    window_config: WindowConfig,
    /// Camera zoom target (smooth lerp handled by the camera system).
    zoom_target:   f32,
    /// Settings panel toggle (F2).
    settings_open:    bool,
    /// Settings — "Resolution" dropdown.
    dd_resolution:    Dropdown,
    /// Settings — "Player Name" input box.
    ib_name:          InputBox,
    /// Settings — "Difficulty" toggle selector.
    ts_difficulty:    ToggleSelector,
}

impl DemoGame {
    fn new() -> Self {
        Self {
            world:         World::new(),
            player:        None,
            map_w:         0,
            map_h:         0,
            initialized:   false,
            move_cooldown: 0,
            ui:            UiState::new(),
            // font_id 0; actual font registered into engine.ui.text on first update.
            status_label:  Label::new([0.0, 0.0], 0, DEFAULT_TILE_H as f32, UI_TEXT.0),
            window_config: WindowConfig::default(),
            zoom_target:   1.0,
            settings_open:  false,
            dd_resolution:  Dropdown::new(["1280×720", "1920×1080", "2560×1440", "3840×2160"]),
            ib_name:        InputBox::new(20),
            ts_difficulty:  ToggleSelector::new(["Story", "Normal", "Hard", "Nightmare"]),
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

        for x in 0..w { solids.push((x, 3)); solids.push((x, h - 4)); }
        for y in 1..h {
            if y < 4 { continue; }
            solids.push((0, y)); solids.push((w - 1, y));
        }

        if w > 6 && h > 4 {
            let wall_y = h / 3;
            for x in 2..w - 4 { solids.push((x, wall_y)); }
        }
        if w > 4 && h > 6 {
            let wall_x = w * 2 / 3;
            for y in h / 2..h - 2 { solids.push((wall_x, y)); }
        }

        for &(x, y) in &solids {
            let wall = self.world.spawn();
            self.world.insert(wall, Position { x, y });
            self.world.insert(wall, Solid);
            self.world.insert(wall, Wall);
            self.world.insert(wall, Sprite {sprite_name: "wall".to_string() });
        }

        let torch_spots: [(u32, u32); 3] = [
            (5, 5),
            (w.saturating_sub(5), 5),
            (5, h.saturating_sub(5)),
        ];
        for &(tx, ty) in &torch_spots {
            if !solids.contains(&(tx, ty)) {
                let t = self.world.spawn();
                self.world.insert(t, Position { x: tx, y: ty });
                self.world.insert(t, Renderable { glyph: '\u{2020}', fg: Color::YELLOW, bg: Color::BLACK });
                self.world.insert(t, Torch);
            }
        }

        let gx = w / 4;
        let gy = h * 2 / 3;
        if !solids.contains(&(gx, gy)) {
            let gt = self.world.spawn();
            self.world.insert(gt, Position { x: gx, y: gy });
            self.world.insert(gt, Renderable { glyph: '\u{2248}', fg: Color::CYAN, bg: Color::BLACK });
            self.world.insert(gt, GlitchTile);
        }

        let fx = w * 3 / 4;
        let fy = h * 2 / 3;
        if !solids.contains(&(fx, fy)) {
            let ft = self.world.spawn();
            self.world.insert(ft, Position { x: fx, y: fy });
            self.world.insert(ft, Renderable { glyph: '\u{25C6}', fg: Color([1.0, 0.45, 0.0, 1.0]), bg: Color::BLACK });
            self.world.insert(ft, FireTile);
        }

        let ex = w / 2 + 3;
        let ey = h / 2;
        if !solids.contains(&(ex, ey)) {
            let en = self.world.spawn();
            self.world.insert(en, Position { x: ex, y: ey });
            self.world.insert(en, Solid);
            self.world.insert(en, Enemy);
            self.world.insert(en, Sprite {sprite_name: "small_enemy".to_string() });
        }

        let bx = w / 3;
        let by = h * 2 / 3 - 2;
        if !solids.contains(&(bx, by)) {
            let be = self.world.spawn();
            self.world.insert(be, Position { x: bx, y: by });
            self.world.insert(be, Size { w: 3, h: 3 });
            self.world.insert(be, Solid);
            self.world.insert(be, BigEnemy);
            self.world.insert(be, Sprite {sprite_name: "big_enemy".to_string() })
        }

        // Dialogue NPC — bottom-left area.
        let dx = 3u32;
        let dy = h.saturating_sub(4);
        if !solids.contains(&(dx, dy)) {
            let npc = self.world.spawn();
            self.world.insert(npc, Position { x: dx + 5, y: dy + 5 });
            self.world.insert(npc, Renderable {
                glyph: '\u{263A}',
                fg: Color([0.9, 0.8, 0.3, 1.0]),
                bg: Color::BLACK,
            });
            self.world.insert(npc, Solid);
            self.world.insert(npc, DialogueNpc);
            self.world.insert(npc, Sprite {sprite_name: "small_enemy".to_string() });
        }

        let player = self.world.spawn();
        self.world.insert(player, Position { x: w / 2, y: h / 2 });
        self.world.insert(player, Player);
        self.world.insert(player, Sprite {sprite_name: "player".to_string() });
        self.player = Some(player);
    }

    fn is_solid(&self, x: u32, y: u32) -> bool {
        self.world.query::<Solid>().any(|(e, _)| {
            self.world.get::<Position>(e).map_or(false, |p| {
                let (sw, sh) = self.world.get::<Size>(e).map_or((1, 1), |s| (s.w, s.h));
                x >= p.x && x < p.x + sw && y >= p.y && y < p.y + sh
            })
        })
    }

    // ── UI rendering sub-routines ─────────────────────────────────────────

    fn draw_hud_top(&mut self, engine: &mut Engine) {
        //let sw = engine.grid_width()  as f32 * engine.tile_width()  as f32; // screen width
        let sw = self.window_config.physical_width as f32;
        let tw = engine.tile_width()  as f32;
        let th = engine.tile_height() as f32;

        // Row 0: dark status bar — text rendered via status_label (bitmap font).
        engine.ui.ui_rect(0.0, 0.0, sw, th, PANEL_BG);

        // Row 1: HP progress bar + label
        engine.ui.ui_rect(0.0, th, sw, th, PANEL_BG);
        let hp_pct = (self.ui.player_hp / self.ui.player_max_hp).clamp(0.0, 1.0);
        engine.ui.ui_progress_bar(0.0, th, sw - tw * 3.0, th, hp_pct, HP_FILL, HP_EMPTY);
        engine.ui.ui_text(0.0, th,
            &format!("HP: {:.0} / {:.0}", self.ui.player_hp, self.ui.player_max_hp),
            Color::WHITE, Color::TRANSPARENT);
        // [i] and [l] icon buttons on the right
        let icon_x = sw - (2.0 * tw) * 3.0;
        engine.ui.ui_rect(icon_x, th, tw * 3.0, th, PANEL_BG);
        let inv_color = if self.ui.inventory_open { UI_BRIGHT } else { UI_DIM };
        let log_color = if self.ui.log_open       { UI_BRIGHT } else { UI_DIM };
        engine.ui.ui_text(icon_x,          th, "[i]", inv_color, Color::TRANSPARENT);
        engine.ui.ui_text(icon_x + (2.0 + tw) * 2.0, th, "[l]", log_color, Color::TRANSPARENT);

        // Row 2: XP progress bar + label
        engine.ui.ui_rect(0.0, th * 2.0, sw, th, PANEL_BG);
        let xp_pct = (self.ui.player_xp / self.ui.player_max_xp).clamp(0.0, 1.0);
        engine.ui.ui_progress_bar(0.0, th * 2.0, sw - tw * 3.0, th, xp_pct, XP_FILL, XP_EMPTY);
        engine.ui.ui_text(0.0, th * 2.0,
            &format!("LVL: 1   EXP: {:.0} / {:.0}", self.ui.player_xp, self.ui.player_max_xp),
            Color::WHITE, Color::TRANSPARENT);
        engine.ui.ui_rect(icon_x, th * 2.0, tw * 3.0, th, PANEL_BG);
    }

    fn draw_hud_bottom(&mut self, engine: &mut Engine) {
        let sw = engine.grid_width()  as f32 * engine.tile_width()  as f32;
        let sh = engine.grid_height() as f32 * engine.tile_height() as f32;
        let tw = engine.tile_width()  as f32;
        let th = engine.tile_height() as f32;

        // Three rows from the bottom
        let y0 = sh - th * 3.0;
        let y1 = sh - th * 2.0;
        let y2 = sh - th;

        // Row 0 (effects / target / weapon), divided into three panels
        let p1w = sw / 3.0;
        let p2w = sw / 3.0;
        let p3w = sw - p1w - p2w;

        engine.ui.ui_rect(0.0,        y0, p1w, th, PANEL_BG);
        engine.ui.ui_rect(p1w,        y0, p2w, th, Color([0.04, 0.08, 0.08, 1.0]));
        engine.ui.ui_rect(p1w + p2w,  y0, p3w, th, PANEL_BG);
        engine.ui.ui_vline(p1w,       y0, th,  PANEL_BORDER);
        engine.ui.ui_vline(p1w + p2w, y0, th,  PANEL_BORDER);

        engine.ui.ui_text(tw * 0.5,         y0, "ACTIVE EFFECTS:", UI_DIM, Color::TRANSPARENT);
        engine.ui.ui_text(tw * 10.5,        y0, "[burning]", UI_RED, Color::TRANSPARENT);
        engine.ui.ui_text(tw * 19.5,        y0, "[haste]", UI_ACCENT, Color::TRANSPARENT);
        engine.ui.ui_text(p1w + tw * 0.5,   y0, "TARGET:", UI_DIM, Color::TRANSPARENT);
        engine.ui.ui_text(p1w + tw * 8.0,   y0, "Small Enemy", UI_TEXT, Color::TRANSPARENT);
        engine.ui.ui_text(p1w + tw * 20.0,  y0, "[Hostile]", UI_RED, Color::TRANSPARENT);
        engine.ui.ui_text(p1w+p2w + tw*0.5, y0, "[f] attack  [r] parry  [w] dodge", UI_TEXT, Color::TRANSPARENT);

        // Row 1: skillbar label
        engine.ui.ui_rect(0.0, y1, sw, th, PANEL_BG);
        engine.ui.ui_hline(0.0, y1, sw, PANEL_BORDER);
        engine.ui.ui_text(0.0, y1, "ABILITIES:", UI_DIM, Color::TRANSPARENT);

        // Row 2: skill slots
        engine.ui.ui_rect(0.0, y2, sw, th, PANEL_BG);
        let skills = ["[1] Strike", "[2] Dash", "[3] Heal", "[4] Block", "[5] Roll"];
        let slot_w = sw / skills.len() as f32;
        for (i, s) in skills.iter().enumerate() {
            let sx = i as f32 * slot_w;
            engine.ui.ui_rect(sx + 1.0, y2 + 1.0, slot_w - 2.0, th - 2.0,
                Color([0.08, 0.12, 0.12, 1.0]));
            engine.ui.ui_text(sx + tw * 0.5, y2, s, UI_TEXT, Color::TRANSPARENT);
            if i + 1 < skills.len() {
                engine.ui.ui_vline(sx + slot_w, y2, th, PANEL_BORDER);
            }
        }
    }

    fn draw_log_panel(&mut self, engine: &mut Engine) {
        if !self.ui.log_open { return; }

        let sw = engine.grid_width()  as f32 * engine.tile_width()  as f32;
        let sh = engine.grid_height() as f32 * engine.tile_height() as f32;
        let tw = engine.tile_width()  as f32;
        let th = engine.tile_height() as f32;

        let panel_w   = tw * 18.0;
        let panel_x   = sw - panel_w;
        let panel_y   = th * 3.0;           // below top HUD
        let panel_h   = sh - th * 6.0;     // above bottom HUD
        let inner_x   = panel_x + tw;
        let inner_y   = panel_y + th;
        let inner_w   = panel_w - tw * 2.0;
        let inner_h   = panel_h - th * 2.0;
        let max_rows  = (inner_h / th) as usize;

        engine.ui.ui_box(panel_x, panel_y, panel_w, panel_h, BorderStyle::Single, PANEL_BORDER, LOG_BG);
        engine.ui.ui_text(panel_x + tw * 2.0, panel_y, " MESSAGE LOG ", UI_BRIGHT, LOG_BG);

        // Show most-recent messages at the bottom
        let msgs: Vec<&LogEntry> = self.ui.log_messages.iter().rev().take(max_rows).collect();
        for (row, entry) in msgs.into_iter().rev().enumerate() {
            let row_y = inner_y + row as f32 * th;
            let max_cols = (inner_w / tw) as usize;
            let truncated: String = entry.text.chars().take(max_cols).collect();
            engine.ui.ui_text(inner_x, row_y, &truncated, entry.color, Color::TRANSPARENT);
        }
    }

    fn draw_inventory_modal(&mut self, engine: &mut Engine) {
        if !self.ui.inventory_open { return; }

        let sw = engine.grid_width()  as f32 * engine.tile_width()  as f32;
        let sh = engine.grid_height() as f32 * engine.tile_height() as f32;
        let tw = engine.tile_width()  as f32;
        let th = engine.tile_height() as f32;

        let bx = tw;
        let by = th;
        let bw = sw - tw * 2.0;
        let bh = sh - th * 2.0;

        engine.ui.ui_box(bx, by, bw, bh, BorderStyle::Double, PANEL_BORDER, PANEL_BG);

        // ── Tabs ────────────────────────────────────────────────────────────
        let tabs = [(Tab::Inventory, " Inventory "),
                    (Tab::SkillTree, " Skill Tree "),
                    (Tab::Relationship, " Relationship ")];
        let mut tab_x = bx + tw * 2.0;
        for (tab, label) in &tabs {
            let is_active = self.ui.active_tab == *tab;
            let fg = if is_active { UI_BRIGHT } else { UI_DIM };
            let bg = if is_active { Color([0.10, 0.15, 0.14, 1.0]) } else { PANEL_BG };
            let w  = label.chars().count() as f32 * tw;
            engine.ui.ui_rect(tab_x, by, w, th, bg);
            engine.ui.ui_text(tab_x, by, label, fg, bg);
            tab_x += w + tw;
        }

        let inner_x = bx + tw;
        let inner_y = by + th * 2.0;
        let inner_w = bw - tw * 2.0;
        let inner_h = bh - th * 3.0;

        match self.ui.active_tab {
            Tab::Inventory => self.draw_inventory_content(engine, inner_x, inner_y, inner_w, inner_h),
            Tab::SkillTree => self.draw_skill_tree_content(engine, inner_x, inner_y, inner_w, inner_h),
            Tab::Relationship => self.draw_relationship_content(engine, inner_x, inner_y, inner_w, inner_h),
        }

        // ── Footer hint ─────────────────────────────────────────────────────
        let footer_y = by + bh - th;
        engine.ui.ui_hline(bx + tw, footer_y, bw - tw * 2.0, PANEL_BORDER);
        engine.ui.ui_text(bx + tw * 2.0, footer_y,
            " [Tab] Switch  [Esc] Close ", UI_DIM, Color::TRANSPARENT);
    }

    fn draw_inventory_content(&mut self, engine: &mut Engine,
                               x: f32, y: f32, w: f32, h: f32) {
        let tw = engine.tile_width()  as f32;
        let th = engine.tile_height() as f32;

        // Left column: equipment slots (roughly 1/3 of width)
        let col_w = w / 3.0;
        engine.ui.ui_text(x, y, "EQUIPMENT", UI_ACCENT, Color::TRANSPARENT);
        engine.ui.ui_hline(x, y + th, col_w, UI_DIM);

        let slots = [
            ("Head",      "Iron Helmet"),
            ("Body",      "Leather Armour"),
            ("Left Hand", "Rusty Blade"),
            ("Right Hand","Wooden Shield"),
            ("Feet",      "Worn Boots"),
            ("Back",      "(empty)"),
        ];
        for (i, (slot, item)) in slots.iter().enumerate() {
            let sy = y + th * (i as f32 + 2.0);
            engine.ui.ui_text(x, sy, slot, UI_DIM, Color::TRANSPARENT);
            let item_color = if *item == "(empty)" { UI_DIM } else { UI_TEXT };
            engine.ui.ui_text(x + tw * 12.0, sy, item, item_color, Color::TRANSPARENT);
        }

        // Right column: item list
        let list_x = x + col_w + tw;
        engine.ui.ui_text(list_x, y, "ITEMS", UI_ACCENT, Color::TRANSPARENT);
        engine.ui.ui_hline(list_x, y + th, w - col_w - tw, UI_DIM);

        let items = [
            ("a)", "Meds x3",           UI_TEXT),
            ("b)", "Bandage x5",        UI_TEXT),
            ("c)", "Torch x2",          Color::YELLOW),
            ("d)", "Ration x1",         UI_TEXT),
            ("e)", "Lockpick x1",       UI_ACCENT),
            ("─)", "─── Weapons ───",   UI_DIM),
            ("f)", "Rusty Dagger",      Color([0.8, 0.7, 0.5, 1.0])),
            ("g)", "Short Bow",         Color([0.7, 0.6, 0.4, 1.0])),
            ("─)", "─── Misc ───────",  UI_DIM),
            ("h)", "Old Map",           Color([0.9, 0.85, 0.6, 1.0])),
            ("i)", "Strange Amulet",    Color([0.6, 0.3, 1.0, 1.0])),
        ];
        for (i, (key, name, color)) in items.iter().enumerate() {
            let iy = y + th * (i as f32 + 2.0);
            if iy + th > y + h { break; }
            engine.ui.ui_text(list_x,          iy, key,  UI_DIM,   Color::TRANSPARENT);
            engine.ui.ui_text(list_x + tw * 3.0, iy, name, *color, Color::TRANSPARENT);
        }
    }

    fn draw_skill_tree_content(&mut self, engine: &mut Engine,
                                x: f32, y: f32, _w: f32, _h: f32) {
        let th = engine.tile_height() as f32;

        engine.ui.ui_text(x, y, "SKILL TREE", UI_ACCENT, Color::TRANSPARENT);
        engine.ui.ui_hline(x, y + th, _w, UI_DIM);

        // Simple ASCII skill tree placeholder
        let tree = [
            ("                [Combat]",               UI_BRIGHT),
            ("               /   |   \\",              UI_DIM),
            ("       [Strike] [Block] [Dash]",         UI_TEXT),
            ("          |               |",            UI_DIM),
            ("       [Parry]         [Dodge]",         UI_TEXT),
            ("",                                       UI_TEXT),
            ("                [Magic]",                UI_ACCENT),
            ("              /       \\",               UI_DIM),
            ("         [Fireball]  [Heal]",            UI_TEXT),
            ("",                                       UI_TEXT),
            ("  Points available: 3",                  UI_BRIGHT),
            ("  Press [Enter] to unlock selected.",    UI_DIM),
        ];
        for (i, (line, color)) in tree.iter().enumerate() {
            engine.ui.ui_text(x, y + th * (i as f32 + 2.0), line, *color, Color::TRANSPARENT);
        }
    }

    fn draw_relationship_content(&mut self, engine: &mut Engine,
                                  x: f32, y: f32, w: f32, h: f32) {
        let tw = engine.tile_width()  as f32;
        let th = engine.tile_height() as f32;

        engine.ui.ui_text(x, y, "FACTIONS & RELATIONSHIPS", UI_ACCENT, Color::TRANSPARENT);
        engine.ui.ui_hline(x, y + th, w, UI_DIM);

        let factions = [
            ("The Wanderers",    "+100", "They trade freely with you.",
             "Wanderers are nomads who roam the ruins. They share knowledge freely."),
            ("The Old Guard",    "- 50", "They are wary of strangers.",
             "The Old Guard protect ancient sites. They distrust outsiders."),
            ("Ruin Keepers",     "   0", "They observe you neutrally.",
             "Ruin Keepers catalogue every artifact found in these halls."),
            ("Shadow Collective","−200", "They consider you an enemy.",
             "A secretive group operating in the deep ruins. Very dangerous."),
        ];
        let col2 = x + tw * 18.0;
        let col3 = x + tw * 30.0;

        for (i, (name, rep, attitude, desc)) in factions.iter().enumerate() {
            let fy = y + th * (i as f32 * 3.0 + 2.0);
            if fy + th * 3.0 > y + h { break; }

            let rep_color = if rep.contains('-') || rep.contains('−') { UI_RED }
                            else if rep.contains('+') { HP_FILL }
                            else { UI_DIM };

            engine.ui.ui_text(x,    fy, name,     UI_TEXT,    Color::TRANSPARENT);
            engine.ui.ui_text(col2, fy, "Rep:",   UI_DIM,     Color::TRANSPARENT);
            engine.ui.ui_text(col2 + tw * 5.0, fy, rep, rep_color, Color::TRANSPARENT);
            engine.ui.ui_text(col3, fy, attitude, UI_DIM,     Color::TRANSPARENT);
            engine.ui.ui_text(x,    fy + th, desc, UI_DIM,  Color::TRANSPARENT);
            engine.ui.ui_hline(x,   fy + th * 2.0, w, Color([0.10, 0.15, 0.14, 1.0]));
        }
    }

    fn draw_dialogue_modal(&mut self, engine: &mut Engine) {
        let dialogue = match &self.ui.dialogue {
            Some(d) => d,
            None    => return,
        };

        let sw = engine.grid_width()  as f32 * engine.tile_width()  as f32;
        let sh = engine.grid_height() as f32 * engine.tile_height() as f32;
        let tw = engine.tile_width()  as f32;
        let th = engine.tile_height() as f32;

        let bw = tw * 36.0;
        let bh = th * 18.0;
        let bx = (sw - bw) * 0.5;
        let by = (sh - bh) * 0.5;

        engine.ui.ui_box(bx, by, bw, bh, BorderStyle::Double, PANEL_BORDER, PANEL_BG);

        // NPC name
        let name = dialogue.npc_name.clone();
        let name_x = bx + (bw - name.chars().count() as f32 * tw) * 0.5;
        engine.ui.ui_text(name_x, by + th * 1.5, &name, UI_BRIGHT, Color::TRANSPARENT);
        engine.ui.ui_hline(bx + tw, by + th * 2.5, bw - tw * 2.0, PANEL_BORDER);

        // Body text
        engine.ui.ui_text_wrapped(
            bx + tw * 2.0, by + th * 3.0,
            bw - tw * 4.0, th * 4.0,
            &dialogue.body,
            UI_TEXT, Color::TRANSPARENT,
        );

        // Divider before options
        engine.ui.ui_hline(bx + tw, by + th * 7.0, bw - tw * 2.0, UI_DIM);

        // Dialogue options
        let selected = dialogue.selected;
        for (i, opt) in dialogue.options.iter().enumerate() {
            let oy = by + th * (8.0 + i as f32 * 1.5);
            if oy + th > by + bh - th * 2.0 { break; }
            let is_sel = i == selected;
            let row_bg = if is_sel { Color([0.10, 0.18, 0.16, 1.0]) } else { Color::TRANSPARENT };
            let row_fg = if is_sel { UI_BRIGHT } else { UI_TEXT };
            let prefix = if is_sel { "> " } else { "  " };
            engine.ui.ui_text(bx + tw, oy,
                &format!("{prefix}[{}] {opt}", i + 1),
                row_fg, row_bg);
        }

        // Footer
        let footer_y = by + bh - th;
        engine.ui.ui_hline(bx + tw, footer_y, bw - tw * 2.0, PANEL_BORDER);
        engine.ui.ui_text(bx + tw * 2.0, footer_y,
            " [↑/↓] Navigate  [Enter] Select  [Esc] Cancel ",
            UI_DIM, Color::TRANSPARENT);
    }

    fn draw_settings_panel(&mut self, engine: &mut Engine) {
        if !self.settings_open { return; }

        let sw = engine.grid_width()  as f32 * engine.tile_width()  as f32;
        let sh = engine.grid_height() as f32 * engine.tile_height() as f32;
        let tw = engine.tile_width()  as f32;
        let th = engine.tile_height() as f32;

        // Centre a fixed-size panel.
        let panel_w = tw * 32.0;
        let panel_h = th * 16.0;
        let bx = (sw - panel_w) * 0.5;
        let by = (sh - panel_h) * 0.5;

        engine.ui.ui_box(bx, by, panel_w, panel_h, BorderStyle::Double, PANEL_BORDER, PANEL_BG);

        // Title
        let title = " SETTINGS ";
        let title_x = bx + (panel_w - title.chars().count() as f32 * tw) * 0.5;
        engine.ui.ui_text(title_x, by + th, title, UI_BRIGHT, PANEL_BG);
        engine.ui.ui_hline(bx + tw, by + th * 2.0, panel_w - tw * 2.0, PANEL_BORDER);

        let col_x  = bx + tw * 2.0;
        let ctrl_x = bx + tw * 14.0;
        let ctrl_w = panel_w - tw * 16.0;

        // Row 0 — Resolution dropdown
        engine.ui.ui_text(col_x, by + th * 3.5, "Resolution", UI_TEXT, PANEL_BG);
        self.dd_resolution.draw(engine, ctrl_x, by + th * 3.5, ctrl_w);

        // Row 1 — Player name input
        engine.ui.ui_text(col_x, by + th * 6.0, "Player Name", UI_TEXT, PANEL_BG);
        self.ib_name.draw(engine, ctrl_x, by + th * 6.0, ctrl_w);

        // Row 2 — Difficulty toggle selector
        engine.ui.ui_text(col_x, by + th * 8.5, "Difficulty", UI_TEXT, PANEL_BG);
        self.ts_difficulty.draw(engine, ctrl_x, by + th * 8.5, ctrl_w);

        // Footer hint
        let footer_y = by + panel_h - th;
        engine.ui.ui_hline(bx + tw, footer_y, panel_w - tw * 2.0, PANEL_BORDER);
        engine.ui.ui_text(bx + tw * 2.0, footer_y,
            " [F2/Esc] Close ", UI_DIM, PANEL_BG);
    }
}

// ── Game impl ─────────────────────────────────────────────────────────────────

impl Game for DemoGame {
    fn update(&mut self, engine: &mut Engine) {
        // ── Window mode / resolution keys ─────────────────────────────────────

        // F2: toggle Settings panel.
        if engine.is_key_pressed(KeyCode::F2) {
            self.settings_open = !self.settings_open;
        }
        // Escape closes settings before anything else.
        if self.settings_open && engine.is_key_pressed(KeyCode::Escape) {
            self.settings_open = false;
            return;
        }
        // While settings panel is open, block all game input.
        if self.settings_open { return; }

        // F11: toggle Windowed ↔ Borderless.
        if engine.is_key_pressed(KeyCode::F11) {
            self.window_config.mode = match self.window_config.mode {
                WindowMode::Windowed   => WindowMode::Borderless,
                WindowMode::Borderless |
                WindowMode::Fullscreen => WindowMode::Windowed,
            };
            apply_window_settings(&engine.ui.renderer.window, &self.window_config);
            println!("[window] mode → {:?}", self.window_config.mode);
        }

        // ─────────────────────────────────────────────────────────────────────

        let gw = engine.grid_width();
        let gh = engine.grid_height();

        if !self.initialized || gw != self.map_w || gh != self.map_h {
            self.build_map(gw, gh);

            // Keep window_config in sync with the actual pixel size so that F11
            // can restore the correct dimensions when returning to windowed mode.
            let actual_w = gw * engine.tile_width();
            let actual_h = gh * engine.tile_height();
            self.window_config.physical_width  = actual_w;
            self.window_config.physical_height = actual_h;
            self.window_config.logical_width   = actual_w;
            self.window_config.logical_height  = actual_h;

            // Register the bitmap font once (on first init).
            if engine.ui.text.fonts.is_empty() {
                if let Ok(font) = Font::from_atlas_json(
                    DEFAULT_FONT_GLYPHS,
                    DEFAULT_TILE_W * 16,   // atlas is 16 glyphs wide
                    DEFAULT_TILE_H * 16,   // atlas is 16 glyphs tall
                ) {
                    engine.ui.text.add_font(font);
                }
            }

            // Seed the label text (static placeholder — update via set_text when data changes).
            self.status_label.set_text(
                "[@] T:25  Hungry Tumescent  100/100# 50$  QN:10  MS:5  baroque ruins, surface",
            );

            self.initialized = true;
        }

        let tw = engine.tile_width();
        let th = engine.tile_height();

        update_particles(&mut self.world, engine.dt());
        spawn_smoke(&mut self.world, engine.tick(), tw, th);
        spawn_fire_ambient(&mut self.world, engine.tick(), tw, th);
        spawn_glitch_ambient(&mut self.world, engine.tick(), tw, th);

        // ── Camera: track player ──────────────────────────────────────────
        if let Some(player) = self.player {
            if let Some(pos) = self.world.get::<Position>(player) {
                let cx = (pos.x as f32 + 0.5) * tw as f32;
                let cy = (pos.y as f32 + 0.5) * th as f32;
                engine.set_camera_pos(cx, cy);
            }
        }

        // ── UI toggle keys ────────────────────────────────────────────────
        if engine.is_key_pressed(KeyCode::KeyI) {
            self.ui.inventory_open = !self.ui.inventory_open;
        }
        if engine.is_key_pressed(KeyCode::KeyL) {
            self.ui.log_open = !self.ui.log_open;
        }
        if engine.is_key_pressed(KeyCode::Tab) && self.ui.inventory_open {
            self.ui.active_tab = match self.ui.active_tab {
                Tab::Inventory     => Tab::SkillTree,
                Tab::SkillTree     => Tab::Relationship,
                Tab::Relationship  => Tab::Inventory,
            };
        }
        if engine.is_key_pressed(KeyCode::Escape) {
            if self.ui.dialogue.is_some() {
                self.ui.dialogue = None;
                return;
            }
            if self.ui.inventory_open {
                self.ui.inventory_open = false;
                return;
            }
        }

        // ── Camera zoom (+/-) with smooth lerp ───────────────────────────
        // "=" key (same physical key as "+") or NumpadAdd → zoom in.
        // "-" key or NumpadSubtract → zoom out.
        if engine.is_key_pressed(KeyCode::Equal) || engine.is_key_pressed(KeyCode::NumpadAdd) {
            self.zoom_target = (self.zoom_target * 1.25).min(4.0);
            engine.set_camera_zoom(self.zoom_target);
        }
        if engine.is_key_pressed(KeyCode::Minus) || engine.is_key_pressed(KeyCode::NumpadSubtract) {
            self.zoom_target = (self.zoom_target / 1.25).max(0.25);
            engine.set_camera_zoom(self.zoom_target);
        }

        // ── Inventory clicks on tabs ──────────────────────────────────────
        if self.ui.inventory_open {
            let sw = engine.grid_width()  as f32 * engine.tile_width()  as f32;
            let sh = engine.grid_height() as f32 * engine.tile_height() as f32;
            let tw_f = engine.tile_width()  as f32;
            let th_f = engine.tile_height() as f32;
            let by = th_f;
            let tabs = [(Tab::Inventory, " Inventory "),
                        (Tab::SkillTree, " Skill Tree "),
                        (Tab::Relationship, " Relationship ")];
            let mut tab_x = tw_f * 2.0 + tw_f;
            for (tab, label) in &tabs {
                let tw_label = label.chars().count() as f32 * tw_f;
                if engine.ui.was_clicked(tab_x, by, tw_label, th_f) {
                    self.ui.active_tab = *tab;
                }
                tab_x += tw_label + tw_f;
            }
            let _ = sw; let _ = sh; // suppress warnings
        }

        // ── Dialogue navigation ────────────────────────────────────────────
        if let Some(ref mut dlg) = self.ui.dialogue {
            let n = dlg.options.len();
            if engine.is_key_pressed(KeyCode::ArrowUp)   && dlg.selected > 0     { dlg.selected -= 1; }
            if engine.is_key_pressed(KeyCode::ArrowDown)  && dlg.selected + 1 < n { dlg.selected += 1; }
            for digit in 1..=9usize {
                let key = match digit {
                    1 => KeyCode::Digit1, 2 => KeyCode::Digit2, 3 => KeyCode::Digit3,
                    4 => KeyCode::Digit4, 5 => KeyCode::Digit5, 6 => KeyCode::Digit6,
                    7 => KeyCode::Digit7, 8 => KeyCode::Digit8, 9 => KeyCode::Digit9,
                    _ => continue,
                };
                if engine.is_key_pressed(key) && digit <= n { dlg.selected = digit - 1; }
            }
            if engine.is_key_pressed(KeyCode::Enter) {
                let sel = dlg.selected;
                let chosen = dlg.options.get(sel).cloned().unwrap_or_default();
                self.ui.log(&format!(">> {chosen}"), UI_BRIGHT);
                if chosen.contains("[End]") || chosen.to_lowercase().contains("farewell") {
                    self.ui.dialogue = None;
                } else {
                    self.ui.log("The wanderer nods knowingly.", UI_TEXT);
                }
            }
            return; // Dialogue captures all input.
        }

        // Block movement while inventory is open.
        if self.ui.inventory_open { return; }

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
            if engine.is_key_held(KeyCode::ArrowUp)    || engine.is_key_held(KeyCode::Numpad8) { dy = -1; }
            else if engine.is_key_held(KeyCode::ArrowDown)  || engine.is_key_held(KeyCode::Numpad2) { dy = 1; }
            else if engine.is_key_held(KeyCode::ArrowLeft)  || engine.is_key_held(KeyCode::Numpad4) { dx = -1; }
            else if engine.is_key_held(KeyCode::ArrowRight) || engine.is_key_held(KeyCode::Numpad6) { dx = 1; }
            else if engine.is_key_held(KeyCode::Numpad7) { dx = -1; dy = -1; }
            else if engine.is_key_held(KeyCode::Numpad9) { dx = 1;  dy = -1; }
            else if engine.is_key_held(KeyCode::Numpad1) { dx = -1; dy = 1; }
            else if engine.is_key_held(KeyCode::Numpad3) { dx = 1;  dy = 1; }
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
                            engine.camera_shake(10.0);
                            self.ui.log("Reality flickers around you.", UI_ACCENT);
                        }

                        let on_fire = self.world.query::<FireTile>().any(|(e, _)| {
                            self.world.get::<Position>(e).map_or(false, |p| p.x == new_x && p.y == new_y)
                        });
                        if on_fire {
                            engine.play_animation(player, AnimationType::Bash {
                                direction: [0.0, -1.0], magnitude: 4.0,
                            });
                            spawn_fire_burst(&mut self.world, npx, npy, engine.tick());
                            self.ui.log("The floor ignites beneath your feet!", UI_RED);
                        }
                    } else {
                        // Check for dialogue NPC
                        let dialogue_ent = self.world.query::<DialogueNpc>().find_map(|(e, _)| {
                            if self.world.get::<Position>(e).map_or(false, |p| p.x == new_x && p.y == new_y) {
                                Some(e)
                            } else { None }
                        });
                        if dialogue_ent.is_some() {
                            self.ui.dialogue = Some(DialogueState {
                                npc_name: "Mysterious Wanderer".to_string(),
                                body: "Greetings, traveler. I have wandered these ruins for an age. Many secrets are buried here — and many dangers yet to be found.".to_string(),
                                options: vec![
                                    "What secrets do you speak of?".to_string(),
                                    "How long have you been here?".to_string(),
                                    "Do you have anything to trade?".to_string(),
                                    "Tell me about the dangers ahead.".to_string(),
                                    "Farewell. [End]".to_string(),
                                ],
                                selected: 0,
                            });
                            self.ui.log("You approach the Mysterious Wanderer.", UI_TEXT);
                            return;
                        }

                        // Check for enemy
                        let enemy_ent: Option<Entity> = self.world
                            .query::<Enemy>()
                            .find_map(|(e, _)| {
                                if self.world.get::<Position>(e).map_or(false, |p| p.x == new_x && p.y == new_y) {
                                    Some(e)
                                } else { None }
                            })
                            .or_else(|| {
                                self.world.query::<BigEnemy>().find_map(|(e, _)| {
                                    if self.world.get::<Position>(e).map_or(false, |p| {
                                        let (w, h) = self.world.get::<Size>(e).map_or((1, 1), |s| (s.w, s.h));
                                        new_x >= p.x && new_x < p.x + w && new_y >= p.y && new_y < p.y + h
                                    }) {
                                        Some(e)
                                    } else { None }
                                })
                            });

                        if let Some(enemy) = enemy_ent {
                            let is_big = self.world.get::<BigEnemy>(enemy).is_some();
                            engine.play_animation(player, AnimationType::Bash {
                                direction: [dx as f32, dy as f32], magnitude: 4.0,
                            });
                            engine.play_animation(enemy, AnimationType::Shiver { magnitude: 2.5 });
                            spawn_slash(&mut self.world, px, py, [dx as f32, dy as f32], engine.tick());
                            if is_big {
                                self.ui.log("You struggle against the massive creature!", UI_RED);
                            } else {
                                self.ui.log("You strike the enemy!", UI_TEXT);
                            }
                        } else {
                            spawn_blood_burst(&mut self.world, px, py, engine.tick());
                            self.ui.log("You bash against the wall.", UI_DIM);
                        }
                    }
                }
            }
        }
    }

    fn render(&mut self, engine: &mut Engine) {
        engine.clear();

        // ── Char-atlas entities ──────────────────────────────────────────────
        for (entity, renderable) in self.world.query::<Renderable>() {
            if let Some(pos) = self.world.get::<Position>(entity) {
                engine.set_background(pos.x, pos.y, renderable.bg);
                engine.set_foreground_entity(pos.x, pos.y, entity, renderable.glyph, renderable.fg);
            }
        }

        // ── Sprite-atlas entities ────────────────────────────────────────────
        let walls: Vec<(u32, u32)> = self.world
            .query::<Wall>()
            .filter_map(|(e, _)| self.world.get::<Position>(e).map(|p| (p.x, p.y)))
            .collect();
        for (wx, wy) in walls {
            engine.draw_sprite(wx, wy, "wall", 1, Color::WHITE);
        }

        let small_enemies: Vec<(Entity, u32, u32)> = self.world
            .query::<Enemy>()
            .filter_map(|(e, _)| self.world.get::<Position>(e).map(|p| (e, p.x, p.y)))
            .collect();
        for (entity, ex, ey) in small_enemies {
            let sprite = self.world.get::<Sprite>(entity);
            if let Some(sprite) = sprite {
                engine.draw_sprite_entity(ex, ey, sprite.sprite_name.as_str(), entity, Color::WHITE);
            }
        }

        let big_enemies: Vec<(Entity, u32, u32)> = self.world
            .query::<BigEnemy>()
            .filter_map(|(e, _)| self.world.get::<Position>(e).map(|p| (e, p.x, p.y)))
            .collect();
        for (entity, bx, by) in big_enemies {
            engine.draw_sprite_entity(bx, by, "big_enemy", entity, Color::WHITE);
        }

        if let Some(player) = self.player {
            if let Some(pos) = self.world.get::<Position>(player) {
                engine.draw_sprite_entity(pos.x, pos.y, "player", player, Color::WHITE);
            }
        }

        render_particles(&self.world, engine);

        // ── Text layer (bitmap font labels) ─────────────────────────────────
        // Clear accumulated geometry from the previous frame, then draw all
        // persistent labels.  The resulting buffers in engine.ui.text are
        // uploaded to the GPU and rendered via the text pipeline.
        engine.ui.text.clear();
        self.status_label.draw(&mut engine.ui.text);

        // ── UI overlay (Layer 2) ─────────────────────────────────────────────
        self.draw_hud_top(engine);
        self.draw_hud_bottom(engine);
        self.draw_log_panel(engine);
        self.draw_inventory_modal(engine);
        self.draw_dialogue_modal(engine);
        self.draw_settings_panel(engine);
    }
}

fn main() {
    Engine::builder()
        .with_title("jengine demo")
        .with_size(1280, 720)
        .with_tileset(DEFAULT_TILESET, 16, 24)
        .with_sprite_folder("resources/sprites")
        .retro_scan_lines()
        .run(DemoGame::new());
}
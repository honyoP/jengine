use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
pub use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use crate::camera::Camera;
use crate::ui::UI;
use crate::ecs::Entity;
use crate::renderer::Renderer;
use crate::renderer::particle_pipeline::ParticleVertex;
use crate::renderer::pipeline::TileVertex;

// ── Color ──────────────────────────────────────────────────────────────────

#[derive(Copy, Clone, Debug)]
pub struct Color(pub [f32; 4]);

impl Color {
    pub const WHITE: Self = Self([1.0, 1.0, 1.0, 1.0]);
    pub const BLACK: Self = Self([0.0, 0.0, 0.0, 1.0]);
    pub const GRAY: Self = Self([0.6, 0.6, 0.6, 1.0]);
    pub const DARK_GRAY: Self = Self([0.2, 0.2, 0.2, 1.0]);
    pub const RED: Self = Self([1.0, 0.0, 0.0, 1.0]);
    pub const GREEN: Self = Self([0.0, 1.0, 0.0, 1.0]);
    pub const BLUE: Self = Self([0.0, 0.0, 1.0, 1.0]);
    pub const YELLOW: Self = Self([1.0, 1.0, 0.0, 1.0]);
    pub const CYAN: Self = Self([0.0, 1.0, 1.0, 1.0]);
    pub const MAGENTA: Self = Self([1.0, 0.0, 1.0, 1.0]);
    pub const TRANSPARENT: Self = Self([0.0, 0.0, 0.0, 0.0]);
    pub const ORANGE: Self = Self([1.0, 0.55, 0.0, 1.0]);
    pub const DARK_GREEN: Self = Self([0.0, 0.35, 0.05, 1.0]);
    pub const DARK_BLUE: Self = Self([0.0, 0.1, 0.4, 1.0]);
    pub const DARK_RED: Self = Self([0.45, 0.0, 0.0, 1.0]);
}

// ── Game trait ──────────────────────────────────────────────────────────────

pub trait Game {
    fn update(&mut self, engine: &mut jEngine);
    fn render(&mut self, engine: &mut jEngine);
}

// ── Animation system ────────────────────────────────────────────────────────

/// Visual animation types for "juice" effects.  These apply sub-pixel offsets
/// to a tile's rendered position without touching the logical ECS `Position`.
#[derive(Clone, Debug)]
pub enum AnimationType {
    /// Sinusoidal lunge: entity visually snaps `magnitude` pixels in
    /// `direction` and returns — follows `sin(t * π)` over `duration`.
    Bash {
        /// Normalised direction vector (e.g. `[1.0, 0.0]` for right).
        direction: [f32; 2],
        /// Peak displacement in pixels.
        magnitude: f32,
    },
    /// Rapid high-frequency jitter with a smooth bell-shaped envelope.
    Shiver {
        /// Peak jitter radius in pixels.
        magnitude: f32,
    },
}

impl AnimationType {
    /// Default playback duration in seconds for each animation type.
    pub fn duration(&self) -> f32 {
        match self {
            AnimationType::Bash { .. } => 0.18,
            AnimationType::Shiver { .. } => 0.45,
        }
    }
}

/// Compute the pixel offset `[dx, dy]` for an animation at a given elapsed
/// time.  This is a pure function so it can be tested independently of the
/// full engine.
pub fn compute_offset(anim_type: &AnimationType, elapsed: f32, duration: f32) -> [f32; 2] {
    use std::f32::consts::{PI, TAU};
    let progress = (elapsed / duration).clamp(0.0, 1.0);
    match anim_type {
        AnimationType::Bash { direction, magnitude } => {
            let t = (progress * PI).sin();
            [direction[0] * magnitude * t, direction[1] * magnitude * t]
        }
        AnimationType::Shiver { magnitude } => {
            let envelope = (progress * PI).sin();
            let jitter_x = (elapsed * 42.0 * TAU).sin();
            let jitter_y = (elapsed * 37.0 * TAU + 1.3).sin();
            [magnitude * envelope * jitter_x, magnitude * envelope * jitter_y]
        }
    }
}

struct ActiveAnimation {
    entity_id: u32,
    anim_type: AnimationType,
    elapsed: f32,
    duration: f32,
}

// ── Grid cells ───────────────────────────────────────────────────────────────

/// Sentinel: tile is not linked to any ECS entity (no animation).
const NO_ENTITY: u32 = u32::MAX;
/// Sentinel: foreground cell is empty (no glyph to render).
const NO_GLYPH: u32 = u32::MAX;

/// Layer 0 — static background fill (solid color quad, no glyph, no animation).
#[derive(Copy, Clone)]
struct BgCell {
    color: Color,
}

impl Default for BgCell {
    fn default() -> Self {
        Self { color: Color::BLACK }
    }
}

/// Layer 1 — foreground character glyph (char atlas, optional entity animation).
#[derive(Copy, Clone)]
struct FgCell {
    /// Atlas glyph index, or `NO_GLYPH` if this cell is empty.
    index: u32,
    fg: Color,
    /// ECS entity ID owning this cell, or `NO_ENTITY`.
    entity_id: u32,
}

impl Default for FgCell {
    fn default() -> Self {
        Self { index: NO_GLYPH, fg: Color::WHITE, entity_id: NO_ENTITY }
    }
}

// ── Sprite draw command ──────────────────────────────────────────────────────

struct SpriteCommand {
    x: u32,
    y: u32,
    sprite_name: String,
    /// 0 = drawn below layer-1 sprites; 1 = drawn above layer-0 sprites.
    layer: u8,
    tint: Color,
    /// Linked ECS entity for animation, or `NO_ENTITY`.
    entity_id: u32,
}

// ── Engine ──────────────────────────────────────────────────────────────────

// Why non_camel_case? Just to style on the plebeians
#[allow(non_camel_case_types)]
pub struct jEngine {
    /// UI subsystem — holds the renderer, tile dimensions, UI vertices, and
    /// mouse state.  Game code draws UI via `engine.ui.ui_*()`.
    pub ui: UI,
    grid_w: u32,
    grid_h: u32,
    /// Layer 0: static background color fills (char atlas path).
    bg_grid: Vec<BgCell>,
    /// Layer 1: character glyphs with optional animation (char atlas path).
    fg_grid: Vec<FgCell>,
    /// Queued sprite draw calls (sprite atlas path); cleared before each render.
    sprite_commands: Vec<SpriteCommand>,
    particle_vertices: Vec<ParticleVertex>,
    active_animations: Vec<ActiveAnimation>,
    /// 2D camera — tracks position, zoom, and shake.
    pub(crate) camera: Camera,
    dt: f32,
    tick: u64,
    keys_held: HashSet<KeyCode>,
    keys_pressed: HashSet<KeyCode>,
    keys_released: HashSet<KeyCode>,
    /// Printable characters typed this frame (populated from `KeyEvent.text`).
    /// Cleared at the start of each frame alongside `keys_pressed`.
    /// Widgets drain this during `draw()` to implement text input.
    pub chars_typed: Vec<char>,
    /// Set to `true` by `request_quit()`; the event loop exits after the current tick.
    pub(crate) quit_requested: bool,
}

impl jEngine {
    pub fn builder() -> EngineBuilder {
        EngineBuilder::default()
    }

    fn from_builder(renderer: Renderer, tile_w: u32, tile_h: u32) -> Self {
        let size = renderer.window.inner_size();
        let grid_w = size.width / tile_w;
        let grid_h = size.height / tile_h;
        let grid_size = (grid_w * grid_h) as usize;

        // Default camera: centred on the screen so the world view matches the
        // old fixed projection (i.e. shows [0..w] × [0..h] at zoom = 1).
        let camera = Camera::new(
            size.width as f32 / 2.0,
            size.height as f32 / 2.0,
        );

        let ui = UI::new(renderer, tile_w, tile_h);
        Self {
            ui,
            grid_w,
            grid_h,
            bg_grid: vec![BgCell::default(); grid_size],
            fg_grid: vec![FgCell::default(); grid_size],
            sprite_commands: Vec::new(),
            particle_vertices: Vec::new(),
            active_animations: Vec::new(),
            camera,
            dt: 0.0,
            tick: 0,
            keys_held: HashSet::new(),
            keys_pressed: HashSet::new(),
            keys_released: HashSet::new(),
            chars_typed: Vec::new(),
            quit_requested: false,
        }
    }

    // ── Accessors ──────────────────────────────────────────────────────────

    pub fn dt(&self) -> f32 { self.dt }
    pub fn tick(&self) -> u64 { self.tick }
    pub fn grid_width(&self) -> u32 { self.grid_w }
    pub fn grid_height(&self) -> u32 { self.grid_h }
    pub fn tile_width(&self) -> u32 { self.ui.tile_w }
    pub fn tile_height(&self) -> u32 { self.ui.tile_h }

    pub fn is_key_held(&self, key: KeyCode) -> bool { self.keys_held.contains(&key) }
    pub fn is_key_pressed(&self, key: KeyCode) -> bool { self.keys_pressed.contains(&key) }
    pub fn is_key_released(&self, key: KeyCode) -> bool { self.keys_released.contains(&key) }

    // ── Camera API ─────────────────────────────────────────────────────────

    /// Move the camera so that world-pixel coordinate `(x, y)` is centred on screen.
    pub fn set_camera_pos(&mut self, x: f32, y: f32) {
        self.camera.position = glam::Vec2::new(x, y);
    }

    pub fn move_camera_pos(&mut self, x: f32, y: f32) {
        self.camera.target_position = glam::Vec2::new(x, y);
    }

    /// Set the zoom target.  The camera smoothly lerps toward this value each frame.
    /// Values > 1.0 zoom in; values < 1.0 zoom out.  Clamped to a minimum of 0.05.
    pub fn set_camera_zoom(&mut self, zoom: f32) {
        self.camera.target_zoom = zoom.max(0.05);
    }

    /// Return the current zoom level (instantaneous, after lerp).
    pub fn camera_zoom(&self) -> f32 { self.camera.zoom }

    /// Return the zoom target (what `set_camera_zoom` last set).
    pub fn camera_target_zoom(&self) -> f32 { self.camera.target_zoom }

    /// Trigger a camera shake.  `intensity` is peak displacement in pixels.
    /// The shake lasts 0.5 s and decays linearly.
    pub fn camera_shake(&mut self, intensity: f32) {
        self.camera.shake(intensity);
    }

    /// Signal that the application should exit.  The event loop will call
    /// `exit()` after the current update tick completes.
    pub fn request_quit(&mut self) {
        self.quit_requested = true;
    }

    // ── Grid drawing (character / char-atlas path) ─────────────────────────

    /// Clear both character layers to their defaults (black bg, no fg glyphs).
    pub fn clear(&mut self) {
        self.bg_grid.fill(BgCell::default());
        self.fg_grid.fill(FgCell::default());
    }

    /// Set the background (Layer 0) at `(x, y)` to a solid color.
    pub fn set_background(&mut self, x: u32, y: u32, color: Color) {
        if x < self.grid_w && y < self.grid_h {
            self.bg_grid[(y * self.grid_w + x) as usize].color = color;
        }
    }

    /// Place a foreground character glyph (Layer 1) at `(x, y)` with no entity linkage.
    pub fn set_foreground(&mut self, x: u32, y: u32, ch: char, fg: Color) {
        if x < self.grid_w && y < self.grid_h {
            self.fg_grid[(y * self.grid_w + x) as usize] = FgCell {
                index: ch as u32,
                fg,
                entity_id: NO_ENTITY,
            };
        }
    }

    /// Place a foreground character glyph (Layer 1) linked to an ECS entity
    /// so that animation offsets apply to it.
    pub fn set_foreground_entity(&mut self, x: u32, y: u32, entity: Entity, ch: char, fg: Color) {
        if x < self.grid_w && y < self.grid_h {
            self.fg_grid[(y * self.grid_w + x) as usize] = FgCell {
                index: ch as u32,
                fg,
                entity_id: entity.id(),
            };
        }
    }

    // ── Sprite drawing (sprite-atlas path) ────────────────────────────────

    /// Queue a sprite from the loaded sprite folder at grid position `(x, y)`.
    ///
    /// `layer`: `0` = drawn before layer-1 sprites (background objects),
    ///          `1` = drawn after layer-0 sprites (foreground entities).
    pub fn draw_sprite(&mut self, x: u32, y: u32, name: &str, layer: u8, tint: Color) {
        self.sprite_commands.push(SpriteCommand {
            x,
            y,
            sprite_name: name.to_string(),
            layer,
            tint,
            entity_id: NO_ENTITY,
        });
    }

    /// Queue a sprite linked to an ECS entity (always Layer 1).
    pub fn draw_sprite_entity(&mut self, x: u32, y: u32, name: &str, entity: Entity, tint: Color) {
        self.sprite_commands.push(SpriteCommand {
            x,
            y,
            sprite_name: name.to_string(),
            layer: 1,
            tint,
            entity_id: entity.id(),
        });
    }

    // ── Particle drawing ───────────────────────────────────────────────────

    /// Queue a sub-pixel particle quad (`size × size` pixels) at pixel position `(x, y)`.
    pub fn draw_particle(&mut self, x: f32, y: f32, color: Color, size: f32) {
        let h = size * 0.5;
        let c = color.0;
        let tl = ParticleVertex { position: [x - h, y - h], color: c };
        let tr = ParticleVertex { position: [x + h, y - h], color: c };
        let bl = ParticleVertex { position: [x - h, y + h], color: c };
        let br = ParticleVertex { position: [x + h, y + h], color: c };
        self.particle_vertices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
    }

    // ── Animation API ──────────────────────────────────────────────────────

    /// Start (or restart) an animation on an ECS entity.  Fire-and-forget:
    /// the engine removes it automatically once its duration elapses.
    pub fn play_animation(&mut self, entity: Entity, anim_type: AnimationType) {
        let duration = anim_type.duration();
        self.active_animations.retain(|a| a.entity_id != entity.id());
        self.active_animations.push(ActiveAnimation {
            entity_id: entity.id(),
            anim_type,
            elapsed: 0.0,
            duration,
        });
    }

    /// Advance all active animations and camera state by `dt` seconds.
    pub(crate) fn tick_animations(&mut self, dt: f32) {
        for a in &mut self.active_animations {
            a.elapsed += dt;
        }
        self.active_animations.retain(|a| a.elapsed < a.duration);
        self.camera.tick(dt);
    }

    // ── Internal rendering helpers ─────────────────────────────────────────

    /// Upload the current camera view-projection matrix to the GPU.
    /// Must be called once per frame before `renderer.render()`.
    pub(crate) fn sync_camera(&mut self) {
        let size = self.ui.renderer.window.inner_size();
        let uniform = self.camera.build_view_proj(
            size.width as f32,
            size.height as f32,
        );
        self.ui.renderer.update_camera(&uniform);
    }

    /// Build vertex data for the current frame.
    fn build_vertices(&self) -> (Vec<TileVertex>, Vec<TileVertex>) {
        let offsets: HashMap<u32, [f32; 2]> = self
            .active_animations
            .iter()
            .map(|a| (a.entity_id, compute_offset(&a.anim_type, a.elapsed, a.duration)))
            .collect();

        let tile_w = self.ui.tile_w;
        let tile_h = self.ui.tile_h;
        let cells = (self.grid_w * self.grid_h) as usize;
        let mut char_verts = Vec::with_capacity(cells * 12);

        // ── bg_grid → Layer 0 solid fills (layer_id = 0.0) ───────────────
        for y in 0..self.grid_h {
            for x in 0..self.grid_w {
                let cell = &self.bg_grid[(y * self.grid_w + x) as usize];
                let px = (x * tile_w) as f32;
                let py = (y * tile_h) as f32;
                let pw = tile_w as f32;
                let ph = tile_h as f32;
                let dummy_uv = [0.0f32, 0.0];

                let tl = TileVertex { position: [px,      py     ], uv: dummy_uv, fg_color: [0.0; 4], bg_color: cell.color.0, v_offset: [0.0, 0.0], layer_id: 0.0 };
                let tr = TileVertex { position: [px + pw, py     ], uv: dummy_uv, fg_color: [0.0; 4], bg_color: cell.color.0, v_offset: [0.0, 0.0], layer_id: 0.0 };
                let bl = TileVertex { position: [px,      py + ph], uv: dummy_uv, fg_color: [0.0; 4], bg_color: cell.color.0, v_offset: [0.0, 0.0], layer_id: 0.0 };
                let br = TileVertex { position: [px + pw, py + ph], uv: dummy_uv, fg_color: [0.0; 4], bg_color: cell.color.0, v_offset: [0.0, 0.0], layer_id: 0.0 };
                char_verts.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
            }
        }

        // ── fg_grid → char glyphs (layer_id = 1.0, animated) ─────────────
        for y in 0..self.grid_h {
            for x in 0..self.grid_w {
                let cell = &self.fg_grid[(y * self.grid_w + x) as usize];
                if cell.index == NO_GLYPH { continue; }

                let px = (x * tile_w) as f32;
                let py = (y * tile_h) as f32;
                let pw = tile_w as f32;
                let ph = tile_h as f32;

                let (uv_min, uv_max) = self.ui.renderer.atlas.uv_for_index(cell.index);
                let v_offset = offsets.get(&cell.entity_id).copied().unwrap_or([0.0, 0.0]);

                let tl = TileVertex { position: [px,      py     ], uv: uv_min,                  fg_color: cell.fg.0, bg_color: [0.0; 4], v_offset, layer_id: 1.0 };
                let tr = TileVertex { position: [px + pw, py     ], uv: [uv_max[0], uv_min[1]], fg_color: cell.fg.0, bg_color: [0.0; 4], v_offset, layer_id: 1.0 };
                let bl = TileVertex { position: [px,      py + ph], uv: [uv_min[0], uv_max[1]], fg_color: cell.fg.0, bg_color: [0.0; 4], v_offset, layer_id: 1.0 };
                let br = TileVertex { position: [px + pw, py + ph], uv: uv_max,                  fg_color: cell.fg.0, bg_color: [0.0; 4], v_offset, layer_id: 1.0 };
                char_verts.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
            }
        }

        // ── sprite_commands → sprite atlas quads ──────────────────────────
        let Some(sprite_atlas) = self.ui.renderer.sprite_atlas.as_ref() else {
            return (char_verts, Vec::new());
        };

        let mut sorted: Vec<&SpriteCommand> = self.sprite_commands.iter().collect();
        sorted.sort_by_key(|c| c.layer);

        let mut sprite_verts = Vec::with_capacity(sorted.len() * 6);

        for cmd in sorted {
            let Some(sprite) = sprite_atlas.sprites.get(&cmd.sprite_name) else {
                continue;
            };

            let px = (cmd.x * tile_w) as f32;
            let py = (cmd.y * tile_h) as f32;
            let pw = (sprite.tile_w_span * tile_w) as f32;
            let ph = (sprite.tile_h_span * tile_h) as f32;

            let (v_offset, layer_id) = if cmd.entity_id != NO_ENTITY {
                (offsets.get(&cmd.entity_id).copied().unwrap_or([0.0, 0.0]), 1.0f32)
            } else {
                ([0.0f32, 0.0], 0.5f32)
            };

            let uv_min = sprite.uv_min;
            let uv_max = sprite.uv_max;
            let fg = cmd.tint.0;

            let tl = TileVertex { position: [px,      py     ], uv: uv_min,                  fg_color: fg, bg_color: [0.0; 4], v_offset, layer_id };
            let tr = TileVertex { position: [px + pw, py     ], uv: [uv_max[0], uv_min[1]], fg_color: fg, bg_color: [0.0; 4], v_offset, layer_id };
            let bl = TileVertex { position: [px,      py + ph], uv: [uv_min[0], uv_max[1]], fg_color: fg, bg_color: [0.0; 4], v_offset, layer_id };
            let br = TileVertex { position: [px + pw, py + ph], uv: uv_max,                  fg_color: fg, bg_color: [0.0; 4], v_offset, layer_id };
            sprite_verts.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }

        (char_verts, sprite_verts)
    }

    fn handle_resize(&mut self) {
        let size = self.ui.renderer.window.inner_size();
        let new_gw = size.width / self.ui.tile_w;
        let new_gh = size.height / self.ui.tile_h;
        if new_gw != self.grid_w || new_gh != self.grid_h {
            self.grid_w = new_gw;
            self.grid_h = new_gh;
            let grid_size = (new_gw * new_gh) as usize;
            self.bg_grid = vec![BgCell::default(); grid_size];
            self.fg_grid = vec![FgCell::default(); grid_size];
        }
    }
}

// ── EngineBuilder ───────────────────────────────────────────────────────────

pub struct EngineBuilder {
    title: String,
    width: u32,
    height: u32,
    png_bytes: &'static [u8],
    tile_w: u32,
    tile_h: u32,
    target_ups: u32,
    sprite_folder: Option<String>,
    use_scanlines: bool,
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self {
            title: "jengine".into(),
            width: 800,
            height: 600,
            png_bytes: &[],
            tile_w: 16,
            tile_h: 16,
            target_ups: 60,
            sprite_folder: None,
            use_scanlines: false,
        }
    }
}

impl EngineBuilder {
    pub fn with_title(mut self, title: &str) -> Self { self.title = title.into(); self }
    pub fn with_size(mut self, width: u32, height: u32) -> Self { self.width = width; self.height = height; self }
    pub fn with_tileset(mut self, png_bytes: &'static [u8], tile_w: u32, tile_h: u32) -> Self {
        self.png_bytes = png_bytes; self.tile_w = tile_w; self.tile_h = tile_h; self
    }
    pub fn with_ups(mut self, ups: u32) -> Self { self.target_ups = ups; self }

    /// Specify a directory to scan recursively for `.png` sprite files.
    /// The atlas is baked once at startup before the game loop begins.
    pub fn with_sprite_folder(mut self, path: &str) -> Self {
        self.sprite_folder = Some(path.to_string()); self
    }

    /// Enable CRT-style scanline post-processing (darkens every other logical
    /// pixel row by ~18 %).  Opt-in; off by default.
    pub fn retro_scan_lines(mut self) -> Self {
        self.use_scanlines = true; self
    }

    pub fn run(self, game: impl Game + 'static) {
        let event_loop = EventLoop::new().unwrap();
        let fixed_dt = 1.0 / self.target_ups as f32;
        let mut app = App {
            config: self,
            game: Box::new(game),
            engine: None,
            last_instant: None,
            accumulator: 0.0,
            fixed_dt,
        };
        event_loop.run_app(&mut app).unwrap();
    }
}

// ── App (winit ApplicationHandler) ──────────────────────────────────────────

struct App {
    config: EngineBuilder,
    game: Box<dyn Game>,
    engine: Option<jEngine>,
    last_instant: Option<Instant>,
    accumulator: f32,
    fixed_dt: f32,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(&self.config.title)
                        .with_inner_size(winit::dpi::PhysicalSize::new(
                            self.config.width,
                            self.config.height,
                        ))
                        .with_resizable(false),
                )
                .unwrap(),
        );
        let mut renderer = pollster::block_on(Renderer::new(
            window,
            self.config.png_bytes,
            self.config.tile_w,
            self.config.tile_h,
            self.config.use_scanlines,
        ));

        if let Some(folder) = &self.config.sprite_folder {
            renderer.load_sprite_folder(folder, self.config.tile_w, self.config.tile_h);
        }

        self.engine = Some(jEngine::from_builder(
            renderer,
            self.config.tile_w,
            self.config.tile_h,
        ));
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(engine) = self.engine.as_ref() {
            engine.ui.renderer.window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(engine) = self.engine.as_mut() else { return };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                engine.ui.renderer.resize(size);
                engine.handle_resize();
            }

            WindowEvent::CursorMoved { position, .. } => {
                engine.ui.mouse_pos = [position.x as f32, position.y as f32];
            }

            WindowEvent::MouseInput { button: MouseButton::Left, state, .. } => {
                match state {
                    ElementState::Pressed => {
                        engine.ui.mouse_clicked = true;
                        engine.ui.mouse_held = true;
                    }
                    ElementState::Released => {
                        engine.ui.mouse_held = false;
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let elapsed = match self.last_instant {
                    Some(prev) => now.duration_since(prev).as_secs_f32().min(0.25),
                    None => self.fixed_dt,
                };
                self.last_instant = Some(now);
                self.accumulator += elapsed;

                while self.accumulator >= self.fixed_dt {
                    engine.dt = self.fixed_dt;
                    engine.tick += 1;
                    self.game.update(engine);
                    if engine.quit_requested {
                        event_loop.exit();
                        return;
                    }
                    self.accumulator -= self.fixed_dt;
                }

                engine.tick_animations(elapsed);
                engine.keys_pressed.clear();
                engine.keys_released.clear();
                engine.chars_typed.clear();

                engine.sprite_commands.clear();
                engine.particle_vertices.clear();
                engine.ui.ui_vertices.clear();
                self.game.render(engine);

                let (char_verts, sprite_verts) = engine.build_vertices();
                let particle_verts = std::mem::take(&mut engine.particle_vertices);
                let ui_verts = std::mem::take(&mut engine.ui.ui_vertices);

                // Upload the current camera matrix to the GPU before rendering.
                engine.sync_camera();

                match engine.ui.renderer.render(&char_verts, &sprite_verts, &particle_verts, &ui_verts) {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => {
                        let size = engine.ui.renderer.window.inner_size();
                        engine.ui.renderer.resize(size);
                    }
                    Err(e) => eprintln!("render error: {e}"),
                }
                engine.ui.mouse_clicked = false;
                engine.ui.click_consumed = false;
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state,
                        ref text,
                        ..
                    },
                ..
            } => match state {
                ElementState::Pressed => {
                    if engine.keys_held.insert(code) {
                        engine.keys_pressed.insert(code);
                    }
                    // Capture printable characters for text-input widgets.
                    if let Some(t) = text {
                        for ch in t.chars() {
                            if !ch.is_control() {
                                engine.chars_typed.push(ch);
                            }
                        }
                    }
                }
                ElementState::Released => {
                    engine.keys_held.remove(&code);
                    engine.keys_released.insert(code);
                }
            },

            _ => {}
        }
    }
}
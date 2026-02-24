use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
pub use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

/// Maximum number of simultaneously animated entities.
/// Entity IDs at or above this limit will not receive animation offsets.
/// Matches the GPU storage buffer size allocated in the renderer.
pub const MAX_ANIMATED_ENTITIES: usize = 10_000;

use crate::camera::Camera;
use crate::ui::{UI, Padding, BorderStyle};
use crate::ecs::Entity;
use crate::input::InputState;
use crate::audio::AudioContext;
use crate::renderer::Renderer;
use crate::renderer::particle_pipeline::ParticleVertex;
use crate::renderer::pipeline::TileVertex;
use crate::renderer::sprite_atlas::SpriteData;

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
    fn on_enter(&mut self, _engine: &mut jEngine) {}
    fn update(&mut self, engine: &mut jEngine);
    fn render(&mut self, engine: &mut jEngine);
    /// Optional: Provide extra debug info (colliders, ECS stats) for the F1 inspector.
    fn debug_render(&mut self, _engine: &mut jEngine) -> Option<Box<dyn crate::ui::widgets::Widget>> { None }
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
    data: SpriteData,
    /// 0 = drawn below layer-1 sprites; 1 = drawn above layer-0 sprites.
    layer: u8,
    tint: Color,
    /// Linked ECS entity for animation, or `NO_ENTITY`.
    entity_id: u32,
}

// ── Engine ──────────────────────────────────────────────────────────────────

/// Persistent state for the F1 debug inspector.
#[derive(Debug)]
pub struct DebugState {
    pub enabled: bool,
    pub active: bool,
    pub pos: [f32; 2],
    pub is_dragging: bool,
    pub drag_offset: [f32; 2],
    pub scroll: f32,
}

impl DebugState {
    pub fn new(enabled: bool, sw: f32, sh: f32) -> Self {
        Self {
            enabled,
            active: false,
            pos: [sw * 0.5 - 125.0, sh * 0.5 - 50.0],
            is_dragging: false,
            drag_offset: [0.0, 0.0],
            scroll: 0.0,
        }
    }
}

// Why non_camel_case? Just to style on the plebeians
#[allow(non_camel_case_types)]
pub struct jEngine {
    /// GPU renderer — holds the WGPU surface, pipelines, and atlas textures.
    pub renderer: Renderer,
    /// UI subsystem — tile dimensions, UI vertices, mouse state, and text layer.
    /// Game code draws UI via `engine.ui.ui_*()`.
    pub ui: UI,
    grid_w: u32,
    grid_h: u32,
    /// Layer 0: static background color fills (char atlas path).
    bg_grid: Vec<BgCell>,
    /// Layer 1: character glyphs with optional animation (char atlas path).
    fg_grid: Vec<FgCell>,
    /// True if the grid meshes need to be rebuilt.
    grid_dirty: bool,
    /// Cached vertices for the background and foreground layers.
    cached_char_verts: Vec<TileVertex>,
    /// Queued sprite draw calls (sprite atlas path); cleared before each render.
    sprite_commands: Vec<SpriteCommand>,
    particle_vertices: Vec<ParticleVertex>,
    active_animations: Vec<ActiveAnimation>,
    /// Visual offsets for each entity, uploaded to the GPU as a storage buffer.
    /// We use [f32; 4] to ensure 16-byte alignment required by many GPUs for storage arrays.
    entity_offsets: Vec<[f32; 4]>,
    /// True when entity_offsets contain data that has not yet been uploaded to the GPU.
    /// Stays true for one extra frame after the last animation ends to push the final
    /// zeroed values, then goes false until the next animation starts.
    entity_offsets_dirty: bool,
    /// 2D camera — tracks position, zoom, and shake.
    pub(crate) camera: Camera,
    dt: f32,
    tick: u64,
    /// Unified input state (keyboard, mouse, chars).
    pub input: InputState,
    /// Set to `true` by `request_quit()`; the event loop exits after the current tick.
    pub(crate) quit_requested: bool,
    /// Persistent debug inspector state.
    pub debug: DebugState,
    /// Audio subsystem for music and sound effects.
    pub audio: AudioContext,
    /// Rolling buffer of recent frame times for FPS calculation.
    pub(crate) frame_times: VecDeque<f32>,
}

impl jEngine {
    pub fn builder() -> EngineBuilder {
        EngineBuilder::default()
    }

    fn from_builder(renderer: Renderer, tile_w: u32, tile_h: u32, debug_enabled: bool) -> Self {
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

        let ui = UI::new(tile_w, tile_h);
        Self {
            renderer,
            ui,
            grid_w,
            grid_h,
            bg_grid: vec![BgCell::default(); grid_size],
            fg_grid: vec![FgCell::default(); grid_size],
            grid_dirty: true,
            cached_char_verts: Vec::new(),
            sprite_commands: Vec::new(),
            particle_vertices: Vec::new(),
            active_animations: Vec::new(),
            entity_offsets: vec![[0.0, 0.0, 0.0, 0.0]; MAX_ANIMATED_ENTITIES],
            entity_offsets_dirty: false,
            camera,
            dt: 0.0,
            tick: 0,
            input: InputState::new(),
            quit_requested: false,
            debug: DebugState::new(debug_enabled, size.width as f32, size.height as f32),
            audio: AudioContext::new(),
            frame_times: VecDeque::with_capacity(60),
        }
    }

    // ── Accessors ──────────────────────────────────────────────────────────

    pub fn dt(&self) -> f32 { self.dt }
    pub fn tick(&self) -> u64 { self.tick }
    pub fn grid_width(&self) -> u32 { self.grid_w }
    pub fn grid_height(&self) -> u32 { self.grid_h }
    pub fn tile_width(&self) -> u32 { self.ui.tile_w }
    pub fn tile_height(&self) -> u32 { self.ui.tile_h }

    pub fn is_key_held(&self, key: KeyCode) -> bool { self.input.is_key_held(key) }
    pub fn is_key_pressed(&self, key: KeyCode) -> bool { self.input.is_key_pressed(key) }
    pub fn is_key_released(&self, key: KeyCode) -> bool { self.input.is_key_released(key) }

    pub fn is_mouse_held(&self, button: MouseButton) -> bool { self.input.is_mouse_held(button) }
    pub fn is_mouse_pressed(&self, button: MouseButton) -> bool { self.input.is_mouse_pressed(button) }
    pub fn mouse_pos(&self) -> [f32; 2] { self.input.mouse_pos }

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

    /// Convert screen-pixel coordinates to world-space pixel coordinates,
    /// accounting for camera position and zoom.
    ///
    /// **Note:** camera rotation is not accounted for. If `camera.rotation != 0.0`
    /// the returned coordinates will be incorrect.
    pub fn screen_to_world(&self, screen_x: f32, screen_y: f32) -> [f32; 2] {
        let size = self.renderer.window.inner_size();
        let sw = size.width as f32;
        let sh = size.height as f32;
        
        // 1. Convert screen to NDC [-1, +1]
        let ndc_x = (screen_x / sw) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_y / sh) * 2.0;
        
        // 2. Invert camera transform
        let z = self.camera.zoom.max(0.01);
        let world_x = (ndc_x * sw / (2.0 * z)) + self.camera.position.x + self.camera.shake_offset.x;
        let world_y = (-ndc_y * sh / (2.0 * z)) + self.camera.position.y + self.camera.shake_offset.y;
        
        [world_x, world_y]
    }

    /// Convert world-space pixel coordinates to screen-pixel coordinates.
    ///
    /// **Note:** camera rotation is not accounted for. If `camera.rotation != 0.0`
    /// the returned coordinates will be incorrect.
    pub fn world_to_screen(&self, world_x: f32, world_y: f32) -> [f32; 2] {
        let size = self.renderer.window.inner_size();
        let sw = size.width as f32;
        let sh = size.height as f32;
        
        let cx = self.camera.position.x + self.camera.shake_offset.x;
        let cy = self.camera.position.y + self.camera.shake_offset.y;
        let z = self.camera.zoom.max(0.01);

        let ndc_x = (world_x - cx) * (2.0 * z / sw);
        let ndc_y = (world_y - cy) * (-2.0 * z / sh);
        
        let screen_x = (ndc_x + 1.0) * 0.5 * sw;
        let screen_y = (1.0 - ndc_y) * 0.5 * sh;
        
        [screen_x, screen_y]
    }

    /// Signal that the application should exit.  The event loop will call
    /// `exit()` after the current update tick completes.
    pub fn request_quit(&mut self) {
        self.quit_requested = true;
    }

    // ── Audio API ──────────────────────────────────────────────────────────

    pub fn play_sound(&mut self, name: &str) {
        self.audio.play(name, crate::audio::SoundConfig::default());
    }

    pub fn play_sound_varied(&mut self, name: &str, volume: f32, pitch_variation: f32) {
        self.audio.play(name, crate::audio::SoundConfig {
            volume,
            pitch: 1.0,
            pitch_variation,
            volume_variation: 0.05,
        });
    }

    pub fn play_spatial(&mut self, name: &str, x: f32, y: f32, max_dist: f32) {
        let listener = self.camera.position;
        self.audio.play_spatial(name, x, y, listener.x, listener.y, max_dist);
    }

    // ── Grid drawing (character / char-atlas path) ─────────────────────────

    /// Clear both character layers to their defaults (black bg, no fg glyphs).
    pub fn clear(&mut self) {
        self.bg_grid.fill(BgCell::default());
        self.fg_grid.fill(FgCell::default());
        self.grid_dirty = true;
    }

    /// Set the background (Layer 0) at `(x, y)` to a solid color.
    pub fn set_background(&mut self, x: u32, y: u32, color: Color) {
        if x < self.grid_w && y < self.grid_h {
            self.bg_grid[(y * self.grid_w + x) as usize].color = color;
            self.grid_dirty = true;
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
            self.grid_dirty = true;
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
            self.grid_dirty = true;
        }
    }

    // ── Sprite drawing (sprite-atlas path) ────────────────────────────────

    /// Queue a sprite from the loaded sprite folder at grid position `(x, y)`.
    ///
    /// `layer`: `0` = drawn before layer-1 sprites (background objects),
    ///          `1` = drawn after layer-0 sprites (foreground entities).
    pub fn draw_sprite(&mut self, x: u32, y: u32, name: &str, layer: u8, tint: Color) {
        if let Some(data) = self.renderer.get_sprite_data(name) {
            self.sprite_commands.push(SpriteCommand {
                x,
                y,
                data,
                layer,
                tint,
                entity_id: NO_ENTITY,
            });
        }
    }

    /// Queue a sprite linked to an ECS entity (always Layer 1).
    pub fn draw_sprite_entity(&mut self, x: u32, y: u32, name: &str, entity: Entity, tint: Color) {
        if let Some(data) = self.renderer.get_sprite_data(name) {
            self.sprite_commands.push(SpriteCommand {
                x,
                y,
                data,
                layer: 1,
                tint,
                entity_id: entity.id(),
            });
        }
    }

    // ── Particle drawing ───────────────────────────────────────────────────

    /// Queue a sub-pixel particle quad (`size × size` pixels) at pixel position `(x, y)`.
    pub fn draw_particle(&mut self, x: f32, y: f32, color: Color, size: f32) {
        let h = size * 0.5;
        let c = color.0;
        let tl = ParticleVertex { position: [x - h, y - h, 0.9], color: c };
        let tr = ParticleVertex { position: [x + h, y - h, 0.9], color: c };
        let bl = ParticleVertex { position: [x - h, y + h, 0.9], color: c };
        let br = ParticleVertex { position: [x + h, y + h, 0.9], color: c };
        self.particle_vertices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
    }

    // ── Animation API ──────────────────────────────────────────────────────

    /// Start (or restart) an animation on an ECS entity.  Fire-and-forget:
    /// the engine removes it automatically once its duration elapses.
    pub fn play_animation(&mut self, entity: Entity, anim_type: AnimationType) {
        if (entity.id() as usize) >= MAX_ANIMATED_ENTITIES {
            eprintln!(
                "[engine] play_animation: entity id {} >= MAX_ANIMATED_ENTITIES ({}). \
                 Animation will not play. Increase MAX_ANIMATED_ENTITIES if needed.",
                entity.id(), MAX_ANIMATED_ENTITIES
            );
            return;
        }
        let duration = anim_type.duration();
        self.active_animations.retain(|a| a.entity_id != entity.id());
        self.active_animations.push(ActiveAnimation {
            entity_id: entity.id(),
            anim_type,
            elapsed: 0.0,
            duration,
        });
        self.entity_offsets_dirty = true;
    }

    /// Advance all active animations and camera state by `dt` seconds.
    pub(crate) fn tick_animations(&mut self, dt: f32) {
        let had_active = !self.active_animations.is_empty();

        // Zero out offsets before advancing time. Entities whose animations expire
        // this tick will have their offsets zeroed here and won't be re-written below
        // (since retain removes them), ensuring the GPU sees zeros on the next upload.
        for a in &self.active_animations {
            if (a.entity_id as usize) < self.entity_offsets.len() {
                self.entity_offsets[a.entity_id as usize] = [0.0, 0.0, 0.0, 0.0];
            }
        }

        for a in &mut self.active_animations {
            a.elapsed += dt;
        }
        self.active_animations.retain(|a| a.elapsed < a.duration);

        // Write non-zero offsets for still-active animations.
        for a in &self.active_animations {
            if (a.entity_id as usize) < self.entity_offsets.len() {
                let off = compute_offset(&a.anim_type, a.elapsed, a.duration);
                self.entity_offsets[a.entity_id as usize] = [off[0], off[1], 0.0, 0.0];
            }
        }

        // Mark dirty when any animation ran this tick. This ensures one final upload
        // that pushes zeroed offsets to the GPU after the last animation completes.
        if had_active {
            self.entity_offsets_dirty = true;
        }

        self.camera.tick(dt);
    }

    // ── Internal rendering helpers ─────────────────────────────────────────

    /// Upload the current camera view-projection matrix to the GPU.
    /// Must be called once per frame before `renderer.render()`.
    pub(crate) fn sync_camera(&mut self) {
        let size = self.renderer.window.inner_size();
        let uniform = self.camera.build_view_proj(
            size.width as f32,
            size.height as f32,
        );
        self.renderer.update_camera(&uniform);
    }

    /// Render the toggleable F1 debug inspector.
    pub(crate) fn draw_debug_inspector(&mut self, extra_content: Option<Box<dyn crate::ui::widgets::Widget>>) {
        use crate::ui::widgets::{VStack, TextWidget, Widget, Spacer, ScrollContainer};
        use crate::ui::Alignment;

        let debug_fs = 12.0;

        // ── 1. Calculate Stats ──
        let avg_ft: f32 = if self.frame_times.is_empty() {
            0.0
        } else {
            self.frame_times.iter().sum::<f32>() / self.frame_times.len() as f32
        };
        let fps = if avg_ft > 0.0 { 1.0 / avg_ft } else { 0.0 };
        let stats_text = format!("FPS: {:.1} | FT: {:.2}ms", fps, avg_ft * 1000.0);

        // ── 2. Persistent Drag Logic ──
        let panel_w = 250.0;
        let header_h = 25.0;
        let [mx, my] = self.input.mouse_pos;
        let [px, py] = self.debug.pos;

        let is_over_header = mx >= px && mx <= px + panel_w && my >= py && my <= py + header_h;

        if self.input.is_mouse_pressed(MouseButton::Left) && is_over_header && !self.input.mouse_consumed {
            self.debug.is_dragging = true;
            self.debug.drag_offset = [mx - px, my - py];
            self.input.mouse_consumed = true;
        }

        if self.debug.is_dragging {
            if !self.input.is_mouse_held(MouseButton::Left) {
                self.debug.is_dragging = false;
            } else {
                self.debug.pos = [mx - self.debug.drag_offset[0], my - self.debug.drag_offset[1]];
            }
        }

        // ── 3. Render the Panel ──
        let [nx, ny] = self.debug.pos;
        
        // Use a local copy to avoid double-borrowing self while building/drawing the UI
        let mut scroll = self.debug.scroll;

        {
            let mut main_stack = VStack::new(Alignment::Start)
                .with_spacing(5.0)
                .with_padding(Padding { top: header_h, left: 10.0, right: 10.0, bottom: 10.0 })
                .with_min_width(panel_w)
                .with_bg(Color([1.0, 1.0, 1.0, 0.8]))
                .with_border(BorderStyle::Thin, Color::BLACK)
                .add(TextWidget {
                    text: "DEBUG INSPECTOR [F1]".to_string(),
                    size: Some(debug_fs),
                    color: Some(Color::DARK_BLUE),
                })
                .add(TextWidget {
                    text: stats_text,
                    size: Some(debug_fs),
                    color: Some(Color::BLACK),
                });

            if let Some(extra) = extra_content {
                main_stack = main_stack.add(Spacer { size: 10.0, horizontal: false });
                main_stack = main_stack.add(ScrollContainer {
                    inner: extra,
                    max_height: 400.0,
                    scroll_offset: &mut scroll,
                });
            }

            main_stack.draw(self, nx, ny, panel_w, None);
        }
        
        self.debug.scroll = scroll;
        
        // Draw header highlight
        self.ui.ui_rect(nx + 1.0, ny + 1.0, panel_w - 2.0, header_h - 1.0, Color([0.1, 0.2, 0.3, 0.3]));
    }

    /// Build vertex data for the current frame.
    fn build_vertices(&mut self) -> (Vec<TileVertex>, Vec<TileVertex>) {
        let tile_w = self.ui.tile_w;
        let tile_h = self.ui.tile_h;

        // ── 1. Grid Reconstruction (only if dirty) ──
        if self.grid_dirty {
            let cells = (self.grid_w * self.grid_h) as usize;
            let mut char_verts = Vec::with_capacity(cells * 12);

            // Layer 0: background
            for y in 0..self.grid_h {
                for x in 0..self.grid_w {
                    let cell = &self.bg_grid[(y * self.grid_w + x) as usize];
                    let px = (x * tile_w) as f32;
                    let py = (y * tile_h) as f32;
                    let pw = tile_w as f32;
                    let ph = tile_h as f32;
                    let dummy_uv = [0.0f32, 0.0];

                    let tl = TileVertex { position: [px,      py,      0.9], uv: dummy_uv, fg_color: [0.0; 4], bg_color: cell.color.0, entity_id: NO_ENTITY, layer_id: 0.0 };
                    let tr = TileVertex { position: [px + pw, py,      0.9], uv: dummy_uv, fg_color: [0.0; 4], bg_color: cell.color.0, entity_id: NO_ENTITY, layer_id: 0.0 };
                    let bl = TileVertex { position: [px,      py + ph, 0.9], uv: dummy_uv, fg_color: [0.0; 4], bg_color: cell.color.0, entity_id: NO_ENTITY, layer_id: 0.0 };
                    let br = TileVertex { position: [px + pw, py + ph, 0.9], uv: dummy_uv, fg_color: [0.0; 4], bg_color: cell.color.0, entity_id: NO_ENTITY, layer_id: 0.0 };
                    char_verts.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
                }
            }

            // Layer 1: glyphs
            for y in 0..self.grid_h {
                for x in 0..self.grid_w {
                    let cell = &self.fg_grid[(y * self.grid_w + x) as usize];
                    if cell.index == NO_GLYPH { continue; }

                    let px = (x * tile_w) as f32;
                    let py = (y * tile_h) as f32;
                    let pw = tile_w as f32;
                    let ph = tile_h as f32;

                    let (uv_min, uv_max) = self.renderer.atlas.uv_for_index(cell.index);
                    
                    let tl = TileVertex { position: [px,      py,      0.9], uv: uv_min,                  fg_color: cell.fg.0, bg_color: [0.0; 4], entity_id: cell.entity_id, layer_id: 1.0 };
                    let tr = TileVertex { position: [px + pw, py,      0.9], uv: [uv_max[0], uv_min[1]], fg_color: cell.fg.0, bg_color: [0.0; 4], entity_id: cell.entity_id, layer_id: 1.0 };
                    let bl = TileVertex { position: [px,      py + ph, 0.9], uv: [uv_min[0], uv_max[1]], fg_color: cell.fg.0, bg_color: [0.0; 4], entity_id: cell.entity_id, layer_id: 1.0 };
                    let br = TileVertex { position: [px + pw, py + ph, 0.9], uv: uv_max,                  fg_color: cell.fg.0, bg_color: [0.0; 4], entity_id: cell.entity_id, layer_id: 1.0 };
                    char_verts.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
                }
            }
            self.cached_char_verts = char_verts;
            self.grid_dirty = false; // Grid is now static!
        }

        // ── 2. Sprite Rendering (Using raw data from SpriteCommand) ──
        let mut sorted: Vec<&SpriteCommand> = self.sprite_commands.iter().collect();
        sorted.sort_by_key(|c| c.layer);

        let mut sprite_verts = Vec::with_capacity(sorted.len() * 6);

        for cmd in sorted {
            let px = (cmd.x * tile_w) as f32;
            let py = (cmd.y * tile_h) as f32;
            let pw = (cmd.data.tile_w_span * tile_w) as f32;
            let ph = (cmd.data.tile_h_span * tile_h) as f32;

            let layer_id = if cmd.entity_id != NO_ENTITY { 1.0f32 } else { 0.5f32 };

            let uv_min = cmd.data.uv_min;
            let uv_max = cmd.data.uv_max;
            let fg = cmd.tint.0;

            let tl = TileVertex { position: [px,      py,      0.9], uv: uv_min,                  fg_color: fg, bg_color: [0.0; 4], entity_id: cmd.entity_id, layer_id };
            let tr = TileVertex { position: [px + pw, py,      0.9], uv: [uv_max[0], uv_min[1]], fg_color: fg, bg_color: [0.0; 4], entity_id: cmd.entity_id, layer_id };
            let bl = TileVertex { position: [px,      py + ph, 0.9], uv: [uv_min[0], uv_max[1]], fg_color: fg, bg_color: [0.0; 4], entity_id: cmd.entity_id, layer_id };
            let br = TileVertex { position: [px + pw, py + ph, 0.9], uv: uv_max,                  fg_color: fg, bg_color: [0.0; 4], entity_id: cmd.entity_id, layer_id };
            sprite_verts.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
        }

        // Only upload entity offsets when they have changed. The flag stays true for
        // one extra frame after the last animation ends to push zeroed values, then
        // clears so the 160 KB write is skipped on fully idle frames.
        if self.entity_offsets_dirty {
            self.renderer.update_entity_offsets(&self.entity_offsets);
            if self.active_animations.is_empty() {
                self.entity_offsets_dirty = false;
            }
        }

        // Move the cached char verts out (O(1) pointer swap, no allocation).
        // The caller in window_event restores them after the render call so the
        // cache is available for the next frame without reallocation.
        (std::mem::take(&mut self.cached_char_verts), sprite_verts)
    }

    fn handle_resize(&mut self) {
        let size = self.renderer.window.inner_size();
        let new_gw = size.width / self.ui.tile_w;
        let new_gh = size.height / self.ui.tile_h;
        if new_gw != self.grid_w || new_gh != self.grid_h {
            self.grid_w = new_gw;
            self.grid_h = new_gh;
            let grid_size = (new_gw * new_gh) as usize;
            self.bg_grid = vec![BgCell::default(); grid_size];
            self.fg_grid = vec![FgCell::default(); grid_size];
            self.grid_dirty = true;
        }
    }

    // ── Post-processing API ───────────────────────────────────────────────

    pub fn set_scanlines(&mut self, enabled: bool) {
        if enabled {
            let scale = self.renderer.window.scale_factor() as f32;
            self.renderer.post_process.add_effect(Box::new(
                crate::renderer::post_process::ScanlineEffect::new(
                    &self.renderer.device,
                    self.renderer.surface_format(),
                    scale,
                )
            ));
        } else {
            self.renderer.post_process.remove_effect("scanline");
        }
    }

    pub fn set_vignette(&mut self, enabled: bool) {
        if enabled {
            self.renderer.post_process.add_effect(Box::new(
                crate::renderer::post_process::VignetteEffect::new(
                    &self.renderer.device,
                    self.renderer.surface_format(),
                )
            ));
        } else {
            self.renderer.post_process.remove_effect("vignette");
        }
    }

    pub fn set_chromatic_aberration(&mut self, enabled: bool) {
        if enabled {
            self.renderer.post_process.add_effect(Box::new(
                crate::renderer::post_process::ChromaticAberrationEffect::new(
                    &self.renderer.device,
                    self.renderer.surface_format(),
                )
            ));
        } else {
            self.renderer.post_process.remove_effect("chromatic_aberration");
        }
    }

    pub fn set_bloom(&mut self, enabled: bool) {
        if enabled {
            self.renderer.post_process.add_effect(Box::new(
                crate::renderer::post_process::BloomEffect::new(
                    &self.renderer.device,
                    self.renderer.surface_format(),
                )
            ));
        } else {
            self.renderer.post_process.remove_effect("bloom");
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
    debug_enabled: bool,
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
            debug_enabled: false,
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

    pub fn run(mut self, game: impl Game + 'static) {
        // Check for --debug flag in command line arguments
        if std::env::args().any(|arg| arg == "--debug") {
            self.debug_enabled = true;
        }

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

        let mut engine = jEngine::from_builder(
            renderer,
            self.config.tile_w,
            self.config.tile_h,
            self.config.debug_enabled,
        );

        self.game.on_enter(&mut engine);
        self.engine = Some(engine);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(engine) = self.engine.as_ref() {
            engine.renderer.window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(engine) = self.engine.as_mut() else { return };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                engine.renderer.resize(size);
                engine.handle_resize();
            }

            WindowEvent::CursorMoved { position, .. } => {
                engine.input.mouse_pos = [position.x as f32, position.y as f32];
            }

            WindowEvent::MouseInput { button, state, .. } => {
                match state {
                    ElementState::Pressed => {
                        if engine.input.mouse_held.insert(button) {
                            engine.input.mouse_pressed.insert(button);
                        }
                    }
                    ElementState::Released => {
                        engine.input.mouse_held.remove(&button);
                        engine.input.mouse_released.insert(button);
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                use winit::event::MouseScrollDelta;
                match delta {
                    MouseScrollDelta::LineDelta(_, y) => engine.input.mouse_wheel = y,
                    MouseScrollDelta::PixelDelta(pos) => engine.input.mouse_wheel = (pos.y / 100.0) as f32,
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
                
                // Track frame time for debug inspector
                if engine.debug.enabled {
                    if engine.frame_times.len() >= 60 {
                        engine.frame_times.pop_front();
                    }
                    engine.frame_times.push_back(elapsed);
                }

                engine.sprite_commands.clear();
                engine.particle_vertices.clear();
                engine.ui.clear();
                self.game.render(engine);

                // Draw debug inspector if active
                if engine.debug.enabled && engine.debug.active {
                    let debug_widget = self.game.debug_render(engine);
                    engine.draw_debug_inspector(debug_widget);
                }

                let (char_verts, sprite_verts) = engine.build_vertices();
                let particle_verts = std::mem::take(&mut engine.particle_vertices);
                let ui_verts = std::mem::take(&mut engine.ui.ui_vertices);

                // Upload the current camera matrix to the GPU before rendering.
                engine.sync_camera();

                let text_verts   = std::mem::take(&mut engine.ui.text.vertices);
                let text_indices = std::mem::take(&mut engine.ui.text.indices);
                match engine.renderer.render(&char_verts, &sprite_verts, &particle_verts, &ui_verts, &text_verts, &text_indices) {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => {
                        let size = engine.renderer.window.inner_size();
                        engine.renderer.resize(size);
                    }
                    Err(e) => eprintln!("render error: {e}"),
                }

                // Restore the char vert cache so it survives to the next frame
                // without reallocation. O(1) pointer move — no heap allocation.
                engine.cached_char_verts = char_verts;

                // End of frame cleanup
                engine.input.clear_frame_state();
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
                    if engine.input.keys_held.insert(code) {
                        engine.input.keys_pressed.insert(code);
                    }

                    // F1 toggles debug inspector if enabled.
                    if code == KeyCode::F1 && engine.debug.enabled {
                        engine.debug.active = !engine.debug.active;
                    }

                    // Capture printable characters for text-input widgets.
                    if let Some(t) = text {
                        for ch in t.chars() {
                            if !ch.is_control() {
                                engine.input.chars_typed.push(ch);
                            }
                        }
                    }
                }
                ElementState::Released => {
                    engine.input.keys_held.remove(&code);
                    engine.input.keys_released.insert(code);
                }
            },

            _ => {}
        }
    }
}
//! # Camera Example
//!
//! Demonstrates jengine's 2D camera system.
//!
//! Concepts shown:
//!   · `set_camera_pos(x, y)`   — snap the camera instantly to a world-pixel position
//!   · `move_camera_pos(x, y)`  — set a new target; camera lerps there smoothly (8×/s)
//!   · `set_camera_zoom(z)`     — target zoom level; lerped toward each frame
//!   · `camera_shake(intensity)` — trigger a 0.5-second sinusoidal screen-shake
//!   · `camera_zoom()`          — read the current (interpolated) zoom level
//!
//! The world is a large colourful tile grid that stays fixed; only the camera moves.
//! UI elements (the HUD) are drawn in screen-space and are therefore unaffected by
//! the camera transform.
//!
//! Controls:
//!   Arrow keys   — pan camera (smooth lerp)
//!   = / +        — zoom in  ×1.25
//!   - / _        — zoom out ÷1.25
//!   Space        — camera shake
//!   R            — reset camera to default position and zoom
//!   Esc          — quit

use jengine::engine::{Color, Game, jEngine, KeyCode};
use jengine::renderer::text::Font;
use jengine::{DEFAULT_FONT_METADATA, DEFAULT_TILE_H, DEFAULT_TILE_W, DEFAULT_TILESET};

// ── World constants ───────────────────────────────────────────────────────────

/// The world is WORLD_W × WORLD_H tiles regardless of window size.
const WORLD_W: u32 = 80;
const WORLD_H: u32 = 50;

// ── Camera state (desired values, independent of the engine camera) ───────────

struct CameraDemo {
    font_loaded: bool,
    /// Current zoom target we track independently to display it.
    zoom_target: f32,
    /// Camera target position in world pixels.
    cam_x: f32,
    cam_y: f32,
}

impl CameraDemo {
    fn new() -> Self {
        // Initial camera position: the centre of the world.
        let cx = WORLD_W as f32 * DEFAULT_TILE_W as f32 * 0.5;
        let cy = WORLD_H as f32 * DEFAULT_TILE_H as f32 * 0.5;
        Self {
            font_loaded: false,
            zoom_target: 1.0,
            cam_x: cx,
            cam_y: cy,
        }
    }

    fn default_camera_pos() -> (f32, f32) {
        (
            WORLD_W as f32 * DEFAULT_TILE_W as f32 * 0.5,
            WORLD_H as f32 * DEFAULT_TILE_H as f32 * 0.5,
        )
    }
}

impl Game for CameraDemo {
    fn update(&mut self, engine: &mut jEngine) {
        if engine.is_key_pressed(KeyCode::Escape) {
            engine.request_quit();
            return;
        }

        let step = engine.tile_width() as f32 * 3.0; // pan speed in world pixels

        // ── Camera pan (smooth lerp target) ──────────────────────────────────
        if engine.is_key_held(KeyCode::ArrowLeft)  { self.cam_x -= step; }
        if engine.is_key_held(KeyCode::ArrowRight) { self.cam_x += step; }
        if engine.is_key_held(KeyCode::ArrowUp)    { self.cam_y -= step; }
        if engine.is_key_held(KeyCode::ArrowDown)  { self.cam_y += step; }

        // Clamp the target within world bounds so the user cannot scroll off-map.
        let tw = engine.tile_width() as f32;
        let th = engine.tile_height() as f32;
        self.cam_x = self.cam_x.clamp(0.0, WORLD_W as f32 * tw);
        self.cam_y = self.cam_y.clamp(0.0, WORLD_H as f32 * th);

        // Apply the smooth-lerp target.  The camera will glide to this position.
        engine.move_camera_pos(self.cam_x, self.cam_y);

        // ── Zoom ──────────────────────────────────────────────────────────────
        if engine.is_key_pressed(KeyCode::Equal) || engine.is_key_pressed(KeyCode::NumpadAdd) {
            self.zoom_target = (self.zoom_target * 1.25).min(4.0);
            engine.set_camera_zoom(self.zoom_target);
        }
        if engine.is_key_pressed(KeyCode::Minus) || engine.is_key_pressed(KeyCode::NumpadSubtract) {
            self.zoom_target = (self.zoom_target / 1.25).max(0.25);
            engine.set_camera_zoom(self.zoom_target);
        }

        // ── Shake ─────────────────────────────────────────────────────────────
        if engine.is_key_pressed(KeyCode::Space) {
            // Intensity in world pixels; decays over 0.5 s.
            engine.camera_shake(12.0);
        }

        // ── Reset ─────────────────────────────────────────────────────────────
        if engine.is_key_pressed(KeyCode::KeyR) {
            let (cx, cy) = Self::default_camera_pos();
            self.cam_x = cx;
            self.cam_y = cy;
            self.zoom_target = 1.0;
            // `set_camera_pos` snaps immediately (no lerp).
            engine.set_camera_pos(cx, cy);
            engine.set_camera_zoom(1.0);
        }
    }

    fn render(&mut self, engine: &mut jEngine) {
        // Register the bitmap font once for ui_text.
        if !self.font_loaded {
            if let Ok(font) = Font::from_mtsdf_json(DEFAULT_FONT_METADATA) {
                engine.ui.text.set_font(font);
            }
            self.font_loaded = true;
        }

        engine.clear();

        // ── World tiles ───────────────────────────────────────────────────────
        // Only draw tiles that fall within the viewable grid.  The engine tile
        // grid is sized to the window; extra world tiles outside the window are
        // clipped by the camera transform.
        let gw = engine.grid_width().min(WORLD_W);
        let gh = engine.grid_height().min(WORLD_H);

        for y in 0..gh {
            for x in 0..gw {
                // Create a colourful but regular pattern so panning is obvious.
                let color = world_color(x, y);
                engine.set_background(x, y, color);

                // Label every fifth tile with its grid coordinate.
                if x % 5 == 0 && y % 5 == 0 {
                    engine.set_foreground(x, y, '+', Color([0.3, 0.3, 0.35, 1.0]));
                }
            }
        }

        // Centre-of-world marker.
        let mx = WORLD_W / 2;
        let my = WORLD_H / 2;
        if mx < gw && my < gh {
            engine.set_background(mx, my, Color::WHITE);
            engine.set_foreground(mx, my, 'X', Color::BLACK);
        }

        // ── HUD (screen-space — unaffected by camera) ─────────────────────────
        let tw = engine.tile_width() as f32;
        let th = engine.tile_height() as f32;
        let sw = engine.grid_width() as f32 * tw;
        let zoom_now = engine.camera_zoom();

        // Top bar.
        engine.ui.ui_rect(0.0, 0.0, sw, th, Color([0.0, 0.0, 0.0, 0.85]));
        engine.ui.ui_text(
            tw,
            0.0,
            &format!(
                "Camera: ({:.0}, {:.0})  Zoom: {:.2}  Target zoom: {:.2}",
                self.cam_x, self.cam_y, zoom_now, self.zoom_target
            ),
            Color::WHITE,
            Color::TRANSPARENT, None);

        // Bottom hint bar.
        let sh = engine.grid_height() as f32 * th;
        engine.ui.ui_rect(0.0, sh - th, sw, th, Color([0.0, 0.0, 0.0, 0.85]));
        engine.ui.ui_text(
            tw,
            sh - th,
            "[Arrows] pan   [=/-] zoom   [Space] shake   [R] reset   [Esc] quit",
            Color([0.6, 0.7, 0.65, 1.0]),
            Color::TRANSPARENT, None);
    }
}

/// Produce a visually varied colour for world tile `(x, y)`.
///
/// Uses a simple diagonal stripe pattern with colour bands so that camera
/// movement creates an obvious parallax-free scrolling effect.
fn world_color(x: u32, y: u32) -> Color {
    let band = (x + y) / 4 % 6;
    match band {
        0 => Color([0.10, 0.12, 0.18, 1.0]),
        1 => Color([0.12, 0.18, 0.12, 1.0]),
        2 => Color([0.18, 0.12, 0.10, 1.0]),
        3 => Color([0.14, 0.14, 0.20, 1.0]),
        4 => Color([0.10, 0.18, 0.18, 1.0]),
        _ => Color([0.16, 0.16, 0.12, 1.0]),
    }
}

fn main() {
    jEngine::builder()
        .with_title("jengine — Camera")
        .with_size(800, 576)
        .with_tileset(DEFAULT_TILESET, DEFAULT_TILE_W, DEFAULT_TILE_H)
        .run(CameraDemo::new());
}

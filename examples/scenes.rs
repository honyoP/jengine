//! # Scenes Example
//!
//! Demonstrates jengine's `SceneStack` — a stack-based state machine for game
//! screens.  Scenes are pushed and popped at runtime; only the topmost active
//! scene receives `update()` calls, but multiple scenes may render when the
//! top scene is transparent.
//!
//! Concepts shown:
//!   · `Scene` trait — `on_enter`, `on_exit`, `update`, `draw`, `is_transparent`
//!   · `SceneStack::new(initial)` — create a stack with one starting scene
//!   · `SceneAction` variants:
//!       - `None`        — do nothing this tick
//!       - `Push(scene)` — push a new scene (old scene stays beneath)
//!       - `Pop`         — remove the top scene (previous scene resumes)
//!       - `Switch(s)`   — pop + push in one step (no stack growth)
//!       - `ReplaceAll`  — pop every scene then push a fresh one
//!       - `Quit`        — signal the event loop to exit
//!   · Transparent overlay — a scene with `is_transparent() → true` lets the
//!     scene beneath it render, creating a "pause over gameplay" effect
//!
//! Scene flow:
//!   TitleScene  →[Enter]→  GameScene
//!   GameScene   →[Esc]→   Push(PauseScene, transparent)
//!   PauseScene  →[R]→     Pop  (resume GameScene)
//!   PauseScene  →[Q]→     ReplaceAll(TitleScene)
//!   GameScene   →[O]→     Push(StatsScene, opaque)
//!   StatsScene  →[Esc]→   Pop  (resume GameScene)

use jengine::engine::{Color, jEngine, KeyCode};
use jengine::renderer::text::Font;
use jengine::scene::{Scene, SceneAction, SceneStack};
use jengine::ui::BorderStyle;
use jengine::{DEFAULT_FONT_METADATA, DEFAULT_TILE_H, DEFAULT_TILE_W, DEFAULT_TILESET};

// ── Palette ───────────────────────────────────────────────────────────────────

const BG:       Color = Color([0.04, 0.06, 0.07, 1.0]);
const PANEL_BG: Color = Color([0.06, 0.09, 0.10, 1.0]);
const BORDER:   Color = Color([0.25, 0.65, 0.50, 1.0]);
const BRIGHT:   Color = Color([1.00, 0.95, 0.20, 1.0]);
const BODY:     Color = Color([0.75, 0.85, 0.80, 1.0]);
const DIM:      Color = Color([0.40, 0.50, 0.48, 1.0]);

// ── Font helper ───────────────────────────────────────────────────────────────

/// Register the default bitmap font into the engine if not already loaded.
/// Must be called from `draw()` because that is when `engine` is available.
fn ensure_font(engine: &mut jEngine) {
    if engine.ui.text.font.is_none() {
        if let Ok(font) = Font::from_mtsdf_json(DEFAULT_FONT_METADATA) {
            engine.ui.text.set_font(font);
        }
    }
}

// ── TitleScene ────────────────────────────────────────────────────────────────

/// Initial title / main-menu screen.
///
/// `SceneAction::Switch` is used here: when the player presses Enter the
/// TitleScene is replaced by GameScene (they do not stack — no "back" exists).
struct TitleScene {
    tick: u32,
}

impl TitleScene {
    fn new() -> Box<Self> {
        Box::new(Self { tick: 0 })
    }
}

impl Scene for TitleScene {
    fn on_enter(&mut self, _engine: &mut jEngine) {
        // Lifecycle hook — called once when this scene becomes the top of the stack.
        self.tick = 0;
    }

    fn update(&mut self, engine: &mut jEngine) -> SceneAction {
        self.tick += 1;
        if engine.is_key_pressed(KeyCode::Escape) {
            return SceneAction::Quit;
        }
        if engine.is_key_pressed(KeyCode::Enter) {
            // `Switch` replaces the current scene without growing the stack.
            return SceneAction::Switch(GameScene::new());
        }
        SceneAction::None
    }

    fn draw(&mut self, engine: &mut jEngine) {
        ensure_font(engine);

        let tw = engine.tile_width() as f32;
        let th = engine.tile_height() as f32;
        let sw = engine.grid_width() as f32 * tw;
        let sh = engine.grid_height() as f32 * th;

        engine.ui.ui_rect(0.0, 0.0, sw, sh, BG);

        let pw = tw * 30.0;
        let ph = th * 14.0;
        let px = (sw - pw) * 0.5;
        let py = (sh - ph) * 0.5;
        engine.ui.ui_box(px, py, pw, ph, BorderStyle::Double, BORDER, PANEL_BG);

        let title = "JENGINE SCENE DEMO";
        engine.ui.ui_text(px + (pw - title.len() as f32 * tw) * 0.5, py + th, title, BRIGHT, PANEL_BG, None);
        engine.ui.ui_hline(px + tw, py + th * 2.0, pw - tw * 2.0, BORDER);

        engine.ui.ui_text(px + tw * 2.0, py + th * 3.5, "Scene stack:  1 scene  (TitleScene)", BODY, PANEL_BG, None);
        engine.ui.ui_text(px + tw * 2.0, py + th * 5.0, "[Enter]  Switch to GameScene", BODY, PANEL_BG, None);
        engine.ui.ui_text(px + tw * 2.0, py + th * 6.5, "[Esc]    Quit", BODY, PANEL_BG, None);

        // Blinking prompt.
        if (self.tick / 30) % 2 == 0 {
            engine.ui.ui_text(
                px + (pw - tw * 14.0) * 0.5,
                py + ph - th * 2.0,
                "Press Enter",
                BRIGHT,
                PANEL_BG, None);
        }
    }
}

// ── GameScene ─────────────────────────────────────────────────────────────────

/// The main "gameplay" scene.
///
/// Pressing Esc pushes a `PauseScene` on top (transparent overlay).
/// Pressing O pushes a `StatsScene` on top (opaque, hides game).
struct GameScene {
    tick:       u32,
    player_x:   u32,
    player_y:   u32,
}

impl GameScene {
    fn new() -> Box<Self> {
        Box::new(Self { tick: 0, player_x: 10, player_y: 8 })
    }
}

impl Scene for GameScene {
    fn on_enter(&mut self, _engine: &mut jEngine) {
        // Called when we first enter AND when a scene above us is popped.
        // Use it to restart timers, play sound, etc.
    }

    fn on_exit(&mut self, _engine: &mut jEngine) {
        // Called before this scene is removed or hidden by a push.
    }

    fn update(&mut self, engine: &mut jEngine) -> SceneAction {
        self.tick += 1;

        if engine.is_key_pressed(KeyCode::Escape) {
            // Push a transparent pause overlay — GameScene keeps rendering beneath.
            return SceneAction::Push(PauseScene::new());
        }
        if engine.is_key_pressed(KeyCode::KeyO) {
            // Push an opaque scene — GameScene is NOT rendered while it is open.
            return SceneAction::Push(StatsScene::new());
        }

        // Simple 4-directional movement.
        let gw = engine.grid_width().saturating_sub(1);
        let gh = engine.grid_height().saturating_sub(1);
        if engine.is_key_pressed(KeyCode::ArrowLeft)  && self.player_x > 1 { self.player_x -= 1; }
        if engine.is_key_pressed(KeyCode::ArrowRight) && self.player_x < gw { self.player_x += 1; }
        if engine.is_key_pressed(KeyCode::ArrowUp)    && self.player_y > 1 { self.player_y -= 1; }
        if engine.is_key_pressed(KeyCode::ArrowDown)  && self.player_y < gh { self.player_y += 1; }

        SceneAction::None
    }

    fn draw(&mut self, engine: &mut jEngine) {
        ensure_font(engine);

        let gw = engine.grid_width();
        let gh = engine.grid_height();
        let tw = engine.tile_width() as f32;
        let th = engine.tile_height() as f32;
        let sw = gw as f32 * tw;

        // World background — subtle checkerboard.
        for y in 0..gh {
            for x in 0..gw {
                let c = if (x + y) % 2 == 0 { Color([0.07, 0.08, 0.09, 1.0]) }
                        else                  { Color([0.05, 0.06, 0.07, 1.0]) };
                engine.set_background(x, y, c);
            }
        }

        // A pulsing circle of decorative tiles to show the scene is "alive".
        let cx = gw / 2;
        let cy = gh / 2;
        let r = 6.0 + (self.tick as f32 * 0.05).sin() * 1.5;
        for y in 0..gh {
            for x in 0..gw {
                let dx = x as f32 - cx as f32;
                let dy = y as f32 - cy as f32;
                let dist = (dx * dx + dy * dy).sqrt();
                if (dist - r).abs() < 1.2 {
                    engine.set_background(x, y, Color([0.15, 0.25, 0.20, 1.0]));
                }
            }
        }

        // Player glyph.
        engine.set_background(self.player_x, self.player_y, Color::BLACK);
        engine.set_foreground(self.player_x, self.player_y, '@', Color::YELLOW);

        // HUD.
        engine.ui.ui_rect(0.0, 0.0, sw, th, Color([0.0, 0.0, 0.0, 0.85]));
        engine.ui.ui_text(
            tw, 0.0,
            "GameScene  |  [Arrows] move  [Esc] pause  [O] stats",
            BODY, Color::TRANSPARENT, None);
        engine.ui.ui_text(
            sw - tw * 22.0, 0.0,
            &format!("Stack: 1 scene  tick={}", self.tick),
            DIM, Color::TRANSPARENT, None);
    }
}

// ── PauseScene ────────────────────────────────────────────────────────────────

/// Transparent overlay — GameScene renders beneath it.
///
/// `is_transparent() → true` tells `SceneStack` to also render the scene below.
/// This creates the "see-through pause menu" effect without any special code in
/// `GameScene`.
struct PauseScene;

impl PauseScene {
    fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl Scene for PauseScene {
    /// Returning `true` lets the scene beneath continue to draw.
    fn is_transparent(&self) -> bool { true }

    fn update(&mut self, engine: &mut jEngine) -> SceneAction {
        if engine.is_key_pressed(KeyCode::KeyR) || engine.is_key_pressed(KeyCode::Escape) {
            return SceneAction::Pop; // resume GameScene
        }
        if engine.is_key_pressed(KeyCode::KeyQ) {
            // `ReplaceAll` pops every scene and pushes a fresh TitleScene.
            return SceneAction::ReplaceAll(TitleScene::new());
        }
        SceneAction::None
    }

    fn draw(&mut self, engine: &mut jEngine) {
        ensure_font(engine);

        let tw = engine.tile_width() as f32;
        let th = engine.tile_height() as f32;
        let sw = engine.grid_width() as f32 * tw;
        let sh = engine.grid_height() as f32 * th;

        // Semi-transparent dark overlay — GameScene is still visible underneath.
        engine.ui.ui_rect(0.0, 0.0, sw, sh, Color([0.0, 0.0, 0.0, 0.55]));

        let pw = tw * 26.0;
        let ph = th * 10.0;
        let px = (sw - pw) * 0.5;
        let py = (sh - ph) * 0.5;
        engine.ui.ui_box(px, py, pw, ph, BorderStyle::Double, BORDER, PANEL_BG);

        engine.ui.ui_text(px + (pw - tw * 6.0) * 0.5, py + th, "PAUSED", BRIGHT, PANEL_BG, None);
        engine.ui.ui_hline(px + tw, py + th * 2.0, pw - tw * 2.0, BORDER);
        engine.ui.ui_text(px + tw * 2.0, py + th * 3.5, "[R / Esc]  Resume", BODY, PANEL_BG, None);
        engine.ui.ui_text(px + tw * 2.0, py + th * 5.0, "[Q]        Quit to Title", BODY, PANEL_BG, None);
        engine.ui.ui_text(px + tw * 2.0, py + th * 7.0, "Stack: 2 scenes (transparent)", DIM, PANEL_BG, None);
    }
}

// ── StatsScene ────────────────────────────────────────────────────────────────

/// Opaque scene — GameScene does NOT render while this is on top.
///
/// With `is_transparent() → false` (the default), `SceneStack` only calls
/// `draw()` on THIS scene, saving draw calls and avoiding visual bleed-through.
struct StatsScene;

impl StatsScene {
    fn new() -> Box<Self> {
        Box::new(Self)
    }
}

impl Scene for StatsScene {
    // `is_transparent()` returns `false` by default — GameScene is hidden.

    fn update(&mut self, engine: &mut jEngine) -> SceneAction {
        if engine.is_key_pressed(KeyCode::Escape) {
            return SceneAction::Pop;
        }
        SceneAction::None
    }

    fn draw(&mut self, engine: &mut jEngine) {
        ensure_font(engine);

        let tw = engine.tile_width() as f32;
        let th = engine.tile_height() as f32;
        let sw = engine.grid_width() as f32 * tw;
        let sh = engine.grid_height() as f32 * th;

        // Solid background — GameScene is completely hidden (opaque scene).
        engine.ui.ui_rect(0.0, 0.0, sw, sh, BG);

        let pw = tw * 34.0;
        let ph = th * 16.0;
        let px = (sw - pw) * 0.5;
        let py = (sh - ph) * 0.5;
        engine.ui.ui_box(px, py, pw, ph, BorderStyle::Single, BORDER, PANEL_BG);

        engine.ui.ui_text(px + (pw - tw * 11.0) * 0.5, py + th, "STATS SCREEN", BRIGHT, PANEL_BG, None);
        engine.ui.ui_hline(px + tw, py + th * 2.0, pw - tw * 2.0, BORDER);

        let rows = [
            ("Scene type",   "Opaque (is_transparent = false)"),
            ("Stack depth",  "2 scenes (GameScene hidden)"),
            ("Back action",  "SceneAction::Pop"),
            ("Level",        "1"),
            ("HP",           "100 / 100"),
            ("Strength",     "12"),
            ("Dexterity",    "8"),
        ];
        for (i, (label, value)) in rows.iter().enumerate() {
            let row_y = py + th * (3.5 + i as f32 * 1.4);
            engine.ui.ui_text(px + tw * 2.0, row_y, label, DIM, PANEL_BG, None);
            engine.ui.ui_text(px + tw * 16.0, row_y, value, BODY, PANEL_BG, None);
        }

        let footer_y = py + ph - th;
        engine.ui.ui_hline(px + tw, footer_y, pw - tw * 2.0, BORDER);
        engine.ui.ui_text(px + tw * 2.0, footer_y, "[Esc]  Back to game", DIM, PANEL_BG, None);
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    jEngine::builder()
        .with_title("jengine — Scenes")
        .with_size(800, 576)
        .with_tileset(DEFAULT_TILESET, DEFAULT_TILE_W, DEFAULT_TILE_H)
        // `SceneStack` implements `Game`, so it can be passed directly to `run`.
        .run(SceneStack::new(TitleScene::new()));
}

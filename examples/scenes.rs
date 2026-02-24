//! # Scenes Example
//!
//! Demonstrates the `Scene` trait and `SceneStack` for state management.
//! Features modern UGUI panels and sub-pixel text.

use jengine::engine::{Color, Game, jEngine, KeyCode};
use jengine::renderer::text::Font;
use jengine::ui::{Alignment, BorderStyle, Padding};
use jengine::ui::modern::Panel;
use jengine::ui::widgets::{VStack, TextWidget, Widget, Spacer};
use jengine::scene::{Scene, SceneAction, SceneStack};
use jengine::{DEFAULT_FONT_METADATA, DEFAULT_TILE_H, DEFAULT_TILE_W, DEFAULT_TILESET};

const PANEL_BG: Color = Color([0.06, 0.09, 0.09, 1.0]);
const BORDER:   Color = Color([0.25, 0.65, 0.50, 1.0]);
const TEXT:     Color = Color([0.85, 0.92, 0.88, 1.0]);

struct TitleScene;
impl Scene for TitleScene {
    fn update(&mut self, engine: &mut jEngine) -> SceneAction {
        if engine.is_key_pressed(KeyCode::Enter) { SceneAction::Switch(Box::new(GameplayScene::new())) }
        else if engine.is_key_pressed(KeyCode::Escape) { SceneAction::Quit }
        else { SceneAction::None }
    }
    fn draw(&mut self, engine: &mut jEngine) {
        let sw = engine.grid_width() as f32 * engine.tile_width() as f32;
        let sh = engine.grid_height() as f32 * engine.tile_height() as f32;
        engine.ui.ui_rect(0.0, 0.0, sw, sh, Color([0.03, 0.05, 0.05, 1.0]));
        Panel::new(sw * 0.5 - 200.0, sh * 0.5 - 100.0, 400.0, 200.0).with_color(PANEL_BG).with_border(BORDER, 1.0).with_radius(12.0).draw(engine);
        engine.ui.ui_text(sw * 0.5 - 60.0, sh * 0.5 - 40.0, "SCENE DEMO", TEXT, Color::TRANSPARENT, Some(24.0));
        engine.ui.ui_text(sw * 0.5 - 80.0, sh * 0.5 + 20.0, "Press [Enter] to Start", BORDER, Color::TRANSPARENT, Some(14.0));
    }
}

struct GameplayScene { player_pos: [f32; 2] }
impl GameplayScene { fn new() -> Self { Self { player_pos: [400.0, 300.0] } } }
impl Scene for GameplayScene {
    fn update(&mut self, engine: &mut jEngine) -> SceneAction {
        if engine.is_key_pressed(KeyCode::Escape) { return SceneAction::Push(Box::new(PauseScene)); }
        let speed = 200.0 * engine.dt();
        if engine.is_key_held(KeyCode::ArrowUp) { self.player_pos[1] -= speed; }
        if engine.is_key_held(KeyCode::ArrowDown) { self.player_pos[1] += speed; }
        if engine.is_key_held(KeyCode::ArrowLeft) { self.player_pos[0] -= speed; }
        if engine.is_key_held(KeyCode::ArrowRight) { self.player_pos[0] += speed; }
        SceneAction::None
    }
    fn draw(&mut self, engine: &mut jEngine) {
        let sw = engine.grid_width() as f32 * engine.tile_width() as f32;
        let sh = engine.grid_height() as f32 * engine.tile_height() as f32;
        engine.ui.ui_rect(0.0, 0.0, sw, sh, Color([0.05, 0.08, 0.08, 1.0]));
        engine.ui.ui_text(20.0, 20.0, "Gameplay: Use Arrows to move, Esc to Pause", TEXT, Color::TRANSPARENT, Some(14.0));
        Panel::new(self.player_pos[0] - 15.0, self.player_pos[1] - 15.0, 30.0, 30.0).with_color(Color::CYAN).with_radius(15.0).draw(engine);
    }
}

struct PauseScene;
impl Scene for PauseScene {
    fn is_transparent(&self) -> bool { true }
    fn update(&mut self, engine: &mut jEngine) -> SceneAction {
        if engine.is_key_pressed(KeyCode::Escape) || engine.is_key_pressed(KeyCode::Enter) { SceneAction::Pop }
        else if engine.is_key_pressed(KeyCode::Backspace) { SceneAction::ReplaceAll(Box::new(TitleScene)) }
        else { SceneAction::None }
    }
    fn draw(&mut self, engine: &mut jEngine) {
        let sw = engine.grid_width() as f32 * engine.tile_width() as f32;
        let sh = engine.grid_height() as f32 * engine.tile_height() as f32;
        engine.ui.ui_rect(0.0, 0.0, sw, sh, Color([0.0, 0.0, 0.0, 0.6]));
        Panel::new(sw * 0.5 - 150.0, sh * 0.5 - 80.0, 300.0, 160.0).with_color(PANEL_BG).with_border(BORDER, 1.0).with_radius(8.0).draw(engine);
        engine.ui.ui_text(sw * 0.5 - 40.0, sh * 0.5 - 30.0, "PAUSED", TEXT, Color::TRANSPARENT, Some(20.0));
        engine.ui.ui_text(sw * 0.5 - 100.0, sh * 0.5 + 20.0, "[Enter] Resume  [Bksp] Menu", BORDER, Color::TRANSPARENT, Some(12.0));
    }
}

fn main() {
    jEngine::builder().with_title("jengine — Scenes").with_size(1280, 720).with_tileset(DEFAULT_TILESET, DEFAULT_TILE_W, DEFAULT_TILE_H).run(SceneStack::new(Box::new(TitleScene)));
}

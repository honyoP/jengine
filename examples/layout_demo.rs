//! # Modern Layout Demo
//!
//! Showcases the flexbox-inspired layout engine with modern SDF visuals.

use jengine::engine::{Color, Game, jEngine, KeyCode};
use jengine::renderer::text::Font;
use jengine::ui::{Alignment, BorderStyle, Padding};
use jengine::ui::modern::Panel;
use jengine::ui::widgets::{VStack, HStack, TextWidget, RectWidget, Spacer, Widget};
use jengine::{DEFAULT_FONT_METADATA, DEFAULT_TILE_H, DEFAULT_TILE_W, DEFAULT_TILESET};

struct LayoutDemo {
    font_loaded: bool,
}

impl LayoutDemo {
    fn new() -> Self {
        Self { font_loaded: false }
    }
}

impl Game for LayoutDemo {
    fn update(&mut self, engine: &mut jEngine) {
        if engine.is_key_pressed(KeyCode::Escape) { engine.request_quit(); }
    }

    fn render(&mut self, engine: &mut jEngine) {
        if !self.font_loaded {
            if let Ok(font) = Font::from_mtsdf_json(DEFAULT_FONT_METADATA) {
                engine.ui.text.set_font(font);
            }
            self.font_loaded = true;
        }

        engine.clear();
        let sw = engine.grid_width() as f32 * engine.tile_width() as f32;
        let sh = engine.grid_height() as f32 * engine.tile_height() as f32;

        // Background
        engine.ui.ui_rect(0.0, 0.0, sw, sh, Color([0.05, 0.05, 0.08, 1.0]));

        // ── Centered Menu ──
        let mut menu = VStack::new(Alignment::Center)
            .with_spacing(20.0)
            .with_padding(Padding::all(40.0))
            .with_bg(Color([0.08, 0.08, 0.12, 0.9]))
            .with_border(BorderStyle::Thin, Color([0.3, 0.6, 0.5, 1.0]))
            .with_radius(16.0)
            .add(TextWidget { text: "JENGINE".to_string(), size: Some(64.0), color: Some(Color([1.0, 0.9, 0.2, 1.0])) })
            .add(TextWidget { text: "Modern Layout Demo".to_string(), size: Some(24.0), color: Some(Color([0.4, 0.8, 0.7, 1.0])) })
            .add(Spacer { size: 20.0, horizontal: false })
            .add(
                HStack::new(Alignment::Center)
                    .with_spacing(15.0)
                    .add(RectWidget { w: 80.0, h: 2.0, color: Color([0.5, 0.5, 0.5, 1.0]), radius: 1.0 })
                    .add(TextWidget { text: "v1.0".to_string(), size: Some(14.0), color: Some(Color([0.5, 0.5, 0.5, 1.0])) })
                    .add(RectWidget { w: 80.0, h: 2.0, color: Color([0.5, 0.5, 0.5, 1.0]), radius: 1.0 })
            )
            .add(Spacer { size: 30.0, horizontal: false })
            .add(TextWidget { text: "> Start Game".to_string(), size: Some(32.0), color: Some(Color::WHITE) })
            .add(TextWidget { text: "  Options".to_string(), size: Some(32.0), color: Some(Color([0.7, 0.7, 0.7, 1.0])) })
            .add(TextWidget { text: "  Quit".to_string(), size: Some(32.0), color: Some(Color([0.7, 0.7, 0.7, 1.0])) });

        let (mw, mh) = menu.size(engine);
        Widget::draw(&mut menu, engine, (sw - mw) * 0.5, (sh - mh) * 0.5, mw, None);

        engine.ui.ui_text(sw * 0.5 - 60.0, sh - 40.0, "[Esc] to Quit", Color([0.4, 0.4, 0.4, 1.0]), Color::TRANSPARENT, Some(16.0));
    }
}

fn main() {
    jEngine::builder()
        .with_title("jengine — Modern Layout")
        .with_size(1280, 720)
        .with_tileset(DEFAULT_TILESET, DEFAULT_TILE_W, DEFAULT_TILE_H)
        .run(LayoutDemo::new());
}

use jengine::engine::{Color, Game, jEngine, KeyCode};
use jengine::ui::{Alignment, Padding, BorderStyle};
use jengine::ui::widgets::{VStack, HStack, TextWidget, RectWidget, Spacer, Widget};
use jengine::{DEFAULT_TILESET, DEFAULT_FONT_METADATA, DEFAULT_TILE_W, DEFAULT_TILE_H};

struct LayoutDemo {
    font_loaded: bool,
}

impl LayoutDemo {
    fn new() -> Self {
        Self { font_loaded: false }
    }
}

impl Game for LayoutDemo {
    fn on_enter(&mut self, engine: &mut jEngine) {
        engine.audio.load_sound("UI_selection", "resources/audio/UI_selection.wav");
        engine.audio.load_sound("UI_click", "resources/audio/UI_click.wav");
    }

    fn update(&mut self, engine: &mut jEngine) {
        if engine.is_key_pressed(KeyCode::Escape) {
            engine.play_sound("UI_click");
            engine.request_quit();
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

        let sw = engine.grid_width() as f32 * engine.tile_width() as f32;
        let sh = engine.grid_height() as f32 * engine.tile_height() as f32;

        // Full-screen background.
        engine.ui.ui_rect(0.0, 0.0, sw, sh, Color([0.05, 0.05, 0.08, 1.0]));

        // ── Centered Menu using Layout Engine ────────────────────────────────
        
        let mut menu = VStack::new(Alignment::Center)
            .with_spacing(20.0)
            .with_padding(Padding::all(40.0))
            .add(TextWidget {
                text: "JENGINE".to_string(),
                size: Some(64.0),
                color: Color([1.0, 0.9, 0.2, 1.0]),
            })
            .add(TextWidget {
                text: "Layout Engine Demo".to_string(),
                size: Some(24.0),
                color: Color([0.4, 0.8, 0.7, 1.0]),
            })
            .add(Spacer { size: 20.0, horizontal: false })
            .add(
                HStack::new(Alignment::Center)
                    .with_spacing(10.0)
                    .add(RectWidget { w: 100.0, h: 2.0, color: Color([0.5, 0.5, 0.5, 1.0]) })
                    .add(TextWidget { text: "v0.4.0".to_string(), size: Some(14.0), color: Color([0.5, 0.5, 0.5, 1.0]) })
                    .add(RectWidget { w: 100.0, h: 2.0, color: Color([0.5, 0.5, 0.5, 1.0]) })
            )
            .add(Spacer { size: 40.0, horizontal: false })
            .add(TextWidget {
                text: "> Start Game".to_string(),
                size: Some(32.0),
                color: Color::WHITE,
            })
            .add(TextWidget {
                text: "  Options".to_string(),
                size: Some(32.0),
                color: Color([0.7, 0.7, 0.7, 1.0]),
            })
            .add(TextWidget {
                text: "  Quit".to_string(),
                size: Some(32.0),
                color: Color([0.7, 0.7, 0.7, 1.0]),
            });

        let (menu_w, menu_h) = Widget::size(&menu, engine);
        let menu_x = (sw - menu_w) * 0.5;
        let menu_y = (sh - menu_h) * 0.5;

        // Draw a decorative box around the menu
        engine.ui.ui_box(menu_x, menu_y, menu_w, menu_h, BorderStyle::Double, Color([0.3, 0.6, 0.5, 1.0]), Color([0.08, 0.08, 0.12, 1.0]));
        
        // Draw the layout
        Widget::draw(&mut menu, engine, menu_x, menu_y, menu_w, None);

        // Footer hint
        engine.ui.ui_text(sw * 0.5 - 80.0, sh - 40.0, "[Esc] to Quit", Color([0.4, 0.4, 0.4, 1.0]), Color::TRANSPARENT, Some(16.0));
    }
}

fn main() {
    jEngine::builder()
        .with_title("jengine — Layout Demo")
        .with_size(1280, 720)
        .with_tileset(DEFAULT_TILESET, DEFAULT_TILE_W, DEFAULT_TILE_H)
        .run(LayoutDemo::new());
}

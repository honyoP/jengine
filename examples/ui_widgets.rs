//! # Modern UI Widgets Showcase
//!
//! Demonstrates every UI primitive and interactive widget with the modern UGUI system.

use jengine::engine::{Color, Game, jEngine, KeyCode};
use jengine::renderer::text::Font;
use jengine::ui::{BorderStyle, Padding};
use jengine::ui::modern::Panel;
use jengine::ui::widgets::{Dropdown, InputBox, ToggleSelector};
use jengine::{DEFAULT_FONT_METADATA, DEFAULT_TILE_H, DEFAULT_TILE_W, DEFAULT_TILESET};

const BG:     Color = Color([0.04, 0.06, 0.06, 1.0]);
const BORDER: Color = Color([0.25, 0.65, 0.50, 1.0]);
const HEAD:   Color = Color([1.00, 0.95, 0.20, 1.0]);
const BODY:   Color = Color([0.75, 0.85, 0.80, 1.0]);
const DIM:    Color = Color([0.40, 0.50, 0.48, 1.0]);

struct ModernWidgetsDemo {
    font_loaded: bool,
    dropdown:    Dropdown,
    toggle:      ToggleSelector,
    input:       InputBox,
    hp_pct:      f32,
    tick:        u32,
}

impl ModernWidgetsDemo {
    fn new() -> Self {
        Self {
            font_loaded: false,
            dropdown: Dropdown::new(["Modern Panel", "SDF Borders", "Smooth Corners", "Glassmorphism"]),
            toggle: ToggleSelector::new(["Dark Theme", "Light Theme", "High Contrast"]),
            input: InputBox::new(24),
            hp_pct: 0.75,
            tick: 0,
        }
    }
}

impl Game for ModernWidgetsDemo {
    fn update(&mut self, engine: &mut jEngine) {
        if engine.is_key_pressed(KeyCode::Escape) { engine.request_quit(); }
        self.tick += 1;
        self.hp_pct = (self.tick as f32 * 0.01).sin() * 0.5 + 0.5;
    }

    fn render(&mut self, engine: &mut jEngine) {
        if !self.font_loaded {
            if let Ok(font) = Font::from_mtsdf_json(DEFAULT_FONT_METADATA) {
                engine.ui.text.set_font(font);
            }
            self.font_loaded = true;
        }

        engine.clear();
        let tw = engine.tile_width() as f32;
        let th = engine.tile_height() as f32;
        let sw = engine.grid_width() as f32 * tw;
        let sh = engine.grid_height() as f32 * th;

        engine.ui.ui_rect(0.0, 0.0, sw, sh, BG);

        // ── Title ──
        Panel::new(0.0, 0.0, sw, 50.0).with_color(Color([0.08, 0.12, 0.12, 1.0])).with_border(BORDER, 1.0).draw(engine);
        engine.ui.ui_text(20.0, 15.0, "JENGINE — Modern UGUI Showcase", HEAD, Color::TRANSPARENT, Some(24.0));

        let col_w = sw * 0.5;
        let start_y = 70.0;

        // ── Left: Primitives ──
        let lx = 20.0;
        engine.ui.ui_text(lx, start_y, "MODERN PRIMITIVES", HEAD, Color::TRANSPARENT, Some(18.0));
        
        // Rounded Box
        engine.ui.ui_text(lx, start_y + 40.0, "Panel with 12px corners:", DIM, Color::TRANSPARENT, Some(14.0));
        Panel::new(lx, start_y + 60.0, col_w - 40.0, 80.0)
            .with_color(Color([0.1, 0.15, 0.15, 1.0]))
            .with_border(BORDER, 1.0)
            .with_radius(12.0)
            .draw(engine);
        engine.ui.ui_text(lx + 20.0, start_y + 90.0, "Smooth SDF edges at any resolution.", BODY, Color::TRANSPARENT, Some(14.0));

        // Progress Bar
        engine.ui.ui_text(lx, start_y + 160.0, "Procedural Progress Bar:", DIM, Color::TRANSPARENT, Some(14.0));
        Panel::new(lx, start_y + 180.0, col_w - 40.0, 25.0).with_color(Color([0.05, 0.1, 0.05, 1.0])).with_radius(12.5).draw(engine);
        Panel::new(lx, start_y + 180.0, (col_w - 40.0) * self.hp_pct, 25.0).with_color(Color([0.2, 0.8, 0.3, 1.0])).with_radius(12.5).draw(engine);

        // Pattern
        engine.ui.ui_text(lx, start_y + 220.0, "Procedural Pattern (Crosshatch):", DIM, Color::TRANSPARENT, Some(14.0));
        Panel::new(lx, start_y + 240.0, col_w - 40.0, 60.0)
            .with_color(Color([0.1, 0.1, 0.2, 0.5]))
            .with_border(Color::CYAN, 1.0)
            .with_pattern(1, 4.0)
            .with_radius(8.0)
            .draw(engine);

        // ── Right: Interactive ──
        let rx = col_w + 20.0;
        let ctrl_w = col_w - 40.0;
        engine.ui.ui_text(rx, start_y, "INTERACTIVE WIDGETS", HEAD, Color::TRANSPARENT, Some(18.0));

        engine.ui.ui_text(rx, start_y + 40.0, "Dropdown:", DIM, Color::TRANSPARENT, Some(14.0));
        self.dropdown.draw(engine, rx, start_y + 60.0, ctrl_w);

        engine.ui.ui_text(rx, start_y + 120.0, "ToggleSelector:", DIM, Color::TRANSPARENT, Some(14.0));
        self.toggle.draw(engine, rx, start_y + 140.0, ctrl_w);

        engine.ui.ui_text(rx, start_y + 200.0, "InputBox:", DIM, Color::TRANSPARENT, Some(14.0));
        self.input.draw(engine, rx, start_y + 220.0, ctrl_w);
    }
}

fn main() {
    jEngine::builder()
        .with_title("jengine — Modern Widgets")
        .with_size(1280, 720)
        .with_tileset(DEFAULT_TILESET, DEFAULT_TILE_W, DEFAULT_TILE_H)
        .run(ModernWidgetsDemo::new());
}

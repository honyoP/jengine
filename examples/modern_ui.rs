//! # Modern UI Showcase
//!
//! Demonstrates the new SDF-based procedural UI system.
//! Features:
//!   · Smooth rounded corners (top-left, bottom-right etc)
//!   · Sub-pixel accurate borders
//!   · Semi-transparent glass-morphism effects
//!   · High-resolution scalable text (MTSDF)

use jengine::engine::{Color, Game, jEngine, KeyCode};
use jengine::renderer::text::Font;
use jengine::ui::modern::Panel;
use jengine::{DEFAULT_FONT_METADATA, DEFAULT_TILE_H, DEFAULT_TILE_W, DEFAULT_TILESET};

struct ModernUiDemo {
    font_loaded: bool,
}

impl ModernUiDemo {
    fn new() -> Self {
        Self { font_loaded: false }
    }
}

impl Game for ModernUiDemo {
    fn update(&mut self, engine: &mut jEngine) {
        if engine.is_key_pressed(KeyCode::Escape) {
            engine.request_quit();
        }
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

        // ── Background ──
        engine.ui.ui_rect(0.0, 0.0, sw, sh, Color([0.05, 0.05, 0.1, 1.0]));

        // ── 1. Main Glass Panel ──
        Panel::new(50.0, 50.0, sw - 100.0, sh - 100.0)
            .with_color(Color([0.1, 0.15, 0.25, 0.6]))
            .with_border(Color([0.4, 0.6, 1.0, 0.8]), 2.0)
            .with_radius(20.0)
            .draw(engine);

        // ── 2. Header Area ──
        Panel::new(50.0, 50.0, sw - 100.0, 80.0)
            .with_color(Color([0.15, 0.2, 0.35, 0.8]))
            .with_border(Color([0.4, 0.6, 1.0, 0.4]), 1.0)
            .with_rounded_corners(20.0, 20.0, 0.0, 0.0)
            .draw(engine);

        engine.ui.ui_text(80.0, 75.0, "MODERN UI SYSTEM", Color::WHITE, Color::TRANSPARENT, Some(32.0));
        engine.ui.ui_text(sw - 200.0, 85.0, "v1.0 (Experimental)", Color([0.6, 0.7, 1.0, 1.0]), Color::TRANSPARENT, Some(14.0));

        // ── 3. Content Cards ──
        let card_w = (sw - 160.0) / 3.0;
        let card_h = 200.0;
        let card_y = 160.0;

        for i in 0..3 {
            let cx = 70.0 + i as f32 * (card_w + 20.0);
            
            // Card background
            Panel::new(cx, card_y, card_w, card_h)
                .with_color(Color([0.05, 0.08, 0.15, 0.9]))
                .with_border(Color::CYAN, 1.0)
                .with_radius(12.0)
                .draw(engine);

            engine.ui.ui_text(cx + 20.0, card_y + 20.0, &format!("Feature 0{}", i + 1), Color::CYAN, Color::TRANSPARENT, Some(18.0));
            
            let desc = match i {
                0 => "Procedural Rounded Corners
No textures needed!",
                1 => "SDF Borders
Perfectly smooth at any scale.",
                2 => "Multi-Layer Blending
Built-in alpha support.",
                _ => ""
            };
            engine.ui.ui_text(cx + 20.0, card_y + 60.0, desc, Color([0.8, 0.8, 0.8, 1.0]), Color::TRANSPARENT, Some(14.0));
        }

        // ── 4. Interactive Element Simulation ──
        let btn_x = 70.0;
        let btn_y = sh - 120.0;
        let btn_w = 180.0;
        let btn_h = 45.0;

        let [mx, my] = engine.mouse_pos();
        let hovered = mx >= btn_x && mx <= btn_x + btn_w && my >= btn_y && my <= btn_y + btn_h;
        
        let btn_color = if hovered { Color([0.2, 0.6, 0.4, 1.0]) } else { Color([0.1, 0.4, 0.3, 1.0]) };
        let btn_radius = if hovered { 22.0 } else { 8.0 }; // Animate radius!

        Panel::new(btn_x, btn_y, btn_w, btn_h)
            .with_color(btn_color)
            .with_border(Color::WHITE, 2.0)
            .with_radius(btn_radius)
            .draw(engine);

        engine.ui.ui_text(btn_x + 45.0, btn_y + 12.0, "HOVER ME", Color::WHITE, Color::TRANSPARENT, Some(18.0));

        // ── Footer ──
        engine.ui.ui_text(sw * 0.5 - 100.0, sh - 40.0, "Resolution Independent Rendering", Color([0.4, 0.5, 0.7, 1.0]), Color::TRANSPARENT, Some(12.0));
    }
}

fn main() {
    jEngine::builder()
        .with_title("jengine — Modern UI System")
        .with_size(1280, 720)
        .with_tileset(DEFAULT_TILESET, DEFAULT_TILE_W, DEFAULT_TILE_H)
        .run(ModernUiDemo::new());
}

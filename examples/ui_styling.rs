//! # Modern UI Styling 1:1 Qud Match (Equipment Focus)
//!
//! Recreates the "Caves of Qud" Equipment screen with 1:1 visual fidelity.
//! Features:
//!   · Complex paper-doll layout with schematic lines
//!   · Filterable inventory list
//!   · Detailed context interaction menu

use jengine::engine::{Color, Game, jEngine, KeyCode};
use jengine::renderer::text::Font;
use jengine::ui::modern::Panel;
use jengine::{DEFAULT_FONT_METADATA, DEFAULT_TILE_H, DEFAULT_TILE_W, DEFAULT_TILESET};

// ── Qud Palette ───────────────────────────────────────────────────────────────

const QUD_BG:      Color = Color([0.02, 0.06, 0.06, 1.0]);
const QUD_PANEL:   Color = Color([0.03, 0.08, 0.08, 0.8]);
const QUD_TEAL:    Color = Color([0.25, 0.63, 0.50, 1.0]);
const QUD_GREEN:   Color = Color([0.00, 1.00, 0.00, 1.0]);
const QUD_GOLD:    Color = Color([0.94, 0.94, 0.19, 1.0]);
const QUD_ORANGE:  Color = Color([1.00, 0.50, 0.00, 1.0]);
const QUD_DIM:     Color = Color([0.38, 0.44, 0.44, 1.0]);
const QUD_WHITE:   Color = Color([0.85, 0.92, 0.88, 1.0]);
const QUD_CYAN:    Color = Color([0.00, 0.80, 0.80, 1.0]);

const BASE_FS: f32 = 12.0;
const PANEL_BG: Color = Color([0.06, 0.09, 0.09, 1.0]);

struct QudModernDemo {
    font_loaded: bool,
    active_tab: usize,
    inv_filter_idx: usize,
    context_menu_open: bool,
    selected_item_name: String,
}

impl QudModernDemo {
    fn new() -> Self {
        Self {
            font_loaded: false,
            active_tab: 3, // EQUIPMENT
            inv_filter_idx: 0,
            context_menu_open: false,
            selected_item_name: "steel long sword".to_string(),
        }
    }
}

impl Game for QudModernDemo {
    fn update(&mut self, engine: &mut jEngine) {
        if engine.is_key_pressed(KeyCode::Escape) {
            if self.context_menu_open { self.context_menu_open = false; }
            else { engine.request_quit(); }
        }
    }

    fn render(&mut self, engine: &mut jEngine) {
        if !self.font_loaded {
            if let Ok(font) = Font::from_mtsdf_json(DEFAULT_FONT_METADATA) {
                engine.ui.text.set_font(font);
            }
            engine.set_scanlines(true);
            engine.set_bloom(true);
            self.font_loaded = true;
        }

        engine.clear();
        let tw = engine.tile_width() as f32;
        let th = engine.tile_height() as f32;
        let sw = engine.grid_width() as f32 * tw;
        let sh = engine.grid_height() as f32 * th;

        engine.ui.ui_rect(0.0, 0.0, sw, sh, QUD_BG);

        // ── 1. Top Tab Bar ──
        self.draw_qud_tabs(engine, sw, th);

        // ── 2. Header Area ──
        self.draw_qud_header(engine, th, sw);

        // ── 3. Main Content ──
        match self.active_tab {
            1 => self.draw_placeholder_skills(engine, th, sw, sh),
            3 => self.draw_equipment_tab(engine, th, sw, sh),
            _ => { engine.ui.ui_text(sw*0.5-50.0, sh*0.5, "Tab Coming Soon", QUD_DIM, Color::TRANSPARENT, Some(24.0)); }
        }

        // ── 4. Footer ──
        self.draw_qud_footer(engine, th, sw, sh);

        // ── 5. Context Menu ──
        if self.context_menu_open {
            engine.ui.push_layer(jengine::ui::UILayer::Overlay); // Draw on top
            self.draw_context_menu(engine, sw, sh);
            engine.ui.pop_layer();
        }
    }
}

impl QudModernDemo {
    fn draw_qud_tabs(&mut self, engine: &mut jEngine, sw: f32, th: f32) {
        Panel::new(0.0, 0.0, sw, th * 1.5).with_color(Color([0.1, 0.15, 0.15, 0.3])).with_border(QUD_TEAL, 1.0).with_pattern(1, 4.0).draw(engine);
        let tabs = [
            "Kp Home", "SKILLS", "ATTRIBUTES & POWERS", "EQUIPMENT", 
            "TINKERING", "JOURNAL", "QUESTS", "REPUTATION", "MESSAGE LOG", "Kp Prior"
        ];
        let mut tx = 20.0;
        for (i, tab) in tabs.iter().enumerate() {
            let is_active = i == self.active_tab;
            let tab_w = (tab.len() as f32 * 7.0) + 20.0;
            
            if engine.input.was_clicked(tx - 5.0, 0.0, tab_w, th * 1.5) { self.active_tab = i; }

            if is_active {
                Panel::new(tx - 5.0, 5.0, tab_w, th).with_color(Color([0.2, 0.4, 0.4, 0.4])).draw(engine);
                engine.ui.ui_text(tx - 12.0, 8.0, "/", QUD_CYAN, Color::TRANSPARENT, Some(BASE_FS));
            }
            engine.ui.ui_text(tx, 8.0, tab, if is_active { QUD_WHITE } else { QUD_DIM }, Color::TRANSPARENT, Some(BASE_FS));
            tx += tab_w + 5.0;
            engine.ui.ui_vline(tx - 2.0, 0.0, th * 1.5, 1.0, QUD_TEAL);
        }
    }

    fn draw_qud_header(&self, engine: &mut jEngine, th: f32, sw: f32) {
        let hy = th * 2.0;
        engine.ui.ui_text(40.0, hy, "@", QUD_GREEN, Color::TRANSPARENT, Some(24.0));
        engine.ui.ui_text(100.0, hy + 4.0, "STR: 18  AGI: 19  TOU: 19  INT: 19  WIL: 18  EGO: 18", QUD_GREEN, Color::TRANSPARENT, Some(BASE_FS));
        engine.ui.ui_text(sw * 0.4, hy + 4.0, "Skill Points (SP): 490", QUD_WHITE, Color::TRANSPARENT, Some(BASE_FS));
        engine.ui.ui_hline(20.0, hy + th * 1.2, sw - 40.0, 1.0, QUD_TEAL);
        engine.ui.ui_text(sw * 0.5 - 6.0, hy + th * 1.2 - 6.0, "◆", QUD_TEAL, Color::TRANSPARENT, Some(BASE_FS));
    }

    fn draw_qud_footer(&self, engine: &mut jEngine, th: f32, sw: f32, sh: f32) {
        let fy = sh - th * 2.0;
        Panel::new(0.0, fy, sw, th * 2.0).with_color(Color([0.1, 0.15, 0.15, 0.3])).with_border(QUD_TEAL, 1.0).with_pattern(1, 4.0).draw(engine);
        Panel::new(20.0, fy + 5.0, 120.0, 20.0).with_color(Color([0.0, 0.0, 0.0, 0.4])).with_border(QUD_DIM, 1.0).with_radius(4.0).draw(engine);
        engine.ui.ui_text(30.0, fy + 8.0, "<search>", QUD_DIM, Color::TRANSPARENT, Some(BASE_FS));
        engine.ui.ui_text(sw * 0.5, fy + 8.0, "navigation  [c:gold][Space][/c] Accept", QUD_WHITE, Color::TRANSPARENT, Some(BASE_FS));
    }

    fn draw_equipment_tab(&mut self, engine: &mut jEngine, th: f32, sw: f32, sh: f32) {
        let tw = engine.tile_width() as f32;
        let start_y = th * 4.5;
        let split_x = sw * 0.45;

        Panel::new(tw, start_y, sw - tw * 2.0, sh - start_y - th * 3.0).with_color(QUD_PANEL).with_border(QUD_TEAL, 1.0).draw(engine);
        engine.ui.ui_vline(split_x, start_y, sh - start_y - th * 3.0, 1.0, QUD_TEAL);

        // ── 1. Schematic Paper Doll ──
        let cx = tw + (split_x - tw) * 0.5;
        let cy = start_y + 150.0;
        let slot_sz = 45.0;

        // Slot Definitions: (name, ox, oy) relative to center
        let slots = [
            ("Face", 0.0, -120.0), ("Head", 0.0, -60.0), ("Floating Nearby", 100.0, -100.0),
            ("Worn on Hands", -100.0, -40.0), ("Body", 0.0, 0.0), ("Left Arm", -80.0, 0.0), ("Right Arm", 80.0, 0.0),
            ("Left Hand", -160.0, 0.0), ("Right Hand", 160.0, 0.0), ("Worn on Back", 0.0, 80.0),
            ("Feet", 0.0, 160.0), ("Thrown Weapon", -120.0, 160.0), 
            ("Left Missile", 80.0, 160.0), ("Right Missile", 160.0, 160.0)
        ];

        // Draw Lines
        engine.ui.ui_hline(cx - 160.0, cy, 320.0, 1.0, QUD_DIM); // Main horizontal
        engine.ui.ui_vline(cx, cy - 120.0, 280.0, 1.0, QUD_DIM); // Main vertical
        engine.ui.ui_hline(cx + 80.0, cy + 160.0, 80.0, 1.0, QUD_DIM); // Missile connector

        for (name, ox, oy) in slots {
            let sx = cx + ox - slot_sz * 0.5;
            let sy = cy + oy - slot_sz * 0.5;
            
            Panel::new(sx, sy, slot_sz, slot_sz)
                .with_color(Color([0.0, 0.0, 0.0, 0.4]))
                .with_border(QUD_TEAL, 1.0)
                .draw(engine);
            
            let short_name = name.split_whitespace().last().unwrap();
            engine.ui.ui_text(sx, sy + slot_sz + 5.0, short_name, QUD_DIM, Color::TRANSPARENT, Some(10.0));
            
            if name == "Right Hand" {
                engine.ui.ui_text(sx + 10.0, sy + 10.0, "🗡", QUD_WHITE, Color::TRANSPARENT, Some(24.0));
            }

            if engine.input.was_clicked(sx, sy, slot_sz, slot_sz) {
                self.context_menu_open = true;
                self.selected_item_name = if name == "Right Hand" { "steel long sword".to_string() } else { format!("{} Slot", name) };
            }
        }

        // ── 2. Filter & List (Right) ──
        let mut fx = split_x + 20.0;
        let filters = ["ALL", "🗡", "🛡", "🧪", "📜", "💍"];
        for (i, f) in filters.iter().enumerate() {
            let active = i == self.inv_filter_idx;
            Panel::new(fx, start_y + 15.0, 35.0, 35.0)
                .with_color(if active { QUD_TEAL } else { Color([0.0, 0.0, 0.0, 0.3]) })
                .with_border(QUD_TEAL, 1.0)
                .with_radius(4.0)
                .draw(engine);
            engine.ui.ui_text(fx + 8.0, start_y + 25.0, f, if active { QUD_BG } else { QUD_WHITE }, Color::TRANSPARENT, Some(16.0));
            if engine.input.was_clicked(fx, start_y + 15.0, 35.0, 35.0) { self.inv_filter_idx = i; }
            fx += 45.0;
        }

        let mut iy = start_y + 80.0;
        let items = [
            ("a)", "[-] Ammo | 0 lbs. |", QUD_DIM),
            ("b)", "   lead slug x18", QUD_WHITE),
            ("c)", "[-] Corpses | 115 lbs. |", QUD_DIM),
            ("d)", "   [c:orange]croc corpse[/c]", QUD_ORANGE),
            ("e)", "   [c:white]cave spider corpse[/c]", QUD_WHITE),
            ("f)", "[-] Misc | 5 lbs. |", QUD_DIM),
            ("g)", "   [c:gold]steel long sword[/c]", QUD_GOLD),
        ];

        for (key, name, _col) in items {
            if name.contains("steel") {
                Panel::new(split_x + 10.0, iy - 2.0, sw - split_x - tw * 3.0, BASE_FS + 4.0).with_color(Color([1.0, 1.0, 1.0, 0.08])).draw(engine);
            }
            engine.ui.ui_text(split_x + 25.0, iy, &format!("{} {}", key, name), QUD_WHITE, Color::TRANSPARENT, Some(BASE_FS));
            iy += BASE_FS * 1.5;
            
            if engine.input.was_clicked(split_x + 10.0, iy - 25.0, sw - split_x - tw * 3.0, BASE_FS + 4.0) {
                self.context_menu_open = true;
                self.selected_item_name = name.replace("[c:gold]", "").replace("[/c]", "").trim().to_string();
            }
        }
    }

    fn draw_context_menu(&mut self, engine: &mut jEngine, sw: f32, sh: f32) {
        let mw = 280.0;
        let mh = 350.0;
        let mx = (sw - mw) * 0.5;
        let my = (sh - mh) * 0.5;

        // Main Panel (Solid background, squared corners)
        Panel::new(mx, my, mw, mh)
            .with_color(QUD_BG) // Use solid background
            .with_border(QUD_GOLD, 2.0)
            .with_pattern(1, 2.0)
            .with_radius(0.0) // Squared corners
            .draw(engine);
        
        // Header with Icon
        engine.ui.ui_text(mx + mw * 0.5 - 15.0, my + 20.0, "🗡", QUD_WHITE, Color::TRANSPARENT, Some(48.0));
        engine.ui.ui_text(mx + 40.0, my + 80.0, &self.selected_item_name, QUD_WHITE, Color::TRANSPARENT, Some(18.0));
        engine.ui.ui_hline(mx + 20.0, my + 110.0, mw - 40.0, 1.0, QUD_TEAL);

        // Options
        let options = ["[i] mark important", "[l] look", "[n] add notes", "[r] remove", "[Esc] Cancel"];
        for (i, opt) in options.iter().enumerate() {
            let oy = my + 130.0 + i as f32 * 35.0;
            if engine.input.is_mouse_over(mx + 20.0, oy - 5.0, mw - 40.0, 30.0) {
                Panel::new(mx + 20.0, oy - 5.0, mw - 40.0, 30.0).with_color(Color([1.0, 1.0, 1.0, 0.1])).with_radius(4.0).draw(engine);
            }
            engine.ui.ui_text(mx + 30.0, oy, opt, QUD_WHITE, Color::TRANSPARENT, Some(BASE_FS));
        }
    }

    fn draw_placeholder_skills(&self, engine: &mut jEngine, _th: f32, sw: f32, sh: f32) {
        engine.ui.ui_text(sw*0.5-80.0, sh*0.5, "Skills Tab Content", QUD_WHITE, Color::TRANSPARENT, Some(20.0));
    }
}

fn main() {
    jEngine::builder().with_title("jengine — 1:1 Qud Modern").with_size(1280, 720).with_tileset(DEFAULT_TILESET, 16, 24).run(QudModernDemo::new());
}

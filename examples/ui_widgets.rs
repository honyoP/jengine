//! # UI Widgets Example
//!
//! Showcases every UI primitive and interactive widget in jengine.
//!
//! Primitives shown:
//!   · `ui_rect`         — solid-colour rectangle
//!   · `ui_box`          — bordered box (Single and Double style)
//!   · `ui_text`         — monospaced text line
//!   · `ui_text_wrapped` — word-wrapped text in a bounding box
//!   · `ui_hline`        — horizontal rule
//!   · `ui_vline`        — vertical rule
//!   · `ui_progress_bar` — filled/empty two-colour bar
//!
//! Interactive widgets (all immediate-mode — call `draw()` every frame):
//!   · `Dropdown`        — click to expand, click option to select
//!   · `ToggleSelector`  — [<] / [>] buttons to cycle through options
//!   · `InputBox`        — click to focus, type to enter text
//!
//! Controls:
//!   Esc  — quit

use jengine::engine::{Color, Game, jEngine, KeyCode};
use jengine::renderer::text::Font;
use jengine::ui::{BorderStyle, word_wrap};
use jengine::ui::widgets::{Dropdown, InputBox, ToggleSelector};
use jengine::{DEFAULT_FONT_METADATA, DEFAULT_TILE_H, DEFAULT_TILE_W, DEFAULT_TILESET};

// ── Palette ───────────────────────────────────────────────────────────────────

const BG:     Color = Color([0.04, 0.06, 0.06, 1.0]);
const BORDER: Color = Color([0.25, 0.65, 0.50, 1.0]);
const HEAD:   Color = Color([1.00, 0.95, 0.20, 1.0]);
const BODY:   Color = Color([0.75, 0.85, 0.80, 1.0]);
const DIM:    Color = Color([0.40, 0.50, 0.48, 1.0]);
const HP_ON:  Color = Color([0.05, 0.75, 0.15, 1.0]);
const HP_OFF: Color = Color([0.05, 0.18, 0.08, 1.0]);
const XP_ON:  Color = Color([0.10, 0.45, 1.00, 1.0]);
const XP_OFF: Color = Color([0.02, 0.05, 0.18, 1.0]);

// ── Game ──────────────────────────────────────────────────────────────────────

struct UiWidgetsDemo {
    font_loaded: bool,

    // Interactive widget state.
    dropdown:    Dropdown,
    toggle:      ToggleSelector,
    input:       InputBox,

    // Simulated bar values that animate over time so the progress bars move.
    hp_pct:  f32,
    xp_pct:  f32,
    tick:    u32,
}

impl UiWidgetsDemo {
    fn new() -> Self {
        Self {
            font_loaded: false,
            dropdown: Dropdown::new(["Option Alpha", "Option Beta", "Option Gamma", "Option Delta"]),
            toggle: ToggleSelector::new(["Windowed", "Borderless", "Fullscreen"]),
            input: InputBox::new(24),
            hp_pct: 0.78,
            xp_pct: 0.45,
            tick: 0,
        }
    }
}

impl Game for UiWidgetsDemo {
    fn update(&mut self, engine: &mut jEngine) {
        if engine.is_key_pressed(KeyCode::Escape) {
            engine.request_quit();
        }
        // Animate bar values with a slow sine wave.
        self.tick += 1;
        let t = self.tick as f32 * 0.008;
        self.hp_pct = (t.sin() * 0.5 + 0.5).clamp(0.05, 0.99);
        self.xp_pct = ((t * 0.7 + 1.0).cos() * 0.5 + 0.5).clamp(0.02, 0.98);
    }

    fn render(&mut self, engine: &mut jEngine) {
        // Register the bitmap font once — required for ui_text to render glyphs.
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

        // Full-screen background.
        engine.ui.ui_rect(0.0, 0.0, sw, sh, BG);

        // ── Title bar ─────────────────────────────────────────────────────────
        engine.ui.ui_rect(0.0, 0.0, sw, th, Color([0.08, 0.12, 0.12, 1.0]));
        engine.ui.ui_text(tw, 0.0, "jengine — UI Widgets Example", HEAD, Color::TRANSPARENT, None);
        engine.ui.ui_text(sw - tw * 13.0, 0.0, "[Esc] quit", DIM, Color::TRANSPARENT, None);
        engine.ui.ui_hline(0.0, th, sw, BORDER);

        // Split into left and right columns.
        let col_w = sw * 0.5;
        let top_y = th * 2.5;

        // ── LEFT COLUMN: Boxes, lines and progress bars ───────────────────────
        draw_left_column(engine, tw, th, col_w, top_y, self.hp_pct, self.xp_pct);

        // Vertical divider between the two columns.
        engine.ui.ui_vline(col_w, top_y, sh - top_y, BORDER);

        // ── RIGHT COLUMN: Interactive widgets ────────────────────────────────
        draw_right_column(engine, tw, th, col_w, top_y, &mut self.dropdown, &mut self.toggle, &mut self.input);
    }
}

// ── Left column ───────────────────────────────────────────────────────────────

fn draw_left_column(
    engine: &mut jEngine,
    tw: f32, th: f32,
    col_w: f32,
    top_y: f32,
    hp_pct: f32,
    xp_pct: f32,
) {
    let x = tw;
    let mut y = top_y;

    // Section: Bordered boxes
    engine.ui.ui_text(x, y, "BORDERED BOXES", HEAD, Color::TRANSPARENT, None);
    y += th * 1.5;

    // Single-line border.
    engine.ui.ui_text(x, y, "BorderStyle::Single", DIM, Color::TRANSPARENT, None);
    y += th;
    engine.ui.ui_box(x, y, col_w - tw * 2.0, th * 3.0, BorderStyle::Single, BORDER, Color([0.06, 0.09, 0.09, 1.0]));
    engine.ui.ui_text(x + tw, y + th, "Content inside a Single-border box.", BODY, Color::TRANSPARENT, None);
    y += th * 4.0;

    // Double-line border.
    engine.ui.ui_text(x, y, "BorderStyle::Double", DIM, Color::TRANSPARENT, None);
    y += th;
    engine.ui.ui_box(x, y, col_w - tw * 2.0, th * 3.0, BorderStyle::Double, BORDER, Color([0.06, 0.09, 0.09, 1.0]));
    engine.ui.ui_text(x + tw, y + th, "Content inside a Double-border box.", BODY, Color::TRANSPARENT, None);
    y += th * 5.0;

    // Section: Separators
    engine.ui.ui_text(x, y, "LINES & RULES", HEAD, Color::TRANSPARENT, None);
    y += th * 1.5;
    engine.ui.ui_text(x, y, "ui_hline:", DIM, Color::TRANSPARENT, None);
    y += th;
    engine.ui.ui_hline(x, y, col_w - tw * 2.0, BORDER);
    y += th * 1.5;
    engine.ui.ui_text(x, y, "ui_vline (right edge):", DIM, Color::TRANSPARENT, None);
    engine.ui.ui_vline(col_w - tw * 2.0, y, th * 3.0, BORDER);
    y += th * 4.5;

    // Section: Progress bars
    engine.ui.ui_text(x, y, "PROGRESS BARS", HEAD, Color::TRANSPARENT, None);
    y += th * 1.5;

    engine.ui.ui_text(x, y, &format!("HP  {:.0}%", hp_pct * 100.0), BODY, Color::TRANSPARENT, None);
    y += th;
    engine.ui.ui_progress_bar(x, y, col_w - tw * 2.0, th, hp_pct, HP_ON, HP_OFF);
    y += th * 1.5;

    engine.ui.ui_text(x, y, &format!("XP  {:.0}%", xp_pct * 100.0), BODY, Color::TRANSPARENT, None);
    y += th;
    engine.ui.ui_progress_bar(x, y, col_w - tw * 2.0, th, xp_pct, XP_ON, XP_OFF);
    y += th * 2.5;

    // Section: Word-wrapped text
    engine.ui.ui_text(x, y, "WORD WRAP", HEAD, Color::TRANSPARENT, None);
    y += th * 1.5;
    let max_w = col_w - tw * 2.0;
    let max_cols = (max_w / tw) as usize;
    let sample = "This paragraph is automatically wrapped to fit the column width. \
                  The word_wrap() helper splits on whitespace and never breaks mid-word.";
    // Show max_cols so the reader understands the wrap boundary.
    engine.ui.ui_text(x, y, &format!("(max_cols = {max_cols})"), DIM, Color::TRANSPARENT, None);
    y += th;
    engine.ui.ui_text_wrapped(x, y, max_w, th * 4.0, sample, BODY, Color::TRANSPARENT, None);
    let _ = word_wrap; // referenced in docs; suppress unused-import warning
}

// ── Right column ──────────────────────────────────────────────────────────────

fn draw_right_column(
    engine: &mut jEngine,
    tw: f32, th: f32,
    col_w: f32,
    top_y: f32,
    dropdown: &mut Dropdown,
    toggle: &mut ToggleSelector,
    input: &mut InputBox,
) {
    let x = col_w + tw;
    let widget_w = col_w - tw * 2.0;
    let mut y = top_y;

    engine.ui.ui_text(x, y, "INTERACTIVE WIDGETS", HEAD, Color::TRANSPARENT, None);
    y += th * 1.5;

    // ── Dropdown ──────────────────────────────────────────────────────────────
    engine.ui.ui_text(x, y, "Dropdown  (click to expand):", DIM, Color::TRANSPARENT, None);
    y += th;
    if let Some(idx) = dropdown.draw(engine, x, y, widget_w) {
        let _ = idx; // selection changed — could react here
    }
    y += th;
    engine.ui.ui_text(
        x,
        y,
        &format!("  Selected: \"{}\"", dropdown.selected_text()),
        BODY,
        Color::TRANSPARENT, None);
    y += th * 3.0;

    // ── ToggleSelector ────────────────────────────────────────────────────────
    engine.ui.ui_text(x, y, "ToggleSelector  ([<] / [>] buttons):", DIM, Color::TRANSPARENT, None);
    y += th;
    if let Some(idx) = toggle.draw(engine, x, y, widget_w) {
        let _ = idx;
    }
    y += th;
    engine.ui.ui_text(
        x,
        y,
        &format!("  Selected: \"{}\"", toggle.selected_text()),
        BODY,
        Color::TRANSPARENT, None);
    y += th * 3.0;

    // ── InputBox ──────────────────────────────────────────────────────────────
    engine.ui.ui_text(x, y, "InputBox  (click to focus, then type):", DIM, Color::TRANSPARENT, None);
    y += th;
    input.draw(engine, x, y, widget_w);
    y += th;
    engine.ui.ui_text(
        x,
        y,
        &format!("  Value: \"{}\"", input.value),
        BODY,
        Color::TRANSPARENT, None);
    y += th * 3.0;

    // ── Reference card ────────────────────────────────────────────────────────
    engine.ui.ui_hline(x, y, widget_w, BORDER);
    y += th;
    engine.ui.ui_text(x, y, "QUICK REFERENCE", HEAD, Color::TRANSPARENT, None);
    y += th * 1.5;

    let notes = [
        ("ui_rect(x,y,w,h,col)",         "solid colour quad"),
        ("ui_box(x,y,w,h,style,fg,bg)",  "bordered panel"),
        ("ui_text(x,y,str,fg,bg)",        "monospaced line"),
        ("ui_text_wrapped(x,y,w,h,...)",  "word-wrapped block"),
        ("ui_hline(x,y,w,col)",           "horizontal rule"),
        ("ui_vline(x,y,h,col)",           "vertical rule"),
        ("ui_progress_bar(x,y,w,h,pct,…)", "filled bar"),
        ("Dropdown::draw(eng,x,y,w)",     "→ Option<selected_idx>"),
        ("ToggleSelector::draw(eng,x,y,w)", "→ Option<selected_idx>"),
        ("InputBox::draw(eng,x,y,w)",     "→ bool (changed)"),
    ];
    for (sig, desc) in notes {
        if y + th > engine.grid_height() as f32 * engine.tile_height() as f32 - th {
            break;
        }
        engine.ui.ui_text(x, y, sig, Color::CYAN, Color::TRANSPARENT, None);
        engine.ui.ui_text(x + tw * 28.0, y, &format!("// {desc}"), DIM, Color::TRANSPARENT, None);
        y += th;
    }
}

fn main() {
    jEngine::builder()
        .with_title("jengine — UI Widgets")
        .with_size(1280, 720)
        .with_tileset(DEFAULT_TILESET, DEFAULT_TILE_W, DEFAULT_TILE_H)
        .run(UiWidgetsDemo::new());
}

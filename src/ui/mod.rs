// ── UI types & pure helpers ──────────────────────────────────────────────────

use crate::engine::Color;
use crate::renderer::pipeline::TileVertex;
use crate::renderer::text::{Font, Vec2, Vertex, generate_text_mesh};
use crate::renderer::Renderer;

/// Border style for `ui_box`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BorderStyle {
    /// ─ │ ┌ ┐ └ ┘
    Single,
    /// ═ ║ ╔ ╗ ╚ ╝
    Double,
}

/// Word-wrap `text` so every returned line is at most `max_cols` characters.
/// Words are split on ASCII whitespace; a word longer than `max_cols` is
/// placed alone on its own line (not split mid-word).
pub fn word_wrap(text: &str, max_cols: usize) -> Vec<String> {
    if max_cols == 0 { return vec![]; }
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let space = if current.is_empty() { 0 } else { 1 };
        if !current.is_empty() && current.len() + space + word.len() > max_cols {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() { current.push(' '); }
        current.push_str(word);
        // Force-wrap a single word that exceeds max_cols.
        while current.len() > max_cols {
            lines.push(current[..max_cols].to_string());
            let rest = current[max_cols..].to_string();
            current = rest;
        }
    }
    if !current.is_empty() { lines.push(current); }
    lines
}

/// Returns `true` if pixel point `(px, py)` falls inside the rectangle
/// defined by origin `(rx, ry)` and size `(rw, rh)` using half-open intervals.
pub fn rect_contains(rx: f32, ry: f32, rw: f32, rh: f32, px: f32, py: f32) -> bool {
    px >= rx && px < rx + rw && py >= ry && py < ry + rh
}

// ── TextLayer ─────────────────────────────────────────────────────────────────

/// Plain-data container for variable-width bitmap font rendering.
/// Owns the loaded `Font` assets and the accumulated vertex/index buffers
/// that `Label::draw` writes into each frame.  No GPU state — fully testable.
pub struct TextLayer {
    pub fonts: Vec<Font>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl TextLayer {
    pub fn new() -> Self {
        Self { fonts: Vec::new(), vertices: Vec::new(), indices: Vec::new() }
    }

    /// Register a font and return its `font_id` for use in `Label::new`.
    pub fn add_font(&mut self, font: Font) -> usize {
        let id = self.fonts.len();
        self.fonts.push(font);
        id
    }

    /// Clear accumulated geometry. Call once per frame before drawing labels.
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }
}

// ── Label ──────────────────────────────────────────────────────────────────────

/// High-level UI component for variable-width bitmap text.
///
/// Owns a cached vertex/index mesh that is only regenerated when `set_text`
/// is called with a new string.  Call `draw` each frame to append the cached
/// mesh into a `TextLayer`, which is then uploaded to the GPU once.
pub struct Label {
    text: String,
    pub position: Vec2,
    pub color: [f32; 4],
    pub font_size: f32,
    pub font_id: usize,
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
    dirty: bool,
}

impl Label {
    pub fn new(position: Vec2, font_id: usize, font_size: f32, color: [f32; 4]) -> Self {
        Self {
            text: String::new(),
            position,
            color,
            font_size,
            font_id,
            vertices: Vec::new(),
            indices: Vec::new(),
            dirty: false,
        }
    }

    /// Update the displayed string. The mesh is **not** rebuilt immediately;
    /// it will be regenerated on the next call to `draw`.
    /// If `text` is identical to the current string, nothing changes.
    pub fn set_text(&mut self, text: &str) {
        if self.text != text {
            self.text = text.to_owned();
            self.dirty = true;
        }
    }

    /// Append this label's geometry into `layer`.
    /// Rebuilds the mesh from the font if the text has changed since the last call.
    /// Indices are offset by the number of vertices already in the layer so that
    /// multiple labels can share a single vertex/index buffer.
    pub fn draw(&mut self, layer: &mut TextLayer) {
        if self.dirty {
            if let Some(font) = layer.fonts.get(self.font_id) {
                let (verts, idxs) =
                    generate_text_mesh(&self.text, font, self.position, self.font_size);
                self.vertices = verts;
                self.indices = idxs;
            }
            self.dirty = false;
        }

        let base = layer.vertices.len() as u32;
        layer.vertices.extend_from_slice(&self.vertices);
        layer.indices.extend(self.indices.iter().map(|i| i + base));
    }
}

// ── UI ────────────────────────────────────────────────────────────────────────

pub struct UI {
    /// UI overlay vertices accumulated during `game.render()`; cleared each frame.
    pub ui_vertices: Vec<TileVertex>,
    pub renderer: Renderer,
    pub tile_w: u32,
    pub tile_h: u32,
    /// Current cursor position in physical window-pixel coordinates.
    pub mouse_pos: [f32; 2],
    /// True for exactly the frame in which the left mouse button was pressed.
    pub mouse_clicked: bool,
    /// True while the left mouse button is held.
    pub mouse_held: bool,
    /// Variable-width bitmap font layer; fonts registered here, labels draw into it.
    pub text: TextLayer,
}

impl UI {
    pub fn new(renderer: Renderer, tile_w: u32, tile_h: u32) -> Self {
        Self {
            ui_vertices: Vec::new(),
            renderer,
            tile_w,
            tile_h,
            mouse_pos: [0.0, 0.0],
            mouse_clicked: false,
            mouse_held: false,
            text: TextLayer::new(),
        }
    }

    /// Current cursor position in physical window-pixel coordinates.
    pub fn mouse_pos(&self) -> [f32; 2] { self.mouse_pos }

    /// Returns `true` if the cursor is inside the given pixel rectangle.
    pub fn is_mouse_over(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        rect_contains(x, y, w, h, self.mouse_pos[0], self.mouse_pos[1])
    }

    /// Returns `true` if the left mouse button was clicked inside the given
    /// pixel rectangle this frame (true for exactly one frame per click).
    pub fn was_clicked(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.mouse_clicked && self.is_mouse_over(x, y, w, h)
    }

    /// Translate screen-pixel coordinates to grid (tile) coordinates.
    pub fn pixel_to_grid(&self, px: f32, py: f32) -> (u32, u32) {
        ((px / self.tile_w as f32) as u32, (py / self.tile_h as f32) as u32)
    }
    // ── UI overlay drawing (Layer 2, screenspace) ──────────────────────────
    //
    // All `ui_*` calls accumulate vertices into `ui_vertices`, which are drawn
    // last (on top of all game content) using the char atlas.
    // The GPU buffer is only re-uploaded when the vertex content changes
    // (invalidation by FNV hash in the renderer).

    /// Draw a solid-colored rectangle in screen-pixel coordinates (Layer 2 bg).
    pub fn ui_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        let dummy = [0.0f32, 0.0];
        let c = color.0;
        let tl = TileVertex { position: [x,     y    ], uv: dummy, fg_color: [0.0;4], bg_color: c, v_offset: [0.0,0.0], layer_id: 0.0 };
        let tr = TileVertex { position: [x + w, y    ], uv: dummy, fg_color: [0.0;4], bg_color: c, v_offset: [0.0,0.0], layer_id: 0.0 };
        let bl = TileVertex { position: [x,     y + h], uv: dummy, fg_color: [0.0;4], bg_color: c, v_offset: [0.0,0.0], layer_id: 0.0 };
        let br = TileVertex { position: [x + w, y + h], uv: dummy, fg_color: [0.0;4], bg_color: c, v_offset: [0.0,0.0], layer_id: 0.0 };
        self.ui_vertices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
    }

    /// Draw a single character glyph from the char atlas at a pixel position.
    /// Space is skipped (transparent glyph, no geometry needed).
    /// Requires a font to be registered in `self.text`; characters absent from
    /// the font are silently skipped.
    fn ui_char_at(&mut self, px: f32, py: f32, ch: char, fg: Color) {
        if ch == ' ' { return; }

        // Use the glyph map loaded into the TextLayer for correct UV lookup.
        // The raw char-code (ch as u32) is NOT a valid atlas index — the atlas
        // starts at ' ' (ASCII 32) = index 0, so a direct code-point lookup
        // returns the wrong glyph (e.g. 'H' would fetch 'h').
        let font = match self.text.fonts.first() {
            Some(f) => f,
            None => return, // font not registered yet
        };
        let glyph = match font.glyphs.get(&ch) {
            Some(g) => g,
            None => return, // char not in atlas
        };

        // Derive actual texture dimensions from the Atlas rather than relying
        // on the Font's stored texture_width/texture_height.
        let atlas_w = (self.renderer.atlas.cols * self.renderer.atlas.tile_w) as f32;
        let atlas_h = (self.renderer.atlas.rows * self.renderer.atlas.tile_h) as f32;
        let uv_min = [glyph.x as f32 / atlas_w,                       glyph.y as f32 / atlas_h];
        let uv_max = [(glyph.x + glyph.width)  as f32 / atlas_w,
                      (glyph.y + glyph.height) as f32 / atlas_h];

        let tw = self.tile_w as f32;
        let th = self.tile_h as f32;
        let c = fg.0;
        let tl = TileVertex { position: [px,      py     ], uv: uv_min,                  fg_color: c, bg_color: [0.0;4], v_offset: [0.0,0.0], layer_id: 0.5 };
        let tr = TileVertex { position: [px + tw, py     ], uv: [uv_max[0], uv_min[1]], fg_color: c, bg_color: [0.0;4], v_offset: [0.0,0.0], layer_id: 0.5 };
        let bl = TileVertex { position: [px,      py + th], uv: [uv_min[0], uv_max[1]], fg_color: c, bg_color: [0.0;4], v_offset: [0.0,0.0], layer_id: 0.5 };
        let br = TileVertex { position: [px + tw, py + th], uv: uv_max,                  fg_color: c, bg_color: [0.0;4], v_offset: [0.0,0.0], layer_id: 0.5 };
        self.ui_vertices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
    }

    /// Draw a single line of text. `bg` alpha 0 → transparent (no bg rect).
    /// Each character occupies `tile_w × tile_h` pixels.
    pub fn ui_text(&mut self, x: f32, y: f32, text: &str, fg: Color, bg: Color) {
        let tw = self.tile_w as f32;
        let th = self.tile_h as f32;
        let n = text.chars().count();
        if n == 0 { return; }
        if bg.0[3] > 0.0 {
            self.ui_rect(x, y, n as f32 * tw, th, bg);
        }
        for (i, ch) in text.chars().enumerate() {
            self.ui_char_at(x + i as f32 * tw, y, ch, fg);
        }
    }

    /// Draw text with word-wrapping within a pixel bounding box.
    /// Lines are clipped to `max_h`; words are never split mid-word.
    pub fn ui_text_wrapped(&mut self, x: f32, y: f32, max_w: f32, max_h: f32,
                           text: &str, fg: Color, bg: Color) {
        let tw = self.tile_w as f32;
        let th = self.tile_h as f32;
        let max_cols = (max_w / tw) as usize;
        let max_rows = (max_h / th) as usize;
        for (row, line) in word_wrap(text, max_cols).into_iter().enumerate().take(max_rows) {
            self.ui_text(x, y + row as f32 * th, &line, fg, bg);
        }
    }

    /// Draw a bordered box. The background fills the entire box area (including
    /// border cells). Border chars are drawn on top.
    pub fn ui_box(&mut self, x: f32, y: f32, w: f32, h: f32,
                  style: BorderStyle, fg: Color, bg: Color) {
        let tw = self.tile_w as f32;
        let th = self.tile_h as f32;
        let cols = (w / tw).round() as i32;
        let rows = (h / th).round() as i32;
        if cols < 2 || rows < 2 { return; }

        if bg.0[3] > 0.0 {
            self.ui_rect(x, y, w, h, bg);
        }

        let (hz, vt, tl_c, tr_c, bl_c, br_c) = match style {
            BorderStyle::Single => ('─', '│', '┌', '┐', '└', '┘'),
            BorderStyle::Double => ('═', '║', '╔', '╗', '╚', '╝'),
        };

        // Top row
        self.ui_char_at(x, y, tl_c, fg);
        for c in 1..cols - 1 { self.ui_char_at(x + c as f32 * tw, y, hz, fg); }
        self.ui_char_at(x + (cols - 1) as f32 * tw, y, tr_c, fg);

        // Bottom row
        let by = y + (rows - 1) as f32 * th;
        self.ui_char_at(x, by, bl_c, fg);
        for c in 1..cols - 1 { self.ui_char_at(x + c as f32 * tw, by, hz, fg); }
        self.ui_char_at(x + (cols - 1) as f32 * tw, by, br_c, fg);

        // Side columns
        for r in 1..rows - 1 {
            let py = y + r as f32 * th;
            self.ui_char_at(x, py, vt, fg);
            self.ui_char_at(x + (cols - 1) as f32 * tw, py, vt, fg);
        }
    }

    /// Draw a horizontal progress bar filling `[0, pct]` of `w` with `filled`
    /// and the remainder with `empty`. `pct` is clamped to `[0, 1]`.
    pub fn ui_progress_bar(&mut self, x: f32, y: f32, w: f32, h: f32,
                           pct: f32, filled: Color, empty: Color) {
        let pct = pct.clamp(0.0, 1.0);
        let fw = w * pct;
        if fw > 0.0              { self.ui_rect(x,      y, fw,      h, filled); }
        if fw < w                { self.ui_rect(x + fw, y, w - fw,  h, empty);  }
    }

    /// Draw a horizontal separator line of `─` chars spanning `w` pixels.
    pub fn ui_hline(&mut self, x: f32, y: f32, w: f32, fg: Color) {
        let tw = self.tile_w as f32;
        let cols = (w / tw).round() as i32;
        for c in 0..cols {
            self.ui_char_at(x + c as f32 * tw, y, '─', fg);
        }
    }

    /// Draw a vertical separator line of `│` chars spanning `h` pixels.
    pub fn ui_vline(&mut self, x: f32, y: f32, h: f32, fg: Color) {
        let th = self.tile_h as f32;
        let rows = (h / th).round() as i32;
        for r in 0..rows {
            self.ui_char_at(x, y + r as f32 * th, '│', fg);
        }
    }

}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::text::{Font, Glyph};
    use std::collections::HashMap;

    fn make_font() -> Font {
        let mut glyphs = HashMap::new();
        // Simple 8×16 glyph for 'A'
        glyphs.insert('A', Glyph { id: 'A', x: 0, y: 0, width: 8, height: 16,
                                   x_offset: 0, y_offset: 0, x_advance: 9 });
        Font { glyphs, line_height: 16, texture_width: 256, texture_height: 256 }
    }

    // ── Label dirty flag ───────────────────────────────────────────────────────

    #[test]
    fn label_starts_clean() {
        let label = Label::new([0.0, 0.0], 0, 16.0, [1.0; 4]);
        // New label has empty text and is not dirty.
        assert!(!label.dirty);
        assert!(label.text.is_empty());
    }

    #[test]
    fn set_text_marks_dirty() {
        let mut label = Label::new([0.0, 0.0], 0, 16.0, [1.0; 4]);
        label.set_text("Hello");
        assert!(label.dirty);
        assert_eq!(label.text, "Hello");
    }

    #[test]
    fn set_same_text_stays_clean() {
        let mut label = Label::new([0.0, 0.0], 0, 16.0, [1.0; 4]);
        label.set_text("Hello");
        // Manually clear dirty to simulate a previous draw call.
        label.dirty = false;
        label.set_text("Hello");
        assert!(!label.dirty, "identical set_text must not re-dirty");
    }

    #[test]
    fn set_different_text_marks_dirty_again() {
        let mut label = Label::new([0.0, 0.0], 0, 16.0, [1.0; 4]);
        label.set_text("Hello");
        label.dirty = false;
        label.set_text("World");
        assert!(label.dirty);
    }

    // ── Label::draw mesh generation ────────────────────────────────────────────

    #[test]
    fn draw_empty_text_produces_no_vertices() {
        let mut layer = TextLayer::new();
        layer.add_font(make_font());
        let mut label = Label::new([0.0, 0.0], 0, 16.0, [1.0; 4]);
        label.set_text("");
        label.draw(&mut layer);
        assert!(layer.vertices.is_empty());
        assert!(layer.indices.is_empty());
    }

    #[test]
    fn draw_produces_vertices_for_known_glyph() {
        let mut layer = TextLayer::new();
        layer.add_font(make_font());
        let mut label = Label::new([0.0, 0.0], 0, 16.0, [1.0; 4]);
        label.set_text("A");
        label.draw(&mut layer);
        // One quad = 4 vertices, 6 indices.
        assert_eq!(layer.vertices.len(), 4);
        assert_eq!(layer.indices.len(), 6);
    }

    #[test]
    fn draw_clears_dirty_flag() {
        let mut layer = TextLayer::new();
        layer.add_font(make_font());
        let mut label = Label::new([0.0, 0.0], 0, 16.0, [1.0; 4]);
        label.set_text("A");
        label.draw(&mut layer);
        assert!(!label.dirty);
    }

    #[test]
    fn draw_twice_does_not_duplicate_geometry() {
        let mut layer = TextLayer::new();
        layer.add_font(make_font());
        let mut label = Label::new([0.0, 0.0], 0, 16.0, [1.0; 4]);
        label.set_text("A");
        label.draw(&mut layer);
        // Second draw with same text: mesh is cached, but appended again.
        label.draw(&mut layer);
        // Two draws → 8 vertices (2 × 4), 12 indices (2 × 6).
        assert_eq!(layer.vertices.len(), 8);
        assert_eq!(layer.indices.len(), 12);
    }

    // ── Index offset for multiple labels ──────────────────────────────────────

    #[test]
    fn second_label_indices_offset_by_first_vertex_count() {
        let mut layer = TextLayer::new();
        layer.add_font(make_font());

        let mut label_a = Label::new([0.0,  0.0], 0, 16.0, [1.0; 4]);
        let mut label_b = Label::new([0.0, 20.0], 0, 16.0, [1.0; 4]);

        label_a.set_text("A");
        label_b.set_text("A");

        label_a.draw(&mut layer);
        label_b.draw(&mut layer);

        // label_a uses indices [0,1,2,2,1,3] (or similar, zero-based).
        // label_b must be offset by 4 (one quad worth of vertices).
        let min_b = layer.indices[6..].iter().copied().min().unwrap();
        assert_eq!(min_b, 4, "label_b indices must start at 4, not 0");
    }

    // ── TextLayer helpers ─────────────────────────────────────────────────────

    #[test]
    fn text_layer_add_font_returns_sequential_ids() {
        let mut layer = TextLayer::new();
        let id0 = layer.add_font(make_font());
        let id1 = layer.add_font(make_font());
        assert_eq!(id0, 0);
        assert_eq!(id1, 1);
    }

    #[test]
    fn text_layer_clear_resets_buffers() {
        let mut layer = TextLayer::new();
        layer.add_font(make_font());
        let mut label = Label::new([0.0, 0.0], 0, 16.0, [1.0; 4]);
        label.set_text("A");
        label.draw(&mut layer);
        assert!(!layer.vertices.is_empty());

        layer.clear();
        assert!(layer.vertices.is_empty());
        assert!(layer.indices.is_empty());
        // Fonts must survive a clear.
        assert_eq!(layer.fonts.len(), 1);
    }
}
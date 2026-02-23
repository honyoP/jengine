pub mod widgets;

// ── UI types & pure helpers ──────────────────────────────────────────────────

use crate::engine::Color;
use crate::renderer::pipeline::TileVertex;
use crate::renderer::text::{Font, Glyph, Vec2, Vertex, append_text_mesh, text_width};

/// Border style for `ui_box`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BorderStyle {
    /// ASCII single-line border using `-`, `|`, `+`
    Single,
    /// ASCII double-line border using `=`, `|`, `+`
    Double,
}

/// Word-wrap `text` so every returned line is at most `max_cols` characters.
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
        while current.len() > max_cols {
            lines.push(current[..max_cols].to_string());
            let rest = current[max_cols..].to_string();
            current = rest;
        }
    }
    if !current.is_empty() { lines.push(current); }
    lines
}

/// Returns `true` if pixel point `(px, py)` falls inside the rectangle.
pub fn rect_contains(rx: f32, ry: f32, rw: f32, rh: f32, px: f32, py: f32) -> bool {
    px >= rx && px < rx + rw && py >= ry && py < ry + rh
}

// ── emit_glyph (module-level helper) ─────────────────────────────────────────

/// Emit a single glyph quad into caller-supplied vertex / index buffers.
///
/// All font metrics are passed as plain scalars so this function can be called
/// from contexts that have already released the `&Font` borrow (e.g. inside a
/// `&mut TextLayer` method where `self.fonts` is no longer borrowable).
#[inline]
fn emit_glyph(
    vertices: &mut Vec<Vertex>,
    indices:  &mut Vec<u16>,
    px: f32,
    py: f32,
    glyph:    Glyph,
    atlas_w:  f32,
    atlas_h:  f32,
    ascender: f32,
    color:    [f32; 4],
    font_size: f32,
) {
    if glyph.atlas_right <= glyph.atlas_left || glyph.atlas_bottom <= glyph.atlas_top {
        return; // invisible glyph (e.g. space)
    }
    let baseline_y = py + (-ascender) * font_size;
    let x0 = px + glyph.plane_left   * font_size;
    let x1 = px + glyph.plane_right  * font_size;
    let y0 = baseline_y + glyph.plane_top    * font_size;
    let y1 = baseline_y + glyph.plane_bottom * font_size;

    let u0 = glyph.atlas_left   / atlas_w;
    let u1 = glyph.atlas_right  / atlas_w;
    let v0 = glyph.atlas_top    / atlas_h;
    let v1 = glyph.atlas_bottom / atlas_h;

    debug_assert!(
        vertices.len() < u16::MAX as usize - 3,
        "text vertex buffer overflow inside emit_glyph"
    );
    let base = vertices.len() as u16;
    vertices.push(Vertex { position: [x0, y0], tex_coords: [u0, v0], color });
    vertices.push(Vertex { position: [x1, y0], tex_coords: [u1, v0], color });
    vertices.push(Vertex { position: [x0, y1], tex_coords: [u0, v1], color });
    vertices.push(Vertex { position: [x1, y1], tex_coords: [u1, v1], color });
    indices.extend_from_slice(&[base, base + 1, base + 2, base + 1, base + 3, base + 2]);
}

// ── TextLayer ─────────────────────────────────────────────────────────────────

/// Container for MTSDF font assets and the per-frame vertex/index buffers.
///
/// Both `Label::draw` and `UI::ui_char_at` append into this layer, which is
/// then submitted as a single indexed draw call through the text pipeline.
///
/// Only one GPU font atlas is currently supported, so exactly one font can be
/// registered at a time via [`set_font`].
///
/// # Frame lifecycle
/// Call `clear()` at the start of each render pass (before drawing any labels),
/// then draw labels into the layer.  The engine transfers ownership of the
/// buffers to the renderer via `std::mem::take`, so the vecs are automatically
/// empty again at the start of the next frame.
///
/// [`set_font`]: TextLayer::set_font
pub struct TextLayer {
    pub font: Option<Font>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

impl TextLayer {
    pub fn new() -> Self {
        Self { font: None, vertices: Vec::new(), indices: Vec::new() }
    }

    /// Register the MTSDF font used for all text rendering.
    ///
    /// Replaces any previously registered font.  Only one font atlas is
    /// supported at the GPU level; calling this more than once simply swaps
    /// the active font.
    pub fn set_font(&mut self, font: Font) {
        self.font = Some(font);
    }

    /// Clear accumulated geometry for the current frame.
    ///
    /// Call this at the start of each `draw()` before appending label geometry.
    /// Fonts registered via [`add_font`] are **not** cleared.
    ///
    /// [`add_font`]: TextLayer::add_font
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    /// Append a single pre-looked-up glyph quad directly into the layer buffers.
    ///
    /// Unlike [`push_char`], the caller has already resolved the `&Glyph` and
    /// `&Font`, so this skips the HashMap lookup entirely.  Prefer this in tight
    /// loops where the same glyph is rendered many times (e.g. drawing a full
    /// row of `'-'` characters).
    ///
    /// [`push_char`]: TextLayer::push_char
    pub fn push_glyph(
        &mut self,
        px: f32,
        py: f32,
        glyph: &Glyph,
        font: &Font,
        color: [f32; 4],
        font_size: f32,
    ) {
        emit_glyph(
            &mut self.vertices, &mut self.indices,
            px, py,
            *glyph,
            font.atlas_width as f32, font.atlas_height as f32, font.ascender,
            color, font_size,
        );
    }

    /// Append a single-character MTSDF quad at a fixed `(px, py)` cell origin.
    ///
    /// `font_size` controls the em-height in pixels. The glyph is positioned
    /// using its `planeBounds` metrics, so it sits correctly relative to the
    /// baseline even within a fixed-width tile cell.
    ///
    /// Writes directly into `self.vertices`/`self.indices` — no intermediate
    /// allocation.
    pub(crate) fn push_char(
        &mut self,
        px: f32,
        py: f32,
        ch: char,
        color: [f32; 4],
        font_size: f32,
    ) {
        // Extract glyph and font metrics. `Glyph: Copy` lets us escape the
        // immutable borrow on `self.font` before taking `&mut self.vertices`.
        let Some((glyph, aw, ah, ascender)) = self.font.as_ref().and_then(|f| {
            f.glyphs.get(&ch).map(|g| {
                (*g, f.atlas_width as f32, f.atlas_height as f32, f.ascender)
            })
        }) else { return };

        emit_glyph(
            &mut self.vertices, &mut self.indices,
            px, py,
            glyph, aw, ah, ascender,
            color, font_size,
        );
    }
}

// ── Label ──────────────────────────────────────────────────────────────────────

/// Variable-width MTSDF text label with a cached mesh.
///
/// All properties that affect the mesh (`position`, `color`, `font_size`)
/// are private and must be mutated through their setters so the cached mesh
/// is invalidated correctly.
pub struct Label {
    text: String,
    position: Vec2,
    color: [f32; 4],
    font_size: f32,
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
    dirty: bool,
}

impl Label {
    pub fn new(position: Vec2, font_size: f32, color: [f32; 4]) -> Self {
        Self {
            text: String::new(),
            position,
            color,
            font_size,
            vertices: Vec::new(),
            indices: Vec::new(),
            dirty: false,
        }
    }

    // ── Setters — each invalidates the cached mesh ─────────────────────────

    /// Update the displayed string. The mesh is regenerated on the next `draw`.
    pub fn set_text(&mut self, text: &str) {
        if self.text != text {
            self.text = text.to_owned();
            self.dirty = true;
        }
    }

    /// Move the label to a new screen-pixel position.
    pub fn set_position(&mut self, position: Vec2) {
        if self.position != position {
            self.position = position;
            self.dirty = true;
        }
    }

    /// Change the RGBA tint applied to every vertex.
    pub fn set_color(&mut self, color: [f32; 4]) {
        if self.color != color {
            self.color = color;
            self.dirty = true;
        }
    }

    /// Change the em-height in pixels.
    pub fn set_font_size(&mut self, font_size: f32) {
        if self.font_size != font_size {
            self.font_size = font_size;
            self.dirty = true;
        }
    }

    // ── Getters ────────────────────────────────────────────────────────────

    pub fn text(&self)      -> &str      { &self.text }
    pub fn position(&self)  -> Vec2      { self.position }
    pub fn color(&self)     -> [f32; 4]  { self.color }
    pub fn font_size(&self) -> f32       { self.font_size }

    /// Append this label's geometry into `layer`.
    ///
    /// The mesh is regenerated only when `dirty` is set (i.e. text or metrics
    /// changed). If the required font is not yet registered in `layer`, the
    /// label stays dirty and retries on the next `draw` call.
    pub fn draw(&mut self, layer: &mut TextLayer) {
        if self.dirty {
            if let Some(font) = layer.font.as_ref() {
                self.vertices.clear();
                self.indices.clear();
                append_text_mesh(
                    &self.text,
                    font,
                    self.position,
                    self.font_size,
                    self.color,
                    &mut self.vertices,
                    &mut self.indices,
                );
                self.dirty = false;
                // dirty stays true if the font was not found, so we retry next frame.
            }
        }

        debug_assert!(
            layer.vertices.len() < u16::MAX as usize - self.vertices.len(),
            "text vertex buffer overflow while appending Label"
        );
        let base = layer.vertices.len() as u16;
        layer.vertices.extend_from_slice(&self.vertices);
        layer.indices.extend(self.indices.iter().map(|&i| i + base));
    }
}

// ── UI ────────────────────────────────────────────────────────────────────────

pub struct UI {
    /// UI solid-fill vertices (TileVertex, layer_id=0.0) accumulated each frame.
    pub ui_vertices: Vec<TileVertex>,
    pub tile_w: u32,
    pub tile_h: u32,
    pub mouse_pos: [f32; 2],
    pub mouse_clicked: bool,
    pub mouse_held: bool,
    pub click_consumed: bool,
    /// MTSDF text layer: both Labels and immediate-mode ui_char_at writes here.
    pub text: TextLayer,
}

impl UI {
    pub fn new(tile_w: u32, tile_h: u32) -> Self {
        Self {
            ui_vertices: Vec::new(),
            tile_w,
            tile_h,
            mouse_pos: [0.0, 0.0],
            mouse_clicked: false,
            mouse_held: false,
            click_consumed: false,
            text: TextLayer::new(),
        }
    }

    pub fn mouse_pos(&self) -> [f32; 2] { self.mouse_pos }

    pub fn is_mouse_over(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        rect_contains(x, y, w, h, self.mouse_pos[0], self.mouse_pos[1])
    }

    pub fn was_clicked(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.mouse_clicked && self.is_mouse_over(x, y, w, h)
    }

    pub fn pixel_to_grid(&self, px: f32, py: f32) -> (u32, u32) {
        ((px / self.tile_w as f32) as u32, (py / self.tile_h as f32) as u32)
    }

    // ── UI solid fills (TileVertex, screen-space) ──────────────────────────

    /// Draw a solid-colored rectangle in screen-pixel coordinates.
    pub fn ui_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        let dummy = [0.0f32, 0.0];
        let c = color.0;
        let tl = TileVertex { position: [x,     y    ], uv: dummy, fg_color: [0.0;4], bg_color: c, v_offset: [0.0,0.0], layer_id: 0.0 };
        let tr = TileVertex { position: [x + w, y    ], uv: dummy, fg_color: [0.0;4], bg_color: c, v_offset: [0.0,0.0], layer_id: 0.0 };
        let bl = TileVertex { position: [x,     y + h], uv: dummy, fg_color: [0.0;4], bg_color: c, v_offset: [0.0,0.0], layer_id: 0.0 };
        let br = TileVertex { position: [x + w, y + h], uv: dummy, fg_color: [0.0;4], bg_color: c, v_offset: [0.0,0.0], layer_id: 0.0 };
        self.ui_vertices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
    }

    // ── MTSDF character rendering (screen-space) ───────────────────────────

    /// Render a single character via the MTSDF text pipeline.
    ///
    /// `font_size` — em-height in pixels.  Pass `None` to default to `tile_h`.
    /// Space is skipped (no geometry needed).
    fn ui_char_at(&mut self, px: f32, py: f32, ch: char, fg: Color, font_size: Option<f32>) {
        if ch == ' ' { return; }
        let fs = font_size.unwrap_or(self.tile_h as f32);
        self.text.push_char(px, py, ch, fg.0, fs);
    }

    /// Draw a single line of text.
    ///
    /// `font_size` — em-height in pixels.  Pass `None` to default to `tile_h`.
    ///
    /// When `font_size` is `None`, each character is placed at a fixed `tile_w`
    /// pitch (monospace grid layout).  When an explicit `font_size` is given,
    /// characters are spaced using the font's natural `x_advance` metrics so
    /// that proportionally-scaled text does not overlap.
    ///
    /// `bg` alpha > 0 → a solid background rect is drawn underneath.
    pub fn ui_text(&mut self, x: f32, y: f32, text: &str, fg: Color, bg: Color,
                   font_size: Option<f32>) {
        let tw = self.tile_w as f32;
        let th = self.tile_h as f32;
        let n = text.chars().count();
        if n == 0 { return; }
        let fs = font_size.unwrap_or(th);

        if bg.0[3] > 0.0 {
            // Background width: tile-grid for default size; advance-based otherwise.
            let bg_w = match font_size {
                None => n as f32 * tw,
                Some(_) => self.text.font.as_ref()
                    .map(|f| text_width(text, f, fs))
                    .unwrap_or(n as f32 * tw),
            };
            self.ui_rect(x, y, bg_w, fs, bg);
        }

        match font_size {
            None => {
                // Monospace tile-grid spacing.
                for (i, ch) in text.chars().enumerate() {
                    self.ui_char_at(x + i as f32 * tw, y, ch, fg, None);
                }
            }
            Some(_) => {
                // Proportional spacing: collect x offsets before mutably
                // borrowing self (avoids conflict with ui_char_at).
                let offsets: Vec<f32> = {
                    let mut v = Vec::with_capacity(n);
                    let mut cursor_x = x;
                    let mut prev: Option<char> = None;
                    for ch in text.chars() {
                        if let Some(font) = &self.text.font {
                            if let Some(p) = prev {
                                if let Some(&kern) = font.kerning.get(&(p, ch)) {
                                    cursor_x += kern * fs;
                                }
                            }
                            v.push(cursor_x);
                            cursor_x += font.glyphs.get(&ch)
                                .map(|g| g.x_advance)
                                .unwrap_or(tw / fs)
                                * fs;
                        } else {
                            v.push(cursor_x);
                            cursor_x += tw;
                        }
                        prev = Some(ch);
                    }
                    v
                };
                for (px, ch) in offsets.into_iter().zip(text.chars()) {
                    self.ui_char_at(px, y, ch, fg, font_size);
                }
            }
        }
    }

    /// Draw text with word-wrapping within a pixel bounding box.
    ///
    /// `font_size` — em-height in pixels.  Pass `None` to default to `tile_h`.
    pub fn ui_text_wrapped(&mut self, x: f32, y: f32, max_w: f32, max_h: f32,
                           text: &str, fg: Color, bg: Color, font_size: Option<f32>) {
        let tw = self.tile_w as f32;
        let th = self.tile_h as f32;
        let row_h = font_size.unwrap_or(th);
        let max_cols = (max_w / tw) as usize;
        let max_rows = (max_h / row_h) as usize;
        for (row, line) in word_wrap(text, max_cols).into_iter().enumerate().take(max_rows) {
            self.ui_text(x, y + row as f32 * row_h, &line, fg, bg, font_size);
        }
    }

    /// Draw a bordered box using ASCII line characters.
    ///
    /// Single style: `- | +`  /  Double style: `= | +`
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

        let (hz, vt, corner) = match style {
            BorderStyle::Single => ('-', '|', '+'),
            BorderStyle::Double => ('=', '|', '+'),
        };

        // Top row
        self.ui_char_at(x, y, corner, fg, None);
        for c in 1..cols - 1 { self.ui_char_at(x + c as f32 * tw, y, hz, fg, None); }
        self.ui_char_at(x + (cols - 1) as f32 * tw, y, corner, fg, None);

        // Bottom row
        let by = y + (rows - 1) as f32 * th;
        self.ui_char_at(x, by, corner, fg, None);
        for c in 1..cols - 1 { self.ui_char_at(x + c as f32 * tw, by, hz, fg, None); }
        self.ui_char_at(x + (cols - 1) as f32 * tw, by, corner, fg, None);

        // Side columns
        for r in 1..rows - 1 {
            let py = y + r as f32 * th;
            self.ui_char_at(x, py, vt, fg, None);
            self.ui_char_at(x + (cols - 1) as f32 * tw, py, vt, fg, None);
        }
    }

    /// Draw a horizontal progress bar.
    pub fn ui_progress_bar(&mut self, x: f32, y: f32, w: f32, h: f32,
                           pct: f32, filled: Color, empty: Color) {
        let pct = pct.clamp(0.0, 1.0);
        let fw = w * pct;
        if fw > 0.0 { self.ui_rect(x,      y, fw,     h, filled); }
        if fw < w   { self.ui_rect(x + fw, y, w - fw, h, empty);  }
    }

    /// Draw a horizontal separator line of `-` chars spanning `w` pixels.
    pub fn ui_hline(&mut self, x: f32, y: f32, w: f32, fg: Color) {
        let tw = self.tile_w as f32;
        let cols = (w / tw).round() as i32;
        for c in 0..cols {
            self.ui_char_at(x + c as f32 * tw, y, '-', fg, None);
        }
    }

    /// Draw a vertical separator line of `|` chars spanning `h` pixels.
    pub fn ui_vline(&mut self, x: f32, y: f32, h: f32, fg: Color) {
        let th = self.tile_h as f32;
        let rows = (h / th).round() as i32;
        for r in 0..rows {
            self.ui_char_at(x, y + r as f32 * th, '|', fg, None);
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
        // Simple glyph for 'A' with MTSDF-style metrics.
        // plane: left=0, top=-1 (above baseline), right=0.5, bottom=0
        // atlas: 8×16 region in a 256×256 atlas
        glyphs.insert('A', Glyph {
            atlas_left: 0.0, atlas_top: 0.0, atlas_right: 8.0, atlas_bottom: 16.0,
            plane_left: 0.0, plane_top: -1.0, plane_right: 0.5, plane_bottom: 0.0,
            x_advance: 0.5,
        });
        Font {
            glyphs,
            line_height: 1.0,
            ascender: -1.0,   // 1 em above the baseline (negative = up in screen Y-down)
            descender: 0.1,
            atlas_width: 256,
            atlas_height: 256,
            distance_range: 4.0,
            kerning: HashMap::new(),
        }
    }

    // ── Label dirty flag ───────────────────────────────────────────────────────

    #[test]
    fn label_starts_clean() {
        let label = Label::new([0.0, 0.0], 16.0, [1.0; 4]);
        assert!(!label.dirty);
        assert!(label.text.is_empty());
    }

    #[test]
    fn set_text_marks_dirty() {
        let mut label = Label::new([0.0, 0.0], 16.0, [1.0; 4]);
        label.set_text("Hello");
        assert!(label.dirty);
        assert_eq!(label.text, "Hello");
    }

    #[test]
    fn set_same_text_stays_clean() {
        let mut label = Label::new([0.0, 0.0], 16.0, [1.0; 4]);
        label.set_text("Hello");
        label.dirty = false;
        label.set_text("Hello");
        assert!(!label.dirty, "identical set_text must not re-dirty");
    }

    #[test]
    fn set_different_text_marks_dirty_again() {
        let mut label = Label::new([0.0, 0.0], 16.0, [1.0; 4]);
        label.set_text("Hello");
        label.dirty = false;
        label.set_text("World");
        assert!(label.dirty);
    }

    // ── Label::draw mesh generation ────────────────────────────────────────────

    #[test]
    fn draw_empty_text_produces_no_vertices() {
        let mut layer = TextLayer::new();
        layer.set_font(make_font());
        let mut label = Label::new([0.0, 0.0], 16.0, [1.0; 4]);
        label.set_text("");
        label.draw(&mut layer);
        assert!(layer.vertices.is_empty());
        assert!(layer.indices.is_empty());
    }

    #[test]
    fn draw_produces_vertices_for_known_glyph() {
        let mut layer = TextLayer::new();
        layer.set_font(make_font());
        let mut label = Label::new([0.0, 0.0], 16.0, [1.0; 4]);
        label.set_text("A");
        label.draw(&mut layer);
        // One quad = 4 vertices, 6 indices.
        assert_eq!(layer.vertices.len(), 4);
        assert_eq!(layer.indices.len(), 6);
    }

    #[test]
    fn draw_clears_dirty_flag() {
        let mut layer = TextLayer::new();
        layer.set_font(make_font());
        let mut label = Label::new([0.0, 0.0], 16.0, [1.0; 4]);
        label.set_text("A");
        label.draw(&mut layer);
        assert!(!label.dirty);
    }

    #[test]
    fn draw_twice_does_not_duplicate_geometry() {
        let mut layer = TextLayer::new();
        layer.set_font(make_font());
        let mut label = Label::new([0.0, 0.0], 16.0, [1.0; 4]);
        label.set_text("A");
        label.draw(&mut layer);
        // Second draw with same text: cached mesh appended again.
        label.draw(&mut layer);
        assert_eq!(layer.vertices.len(), 8);
        assert_eq!(layer.indices.len(), 12);
    }

    // ── Index offset for multiple labels ──────────────────────────────────────

    #[test]
    fn second_label_indices_offset_by_first_vertex_count() {
        let mut layer = TextLayer::new();
        layer.set_font(make_font());

        let mut label_a = Label::new([0.0,  0.0], 16.0, [1.0; 4]);
        let mut label_b = Label::new([0.0, 20.0], 16.0, [1.0; 4]);

        label_a.set_text("A");
        label_b.set_text("A");

        label_a.draw(&mut layer);
        label_b.draw(&mut layer);

        let min_b = layer.indices[6..].iter().copied().min().unwrap();
        assert_eq!(min_b, 4, "label_b indices must start at 4, not 0");
    }

    // ── TextLayer helpers ─────────────────────────────────────────────────────

    #[test]
    fn text_layer_set_font_registers_font() {
        let mut layer = TextLayer::new();
        assert!(layer.font.is_none());
        layer.set_font(make_font());
        assert!(layer.font.is_some());
        // Calling set_font again replaces the previous font.
        layer.set_font(make_font());
        assert!(layer.font.is_some());
    }

    #[test]
    fn text_layer_clear_resets_buffers() {
        let mut layer = TextLayer::new();
        layer.set_font(make_font());
        let mut label = Label::new([0.0, 0.0], 16.0, [1.0; 4]);
        label.set_text("A");
        label.draw(&mut layer);
        assert!(!layer.vertices.is_empty());

        layer.clear();
        assert!(layer.vertices.is_empty());
        assert!(layer.indices.is_empty());
        // Font must survive a clear.
        assert!(layer.font.is_some());
    }

    // ── word_wrap ─────────────────────────────────────────────────────────────

    #[test]
    fn word_wrap_empty_string() {
        assert!(word_wrap("", 10).is_empty());
    }

    #[test]
    fn word_wrap_zero_cols() {
        assert!(word_wrap("hello world", 0).is_empty());
    }

    #[test]
    fn word_wrap_single_word_fits() {
        assert_eq!(word_wrap("hello", 10), vec!["hello"]);
    }

    #[test]
    fn word_wrap_two_words_fit_on_one_line() {
        assert_eq!(word_wrap("hi there", 10), vec!["hi there"]);
    }

    #[test]
    fn word_wrap_two_words_split_to_two_lines() {
        assert_eq!(word_wrap("hello world", 8), vec!["hello", "world"]);
    }

    #[test]
    fn word_wrap_long_word_forced_split() {
        assert_eq!(word_wrap("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
    }

    // ── rect_contains ─────────────────────────────────────────────────────────

    #[test]
    fn rect_contains_inside() {
        assert!(rect_contains(0.0, 0.0, 10.0, 10.0, 5.0, 5.0));
    }

    #[test]
    fn rect_contains_left_edge() {
        assert!(rect_contains(0.0, 0.0, 10.0, 10.0, 0.0, 5.0));
    }

    #[test]
    fn rect_contains_right_edge_exclusive() {
        assert!(!rect_contains(0.0, 0.0, 10.0, 10.0, 10.0, 5.0));
    }

    #[test]
    fn rect_contains_outside() {
        assert!(!rect_contains(0.0, 0.0, 10.0, 10.0, 15.0, 5.0));
    }
}

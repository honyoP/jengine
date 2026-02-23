pub mod widgets;

// ── UI types & pure helpers ──────────────────────────────────────────────────

use crate::engine::Color;
use crate::renderer::pipeline::TileVertex;
use crate::renderer::text::{Font, Glyph, Vec2, Vertex, append_text_mesh, text_width};

/// Border style for `ui_box`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BorderStyle {
    /// 1-pixel solid line.
    Thin,
    /// 2-pixel solid line.
    Thick,
    /// Two 1-pixel lines with a gap.
    Double,
}

/// Horizontal or vertical alignment for layout elements.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Alignment {
    /// Align to top / left.
    Start,
    /// Align to center.
    Center,
    /// Align to bottom / right.
    End,
}

/// Inset spacing for layout containers.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Padding {
    pub left:   f32,
    pub right:  f32,
    pub top:    f32,
    pub bottom: f32,
}

impl Padding {
    /// Create padding with equal value on all four sides.
    pub fn all(v: f32) -> Self { Self { left: v, right: v, top: v, bottom: v } }
    /// Create padding with separate horizontal and vertical values.
    pub fn new(h: f32, v: f32) -> Self { Self { left: h, right: h, top: v, bottom: v } }
}

/// A rectangle in screen pixels.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self { Self { x, y, w, h } }
    
    /// Returns true if this rect overlaps with another.
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.x < other.x + other.w &&
        self.x + self.w > other.x &&
        self.y < other.y + other.h &&
        self.y + self.h > other.y
    }
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
    pub dirty: bool,
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
    /// MTSDF text layer: both Labels and immediate-mode ui_char_at writes here.
    pub text: TextLayer,
    /// Current clipping boundary.
    pub(crate) current_clip: Option<Rect>,
    /// Stack of clipping boundaries for nested scrolling.
    pub(crate) clip_stack: Vec<Option<Rect>>,
}

impl UI {
    pub fn new(tile_w: u32, tile_h: u32) -> Self {
        Self {
            ui_vertices: Vec::new(),
            tile_w,
            tile_h,
            text: TextLayer::new(),
            current_clip: None,
            clip_stack: Vec::new(),
        }
    }

    /// Push a new clipping rectangle. Subsequent UI calls will be clipped to this area.
    pub fn push_scissor(&mut self, rect: Rect) {
        self.clip_stack.push(self.current_clip);
        // Intersect with current clip if it exists
        let new_clip = if let Some(old) = self.current_clip {
            let x = old.x.max(rect.x);
            let y = old.y.max(rect.y);
            let w = (old.x + old.w).min(rect.x + rect.w) - x;
            let h = (old.y + old.h).min(rect.y + rect.h) - y;
            Some(Rect::new(x, y, w.max(0.0), h.max(0.0)))
        } else {
            Some(rect)
        };
        self.current_clip = new_clip;
    }

    /// Restore the previous clipping rectangle.
    pub fn pop_scissor(&mut self) {
        self.current_clip = self.clip_stack.pop().flatten();
    }

    pub fn pixel_to_grid(&self, px: f32, py: f32) -> (u32, u32) {
        ((px / self.tile_w as f32) as u32, (py / self.tile_h as f32) as u32)
    }

    // ── UI solid fills (TileVertex, screen-space) ──────────────────────────

    /// Draw a solid-colored rectangle in screen-pixel coordinates.
    pub fn ui_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        let (mut final_x, mut final_y, mut final_w, mut final_h) = (x, y, w, h);

        if let Some(clip) = self.current_clip {
            // Intersection logic
            let x1 = x.max(clip.x);
            let y1 = y.max(clip.y);
            let x2 = (x + w).min(clip.x + clip.w);
            let y2 = (y + h).min(clip.y + clip.h);
            
            final_w = x2 - x1;
            final_h = y2 - y1;
            
            if final_w <= 0.0 || final_h <= 0.0 {
                return;
            }
            final_x = x1;
            final_y = y1;
        }

        let dummy = [0.0f32, 0.0];
        let c = color.0;
        let no_ent = u32::MAX;
        let tl = TileVertex { position: [final_x,           final_y          ], uv: dummy, fg_color: [0.0;4], bg_color: c, entity_id: no_ent, layer_id: 0.0 };
        let tr = TileVertex { position: [final_x + final_w, final_y          ], uv: dummy, fg_color: [0.0;4], bg_color: c, entity_id: no_ent, layer_id: 0.0 };
        let bl = TileVertex { position: [final_x,           final_y + final_h], uv: dummy, fg_color: [0.0;4], bg_color: c, entity_id: no_ent, layer_id: 0.0 };
        let br = TileVertex { position: [final_x + final_w, final_y + final_h], uv: dummy, fg_color: [0.0;4], bg_color: c, entity_id: no_ent, layer_id: 0.0 };
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
        
        if let Some(clip) = self.current_clip {
            // Note: This is an approximation for performance. It culls entire glyphs
            // if their origin is outside the clip.
            if px < clip.x || px >= clip.x + clip.w || py < clip.y || py >= clip.y + clip.h {
                return;
            }
        }
        
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
        let th = self.tile_h as f32;
        let row_h = font_size.unwrap_or(th);
        
        // Calculate max_cols based on the font size being used.
        // For proportional fonts, we use a 0.6× average width factor to estimate
        // how many characters fit in the pixel width.
        let char_w = if font_size.is_some() { row_h * 0.6 } else { self.tile_w as f32 };
        let max_cols = (max_w / char_w) as usize;
        
        let max_rows = (max_h / row_h) as usize;
        for (row, line) in word_wrap(text, max_cols).into_iter().enumerate().take(max_rows) {
            self.ui_text(x, y + row as f32 * row_h, &line, fg, bg, font_size);
        }
    }

    /// Draw a bordered box using solid lines.
    pub fn ui_box(&mut self, x: f32, y: f32, w: f32, h: f32,
                  style: BorderStyle, fg: Color, bg: Color) {
        if bg.0[3] > 0.0 {
            self.ui_rect(x, y, w, h, bg);
        }

        match style {
            BorderStyle::Thin => {
                self.ui_hline(x, y, w, 1.0, fg);         // Top
                self.ui_hline(x, y + h - 1.0, w, 1.0, fg); // Bottom
                self.ui_vline(x, y, h, 1.0, fg);         // Left
                self.ui_vline(x + w - 1.0, y, h, 1.0, fg); // Right
            }
            BorderStyle::Thick => {
                self.ui_hline(x, y, w, 2.0, fg);
                self.ui_hline(x, y + h - 2.0, w, 2.0, fg);
                self.ui_vline(x, y, h, 2.0, fg);
                self.ui_vline(x + w - 2.0, y, h, 2.0, fg);
            }
            BorderStyle::Double => {
                // Outer
                self.ui_hline(x, y, w, 1.0, fg);
                self.ui_hline(x, y + h - 1.0, w, 1.0, fg);
                self.ui_vline(x, y, h, 1.0, fg);
                self.ui_vline(x + w - 1.0, y, h, 1.0, fg);
                // Inner (2px gap)
                let gap = 2.0;
                if w > gap * 2.0 && h > gap * 2.0 {
                    self.ui_hline(x + gap, y + gap, w - gap * 2.0, 1.0, fg);
                    self.ui_hline(x + gap, y + h - gap - 1.0, w - gap * 2.0, 1.0, fg);
                    self.ui_vline(x + gap, y + gap, h - gap * 2.0, 1.0, fg);
                    self.ui_vline(x + w - gap - 1.0, y + gap, h - gap * 2.0, 1.0, fg);
                }
            }
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

    /// Draw a horizontal solid line.
    pub fn ui_hline(&mut self, x: f32, y: f32, w: f32, thickness: f32, fg: Color) {
        self.ui_rect(x, y, w, thickness, fg);
    }

    /// Draw a vertical solid line.
    pub fn ui_vline(&mut self, x: f32, y: f32, h: f32, thickness: f32, fg: Color) {
        self.ui_rect(x, y, thickness, h, fg);
    }

    // ── Debug Shape Helpers ──────────────────────────────────────────────────

    /// Draw a wireframe box (AABB) for debugging.
    pub fn debug_box(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        self.ui_hline(x, y, w, 1.0, color);         // Top
        self.ui_hline(x, y + h - 1.0, w, 1.0, color); // Bottom
        self.ui_vline(x, y, h, 1.0, color);         // Left
        self.ui_vline(x + w - 1.0, y, h, 1.0, color); // Right
    }

    /// Draw a wireframe circle for debugging.
    /// Approximated as 16 evenly-spaced 2×2 dots around the circumference.
    pub fn debug_circle(&mut self, cx: f32, cy: f32, radius: f32, color: Color) {
        let segments = 16;
        for i in 0..segments {
            let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let x = cx + angle.cos() * radius;
            let y = cy + angle.sin() * radius;
            self.ui_rect(x, y, 2.0, 2.0, color);
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────


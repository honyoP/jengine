pub mod widgets;
pub mod modern;

// ── UI types & pure helpers ──────────────────────────────────────────────────

use crate::engine::Color;
use crate::renderer::ui_pipeline::UIVertex;
use crate::renderer::text::{Font, Glyph, Vertex, append_text_mesh, text_width};

/// Border style for `ui_box`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BorderStyle {
    None, Thin, Thick,
}

/// Dynamic UI layers. Higher layers are drawn ON TOP of lower layers.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub enum UILayer {
    Background = 0,
    Main = 1,
    Foreground = 2,
    Modal = 3,
    Overlay = 4,
    Tooltip = 5,
}

impl UILayer {
    pub fn base_depth(&self) -> f32 {
        // Map layers to Z range [0.1, 0.8]. Smaller Z is closer to camera.
        0.8 - (*self as i32 as f32 * 0.12)
    }
}

/// Global visual theme for the UI.
#[derive(Clone, Debug)]
pub struct Theme {
    pub primary: Color,
    pub background: Color,
    pub surface: Color,
    pub text_normal: Color,
    pub text_dim: Color,
    pub text_accent: Color,
    pub border: Color,
    pub border_focus: Color,
    pub selection_bg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary:      Color([0.25, 0.65, 0.50, 1.0]),
            background:   Color([0.05, 0.07, 0.07, 1.0]),
            surface:      Color([0.07, 0.10, 0.10, 1.0]),
            text_normal:  Color([0.85, 0.92, 0.88, 1.0]),
            text_dim:     Color([0.38, 0.48, 0.45, 1.0]),
            text_accent:  Color([0.95, 0.85, 0.40, 1.0]),
            border:       Color([0.25, 0.65, 0.50, 1.0]),
            border_focus: Color([0.45, 0.90, 0.72, 1.0]),
            selection_bg: Color([0.08, 0.25, 0.18, 1.0]),
        }
    }
}

/// Common styling properties for widgets.
#[derive(Clone, Debug, Default)]
pub struct Style {
    pub padding: Padding,
    pub bg_color: Option<Color>,
    pub border: Option<(BorderStyle, Color)>,
    pub radius: f32,
    pub alignment: Option<Alignment>,
}

/// A segment of text with its own color.
pub struct RichTextSegment {
    pub text: String,
    pub color: Color,
}

/// Parse markdown-like tags `[c:color]text[/c]` into segments.
pub fn parse_rich_text(input: &str, default_color: Color) -> Vec<RichTextSegment> {
    let mut segments = Vec::new();
    let mut current_pos = 0;
    while let Some(start_tag_pos) = input[current_pos..].find("[c:") {
        let start_tag_pos = current_pos + start_tag_pos;
        if start_tag_pos > current_pos { segments.push(RichTextSegment { text: input[current_pos..start_tag_pos].to_string(), color: default_color }); }
        let tag_end_pos = match input[start_tag_pos..].find(']') { Some(pos) => start_tag_pos + pos, None => break };
        let color_name = &input[start_tag_pos + 3..tag_end_pos];
        let color = match color_name {
            "green" => Color::GREEN, "red" => Color::RED, "blue" => Color::BLUE, "yellow" => Color::YELLOW,
            "cyan" => Color::CYAN, "magenta" => Color::MAGENTA, "white" => Color::WHITE, "gray" => Color::GRAY,
            "gold" => Color([0.95, 0.85, 0.40, 1.0]), "dim" => Color([0.38, 0.44, 0.44, 1.0]), "orange" => Color([1.0, 0.5, 0.0, 1.0]),
            _ => default_color,
        };
        let content_start = tag_end_pos + 1;
        match input[content_start..].find("[/c]") {
            Some(end_tag_relative) => {
                let end_tag_pos = content_start + end_tag_relative;
                segments.push(RichTextSegment { text: input[content_start..end_tag_pos].to_string(), color });
                current_pos = end_tag_pos + 4;
            }
            None => { segments.push(RichTextSegment { text: input[content_start..].to_string(), color }); current_pos = input.len(); break; }
        }
    }
    if current_pos < input.len() { segments.push(RichTextSegment { text: input[current_pos..].to_string(), color: default_color }); }
    segments
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Alignment { Start, Center, End }

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Padding { pub left: f32, pub right: f32, pub top: f32, pub bottom: f32 }
impl Padding {
    pub fn all(v: f32) -> Self { Self { left: v, right: v, top: v, bottom: v } }
    pub fn new(h: f32, v: f32) -> Self { Self { left: h, right: h, top: v, bottom: v } }
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct Rect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }
impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self { Self { x, y, w, h } }
    pub fn overlaps(&self, other: &Rect) -> bool { self.x < other.x + other.w && self.x + self.w > other.x && self.y < other.y + other.h && self.y + self.h > other.y }
}

pub fn word_wrap(text: &str, max_cols: usize) -> Vec<String> {
    if max_cols == 0 { return vec![]; }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let space = if current.is_empty() { 0 } else { 1 };
        if !current.is_empty() && current.len() + space + word.len() > max_cols { lines.push(std::mem::take(&mut current)); }
        if !current.is_empty() { current.push(' '); }
        current.push_str(word);
        while current.len() > max_cols { lines.push(current[..max_cols].to_string()); current = current[max_cols..].to_string(); }
    }
    if !current.is_empty() { lines.push(current); }
    lines
}

pub fn rect_contains(rx: f32, ry: f32, rw: f32, rh: f32, px: f32, py: f32) -> bool {
    px >= rx && px < rx + rw && py >= ry && py < ry + rh
}

#[inline]
fn emit_glyph(vertices: &mut Vec<Vertex>, indices: &mut Vec<u16>, px: f32, py: f32, z: f32, glyph: Glyph, atlas_w: f32, atlas_h: f32, ascender: f32, color: [f32; 4], font_size: f32) {
    if glyph.atlas_right <= glyph.atlas_left || glyph.atlas_bottom <= glyph.atlas_top { return; }
    let baseline_y = py + (-ascender) * font_size;
    let x0 = px + glyph.plane_left * font_size; let x1 = px + glyph.plane_right * font_size;
    let y0 = baseline_y + glyph.plane_top * font_size; let y1 = baseline_y + glyph.plane_bottom * font_size;
    let u0 = glyph.atlas_left / atlas_w; let u1 = glyph.atlas_right / atlas_w;
    let v0 = glyph.atlas_top / atlas_h; let v1 = glyph.atlas_bottom / atlas_h;
    let base = vertices.len() as u16;
    vertices.push(Vertex { position: [x0, y0, z], tex_coords: [u0, v0], color });
    vertices.push(Vertex { position: [x1, y0, z], tex_coords: [u1, v0], color });
    vertices.push(Vertex { position: [x0, y1, z], tex_coords: [u0, v1], color });
    vertices.push(Vertex { position: [x1, y1, z], tex_coords: [u1, v1], color });
    indices.extend_from_slice(&[base, base + 1, base + 2, base + 1, base + 3, base + 2]);
}

// ── TextLayer ─────────────────────────────────────────────────────────────────

pub struct TextLayer {
    pub font: Option<Font>,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
}

impl TextLayer {
    pub fn new() -> Self { Self { font: None, vertices: Vec::new(), indices: Vec::new() } }
    pub fn set_font(&mut self, font: Font) { self.font = Some(font); }
    pub fn clear(&mut self) { self.vertices.clear(); self.indices.clear(); }
    pub fn push_char(&mut self, px: f32, py: f32, z: f32, ch: char, color: [f32; 4], font_size: f32) {
        let Some((glyph, aw, ah, ascender)) = self.font.as_ref().and_then(|f| f.glyphs.get(&ch).map(|g| (*g, f.atlas_width as f32, f.atlas_height as f32, f.ascender))) else { return };
        emit_glyph(&mut self.vertices, &mut self.indices, px, py, z, glyph, aw, ah, ascender, color, font_size);
    }
}

// ── Label ──────────────────────────────────────────────────────────────────────

pub struct Label {
    text: String, position: [f32; 2], color: [f32; 4], pub font_size: f32, vertices: Vec<Vertex>, indices: Vec<u16>, pub dirty: bool,
}

impl Label {
    pub fn new(pos: [f32; 2], font_size: f32, color: [f32; 4]) -> Self { Self { text: String::new(), position: pos, color, font_size, vertices: Vec::new(), indices: Vec::new(), dirty: false } }
    pub fn set_text(&mut self, text: &str) { if self.text != text { self.text = text.to_owned(); self.dirty = true; } }
    pub fn set_position(&mut self, pos: [f32; 2]) { if self.position != pos { self.position = pos; self.dirty = true; } }
    pub fn set_color(&mut self, color: [f32; 4]) { if self.color != color { self.color = color; self.dirty = true; } }
    pub fn set_font_size(&mut self, font_size: f32) { if self.font_size != font_size { self.font_size = font_size; self.dirty = true; } }
    pub fn text(&self) -> &str { &self.text }
    pub fn draw(&mut self, layer: &mut TextLayer) {
        if self.dirty {
            if let Some(font) = layer.font.as_ref() {
                self.vertices.clear(); self.indices.clear();
                append_text_mesh(&self.text, font, self.position, self.font_size, self.color, &mut self.vertices, &mut self.indices);
                for v in &mut self.vertices { v.position[2] = 0.1; } // UI Text depth
                self.dirty = false;
            }
        }
        let base = layer.vertices.len() as u16;
        layer.vertices.extend_from_slice(&self.vertices);
        layer.indices.extend(self.indices.iter().map(|&i| i + base));
    }
}

// ── UI ────────────────────────────────────────────────────────────────────────

pub struct UI {
    pub ui_vertices: Vec<UIVertex>,
    pub tile_w: u32,
    pub tile_h: u32,
    pub theme: Theme,
    pub text: TextLayer,
    pub(crate) current_clip: Option<Rect>,
    pub(crate) clip_stack: Vec<Option<Rect>>,
    pub(crate) current_layer: UILayer,
    pub(crate) layer_stack: Vec<UILayer>,
    pub(crate) draw_count: u32,
}

impl UI {
    pub fn new(tile_w: u32, tile_h: u32) -> Self { Self { ui_vertices: Vec::new(), tile_w, tile_h, theme: Theme::default(), text: TextLayer::new(), current_clip: None, clip_stack: Vec::new(), current_layer: UILayer::Main, layer_stack: Vec::new(), draw_count: 0 } }
    
    pub fn clear(&mut self) {
        self.ui_vertices.clear();
        self.text.clear();
        self.draw_count = 0;
        self.current_layer = UILayer::Main;
        self.layer_stack.clear();
    }

    pub fn push_scissor(&mut self, rect: Rect) {
        self.clip_stack.push(self.current_clip);
        let new_clip = if let Some(old) = self.current_clip {
            let x = old.x.max(rect.x); let y = old.y.max(rect.y);
            let w = (old.x + old.w).min(rect.x + rect.w) - x;
            let h = (old.y + old.h).min(rect.y + rect.h) - y;
            Some(Rect::new(x, y, w.max(0.0), h.max(0.0)))
        } else { Some(rect) };
        self.current_clip = new_clip;
    }
    pub fn pop_scissor(&mut self) { self.current_clip = self.clip_stack.pop().flatten(); }

    pub fn set_layer(&mut self, layer: UILayer) { self.current_layer = layer; }
    pub fn push_layer(&mut self, layer: UILayer) { self.layer_stack.push(self.current_layer); self.current_layer = layer; }
    pub fn pop_layer(&mut self) { self.current_layer = self.layer_stack.pop().unwrap_or(UILayer::Main); }

    pub(crate) fn get_next_z(&mut self) -> f32 {
        self.draw_count += 1;
        self.current_layer.base_depth() - (self.draw_count as f32 * 0.00001)
    }

    // ── Modern UI SDF Primitives ──────────────────────────────────────────

    pub fn ui_panel(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color, border: Color, thickness: f32, radius: [f32; 4], mode: u32, param: f32) {
        let z = self.get_next_z();
        // Encode the current scissor clip as a per-vertex rect so the fragment shader
        // can discard pixels outside it. When there is no active clip, use a sentinel
        // that covers the entire screen (effectively no clipping).
        let clip_rect = match self.current_clip {
            Some(r) => [r.x, r.y, r.x + r.w, r.y + r.h],
            None    => [-1.0e6_f32, -1.0e6, 1.0e6, 1.0e6],
        };
        let tl = UIVertex { position: [x,     y,     z], rect_size: [w, h], rect_coord: [0.0, 0.0], color: color.0, border_color: border.0, radius, border_thickness: thickness, shadow_blur: 0.0, mode, mode_param: param, clip_rect };
        let tr = UIVertex { position: [x + w, y,     z], rect_size: [w, h], rect_coord: [1.0, 0.0], color: color.0, border_color: border.0, radius, border_thickness: thickness, shadow_blur: 0.0, mode, mode_param: param, clip_rect };
        let bl = UIVertex { position: [x,     y + h, z], rect_size: [w, h], rect_coord: [0.0, 1.0], color: color.0, border_color: border.0, radius, border_thickness: thickness, shadow_blur: 0.0, mode, mode_param: param, clip_rect };
        let br = UIVertex { position: [x + w, y + h, z], rect_size: [w, h], rect_coord: [1.0, 1.0], color: color.0, border_color: border.0, radius, border_thickness: thickness, shadow_blur: 0.0, mode, mode_param: param, clip_rect };
        self.ui_vertices.extend_from_slice(&[tl, bl, tr, tr, bl, br]);
    }

    pub fn ui_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) { self.ui_panel(x, y, w, h, color, Color::TRANSPARENT, 0.0, [0.0; 4], 0, 0.0); }
    pub fn ui_box(&mut self, x: f32, y: f32, w: f32, h: f32, style: BorderStyle, fg: Color, bg: Color) {
        let thick = match style { BorderStyle::None => 0.0, BorderStyle::Thin => 1.0, BorderStyle::Thick => 2.0 };
        self.ui_panel(x, y, w, h, bg, fg, thick, [0.0; 4], 0, 0.0);
    }
    pub fn ui_pattern(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color, scale: f32) { self.ui_panel(x, y, w, h, color, Color::TRANSPARENT, 0.0, [0.0; 4], 1, scale); }

    pub fn ui_text(&mut self, x: f32, y: f32, text: &str, fg: Color, bg: Color, font_size: Option<f32>) {
        let segments = parse_rich_text(text, fg);
        let mut cursor_x = x; let fs = font_size.unwrap_or(self.tile_h as f32); let tw = self.tile_w as f32;
        if bg.0[3] > 0.0 {
            let mut total_w = 0.0;
            for seg in &segments { total_w += self.text.font.as_ref().map(|f| text_width(&seg.text, f, fs)).unwrap_or(seg.text.len() as f32 * tw); }
            self.ui_rect(x, y, total_w, fs, bg);
        }
        let z = self.get_next_z() - 0.000001;
        let mut glyphs = Vec::new(); let mut prev = None;
        for seg in segments {
            for ch in seg.text.chars() {
                if let Some(font) = &self.text.font {
                    if let Some(p) = prev { if let Some(&k) = font.kerning.get(&(p, ch)) { cursor_x += k * fs; } }
                    glyphs.push((cursor_x, ch, seg.color)); cursor_x += font.glyphs.get(&ch).map(|g| g.x_advance).unwrap_or(0.6) * fs;
                } else { glyphs.push((cursor_x, ch, seg.color)); cursor_x += tw; }
                prev = Some(ch);
            }
        }
        for (px, ch, color) in glyphs {
            // CPU-side cull: skip glyphs that fall entirely outside the active clip rect.
            // This is a conservative check using font_size as an upper-bound glyph width.
            if let Some(clip) = self.current_clip {
                if px + fs < clip.x || px > clip.x + clip.w || y + fs < clip.y || y > clip.y + clip.h {
                    continue;
                }
            }
            self.text.push_char(px, y, z, ch, color.0, fs);
        }
    }

    pub fn ui_text_wrapped(&mut self, x: f32, y: f32, max_w: f32, max_h: f32, text: &str, fg: Color, bg: Color, font_size: Option<f32>) {
        let fs = font_size.unwrap_or(self.tile_h as f32); let max_cols = (max_w / (fs * 0.6)) as usize;
        let stripped = text.replace("[/c]", "").split("[c:").map(|s| if let Some(pos) = s.find(']') { &s[pos+1..] } else { s }).collect::<String>();
        for (row, line) in word_wrap(&stripped, max_cols).into_iter().enumerate() {
            if (row as f32 + 1.0) * fs > max_h { break; }
            self.ui_text(x, y + row as f32 * fs, &line, fg, bg, font_size);
        }
    }

    pub fn ui_hline(&mut self, x: f32, y: f32, w: f32, thickness: f32, color: Color) { self.ui_rect(x, y, w, thickness, color); }
    pub fn ui_vline(&mut self, x: f32, y: f32, h: f32, thickness: f32, color: Color) { self.ui_rect(x, y, thickness, h, color); }
    pub fn ui_progress_bar(&mut self, x: f32, y: f32, w: f32, h: f32, pct: f32, filled: Color, empty: Color) {
        let fw = w * pct.clamp(0.0, 1.0); if fw > 0.0 { self.ui_rect(x, y, fw, h, filled); }
        if fw < w { self.ui_rect(x + fw, y, w - fw, h, empty); }
    }
    pub fn debug_box(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) { self.ui_panel(x, y, w, h, Color::TRANSPARENT, color, 1.0, [0.0; 4], 0, 0.0); }
}

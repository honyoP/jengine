use std::collections::HashMap;

use serde::Deserialize;

// ── Vec2 / Vertex ─────────────────────────────────────────────────────────────

/// Screen-space 2D position `[x, y]` in pixels.
pub type Vec2 = [f32; 2];

/// A single vertex produced by [`generate_text_mesh`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    /// Screen-space position in pixels.
    pub position: [f32; 2],
    /// Normalised atlas texture coordinates in `[0, 1]`.
    pub tex_coords: [f32; 2],
}

// ── generate_text_mesh ────────────────────────────────────────────────────────

/// Convert `text` into a flat vertex + index buffer.
///
/// # Layout
/// Each renderable character produces **4 vertices** and **6 indices**
/// (two counter-clockwise triangles, Y-axis pointing down):
///
/// ```text
/// 0──1
/// │ /│
/// 2──3
/// triangles: (0,1,2) and (1,3,2)
/// ```
///
/// # Parameters
/// - `start_pos` — top-left origin of the text block in screen pixels.
/// - `font_size` — desired line height in pixels.  All glyph dimensions are
///   scaled uniformly by `font_size / font.line_height`.
///
/// # Skipping rules
/// - `'\n'` resets the X cursor to `start_pos.x` and advances Y by one
///   scaled line height; it produces no geometry.
/// - Characters absent from `font.glyphs` are silently skipped.
/// - Returns empty buffers when `font.line_height` is zero.
pub fn generate_text_mesh(
    text: &str,
    font: &Font,
    start_pos: Vec2,
    font_size: f32,
) -> (Vec<Vertex>, Vec<u32>) {
    if font.line_height == 0 {
        return (Vec::new(), Vec::new());
    }

    let scale = font_size / font.line_height as f32;
    let tw = font.texture_width as f32;
    let th = font.texture_height as f32;

    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    let mut current_x = start_pos[0];
    let mut current_y = start_pos[1];

    for ch in text.chars() {
        if ch == '\n' {
            current_x = start_pos[0];
            current_y += font.line_height as f32 * scale;
            continue;
        }

        let Some(glyph) = font.glyphs.get(&ch) else {
            continue;
        };

        let quad_x = current_x + glyph.x_offset as f32 * scale;
        let quad_y = current_y + glyph.y_offset as f32 * scale;
        let quad_w = glyph.width as f32 * scale;
        let quad_h = glyph.height as f32 * scale;

        let uv_x0 = glyph.x as f32 / tw;
        let uv_y0 = glyph.y as f32 / th;
        let uv_x1 = (glyph.x + glyph.width) as f32 / tw;
        let uv_y1 = (glyph.y + glyph.height) as f32 / th;

        let base = vertices.len() as u32;

        // Four corners in reading order: top-left, top-right, bottom-left, bottom-right.
        vertices.push(Vertex { position: [quad_x,          quad_y         ], tex_coords: [uv_x0, uv_y0] });
        vertices.push(Vertex { position: [quad_x + quad_w, quad_y         ], tex_coords: [uv_x1, uv_y0] });
        vertices.push(Vertex { position: [quad_x,          quad_y + quad_h], tex_coords: [uv_x0, uv_y1] });
        vertices.push(Vertex { position: [quad_x + quad_w, quad_y + quad_h], tex_coords: [uv_x1, uv_y1] });

        // Two CCW triangles (Y-down): TL-TR-BL, TR-BR-BL.
        indices.extend_from_slice(&[base, base + 1, base + 2, base + 1, base + 3, base + 2]);

        current_x += glyph.x_advance as f32 * scale;
    }

    (vertices, indices)
}

// ── Glyph ────────────────────────────────────────────────────────────────────

/// Metrics for a single character in the bitmap font atlas.
#[derive(Debug, Clone)]
pub struct Glyph {
    /// The Unicode character this glyph represents.
    pub id: char,
    /// Top-left pixel X of the glyph region in the atlas.
    pub x: u32,
    /// Top-left pixel Y of the glyph region in the atlas.
    pub y: u32,
    /// Pixel width of the glyph region.
    pub width: u32,
    /// Pixel height of the glyph region.
    pub height: u32,
    /// Horizontal offset applied when rendering (may be negative).
    pub x_offset: i32,
    /// Vertical offset applied when rendering (may be negative).
    pub y_offset: i32,
    /// How far to advance the cursor after drawing this glyph.
    pub x_advance: u32,
}

// ── Font ─────────────────────────────────────────────────────────────────────

/// A bitmap font loaded from a JSON descriptor.
///
/// Atlas dimensions are stored so callers can normalise pixel coordinates to
/// UV coordinates: `u = glyph.x as f32 / font.texture_width as f32`.
pub struct Font {
    /// All glyphs in this font, keyed by character.
    pub glyphs: HashMap<char, Glyph>,
    /// Vertical distance between successive baselines in pixels.
    pub line_height: u32,
    /// Width of the backing texture atlas in pixels.
    pub texture_width: u32,
    /// Height of the backing texture atlas in pixels.
    pub texture_height: u32,
}

impl Font {
    /// Deserialise a `Font` from a JSON string.
    ///
    /// Returns a `serde_json::Error` if the input is malformed or missing
    /// required fields.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let raw: RawFont = serde_json::from_str(json)?;

        let glyphs = raw
            .glyphs
            .into_iter()
            .filter_map(|g| {
                // Skip any code-point that isn't a valid Unicode scalar value.
                char::from_u32(g.id).map(|ch| {
                    (ch, Glyph {
                        id: ch,
                        x: g.x,
                        y: g.y,
                        width: g.width,
                        height: g.height,
                        x_offset: g.x_offset,
                        y_offset: g.y_offset,
                        x_advance: g.x_advance,
                    })
                })
            })
            .collect();

        Ok(Self {
            glyphs,
            line_height: raw.line_height,
            texture_width: raw.texture_width,
            texture_height: raw.texture_height,
        })
    }

    /// Deserialise a `Font` from the built-in **atlas JSON** format, where each
    /// key is a single character and the value is a pixel rectangle in the atlas:
    ///
    /// ```json
    /// { "A": { "x": 0, "y": 0, "w": 16, "h": 24, "index": 0 }, ... }
    /// ```
    ///
    /// `texture_width` / `texture_height` are the backing texture dimensions in
    /// pixels, needed so callers can compute normalised UV coordinates.
    /// `x_advance` defaults to the glyph width; `x_offset` and `y_offset` are
    /// both zero (suits uniform-grid atlas fonts).
    pub fn from_atlas_json(
        json: &str,
        texture_width: u32,
        texture_height: u32,
    ) -> Result<Self, serde_json::Error> {
        #[derive(Deserialize)]
        struct AtlasEntry { x: u32, y: u32, w: u32, h: u32 }

        let raw: HashMap<String, AtlasEntry> = serde_json::from_str(json)?;

        let line_height = raw.values().map(|e| e.h).max().unwrap_or(0);

        let glyphs = raw
            .into_iter()
            .filter_map(|(key, entry)| {
                // Only accept single-character keys.
                let mut chars = key.chars();
                let ch = chars.next()?;
                if chars.next().is_some() { return None; }

                Some((ch, Glyph {
                    id: ch,
                    x: entry.x,
                    y: entry.y,
                    width:    entry.w,
                    height:   entry.h,
                    x_offset: 0,
                    y_offset: 0,
                    x_advance: entry.w,
                }))
            })
            .collect();

        Ok(Self { glyphs, line_height, texture_width, texture_height })
    }
}  // end impl Font

// ── Raw (JSON-facing) types ───────────────────────────────────────────────────
//
// Character IDs are stored as u32 in JSON (Unicode code points); we convert
// them to `char` when building the public `Font`.

#[derive(Deserialize)]
struct RawGlyph {
    /// Unicode code point (e.g. 65 for 'A').
    id: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    x_offset: i32,
    y_offset: i32,
    x_advance: u32,
}

#[derive(Deserialize)]
struct RawFont {
    line_height: u32,
    texture_width: u32,
    texture_height: u32,
    glyphs: Vec<RawGlyph>,
}

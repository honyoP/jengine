use std::collections::HashMap;

use serde::Deserialize;

// ── Vec2 / Vertex ─────────────────────────────────────────────────────────────

/// Screen-space 2D position `[x, y]` in pixels.
pub type Vec2 = [f32; 2];

/// A single vertex produced by [`generate_text_mesh`].
///
/// Carries a per-vertex RGBA colour so multiple labels with different tints
/// can share a single draw call.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    /// Screen-space position in pixels (x, y, depth).
    pub position: [f32; 3],
    /// Normalised atlas texture coordinates in `[0, 1]`.
    pub tex_coords: [f32; 2],
    /// RGBA tint for this vertex.
    pub color: [f32; 4],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x3,  // position (x, y, z)
        1 => Float32x2,  // tex_coords
        2 => Float32x4,  // color
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

// ── Glyph ─────────────────────────────────────────────────────────────────────

/// Metrics for a single glyph in an MTSDF font atlas.
#[derive(Debug, Clone, Copy)]
pub struct Glyph {
    // ── Atlas bounds (pixel coordinates in the atlas texture) ──────────────
    pub atlas_left:   f32,
    pub atlas_top:    f32,
    pub atlas_right:  f32,
    pub atlas_bottom: f32,

    // ── Plane bounds (em units; multiply by font_size for screen pixels) ───
    //
    // Y axis convention: origin = baseline, positive = below baseline (screen Y-down).
    // A capital letter like 'A' has a negative plane_top (above baseline).
    pub plane_left:   f32,
    pub plane_top:    f32,
    pub plane_right:  f32,
    pub plane_bottom: f32,

    /// Horizontal advance in em units.
    pub x_advance: f32,
}

// ── Font ──────────────────────────────────────────────────────────────────────

/// An MTSDF font loaded from an msdf-atlas-gen JSON descriptor.
pub struct Font {
    /// All glyphs in this font, keyed by character.
    pub glyphs: HashMap<char, Glyph>,

    /// Vertical distance between successive baselines in em units.
    pub line_height: f32,

    /// Distance from the baseline to the cap-height / top-of-em-square.
    /// **Negative** in this coordinate system (above baseline = negative Y-down).
    /// e.g. `-0.928` means caps reach 0.928 em above the baseline.
    pub ascender: f32,

    /// Distance from the baseline downward to the bottom of descenders.
    /// **Positive** (below baseline).
    pub descender: f32,

    /// Width of the atlas texture in pixels.
    pub atlas_width: u32,

    /// Height of the atlas texture in pixels.
    pub atlas_height: u32,

    /// MTSDF distance range (in pixels at the atlas native size).
    /// Passed to the shader for correct anti-aliasing.
    pub distance_range: f32,

    /// Kerning pairs `(left, right) → advance adjustment` in em units.
    pub kerning: HashMap<(char, char), f32>,
}

impl Font {
    /// Parse a `Font` from an **msdf-atlas-gen** JSON string.
    ///
    /// Expected top-level fields: `atlas`, `metrics`, `glyphs`, `kerning`.
    pub fn from_mtsdf_json(json: &str) -> Result<Self, serde_json::Error> {
        let raw: RawMtsdfFont = serde_json::from_str(json)?;

        let glyphs = raw
            .glyphs
            .into_iter()
            .filter_map(|g| {
                let ch = char::from_u32(g.unicode)?;
                let (pl, pt, pr, pb) = g
                    .plane_bounds
                    .map(|b| (b.left, b.top, b.right, b.bottom))
                    .unwrap_or((0.0, 0.0, 0.0, 0.0));
                let (al, at, ar, ab) = g
                    .atlas_bounds
                    .map(|b| (b.left, b.top, b.right, b.bottom))
                    .unwrap_or((0.0, 0.0, 0.0, 0.0));
                Some((
                    ch,
                    Glyph {
                        atlas_left: al, atlas_top: at,
                        atlas_right: ar, atlas_bottom: ab,
                        plane_left: pl, plane_top: pt,
                        plane_right: pr, plane_bottom: pb,
                        x_advance: g.advance,
                    },
                ))
            })
            .collect();

        let kerning = raw
            .kerning
            .into_iter()
            .filter_map(|k| {
                let l = char::from_u32(k.unicode1)?;
                let r = char::from_u32(k.unicode2)?;
                Some(((l, r), k.advance))
            })
            .collect();

        Ok(Self {
            glyphs,
            line_height: raw.metrics.line_height,
            // Normalise sign convention: store ascender as negative (above baseline
            // in Y-down space) and descender as positive (below baseline).
            // msdf-atlas-gen v1 emits ascender as positive and descender as negative,
            // while other tools may use the opposite sign. Taking abs() and negating
            // for ascender (and abs() for descender) accepts both conventions.
            ascender:  -raw.metrics.ascender.abs(),
            descender:  raw.metrics.descender.abs(),
            atlas_width: raw.atlas.width,
            atlas_height: raw.atlas.height,
            distance_range: raw.atlas.distance_range,
            kerning,
        })
    }
}

// ── append_text_mesh / generate_text_mesh ────────────────────────────────────

/// Append `text` glyph geometry directly into caller-supplied vertex/index buffers.
///
/// This is the core layout engine. [`generate_text_mesh`] is a thin wrapper that
/// allocates fresh buffers and delegates here.
///
/// # Layout
/// Each renderable glyph produces 4 vertices and 6 indices (two CCW triangles,
/// Y-axis pointing down):
///
/// ```text
/// 0──1
/// │ /│
/// 2──3
/// triangles: (0,1,2) and (1,3,2)
/// ```
///
/// # Parameters
/// - `start_pos` — top-left corner of the text block in screen pixels.
///   The **baseline** of the first line is placed at
///   `start_pos.y + (-font.ascender) * font_size`.
/// - `font_size` — desired em-height in pixels (1 em = `font_size` px).
/// - `color` — RGBA tint written into every vertex.
/// - `vertices` / `indices` — buffers to append into. Existing content is
///   preserved; new index values are offset by `vertices.len()` at call time.
///
/// # Skipping rules
/// - `'\n'` resets the X cursor and advances the baseline by `line_height * font_size`.
/// - Characters absent from `font.glyphs` are silently skipped (cursor not advanced).
/// - Glyphs whose `atlas_bounds` have zero area (e.g. space) produce no geometry
///   but still advance the cursor by `x_advance`.
pub fn append_text_mesh(
    text: &str,
    font: &Font,
    start_pos: Vec2,
    font_size: f32,
    color: [f32; 4],
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
) {
    if font_size <= 0.0 {
        return;
    }
    // The baseline sits below start_pos.y by |ascender| * font_size.
    // ascender is negative (e.g. -0.928), so -ascender is positive.
    let baseline_y = start_pos[1] + (-font.ascender) * font_size;
    append_text_mesh_at_baseline(text, font, start_pos[0], baseline_y, font_size, color, vertices, indices);
}

/// Like [`append_text_mesh`] but positions the text at an **explicit baseline Y**
/// coordinate rather than computing it from a top-left corner.
///
/// Use this when you want multiple runs of different `font_size`s to share the
/// same typographic baseline — simply pass the same `baseline_y` to all calls:
///
/// ```text
/// append_text_mesh_at_baseline("Label: ", font, cursor, baseline, small_sz, …, verts, idxs);
/// append_text_mesh_at_baseline("42",      font, cursor, baseline, big_sz,   …, verts, idxs);
/// ```
///
/// Both runs will sit on the same baseline regardless of font size.
///
/// # Parameters
/// - `cursor_x`   — left edge of the run in screen pixels.
/// - `baseline_y` — Y coordinate of the typographic baseline in screen pixels.
/// - `font_size`  — em-height in pixels (1 em = `font_size` px).
pub fn append_text_mesh_at_baseline(
    text: &str,
    font: &Font,
    cursor_x: f32,
    baseline_y: f32,
    font_size: f32,
    color: [f32; 4],
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u16>,
) {
    if font_size <= 0.0 {
        return;
    }

    let aw = font.atlas_width as f32;
    let ah = font.atlas_height as f32;

    let mut x      = cursor_x;
    let mut base_y = baseline_y;
    let mut prev_char: Option<char> = None;

    for ch in text.chars() {
        if ch == '\n' {
            x       = cursor_x;
            base_y += font.line_height * font_size;
            prev_char = None;
            continue;
        }

        let Some(glyph) = font.glyphs.get(&ch) else { continue };

        if let Some(prev) = prev_char {
            if let Some(&kern) = font.kerning.get(&(prev, ch)) {
                x += kern * font_size;
            }
        }

        if glyph.atlas_right > glyph.atlas_left && glyph.atlas_bottom > glyph.atlas_top {
            let x0 = x + glyph.plane_left   * font_size;
            let x1 = x + glyph.plane_right  * font_size;
            let y0 = base_y + glyph.plane_top    * font_size;
            let y1 = base_y + glyph.plane_bottom * font_size;

            let u0 = glyph.atlas_left   / aw;
            let u1 = glyph.atlas_right  / aw;
            let v0 = glyph.atlas_top    / ah;
            let v1 = glyph.atlas_bottom / ah;

            debug_assert!(
                vertices.len() < u16::MAX as usize - 3,
                "text vertex buffer overflow: too many glyphs in one draw call"
            );
            let base = vertices.len() as u16;

            vertices.push(Vertex { position: [x0, y0, 0.1], tex_coords: [u0, v0], color });
            vertices.push(Vertex { position: [x1, y0, 0.1], tex_coords: [u1, v0], color });
            vertices.push(Vertex { position: [x0, y1, 0.1], tex_coords: [u0, v1], color });
            vertices.push(Vertex { position: [x1, y1, 0.1], tex_coords: [u1, v1], color });

            indices.extend_from_slice(&[base, base + 1, base + 2, base + 1, base + 3, base + 2]);
        }

        x += glyph.x_advance * font_size;
        prev_char = Some(ch);
    }
}

/// Convert `text` into a fresh vertex + index buffer using MTSDF font metrics.
///
/// Allocates and returns new `Vec`s. Prefer [`append_text_mesh`] in hot paths
/// where you already own the destination buffers.
pub fn generate_text_mesh(
    text: &str,
    font: &Font,
    start_pos: Vec2,
    font_size: f32,
    color: [f32; 4],
) -> (Vec<Vertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices  = Vec::new();
    append_text_mesh(text, font, start_pos, font_size, color, &mut vertices, &mut indices);
    (vertices, indices)
}

/// Compute the total pixel width a string would occupy at `font_size` without
/// generating any geometry.
///
/// - Kerning between adjacent characters is applied.
/// - Characters absent from `font.glyphs` are silently skipped (cursor not advanced).
/// - For multi-line strings (`'\n'`), returns the width of the **widest line**.
/// - Returns `0.0` when `text` is empty or `font_size <= 0`.
pub fn text_width(text: &str, font: &Font, font_size: f32) -> f32 {
    if font_size <= 0.0 {
        return 0.0;
    }
    let mut max_width  = 0.0f32;
    let mut line_width = 0.0f32;
    let mut prev_char: Option<char> = None;

    for ch in text.chars() {
        if ch == '\n' {
            max_width  = max_width.max(line_width);
            line_width = 0.0;
            prev_char  = None;
            continue;
        }
        let Some(glyph) = font.glyphs.get(&ch) else { continue };
        if let Some(prev) = prev_char {
            if let Some(&kern) = font.kerning.get(&(prev, ch)) {
                line_width += kern * font_size;
            }
        }
        line_width += glyph.x_advance * font_size;
        prev_char = Some(ch);
    }
    max_width.max(line_width)
}

// ── Raw (JSON-facing) types ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct RawAtlas {
    width:  u32,
    height: u32,
    #[serde(rename = "distanceRange")]
    distance_range: f32,
}

#[derive(Deserialize)]
struct RawMetrics {
    #[serde(rename = "lineHeight")]
    line_height: f32,
    ascender:  f32,
    descender: f32,
}

#[derive(Deserialize)]
struct RawBounds {
    left:   f32,
    top:    f32,
    right:  f32,
    bottom: f32,
}

#[derive(Deserialize)]
struct RawGlyph {
    unicode: u32,
    advance: f32,
    #[serde(rename = "planeBounds")]
    plane_bounds: Option<RawBounds>,
    #[serde(rename = "atlasBounds")]
    atlas_bounds: Option<RawBounds>,
}

#[derive(Deserialize)]
struct RawKerning {
    unicode1: u32,
    unicode2: u32,
    advance:  f32,
}

#[derive(Deserialize)]
struct RawMtsdfFont {
    atlas:   RawAtlas,
    metrics: RawMetrics,
    glyphs:  Vec<RawGlyph>,
    // Some tools omit the kerning array when there are no pairs; default to empty.
    #[serde(default)]
    kerning: Vec<RawKerning>,
}

// ── text_shader.wgsl ──────────────────────────────────────────────────────────
//
// MTSDF font atlas shader.
//
// Reconstructs signed distances from the three MSDF channels (RGB), then uses
// the screen-space derivative (`fwidth`) scaled by the atlas distance range for
// pixel-perfect anti-aliasing at any scale.  The alpha channel is unused
// (reserved for glow/shadow effects; in MTSDF it holds the true SDF).
//
// Bind group layout:
//   group(0) binding(0) — orthographic projection matrix  (uniform mat4x4)
//   group(1) binding(0) — MTSDF font atlas texture        (texture_2d<f32>)
//   group(1) binding(1) — font atlas sampler (Linear)     (sampler)
//   group(1) binding(2) — MTSDF render parameters         (uniform MtsdfParams)
//
// Vertex layout matches `renderer::text::Vertex`:
//   location(0) position:   vec2<f32>  — screen-space pixels
//   location(1) tex_coords: vec2<f32>  — normalised atlas UVs [0, 1]
//   location(2) color:      vec4<f32>  — per-vertex RGBA tint

// ── Structs ───────────────────────────────────────────────────────────────────

struct VertexInput {
    @location(0) position:   vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) color:      vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0)       uv:            vec2<f32>,
    @location(1)       color:         vec4<f32>,
};

/// Per-font parameters required for correct SDF anti-aliasing.
struct MtsdfParams {
    /// MTSDF distance range in atlas texels (from msdf-atlas-gen `distanceRange`).
    distance_range: f32,
    /// Atlas texture width in pixels.
    atlas_width:    f32,
    /// Atlas texture height in pixels.
    atlas_height:   f32,
    _pad:           f32,
};

// ── Bind groups ───────────────────────────────────────────────────────────────

@group(0) @binding(0)
var<uniform> projection: mat4x4<f32>;

@group(1) @binding(0)
var font_texture: texture_2d<f32>;

@group(1) @binding(1)
var font_sampler: sampler;

@group(1) @binding(2)
var<uniform> mtsdf_params: MtsdfParams;

// ── Vertex shader ─────────────────────────────────────────────────────────────

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = projection * vec4<f32>(in.position, 0.0, 1.0);
    out.uv    = in.tex_coords;
    out.color = in.color;
    return out;
}

// ── MTSDF helpers ─────────────────────────────────────────────────────────────

/// Median of three values — reconstructs the signed distance from MSDF channels.
fn median(r: f32, g: f32, b: f32) -> f32 {
    return max(min(r, g), min(max(r, g), b));
}

// ── Fragment shader ───────────────────────────────────────────────────────────

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let msd = textureSample(font_texture, font_sampler, in.uv);

    // Reconstruct signed distance: 0.5 = glyph edge, >0.5 = inside.
    let sd = median(msd.r, msd.g, msd.b);

    // Compute the screen-pixel range for this fragment.
    //
    // unit_range converts the atlas-texel distance range into UV space.
    // Dotting with (1 / fwidth(uv)) converts it to screen pixels, giving the
    // number of screen pixels that correspond to one full SDF unit.  This
    // scales the AA band correctly at any font size or zoom level.
    let unit_range     = vec2<f32>(mtsdf_params.distance_range) /
                         vec2<f32>(mtsdf_params.atlas_width, mtsdf_params.atlas_height);
    let screen_uv_size = 1.0 / max(fwidth(in.uv), vec2<f32>(0.0001));
    let screen_px_range = max(0.5 * dot(unit_range, screen_uv_size), 1.0);

    let alpha = clamp((sd - 0.5) * screen_px_range + 0.5, 0.0, 1.0);

    // Discard pixels that are fully transparent to avoid over-draw.
    if alpha < 0.001 {
        discard;
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}

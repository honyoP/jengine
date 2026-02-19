// ── text_shader.wgsl ──────────────────────────────────────────────────────────
//
// Bitmap font atlas shader.
//
// Bind group layout (mirrors the tile pipeline so the same projection buffer
// and atlas bind group can be reused):
//
//   group(0) binding(0) — orthographic projection matrix  (uniform)
//   group(1) binding(0) — font atlas texture              (texture_2d<f32>)
//   group(1) binding(1) — font atlas sampler              (sampler)
//   group(2) binding(0) — text tint color                 (uniform vec4)
//
// Vertex layout matches `renderer::text::Vertex`:
//   location(0) position   : vec2<f32>  — screen-space pixels
//   location(1) tex_coords : vec2<f32>  — normalised atlas UVs [0, 1]

// ── Structs ───────────────────────────────────────────────────────────────────

struct VertexInput {
    @location(0) position:   vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0)       uv:            vec2<f32>,
};

// ── Bind groups ───────────────────────────────────────────────────────────────

@group(0) @binding(0)
var<uniform> projection: mat4x4<f32>;

@group(1) @binding(0)
var font_texture: texture_2d<f32>;

@group(1) @binding(1)
var font_sampler: sampler;

/// RGBA tint applied to every rendered glyph.
/// Pass `vec4(1, 1, 1, 1)` for unmodified atlas colours.
@group(2) @binding(0)
var<uniform> text_color: vec4<f32>;

// ── Vertex shader ─────────────────────────────────────────────────────────────

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = projection * vec4<f32>(in.position, 0.0, 1.0);
    out.uv = in.tex_coords;
    return out;
}

// ── Fragment shader ───────────────────────────────────────────────────────────

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let sampled = textureSample(font_texture, font_sampler, in.uv);

    // Discard fully-transparent pixels to avoid rectangular "box" artefacts
    // around each glyph quad.  Threshold of 0.1 tolerates minor atlas bleeding
    // without clipping visibly into soft glyph edges.
    if sampled.a < 0.1 {
        discard;
    }

    // Modulate the uniform tint by the atlas alpha so anti-aliased glyph
    // edges blend correctly against whatever is rendered underneath.
    return vec4<f32>(text_color.rgb, text_color.a * sampled.a);
}
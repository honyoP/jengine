// ── Scanline post-process shader ─────────────────────────────────────────────
//
// Pass 0: fullscreen-quad vertex shader (no vertex buffer).
// Pass 1: fragment shader darkens every other logical-pixel row by 18 %.
//
// Bind groups:
//   @group(0) binding(0) — texture_2d<f32>  (scene render target, linear view)
//   @group(0) binding(1) — sampler          (nearest, clamp-to-edge)
//   @group(1) binding(0) — vec4<f32>        (x = scale_factor, yzw = padding)

@group(0) @binding(0) var scene_texture: texture_2d<f32>;
@group(0) @binding(1) var scene_sampler: sampler;

struct Uniforms {
    scale_factor: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}
@group(1) @binding(0) var<uniform> uniforms: Uniforms;

// ── Vertex shader ─────────────────────────────────────────────────────────────

struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// 6 vertices, two triangles covering NDC [-1, +1].
// UV (0,0) maps to top-left (NDC -1,+1), matching wgpu's top-left origin.
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOut {
    // Triangle strip positions in NDC
    var positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0,  1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
    );
    var uvs = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 1.0),
    );
    var out: VertexOut;
    out.pos = vec4<f32>(positions[vi], 0.0, 1.0);
    out.uv  = uvs[vi];
    return out;
}

// ── Fragment shader ───────────────────────────────────────────────────────────

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    let color = textureSample(scene_texture, scene_sampler, in.uv);

    // Teal phosphor color grade:
    //   - Suppress red  → pulls the palette toward cyan/teal
    //   - Lift dark areas to dark teal (non-zero black point)
    //   - Slight blue boost for the cool CRT phosphor quality
    let graded = vec3<f32>(
        color.r * 0.82 + 0.015,   // -18 % red  + tiny teal lift
        color.g * 1.00 + 0.045,   // neutral     + teal lift
        color.b * 1.05 + 0.045,   // +5 % blue   + teal lift
    );

    // Scanline darkening: every even logical-pixel row dimmed by ~18 %.
    // in.pos.y is physical-pixel Y (0 = top); divide by scale_factor → logical row.
    let logical_y = floor(in.pos.y / uniforms.scale_factor);
    let factor    = select(1.0, 0.82, (u32(logical_y) % 2u) == 0u);

    return vec4<f32>(saturate(graded) * factor, 1.0);
}

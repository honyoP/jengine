struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) fg_color: vec4<f32>,
    @location(3) bg_color: vec4<f32>,
    /// Sub-pixel visual offset in pixels (does not affect logical grid position).
    @location(4) v_offset: vec2<f32>,
    /// 0.0 = Background layer (static), 1.0 = Foreground layer (animated).
    @location(5) layer_id: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) fg_color: vec4<f32>,
    @location(2) bg_color: vec4<f32>,
    @location(3) layer_id: f32,
};

struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var atlas_texture: texture_2d<f32>;
@group(1) @binding(1)
var atlas_sampler: sampler;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Apply v_offset only for animated layer (layer_id = 1.0, threshold > 0.75).
    // layer_id = 0.0 → solid fill background (no offset)
    // layer_id = 0.5 → static sprite / char (no offset)
    // layer_id = 1.0 → animated sprite / char (offset applied)
    var offset: vec2<f32> = vec2<f32>(0.0, 0.0);
    if in.layer_id > 0.75 {
        offset = in.v_offset;
    }
    out.clip_position = camera.view_proj * vec4<f32>(in.position + offset, 0.0, 1.0);
    out.uv = in.uv;
    out.fg_color = in.fg_color;
    out.bg_color = in.bg_color;
    out.layer_id = in.layer_id;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Solid fill background (layer_id = 0.0): skip atlas, return bg_color directly.
    if in.layer_id < 0.25 {
        return in.bg_color;
    }
    // Atlas-sampled layers (layer_id = 0.5 static, 1.0 animated): glyph/sprite
    // composited over a transparent background so Layer 0 shows through.
    let tex = textureSample(atlas_texture, atlas_sampler, in.uv);
    return mix(vec4<f32>(0.0, 0.0, 0.0, 0.0), in.fg_color * tex, tex.a);
}
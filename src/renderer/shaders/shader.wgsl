struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) fg_color: vec4<f32>,
    @location(3) bg_color: vec4<f32>,
    /// Entity ID for looking up visual offset in the storage buffer.
    @location(4) entity_id: u32,
    /// 0.0 = Background layer (static), 0.5 = Sprite layer, 1.0 = Foreground layer.
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

@group(2) @binding(0)
var<storage, read> entity_offsets: array<vec4<f32>>;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Apply visual offset from storage buffer if entity_id is valid and layer is animated.
    var offset: vec2<f32> = vec2<f32>(0.0, 0.0);
    // 0xFFFFFFFF (u32::MAX) is the sentinel for "no entity".
    if in.layer_id > 0.4 && in.entity_id != 0xFFFFFFFFu && in.entity_id < arrayLength(&entity_offsets) {
        offset = entity_offsets[in.entity_id].xy;
    }
    out.clip_position = camera.view_proj * vec4<f32>(in.position.xy + offset, in.position.z, 1.0);
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
    
    // Standard alpha blending: fg_color tints the texture, and we use texture's alpha.
    return in.fg_color * tex;
}

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) rect_size: vec2<f32>,
    @location(2) rect_coord: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) border_color: vec4<f32>,
    @location(5) radius: vec4<f32>,
    @location(6) border_thickness: f32,
    @location(7) shadow_blur: f32,
    @location(8) mode: u32,
    @location(9) mode_param: f32,
    /// Scissor rect in screen pixels: [min_x, min_y, max_x, max_y].
    @location(10) clip_rect: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) rect_size: vec2<f32>,
    @location(1) rect_coord: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) border_color: vec4<f32>,
    @location(4) radius: vec4<f32>,
    @location(5) border_thickness: f32,
    @location(6) shadow_blur: f32,
    @location(7) @interpolate(flat) mode: u32,
    @location(8) mode_param: f32,
    /// All 4 vertices of a panel share the same clip_rect, so use flat interpolation.
    @location(9) @interpolate(flat) clip_rect: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> projection: mat4x4<f32>;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = projection * vec4<f32>(input.position, 1.0);
    out.rect_size = input.rect_size;
    out.rect_coord = input.rect_coord;
    out.color = input.color;
    out.border_color = input.border_color;
    out.radius = input.radius;
    out.border_thickness = input.border_thickness;
    out.shadow_blur = input.shadow_blur;
    out.mode = input.mode;
    out.mode_param = input.mode_param;
    out.clip_rect = input.clip_rect;
    return out;
}

// Rounded Rectangle SDF
// p: local point (relative to center)
// b: half-extents (size/2)
// r: corner radii [tl, tr, br, bl]
fn sd_rounded_rect(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    var res_r = r.x; // Default to top-left
    if (p.x > 0.0 && p.y < 0.0) { res_r = r.y; } // top-right
    if (p.x > 0.0 && p.y > 0.0) { res_r = r.z; } // bottom-right
    if (p.x < 0.0 && p.y > 0.0) { res_r = r.w; } // bottom-left

    let q = abs(p) - b + res_r;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - res_r;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // ── Scissor Clip ──
    // Discard fragments outside the clip rect (used by ScrollContainer).
    // clip_rect.xy = min corner, clip_rect.zw = max corner, in screen pixels.
    // clip_position.xy is in framebuffer pixel coordinates.
    if (in.clip_position.x < in.clip_rect.x || in.clip_position.y < in.clip_rect.y ||
        in.clip_position.x > in.clip_rect.z || in.clip_position.y > in.clip_rect.w) {
        discard;
    }

    // Convert [0, 1] rect_coord to pixel space relative to center
    let pixel_p = (in.rect_coord - 0.5) * in.rect_size;
    let half_size = in.rect_size * 0.5;

    // Distance to rect edge (negative inside, positive outside)
    let dist = sd_rounded_rect(pixel_p, half_size, in.radius);

    // Antialiasing factor (approx 1 pixel)
    let aff = fwidth(dist);

    // ── Main Shape Mask ──
    let shape_alpha = 1.0 - smoothstep(-aff, 0.0, dist);

    // ── Border Mask ──
    let border_mask = smoothstep(-in.border_thickness - aff, -in.border_thickness, dist);

    // ── Pattern Generation ──
    var pattern_val = 1.0;
    if (in.mode == 1u) {
        // Procedural Crosshatch / Woven
        // We use screen-space coordinates for perfect tiling across multiple panels
        let screen_p = in.clip_position.xy;
        let p_rotated = vec2<f32>(
            screen_p.x + screen_p.y,
            screen_p.x - screen_p.y
        ) * 0.5; // Rotate 45 degrees

        let scale = in.mode_param; // e.g. 4.0 pixels
        let lines = abs(sin(p_rotated * (3.14159 / scale)));
        pattern_val = smoothstep(0.4, 0.6, min(lines.x, lines.y));
        pattern_val = mix(0.7, 1.0, pattern_val); // Subtle texture contrast
    }

    // ── Final Color Calculation ──
    var final_color = in.color;
    final_color = vec4<f32>(final_color.rgb * pattern_val, final_color.a);

    // Blend border over background
    final_color = mix(final_color, in.border_color, border_mask * in.border_color.a);

    // Apply the main shape alpha (for rounded corners)
    final_color.a *= shape_alpha;

    if (final_color.a <= 0.0) {
        discard;
    }

    return final_color;
}

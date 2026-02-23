// ── Tests ─────────────────────────────────────────────────────────────────────

use jengine::renderer::text::*;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a minimal MTSDF font with glyphs for 'A' and 'B'.
///
/// Font metrics (all in em units):
///   line_height = 1.0, ascender = -0.8, descender = 0.2
///   atlas: 512 × 512 px
///
/// Glyph 'A' (unicode 65):
///   plane: left=0.05, top=-0.8, right=0.55, bottom=0.2
///   atlas: left=0,   top=0,   right=14,  bottom=20
///   advance = 0.6
///
/// Glyph 'B' (unicode 66):
///   plane: left=0.05, top=-0.8, right=0.50, bottom=0.2
///   atlas: left=16,  top=0,   right=29,  bottom=20
///   advance = 0.55
fn make_font() -> Font {
    Font::from_mtsdf_json(sample_json()).unwrap()
}

fn sample_json() -> &'static str {
    r#"{
        "atlas": { "type": "mtsdf", "width": 512, "height": 512, "distanceRange": 4.0 },
        "metrics": { "lineHeight": 1.0, "ascender": -0.8, "descender": 0.2 },
        "glyphs": [
            {
                "unicode": 65, "advance": 0.6,
                "planeBounds":  { "left": 0.05, "top": -0.8,  "right": 0.55, "bottom": 0.2 },
                "atlasBounds":  { "left": 0,    "top": 0,     "right": 14,   "bottom": 20  }
            },
            {
                "unicode": 66, "advance": 0.55,
                "planeBounds":  { "left": 0.05, "top": -0.8,  "right": 0.50, "bottom": 0.2 },
                "atlasBounds":  { "left": 16,   "top": 0,     "right": 29,   "bottom": 20  }
            }
        ],
        "kerning": []
    }"#
}

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

// ── from_mtsdf_json ───────────────────────────────────────────────────────────

#[test]
fn from_mtsdf_json_parses_metadata() {
    let font = make_font();
    assert_eq!(font.line_height,  1.0);
    assert_eq!(font.ascender,    -0.8);
    assert_eq!(font.descender,    0.2);
    assert_eq!(font.atlas_width,  512);
    assert_eq!(font.atlas_height, 512);
    assert_eq!(font.distance_range, 4.0);
}

#[test]
fn from_mtsdf_json_populates_glyph_map() {
    let font = make_font();
    assert_eq!(font.glyphs.len(), 2);
    assert!(font.glyphs.contains_key(&'A'));
    assert!(font.glyphs.contains_key(&'B'));
}

#[test]
fn from_mtsdf_json_glyph_fields_correct() {
    let font = make_font();
    let a = &font.glyphs[&'A'];
    assert!(font.glyphs.contains_key(&'A'));
    assert!((a.atlas_left   -  0.0).abs() < 1e-6);
    assert!((a.atlas_top    -  0.0).abs() < 1e-6);
    assert!((a.atlas_right  - 14.0).abs() < 1e-6);
    assert!((a.atlas_bottom - 20.0).abs() < 1e-6);
    assert!((a.plane_left   - 0.05).abs() < 1e-6);
    assert!((a.plane_top    - (-0.8)).abs() < 1e-6);
    assert!((a.plane_right  - 0.55).abs() < 1e-6);
    assert!((a.plane_bottom - 0.2).abs()  < 1e-6);
    assert!((a.x_advance    - 0.6).abs()  < 1e-6);
}

#[test]
fn from_mtsdf_json_invalid_input_returns_error() {
    assert!(Font::from_mtsdf_json("not json").is_err());
}

#[test]
fn from_mtsdf_json_skips_invalid_codepoints() {
    // 0xD800 is a surrogate — not a valid Unicode scalar value.
    let json = r#"{
        "atlas": { "width": 256, "height": 256, "distanceRange": 4.0 },
        "metrics": { "lineHeight": 1.0, "ascender": -0.8, "descender": 0.2 },
        "glyphs": [
            { "unicode": 55296, "advance": 0.6,
              "planeBounds": null, "atlasBounds": null }
        ],
        "kerning": []
    }"#;
    let font = Font::from_mtsdf_json(json).unwrap();
    assert!(font.glyphs.is_empty());
}

// ── generate_text_mesh ────────────────────────────────────────────────────────

#[test]
fn mesh_empty_string_returns_empty_buffers() {
    let font = make_font();
    let (verts, indices) = generate_text_mesh("", &font, [0.0, 0.0], 24.0, WHITE);
    assert!(verts.is_empty());
    assert!(indices.is_empty());
}

#[test]
fn mesh_single_char_produces_4_vertices_and_6_indices() {
    let font = make_font();
    let (verts, indices) = generate_text_mesh("A", &font, [0.0, 0.0], 24.0, WHITE);
    assert_eq!(verts.len(), 4);
    assert_eq!(indices.len(), 6);
}

#[test]
fn mesh_two_chars_produce_8_vertices_and_12_indices() {
    let font = make_font();
    let (verts, indices) = generate_text_mesh("AB", &font, [0.0, 0.0], 24.0, WHITE);
    assert_eq!(verts.len(), 8);
    assert_eq!(indices.len(), 12);
}

#[test]
fn mesh_vertex_positions_apply_start_pos() {
    // start=[10, 20], font_size=24:
    //   baseline_y = 20 + 0.8*24 = 39.2
    //   A: x0=10+0.05*24=11.2, y0=39.2-0.8*24=20.0
    let font = make_font();
    let (verts, _) = generate_text_mesh("A", &font, [10.0, 20.0], 24.0, WHITE);
    let tl = verts[0];
    assert!((tl.position[0] - 11.2).abs() < 1e-4, "TL.x expected 11.2, got {}", tl.position[0]);
    assert!((tl.position[1] - 20.0).abs() < 1e-4, "TL.y expected 20.0, got {}", tl.position[1]);
}

#[test]
fn mesh_vertex_positions_correct_all_corners() {
    // start=[0,0], font_size=24:
    //   baseline_y = 0 + 0.8*24 = 19.2
    //   A: x0=0.05*24=1.2, x1=0.55*24=13.2
    //      y0=19.2+(-0.8*24)=0.0, y1=19.2+0.2*24=24.0
    let font = make_font();
    let (verts, _) = generate_text_mesh("A", &font, [0.0, 0.0], 24.0, WHITE);
    assert!((verts[0].position[0] -  1.2).abs() < 1e-4); // TL.x
    assert!((verts[0].position[1] -  0.0).abs() < 1e-4); // TL.y
    assert!((verts[1].position[0] - 13.2).abs() < 1e-4); // TR.x
    assert!((verts[1].position[1] -  0.0).abs() < 1e-4); // TR.y
    assert!((verts[2].position[0] -  1.2).abs() < 1e-4); // BL.x
    assert!((verts[2].position[1] - 24.0).abs() < 1e-4); // BL.y
    assert!((verts[3].position[0] - 13.2).abs() < 1e-4); // BR.x
    assert!((verts[3].position[1] - 24.0).abs() < 1e-4); // BR.y
}

#[test]
fn mesh_uvs_normalised_correctly() {
    // A: atlas left=0, top=0, right=14, bottom=20; texture 512×512
    //   u0=0, u1=14/512, v0=0, v1=20/512
    let font = make_font();
    let (verts, _) = generate_text_mesh("A", &font, [0.0, 0.0], 24.0, WHITE);
    let expected_u1 = 14.0_f32 / 512.0;
    let expected_v1 = 20.0_f32 / 512.0;
    assert!((verts[0].tex_coords[0] - 0.0).abs()         < 1e-6); // TL.u
    assert!((verts[0].tex_coords[1] - 0.0).abs()         < 1e-6); // TL.v
    assert!((verts[1].tex_coords[0] - expected_u1).abs() < 1e-6); // TR.u
    assert!((verts[2].tex_coords[1] - expected_v1).abs() < 1e-6); // BL.v
}

#[test]
fn mesh_indices_reference_correct_base_offsets() {
    let font = make_font();
    let (_, indices) = generate_text_mesh("A", &font, [0.0, 0.0], 24.0, WHITE);
    assert_eq!(indices, vec![0, 1, 2, 1, 3, 2]);
}

#[test]
fn mesh_second_char_indices_offset_by_4() {
    let font = make_font();
    let (_, indices) = generate_text_mesh("AB", &font, [0.0, 0.0], 24.0, WHITE);
    assert_eq!(&indices[6..], &[4, 5, 6, 5, 7, 6]);
}

#[test]
fn mesh_cursor_advances_by_x_advance() {
    // A x_advance=0.6; font_size=24 → advance=14.4
    // B: x0 = 14.4 + 0.05*24 = 15.6
    let font = make_font();
    let (verts, _) = generate_text_mesh("AB", &font, [0.0, 0.0], 24.0, WHITE);
    let b_tl_x = verts[4].position[0];
    assert!((b_tl_x - 15.6).abs() < 1e-4, "expected 15.6, got {b_tl_x}");
}

#[test]
fn mesh_newline_resets_x_and_advances_y() {
    // "A\nA", start=[5, 10], font_size=24:
    //   baseline_1 = 10 + 0.8*24 = 29.2
    //   A[0]: x0 = 5 + 0.05*24 = 6.2, y0 = 29.2 - 0.8*24 = 10.0
    //   \n: baseline += 1.0*24 = 53.2, cursor_x = 5
    //   A[1]: x0 = 5 + 1.2 = 6.2, y0 = 53.2 - 19.2 = 34.0
    let font = make_font();
    let (verts, _) = generate_text_mesh("A\nA", &font, [5.0, 10.0], 24.0, WHITE);
    assert_eq!(verts.len(), 8); // newline produces no vertices
    assert!((verts[0].position[0] -  6.2).abs() < 1e-4);
    assert!((verts[0].position[1] - 10.0).abs() < 1e-4);
    assert!((verts[4].position[0] -  6.2).abs() < 1e-4);
    assert!((verts[4].position[1] - 34.0).abs() < 1e-4);
}

#[test]
fn mesh_unknown_char_is_skipped() {
    let font = make_font();
    let (verts, indices) = generate_text_mesh("ZA", &font, [0.0, 0.0], 24.0, WHITE);
    assert_eq!(verts.len(), 4);
    assert_eq!(indices.len(), 6);
}

#[test]
fn mesh_scale_factor_applied_to_positions() {
    // font_size=48: quad_w = (0.55-0.05)*48 = 24.0
    let font = make_font();
    let (verts, _) = generate_text_mesh("A", &font, [0.0, 0.0], 48.0, WHITE);
    let quad_w = verts[1].position[0] - verts[0].position[0];
    assert!((quad_w - 24.0).abs() < 1e-4, "expected quad_w=24, got {quad_w}");
}

#[test]
fn mesh_scale_factor_does_not_affect_uvs() {
    let font = make_font();
    let (verts_1x, _) = generate_text_mesh("A", &font, [0.0, 0.0], 24.0, WHITE);
    let (verts_2x, _) = generate_text_mesh("A", &font, [0.0, 0.0], 48.0, WHITE);
    assert_eq!(verts_1x[1].tex_coords, verts_2x[1].tex_coords);
}

#[test]
fn mesh_zero_font_size_returns_empty() {
    let font = make_font();
    let (verts, indices) = generate_text_mesh("A", &font, [0.0, 0.0], 0.0, WHITE);
    assert!(verts.is_empty());
    assert!(indices.is_empty());
}

#[test]
fn mesh_color_propagated_to_all_vertices() {
    let font = make_font();
    let red = [1.0_f32, 0.0, 0.0, 1.0];
    let (verts, _) = generate_text_mesh("A", &font, [0.0, 0.0], 24.0, red);
    for v in &verts {
        assert_eq!(v.color, red);
    }
}

#[test]
fn mesh_negative_font_size_returns_empty() {
    let font = make_font();
    let (verts, indices) = generate_text_mesh("A", &font, [0.0, 0.0], -10.0, WHITE);
    assert!(verts.is_empty(), "negative font_size must produce no geometry");
    assert!(indices.is_empty());
}

// ── kerning ───────────────────────────────────────────────────────────────────

fn sample_json_with_kerning() -> &'static str {
    r#"{
        "atlas": { "type": "mtsdf", "width": 512, "height": 512, "distanceRange": 4.0 },
        "metrics": { "lineHeight": 1.0, "ascender": -0.8, "descender": 0.2 },
        "glyphs": [
            {
                "unicode": 65, "advance": 0.6,
                "planeBounds":  { "left": 0.05, "top": -0.8,  "right": 0.55, "bottom": 0.2 },
                "atlasBounds":  { "left": 0,    "top": 0,     "right": 14,   "bottom": 20  }
            },
            {
                "unicode": 66, "advance": 0.55,
                "planeBounds":  { "left": 0.05, "top": -0.8,  "right": 0.50, "bottom": 0.2 },
                "atlasBounds":  { "left": 16,   "top": 0,     "right": 29,   "bottom": 20  }
            }
        ],
        "kerning": [{ "unicode1": 65, "unicode2": 66, "advance": -0.05 }]
    }"#
}

#[test]
fn mesh_kerning_adjusts_cursor() {
    // Without kerning: B.x0 = 0.6*24 + 0.05*24 = 14.4 + 1.2 = 15.6
    // kern(A→B) = -0.05: cursor_x after A = 14.4 + (-0.05*24) = 13.2
    // B.x0 = 13.2 + 0.05*24 = 13.2 + 1.2 = 14.4
    let font = Font::from_mtsdf_json(sample_json_with_kerning()).unwrap();
    let (verts, _) = generate_text_mesh("AB", &font, [0.0, 0.0], 24.0, WHITE);
    let b_tl_x = verts[4].position[0];
    assert!(
        (b_tl_x - 14.4).abs() < 1e-4,
        "expected 14.4 (kerned), got {b_tl_x}"
    );
}

#[test]
fn mesh_kerning_no_effect_on_single_char() {
    // Single 'B' has no prev char so no kerning; B.x0 = start + plane_left*size
    let font = Font::from_mtsdf_json(sample_json_with_kerning()).unwrap();
    let (verts, _) = generate_text_mesh("B", &font, [0.0, 0.0], 24.0, WHITE);
    let b_tl_x = verts[0].position[0];
    assert!((b_tl_x - 1.2).abs() < 1e-4, "expected 1.2, got {b_tl_x}");
}

// ── missing kerning field ─────────────────────────────────────────────────────

#[test]
fn from_mtsdf_json_missing_kerning_field_is_accepted() {
    // Tools that emit no kerning pairs may omit the array entirely.
    let json = r#"{
        "atlas": { "width": 256, "height": 256, "distanceRange": 4.0 },
        "metrics": { "lineHeight": 1.0, "ascender": -0.8, "descender": 0.2 },
        "glyphs": []
    }"#;
    let font = Font::from_mtsdf_json(json).unwrap();
    assert!(font.kerning.is_empty());
}

// ── ascender sign normalisation ───────────────────────────────────────────────

#[test]
fn from_mtsdf_json_positive_ascender_normalised_to_negative() {
    // msdf-atlas-gen v1 emits ascender as positive; the parser should negate it
    // so the internal convention is always negative-above-baseline.
    let json = r#"{
        "atlas": { "width": 256, "height": 256, "distanceRange": 4.0 },
        "metrics": { "lineHeight": 1.0, "ascender": 0.8, "descender": -0.2 },
        "glyphs": []
    }"#;
    let font = Font::from_mtsdf_json(json).unwrap();
    assert_eq!(font.ascender,  -0.8, "ascender must be stored as negative");
    assert_eq!(font.descender,  0.2, "descender must be stored as positive");
}

// ── text_width ────────────────────────────────────────────────────────────────

#[test]
fn text_width_empty_string_is_zero() {
    let font = make_font();
    assert_eq!(text_width("", &font, 24.0), 0.0);
}

#[test]
fn text_width_zero_font_size_is_zero() {
    let font = make_font();
    assert_eq!(text_width("A", &font, 0.0), 0.0);
}

#[test]
fn text_width_negative_font_size_is_zero() {
    let font = make_font();
    assert_eq!(text_width("A", &font, -8.0), 0.0);
}

#[test]
fn text_width_single_char() {
    // A.advance = 0.6, font_size = 24 → width = 0.6 * 24 = 14.4
    let font = make_font();
    let w = text_width("A", &font, 24.0);
    assert!((w - 14.4).abs() < 1e-4, "expected 14.4, got {w}");
}

#[test]
fn text_width_two_chars_no_kerning() {
    // "AB": A.adv=0.6, B.adv=0.55; width = (0.6+0.55)*24 = 27.6
    let font = make_font();
    let w = text_width("AB", &font, 24.0);
    assert!((w - 27.6).abs() < 1e-4, "expected 27.6, got {w}");
}

#[test]
fn text_width_two_chars_with_kerning() {
    // kern(A→B) = -0.05; width = (0.6 + (-0.05) + 0.55)*24 = 26.4
    let font = Font::from_mtsdf_json(sample_json_with_kerning()).unwrap();
    let w = text_width("AB", &font, 24.0);
    assert!((w - 26.4).abs() < 1e-4, "expected 26.4 (with kern), got {w}");
}

#[test]
fn text_width_unknown_char_not_counted() {
    // 'Z' absent → contributes 0; only 'A' is counted
    let font = make_font();
    let w = text_width("ZA", &font, 24.0);
    assert!((w - 14.4).abs() < 1e-4, "expected 14.4 (Z skipped), got {w}");
}

#[test]
fn text_width_multiline_returns_max_line() {
    // "A\nAA": line 1 = 0.6*24 = 14.4, line 2 = (0.6+0.6)*24 = 28.8 → max = 28.8
    let font = make_font();
    let w = text_width("A\nAA", &font, 24.0);
    assert!((w - 28.8).abs() < 1e-4, "expected 28.8 (widest line), got {w}");
}

// ── append_text_mesh_at_baseline ──────────────────────────────────────────────

#[test]
fn at_baseline_produces_same_vertices_as_top_left() {
    // append_text_mesh([0,0]) should produce the same geometry as
    // append_text_mesh_at_baseline with baseline_y = -ascender * font_size.
    // font.ascender = -0.8, font_size = 24 → baseline_y = 0.8 * 24 = 19.2
    let font = make_font();
    let (verts_tl, _) = generate_text_mesh("A", &font, [0.0, 0.0], 24.0, WHITE);

    let baseline_y = (-font.ascender) * 24.0; // 0.8 * 24 = 19.2
    let mut verts_bl = Vec::new();
    let mut idxs_bl  = Vec::new();
    append_text_mesh_at_baseline("A", &font, 0.0, baseline_y, 24.0, WHITE, &mut verts_bl, &mut idxs_bl);

    assert_eq!(verts_tl.len(), verts_bl.len());
    for (a, b) in verts_tl.iter().zip(verts_bl.iter()) {
        assert!((a.position[0] - b.position[0]).abs() < 1e-4);
        assert!((a.position[1] - b.position[1]).abs() < 1e-4);
    }
}

#[test]
fn at_baseline_two_sizes_share_baseline() {
    // Render 'A' at font_size=24 and font_size=48 with the same baseline_y=100.
    // Both should have y0 = baseline_y + plane_top * font_size:
    //   size 24: y0 = 100 + (-0.8)*24 = 100 - 19.2 = 80.8
    //   size 48: y0 = 100 + (-0.8)*48 = 100 - 38.4 = 61.6
    // More importantly, bottom of glyph = baseline_y + plane_bottom * font_size:
    //   both plane_bottom = 0.2, so y1_24 = 100+4.8=104.8, y1_48 = 100+9.6=109.6
    //
    // The baselines are the same (100.0) — verified by checking that the
    // top-left y of each glyph equals baseline_y + plane_top * font_size.
    let font = make_font();
    let baseline_y = 100.0f32;

    let (verts_sm, _) = {
        let mut v = Vec::new(); let mut i = Vec::new();
        append_text_mesh_at_baseline("A", &font, 0.0, baseline_y, 24.0, WHITE, &mut v, &mut i);
        (v, i)
    };
    let (verts_lg, _) = {
        let mut v = Vec::new(); let mut i = Vec::new();
        append_text_mesh_at_baseline("A", &font, 0.0, baseline_y, 48.0, WHITE, &mut v, &mut i);
        (v, i)
    };

    // y0 = baseline_y + plane_top * font_size
    let y0_sm = verts_sm[0].position[1];
    let y0_lg = verts_lg[0].position[1];
    assert!((y0_sm - 80.8).abs() < 1e-3, "sm y0 expected 80.8, got {y0_sm}");
    assert!((y0_lg - 61.6).abs() < 1e-3, "lg y0 expected 61.6, got {y0_lg}");

    // y1 = baseline_y + plane_bottom * font_size
    let y1_sm = verts_sm[3].position[1];
    let y1_lg = verts_lg[3].position[1];
    assert!((y1_sm - 104.8).abs() < 1e-3, "sm y1 expected 104.8, got {y1_sm}");
    assert!((y1_lg - 109.6).abs() < 1e-3, "lg y1 expected 109.6, got {y1_lg}");
}

#[test]
fn at_baseline_empty_returns_nothing() {
    let font = make_font();
    let mut v = Vec::new(); let mut i = Vec::new();
    append_text_mesh_at_baseline("", &font, 0.0, 50.0, 24.0, WHITE, &mut v, &mut i);
    assert!(v.is_empty());
    assert!(i.is_empty());
}

#[test]
fn at_baseline_negative_font_size_returns_nothing() {
    let font = make_font();
    let mut v = Vec::new(); let mut i = Vec::new();
    append_text_mesh_at_baseline("A", &font, 0.0, 50.0, -1.0, WHITE, &mut v, &mut i);
    assert!(v.is_empty());
    assert!(i.is_empty());
}

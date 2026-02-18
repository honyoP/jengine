// ── Tests ─────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use jengine::renderer::text::*;
    // ── helpers ───────────────────────────────────────────────────────────────

    /// Build a minimal font with glyphs for 'A' and 'B'.
    ///
    /// 'A': atlas (x=0,  y=0, w=14, h=20), offset=(1,2), advance=16
    /// 'B': atlas (x=16, y=0, w=13, h=20), offset=(1,2), advance=15
    /// line_height=24, texture 512×512
    fn make_font() -> Font {
        Font::from_json(sample_json()).unwrap()
    }

    fn sample_json() -> &'static str {
        r#"{
            "line_height": 24,
            "texture_width": 512,
            "texture_height": 512,
            "glyphs": [
                { "id": 65, "x": 0,  "y": 0, "width": 14, "height": 20,
                  "x_offset": 1, "y_offset": 2, "x_advance": 16 },
                { "id": 66, "x": 16, "y": 0, "width": 13, "height": 20,
                  "x_offset": 1, "y_offset": 2, "x_advance": 15 }
            ]
        }"#
    }

    #[test]
    fn from_json_parses_metadata() {
        let font = Font::from_json(sample_json()).unwrap();
        assert_eq!(font.line_height, 24);
        assert_eq!(font.texture_width, 512);
        assert_eq!(font.texture_height, 512);
    }

    #[test]
    fn from_json_populates_glyph_map() {
        let font = Font::from_json(sample_json()).unwrap();
        assert_eq!(font.glyphs.len(), 2);
        assert!(font.glyphs.contains_key(&'A'));
        assert!(font.glyphs.contains_key(&'B'));
    }

    #[test]
    fn from_json_glyph_fields_correct() {
        let font = Font::from_json(sample_json()).unwrap();
        let a = &font.glyphs[&'A'];
        assert_eq!(a.id, 'A');
        assert_eq!(a.x, 0);
        assert_eq!(a.y, 0);
        assert_eq!(a.width, 14);
        assert_eq!(a.height, 20);
        assert_eq!(a.x_offset, 1);
        assert_eq!(a.y_offset, 2);
        assert_eq!(a.x_advance, 16);
    }

    #[test]
    fn from_json_invalid_input_returns_error() {
        assert!(Font::from_json("not json").is_err());
    }

    #[test]
    fn from_json_skips_invalid_codepoints() {
        // 0xD800 is a surrogate — not a valid Unicode scalar value.
        let json = r#"{
            "line_height": 16, "texture_width": 256, "texture_height": 256,
            "glyphs": [
                { "id": 55296, "x": 0, "y": 0, "width": 8, "height": 16,
                  "x_offset": 0, "y_offset": 0, "x_advance": 8 }
            ]
        }"#;
        let font = Font::from_json(json).unwrap();
        assert!(font.glyphs.is_empty());
    }

    // ── generate_text_mesh ────────────────────────────────────────────────────

    #[test]
    fn mesh_empty_string_returns_empty_buffers() {
        let font = make_font();
        let (verts, indices) = generate_text_mesh("", &font, [0.0, 0.0], 24.0);
        assert!(verts.is_empty());
        assert!(indices.is_empty());
    }

    #[test]
    fn mesh_single_char_produces_4_vertices_and_6_indices() {
        let font = make_font();
        let (verts, indices) = generate_text_mesh("A", &font, [0.0, 0.0], 24.0);
        assert_eq!(verts.len(), 4);
        assert_eq!(indices.len(), 6);
    }

    #[test]
    fn mesh_two_chars_produce_8_vertices_and_12_indices() {
        let font = make_font();
        let (verts, indices) = generate_text_mesh("AB", &font, [0.0, 0.0], 24.0);
        assert_eq!(verts.len(), 8);
        assert_eq!(indices.len(), 12);
    }

    #[test]
    fn mesh_vertex_positions_apply_offset_and_start_pos() {
        // 'A': x_offset=1, y_offset=2, width=14, height=20; scale=1 (font_size==line_height)
        // start=[10, 20] → TL should be at (10+1, 20+2) = (11, 22)
        let font = make_font();
        let (verts, _) = generate_text_mesh("A", &font, [10.0, 20.0], 24.0);
        let tl = verts[0];
        assert!((tl.position[0] - 11.0).abs() < 1e-5, "TL.x expected 11, got {}", tl.position[0]);
        assert!((tl.position[1] - 22.0).abs() < 1e-5, "TL.y expected 22, got {}", tl.position[1]);
    }

    #[test]
    fn mesh_vertex_positions_correct_all_corners() {
        // scale=1, start=[0,0], 'A': x_offset=1, y_offset=2, w=14, h=20
        // TL=(1,2), TR=(15,2), BL=(1,22), BR=(15,22)
        let font = make_font();
        let (verts, _) = generate_text_mesh("A", &font, [0.0, 0.0], 24.0);
        assert!((verts[0].position[0] - 1.0).abs()  < 1e-5); // TL.x
        assert!((verts[0].position[1] - 2.0).abs()  < 1e-5); // TL.y
        assert!((verts[1].position[0] - 15.0).abs() < 1e-5); // TR.x
        assert!((verts[1].position[1] - 2.0).abs()  < 1e-5); // TR.y
        assert!((verts[2].position[0] - 1.0).abs()  < 1e-5); // BL.x
        assert!((verts[2].position[1] - 22.0).abs() < 1e-5); // BL.y
        assert!((verts[3].position[0] - 15.0).abs() < 1e-5); // BR.x
        assert!((verts[3].position[1] - 22.0).abs() < 1e-5); // BR.y
    }

    #[test]
    fn mesh_uvs_normalised_correctly() {
        // 'A': atlas x=0, y=0, w=14, h=20; texture 512×512
        // uv_x0=0/512=0, uv_y0=0/512=0, uv_x1=14/512, uv_y1=20/512
        let font = make_font();
        let (verts, _) = generate_text_mesh("A", &font, [0.0, 0.0], 24.0);
        let expected_u1 = 14.0_f32 / 512.0;
        let expected_v1 = 20.0_f32 / 512.0;
        assert!((verts[0].tex_coords[0] - 0.0).abs()       < 1e-6); // TL.u
        assert!((verts[0].tex_coords[1] - 0.0).abs()       < 1e-6); // TL.v
        assert!((verts[1].tex_coords[0] - expected_u1).abs() < 1e-6); // TR.u
        assert!((verts[2].tex_coords[1] - expected_v1).abs() < 1e-6); // BL.v
    }

    #[test]
    fn mesh_indices_reference_correct_base_offsets() {
        // Single char: indices must be [0,1,2, 1,3,2]
        let font = make_font();
        let (_, indices) = generate_text_mesh("A", &font, [0.0, 0.0], 24.0);
        assert_eq!(indices, vec![0, 1, 2, 1, 3, 2]);
    }

    #[test]
    fn mesh_second_char_indices_offset_by_4() {
        // Two chars: second glyph's indices must start at base=4
        let font = make_font();
        let (_, indices) = generate_text_mesh("AB", &font, [0.0, 0.0], 24.0);
        assert_eq!(&indices[6..], &[4, 5, 6, 5, 7, 6]);
    }

    #[test]
    fn mesh_cursor_advances_by_x_advance() {
        // 'A' x_advance=16, 'B' x_offset=1 → B's TL.x = 16 + 1 = 17
        let font = make_font();
        let (verts, _) = generate_text_mesh("AB", &font, [0.0, 0.0], 24.0);
        let b_tl_x = verts[4].position[0]; // first vertex of second glyph
        assert!((b_tl_x - 17.0).abs() < 1e-5, "expected 17.0, got {b_tl_x}");
    }

    #[test]
    fn mesh_newline_resets_x_and_advances_y() {
        // "A\nA": second 'A' must start at x=start_x, y=start_y+line_height
        let font = make_font();
        let (verts, _) = generate_text_mesh("A\nA", &font, [5.0, 10.0], 24.0);
        assert_eq!(verts.len(), 8); // newline produces no vertices
        // Second 'A' TL: x = 5 + x_offset(1) = 6,  y = 10 + 24 + y_offset(2) = 36
        let second_tl = verts[4];
        assert!((second_tl.position[0] - 6.0).abs()  < 1e-5);
        assert!((second_tl.position[1] - 36.0).abs() < 1e-5);
    }

    #[test]
    fn mesh_unknown_char_is_skipped() {
        // 'Z' is not in the font; only 'A' should produce geometry
        let font = make_font();
        let (verts, indices) = generate_text_mesh("ZA", &font, [0.0, 0.0], 24.0);
        assert_eq!(verts.len(), 4);
        assert_eq!(indices.len(), 6);
    }

    #[test]
    fn mesh_scale_factor_applied_to_positions() {
        // font_size=48 with line_height=24 → scale=2; 'A' width=14 → quad_w=28
        let font = make_font();
        let (verts, _) = generate_text_mesh("A", &font, [0.0, 0.0], 48.0);
        let quad_w = verts[1].position[0] - verts[0].position[0]; // TR.x - TL.x
        assert!((quad_w - 28.0).abs() < 1e-5, "expected quad_w=28, got {quad_w}");
    }

    #[test]
    fn mesh_scale_factor_does_not_affect_uvs() {
        // UVs are normalised atlas coords — independent of font_size
        let font = make_font();
        let (verts_1x, _) = generate_text_mesh("A", &font, [0.0, 0.0], 24.0);
        let (verts_2x, _) = generate_text_mesh("A", &font, [0.0, 0.0], 48.0);
        assert_eq!(verts_1x[1].tex_coords, verts_2x[1].tex_coords);
    }

    #[test]
    fn mesh_zero_line_height_returns_empty() {
        let font = Font {
            glyphs: HashMap::new(),
            line_height: 0,
            texture_width: 512,
            texture_height: 512,
        };
        let (verts, indices) = generate_text_mesh("A", &font, [0.0, 0.0], 24.0);
        assert!(verts.is_empty());
        assert!(indices.is_empty());
    }

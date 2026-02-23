use jengine::ui::{rect_contains, word_wrap, Padding, Rect, Label, TextLayer};
use jengine::ui::widgets::{Dropdown, InputBox, ToggleSelector};
use jengine::renderer::text::{Font, Glyph, Vertex};
use std::collections::HashMap;

// ── word_wrap ────────────────────────────────────────────────────────────────

#[test]
fn wrap_empty_string_returns_no_lines() {
    assert!(word_wrap("", 10).is_empty());
}

#[test]
fn wrap_blank_whitespace_returns_no_lines() {
    assert!(word_wrap("   ", 10).is_empty());
}

#[test]
fn wrap_single_word_fits() {
    assert_eq!(word_wrap("hello", 10), vec!["hello"]);
}

#[test]
fn wrap_two_words_fit_on_one_line() {
    assert_eq!(word_wrap("hello world", 12), vec!["hello world"]);
}

#[test]
fn wrap_two_words_break_at_boundary() {
    assert_eq!(word_wrap("hello world", 8), vec!["hello", "world"]);
}

#[test]
fn wrap_single_word_longer_than_max_gets_split() {
    let lines = word_wrap("abcdefghij", 4);
    assert_eq!(lines, vec!["abcd", "efgh", "ij"]);
}

// ── rect_contains ────────────────────────────────────────────────────────────

#[test]
fn rect_contains_center_point() {
    assert!(rect_contains(0.0, 0.0, 100.0, 100.0, 50.0, 50.0));
}

// ── Rect ─────────────────────────────────────────────────────────────────────

#[test]
fn rect_overlaps() {
    let r1 = Rect::new(0.0, 0.0, 10.0, 10.0);
    let r2 = Rect::new(5.0, 5.0, 10.0, 10.0);
    let r3 = Rect::new(20.0, 20.0, 10.0, 10.0);
    assert!(r1.overlaps(&r2));
    assert!(!r1.overlaps(&r3));
}

// ── Label & TextLayer ─────────────────────────────────────────────────────────

fn make_font() -> Font {
    let mut glyphs = HashMap::new();
    glyphs.insert('A', Glyph {
        atlas_left: 0.0, atlas_top: 0.0, atlas_right: 8.0, atlas_bottom: 16.0,
        plane_left: 0.0, plane_top: -1.0, plane_right: 0.5, plane_bottom: 0.0,
        x_advance: 0.5,
    });
    Font {
        glyphs,
        line_height: 1.0,
        ascender: -1.0,
        descender: 0.1,
        atlas_width: 256,
        atlas_height: 256,
        distance_range: 4.0,
        kerning: HashMap::new(),
    }
}

#[test]
fn label_dirty_flag() {
    let mut label = Label::new([0.0, 0.0], 16.0, [1.0; 4]);
    assert!(!label.dirty);
    label.set_text("Hello");
    assert!(label.dirty);
}

#[test]
fn text_layer_clear_resets_buffers() {
    let mut layer = TextLayer::new();
    layer.set_font(make_font());
    layer.vertices.push(Vertex {
        position: [0.0, 0.0],
        tex_coords: [0.0, 0.0],
        color: [1.0, 1.0, 1.0, 1.0],
    });
    assert!(!layer.vertices.is_empty());
    layer.clear();
    assert!(layer.vertices.is_empty());
}

// ── Widgets ──────────────────────────────────────────────────────────────────

#[test]
fn dropdown_new_selected_is_zero() {
    let dd = Dropdown::new(["Alpha", "Beta", "Gamma"]);
    assert_eq!(dd.selected, 0);
}

#[test]
fn inputbox_new_is_empty() {
    let ib = InputBox::new(20);
    assert!(ib.value.is_empty());
}

#[test]
fn toggle_wraps() {
    let mut ts = ToggleSelector::new(["A", "B", "C"]);
    let n = ts.options.len();
    ts.selected = if ts.selected == 0 { n - 1 } else { ts.selected - 1 };
    assert_eq!(ts.selected, 2);
}

// ── Layout Engine ─────────────────────────────────────────────────────────────

#[test]
fn vstack_size_logic() {
    let padding = Padding::all(10.0);
    let mut max_w = 0.0f32;
    let mut total_h = padding.top + padding.bottom;
    let elements = vec![(100.0, 20.0), (50.0, 30.0)];
    for (i, (w, h)) in elements.iter().enumerate() {
        max_w = max_w.max(w + padding.left + padding.right);
        total_h += h;
        if i < elements.len() - 1 {
            total_h += 5.0; // spacing
        }
    }
    assert_eq!(max_w, 120.0);
    assert_eq!(total_h, 75.0);
}

#[test]
fn hstack_size_logic() {
    let padding = Padding::new(10.0, 5.0);
    let mut total_w = padding.left + padding.right;
    let mut max_h = 0.0f32;
    let elements = vec![(40.0, 20.0), (60.0, 40.0)];
    for (i, (w, h)) in elements.iter().enumerate() {
        total_w += w;
        max_h = max_h.max(*h);
        if i < elements.len() - 1 {
            total_w += 10.0; // spacing
        }
    }
    assert_eq!(total_w, 130.0);
    assert_eq!(max_h + padding.top + padding.bottom, 50.0);
}

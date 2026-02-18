use jengine::ui::{rect_contains, word_wrap};

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
fn wrap_exactly_at_limit_keeps_on_one_line() {
    // "ab cd" = 5 chars, limit = 5 → fits
    assert_eq!(word_wrap("ab cd", 5), vec!["ab cd"]);
}

#[test]
fn wrap_one_over_limit_breaks() {
    // "ab cd" = 5 chars, limit = 4 → must break
    let lines = word_wrap("ab cd", 4);
    assert_eq!(lines, vec!["ab", "cd"]);
}

#[test]
fn wrap_long_paragraph_no_line_exceeds_max_cols() {
    let text = "the quick brown fox jumps over the lazy dog and then runs away";
    let max_cols = 15;
    for line in word_wrap(text, max_cols) {
        assert!(line.len() <= max_cols, "line too long: '{line}'");
    }
}

#[test]
fn wrap_long_paragraph_preserves_all_words() {
    let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa";
    let lines = word_wrap(text, 12);
    let rebuilt = lines.join(" ");
    assert_eq!(rebuilt, text);
}

#[test]
fn wrap_single_word_longer_than_max_gets_split() {
    let lines = word_wrap("abcdefghij", 4);
    assert_eq!(lines, vec!["abcd", "efgh", "ij"]);
}

#[test]
fn wrap_zero_max_cols_returns_empty() {
    assert!(word_wrap("anything", 0).is_empty());
}

#[test]
fn wrap_multiple_spaces_treated_as_one_separator() {
    // split_whitespace collapses runs of whitespace
    let lines = word_wrap("a   b   c", 10);
    assert_eq!(lines, vec!["a b c"]);
}

// ── rect_contains ────────────────────────────────────────────────────────────

#[test]
fn rect_contains_center_point() {
    assert!(rect_contains(0.0, 0.0, 100.0, 100.0, 50.0, 50.0));
}

#[test]
fn rect_contains_left_edge_inclusive() {
    assert!(rect_contains(10.0, 10.0, 100.0, 100.0, 10.0, 50.0));
}

#[test]
fn rect_contains_top_edge_inclusive() {
    assert!(rect_contains(10.0, 10.0, 100.0, 100.0, 50.0, 10.0));
}

#[test]
fn rect_contains_right_edge_exclusive() {
    assert!(!rect_contains(10.0, 10.0, 100.0, 100.0, 110.0, 50.0));
}

#[test]
fn rect_contains_bottom_edge_exclusive() {
    assert!(!rect_contains(10.0, 10.0, 100.0, 100.0, 50.0, 110.0));
}

#[test]
fn rect_contains_above_rect_returns_false() {
    assert!(!rect_contains(0.0, 50.0, 100.0, 100.0, 50.0, 10.0));
}

#[test]
fn rect_contains_left_of_rect_returns_false() {
    assert!(!rect_contains(50.0, 0.0, 100.0, 100.0, 10.0, 50.0));
}

#[test]
fn rect_contains_zero_size_rect_never_contains() {
    assert!(!rect_contains(10.0, 10.0, 0.0, 0.0, 10.0, 10.0));
}

// ── Progress bar clamping (pure arithmetic, no GPU) ──────────────────────────

#[test]
fn progress_pct_below_zero_clamps_to_zero() {
    let pct = (-0.5f32).clamp(0.0, 1.0);
    assert_eq!(pct, 0.0);
}

#[test]
fn progress_pct_above_one_clamps_to_one() {
    let pct = (1.5f32).clamp(0.0, 1.0);
    assert_eq!(pct, 1.0);
}

#[test]
fn progress_pct_zero_means_no_filled_portion() {
    let pct = 0.0f32;
    let filled_w = 200.0 * pct;
    assert_eq!(filled_w, 0.0);
    let empty_w = 200.0 - filled_w;
    assert_eq!(empty_w, 200.0);
}

#[test]
fn progress_pct_one_means_full_filled_portion() {
    let pct = 1.0f32;
    let filled_w = 200.0 * pct;
    assert_eq!(filled_w, 200.0);
    let empty_w = 200.0 - filled_w;
    assert_eq!(empty_w, 0.0);
}

#[test]
fn progress_pct_half_splits_evenly() {
    let pct = 0.5f32;
    let total = 200.0f32;
    let filled_w = total * pct;
    assert!((filled_w - 100.0).abs() < 1e-5);
    assert!((total - filled_w - 100.0).abs() < 1e-5);
}
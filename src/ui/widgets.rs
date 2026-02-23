/// Interactive UI widgets: Dropdown, InputBox, ToggleSelector.
///
/// All widgets follow the same immediate-mode pattern used by the rest of the
/// jengine UI: call `draw()` once per frame from inside `Game::render()`.
/// State (selection, focus, open/closed) is stored inside the widget struct and
/// persists across frames.
///
/// # Click Consumption
///
/// When a widget fully handles a mouse click it sets `engine.ui.click_consumed`
/// to `true`.  Game code should check this flag before acting on clicks to
/// prevent UI interactions from also triggering world actions.
///
/// # Text Input
///
/// `InputBox` reads from `engine.chars_typed` (printable characters captured
/// each frame from `KeyEvent.text`) and drains the buffer as it processes
/// them.  This means if an `InputBox` is focused it will consume all typed
/// characters before the game sees them.
use crate::engine::{Color, jEngine, KeyCode};

// ── Internal widget palette ────────────────────────────────────────────────────
// All widgets share these defaults so they look consistent out of the box.

const BG:      Color = Color([0.07, 0.10, 0.10, 1.0]);
const BG_HOV:  Color = Color([0.13, 0.20, 0.18, 1.0]);
const BG_SEL:  Color = Color([0.08, 0.25, 0.18, 1.0]);
const BG_FOC:  Color = Color([0.10, 0.16, 0.15, 1.0]);
const BORDER:  Color = Color([0.25, 0.65, 0.50, 1.0]);
const BORDER_F: Color = Color([0.45, 0.90, 0.72, 1.0]);
const TEXT:    Color = Color([0.85, 0.92, 0.88, 1.0]);
const DIM:     Color = Color([0.38, 0.48, 0.45, 1.0]);
const TRANSP:  Color = Color::TRANSPARENT;

// ── Dropdown ──────────────────────────────────────────────────────────────────

/// Clickable dropdown menu.
///
/// Closed state shows the current selection and a `v`/`^` indicator.
/// Clicking the header toggles the list open; clicking an option selects it
/// and closes the list; clicking anywhere outside closes the list.
///
/// ```ignore
/// // Inside Game::render():
/// if let Some(idx) = self.my_dropdown.draw(engine, x, y, w) {
///     println!("Selected: {}", self.my_dropdown.selected_text());
/// }
/// ```
pub struct Dropdown {
    /// The selectable option strings.
    pub options: Vec<String>,
    /// Index of the currently selected option.
    pub selected: usize,
    /// Whether the option list is currently expanded.
    pub is_open: bool,
}

impl Dropdown {
    pub fn new(options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            options: options.into_iter().map(Into::into).collect(),
            selected: 0,
            is_open: false,
        }
    }

    /// Text of the currently selected option, or `""` if the list is empty.
    pub fn selected_text(&self) -> &str {
        self.options.get(self.selected).map(String::as_str).unwrap_or("")
    }

    /// Draw the dropdown at pixel position `(x, y)` with width `w`.
    ///
    /// Returns `Some(index)` if the user selected a different option this
    /// frame; `None` otherwise (including when the same option is re-clicked).
    pub fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, w: f32) -> Option<usize> {
        let th = engine.tile_height() as f32;
        let tw = engine.tile_width() as f32;
        let n = self.options.len();

        if n == 0 || w < tw * 3.0 {
            return None;
        }

        // ── Header ────────────────────────────────────────────────────────────
        let hov = engine.ui.is_mouse_over(x, y, w, th);
        let clicked = engine.ui.was_clicked(x, y, w, th) && !engine.ui.click_consumed;

        let header_bg = if hov || self.is_open { BG_HOV } else { BG };
        let border_col = if self.is_open { BORDER_F } else { BORDER };

        // Background + 1-px coloured border (outer border rect, inner bg rect).
        engine.ui.ui_rect(x, y, w, th, border_col);
        engine.ui.ui_rect(x + 1.0, y + 1.0, w - 2.0, th - 2.0, header_bg);

        // Label — truncated to leave room for the indicator ("  v") on the right.
        let max_label = ((w / tw) as usize).saturating_sub(4);
        let label: String = self.selected_text().chars().take(max_label).collect();
        engine.ui.ui_text(x + tw, y, &label, TEXT, TRANSP, None);

        // Indicator: "^" when open, "v" when closed.
        let arrow = if self.is_open { " ^" } else { " v" };
        engine.ui.ui_text(x + w - tw * 2.0, y, arrow, DIM, TRANSP, None);

        if clicked {
            self.is_open = !self.is_open;
            engine.ui.click_consumed = true;
        }

        // ── Expanded option list ──────────────────────────────────────────────
        let mut result = None;

        if self.is_open {
            let list_y = y + th;
            let list_h = th * n as f32;

            // Backdrop for the whole list.
            engine.ui.ui_rect(x, list_y, w, list_h, BG);
            // Outer border rect around the list.
            engine.ui.ui_rect(x, list_y, w, list_h, border_col);
            engine.ui.ui_rect(x + 1.0, list_y + 1.0, w - 2.0, list_h - 2.0, BG);

            for (i, option) in self.options.iter().enumerate() {
                let oy = list_y + i as f32 * th;
                let is_sel = i == self.selected;
                let is_hov = engine.ui.is_mouse_over(x, oy, w, th);
                let row_clicked = engine.ui.was_clicked(x, oy, w, th)
                    && !engine.ui.click_consumed;

                let row_bg = if is_hov { BG_HOV } else if is_sel { BG_SEL } else { BG };
                engine.ui.ui_rect(x + 1.0, oy, w - 2.0, th, row_bg);

                let prefix = if is_sel { "> " } else { "  " };
                let max_opt = ((w / tw) as usize).saturating_sub(3);
                let text: String = option.chars().take(max_opt).collect();
                let fg = if is_hov || is_sel { TEXT } else { DIM };
                engine.ui.ui_text(x + tw, oy, &format!("{prefix}{text}"), fg, TRANSP, None);

                if row_clicked {
                    if i != self.selected {
                        result = Some(i);
                        self.selected = i;
                    }
                    self.is_open = false;
                    engine.ui.click_consumed = true;
                }
            }

            // Close when the user clicks outside the entire dropdown area.
            if engine.ui.mouse_clicked && !engine.ui.click_consumed {
                if !engine.ui.is_mouse_over(x, y, w, th + list_h) {
                    self.is_open = false;
                    engine.ui.click_consumed = true;
                }
            }
        }

        result
    }
}

// ── InputBox ──────────────────────────────────────────────────────────────────

/// Single-line text input field.
///
/// Clicking inside gives keyboard focus; clicking outside removes it.
/// While focused, printable characters are appended (up to `max_chars`) and
/// `Backspace` removes the last character.  The caret blinks at ~1 Hz.
///
/// ```ignore
/// // Inside Game::render():
/// if self.my_input.draw(engine, x, y, w) {
///     println!("Text is now: {}", self.my_input.value);
/// }
/// ```
pub struct InputBox {
    /// Current text content.
    pub value: String,
    /// Maximum number of characters allowed.
    pub max_chars: usize,
    /// Whether this box currently holds keyboard focus.
    pub is_focused: bool,
    /// Blink timer [0, 1) — caret is visible when < 0.5.
    cursor_blink: f32,
}

impl InputBox {
    pub fn new(max_chars: usize) -> Self {
        Self {
            value: String::new(),
            max_chars: max_chars.max(1),
            is_focused: false,
            cursor_blink: 0.0,
        }
    }

    /// Draw the input box at pixel position `(x, y)` with width `w`.
    ///
    /// Returns `true` if `value` changed this frame.
    pub fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, w: f32) -> bool {
        let th = engine.tile_height() as f32;
        let tw = engine.tile_width() as f32;
        let dt = engine.dt();

        let hovered = engine.ui.is_mouse_over(x, y, w, th);
        let clicked_inside = engine.ui.was_clicked(x, y, w, th)
            && !engine.ui.click_consumed;
        let clicked_outside = engine.ui.mouse_clicked
            && !engine.ui.is_mouse_over(x, y, w, th)
            && !engine.ui.click_consumed;

        // ── Focus management ──────────────────────────────────────────────────
        if clicked_inside {
            self.is_focused = true;
            engine.ui.click_consumed = true;
        } else if clicked_outside {
            self.is_focused = false;
        }

        // ── Keyboard input (only while focused) ───────────────────────────────
        let mut changed = false;

        if self.is_focused {
            // Advance caret blink (period = 1 s → 0.5 s visible, 0.5 s hidden).
            self.cursor_blink = (self.cursor_blink + dt) % 1.0;

            // Drain and consume typed printable characters.
            let incoming: Vec<char> = engine.chars_typed.drain(..).collect();
            for ch in incoming {
                if self.value.chars().count() < self.max_chars {
                    self.value.push(ch);
                    changed = true;
                }
            }

            // Backspace removes the last character.
            if engine.is_key_pressed(KeyCode::Backspace) && !self.value.is_empty() {
                self.value.pop();
                changed = true;
            }
        } else {
            // Reset blink phase when not focused so caret starts visible on refocus.
            self.cursor_blink = 0.0;
        }

        // ── Visual rendering ──────────────────────────────────────────────────
        let border_col = if self.is_focused {
            BORDER_F
        } else if hovered {
            BORDER
        } else {
            DIM
        };
        let bg = if self.is_focused { BG_FOC } else { BG };

        // Outer border + inner background.
        engine.ui.ui_rect(x, y, w, th, border_col);
        engine.ui.ui_rect(x + 1.0, y + 1.0, w - 2.0, th - 2.0, bg);

        // Compute the visible window of the text.
        // We show the trailing portion + caret so the user always sees what
        // they just typed, even when the string is longer than the widget.
        let max_visible = ((w / tw) as usize).saturating_sub(2); // 1-tile pad each side
        let caret = if self.is_focused && self.cursor_blink < 0.5 { "|" } else { " " };
        let char_count = self.value.chars().count();
        let display: String = if char_count + 1 > max_visible {
            let start = char_count + 1 - max_visible;
            self.value.chars().skip(start).collect::<String>() + caret
        } else {
            self.value.clone() + caret
        };

        let text_fg = if self.is_focused || hovered { TEXT } else { DIM };
        engine.ui.ui_text(x + tw, y, &display, text_fg, TRANSP, None);

        changed
    }
}

// ── ToggleSelector ────────────────────────────────────────────────────────────

/// Arrow-based selector: `[<]  Current Option  [>]`.
///
/// Left/right arrow buttons cycle through the options list.  Both wrap around.
/// If there is only one option, the arrows are dimmed and non-interactive.
///
/// ```ignore
/// // Inside Game::render():
/// if let Some(idx) = self.my_selector.draw(engine, x, y, w) {
///     println!("Now showing option {}", idx);
/// }
/// ```
pub struct ToggleSelector {
    /// The selectable option strings.
    pub options: Vec<String>,
    /// Index of the currently selected option.
    pub selected: usize,
}

impl ToggleSelector {
    pub fn new(options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            options: options.into_iter().map(Into::into).collect(),
            selected: 0,
        }
    }

    /// Text of the currently selected option, or `""` if the list is empty.
    pub fn selected_text(&self) -> &str {
        self.options.get(self.selected).map(String::as_str).unwrap_or("")
    }

    /// Draw the toggle selector at pixel position `(x, y)` with total width `w`.
    ///
    /// Returns `Some(index)` if the selection changed this frame; `None` otherwise.
    pub fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, w: f32) -> Option<usize> {
        let th = engine.tile_height() as f32;
        let tw = engine.tile_width() as f32;
        let n = self.options.len();

        // Minimum width: two 3-char buttons + at least one char of label.
        if n == 0 || w < tw * 7.0 {
            return None;
        }

        let btn_w = tw * 3.0; // "[<]" / "[>]" each occupy 3 tile-widths.
        let lbl_x = x + btn_w;
        let lbl_w = w - btn_w * 2.0;

        let left_hov  = engine.ui.is_mouse_over(x, y, btn_w, th);
        let right_hov = engine.ui.is_mouse_over(x + w - btn_w, y, btn_w, th);
        let left_click  = engine.ui.was_clicked(x, y, btn_w, th)
            && !engine.ui.click_consumed;
        let right_click = engine.ui.was_clicked(x + w - btn_w, y, btn_w, th)
            && !engine.ui.click_consumed;

        let can_cycle = n > 1;

        // ── Background ────────────────────────────────────────────────────────
        // Full-width border, then inner bg.
        engine.ui.ui_rect(x, y, w, th, BORDER);
        engine.ui.ui_rect(x + 1.0, y + 1.0, w - 2.0, th - 2.0, BG);

        // ── Left arrow button ─────────────────────────────────────────────────
        if left_hov && can_cycle {
            engine.ui.ui_rect(x + 1.0, y + 1.0, btn_w - 1.0, th - 2.0, BG_HOV);
        }
        let lfg = if !can_cycle { DIM } else if left_hov { TEXT } else { BORDER };
        engine.ui.ui_text(x, y, "[<]", lfg, TRANSP, None);

        // ── Right arrow button ────────────────────────────────────────────────
        if right_hov && can_cycle {
            engine.ui.ui_rect(x + w - btn_w, y + 1.0, btn_w - 1.0, th - 2.0, BG_HOV);
        }
        let rfg = if !can_cycle { DIM } else if right_hov { TEXT } else { BORDER };
        engine.ui.ui_text(x + w - btn_w, y, "[>]", rfg, TRANSP, None);

        // ── Centred label ─────────────────────────────────────────────────────
        let label = self.selected_text();
        let max_cols = (lbl_w / tw) as usize;
        let truncated: String = label.chars().take(max_cols).collect();
        // Centre the text inside the label zone.
        let pad = ((max_cols.saturating_sub(truncated.chars().count())) / 2) as f32;
        engine.ui.ui_text(lbl_x + pad * tw, y, &truncated, TEXT, TRANSP, None);

        // ── Interaction ───────────────────────────────────────────────────────
        let mut result = None;

        if left_click && can_cycle {
            self.selected = if self.selected == 0 { n - 1 } else { self.selected - 1 };
            result = Some(self.selected);
            engine.ui.click_consumed = true;
        }
        if right_click && can_cycle {
            self.selected = (self.selected + 1) % n;
            result = Some(self.selected);
            engine.ui.click_consumed = true;
        }

        result
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Dropdown ──────────────────────────────────────────────────────────────

    #[test]
    fn dropdown_new_selected_is_zero() {
        let dd = Dropdown::new(["Alpha", "Beta", "Gamma"]);
        assert_eq!(dd.selected, 0);
        assert!(!dd.is_open);
    }

    #[test]
    fn dropdown_selected_text_returns_current() {
        let mut dd = Dropdown::new(["Alpha", "Beta"]);
        dd.selected = 1;
        assert_eq!(dd.selected_text(), "Beta");
    }

    #[test]
    fn dropdown_selected_text_empty_when_no_options() {
        let dd = Dropdown::new(std::iter::empty::<&str>());
        assert_eq!(dd.selected_text(), "");
    }

    #[test]
    fn dropdown_selected_text_oob_is_empty() {
        let mut dd = Dropdown::new(["Only"]);
        dd.selected = 5; // out of range
        assert_eq!(dd.selected_text(), "");
    }

    // ── InputBox ──────────────────────────────────────────────────────────────

    #[test]
    fn inputbox_new_is_empty_and_unfocused() {
        let ib = InputBox::new(20);
        assert!(ib.value.is_empty());
        assert!(!ib.is_focused);
        assert_eq!(ib.max_chars, 20);
    }

    #[test]
    fn inputbox_max_chars_minimum_is_one() {
        let ib = InputBox::new(0);
        assert_eq!(ib.max_chars, 1);
    }

    #[test]
    fn inputbox_cursor_starts_at_zero() {
        let ib = InputBox::new(10);
        assert_eq!(ib.cursor_blink, 0.0);
    }

    // ── ToggleSelector ────────────────────────────────────────────────────────

    #[test]
    fn toggle_new_selected_is_zero() {
        let ts = ToggleSelector::new(["720p", "1080p", "1440p"]);
        assert_eq!(ts.selected, 0);
    }

    #[test]
    fn toggle_selected_text_wraps() {
        let mut ts = ToggleSelector::new(["A", "B", "C"]);
        ts.selected = 2;
        assert_eq!(ts.selected_text(), "C");
    }

    #[test]
    fn toggle_selected_text_empty_when_no_options() {
        let ts = ToggleSelector::new(std::iter::empty::<&str>());
        assert_eq!(ts.selected_text(), "");
    }

    #[test]
    fn toggle_left_wraps_from_zero_to_last() {
        // Simulate the left-click logic without needing a real engine.
        let mut ts = ToggleSelector::new(["A", "B", "C"]);
        let n = ts.options.len();
        ts.selected = if ts.selected == 0 { n - 1 } else { ts.selected - 1 };
        assert_eq!(ts.selected, 2);
    }

    #[test]
    fn toggle_right_wraps_from_last_to_zero() {
        let mut ts = ToggleSelector::new(["A", "B", "C"]);
        ts.selected = 2;
        let n = ts.options.len();
        ts.selected = (ts.selected + 1) % n;
        assert_eq!(ts.selected, 0);
    }

    #[test]
    fn toggle_single_option_no_cycle() {
        // With one option, `can_cycle` is false — clicking should be a no-op.
        let ts = ToggleSelector::new(["Only"]);
        let can_cycle = ts.options.len() > 1;
        assert!(!can_cycle);
    }
}
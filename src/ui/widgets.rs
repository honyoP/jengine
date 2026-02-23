/// Interactive UI widgets: Dropdown, InputBox, ToggleSelector.
///
/// All widgets follow the same immediate-mode pattern used by the rest of the
/// jengine UI: call `draw()` once per frame from inside `Game::render()`.
/// State (selection, focus, open/closed) is stored inside the widget struct and
/// persists across frames.
///
/// # Click Consumption
///
/// When a widget fully handles a mouse click it sets `engine.input.mouse_consumed`
/// to `true`.  Game code should check this flag before acting on clicks to
/// prevent UI interactions from also triggering world actions.
///
/// # Text Input
///
/// `InputBox` reads from `engine.input.chars_typed` (printable characters captured
/// each frame from `KeyEvent.text`) and drains the buffer as it processes
/// them.  This means if an `InputBox` is focused it will consume all typed
/// characters before the game sees them.
use crate::engine::{Color, jEngine, KeyCode};
use crate::input::MouseButton;

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
        let hov = engine.input.is_mouse_over(x, y, w, th);
        let clicked = engine.input.was_clicked(x, y, w, th) && !engine.input.mouse_consumed;

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
            engine.input.mouse_consumed = true;
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
                let is_hov = engine.input.is_mouse_over(x, oy, w, th);
                let row_clicked = engine.input.was_clicked(x, oy, w, th)
                    && !engine.input.mouse_consumed;

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
                    engine.input.mouse_consumed = true;
                }
            }

            // Close when the user clicks outside the entire dropdown area.
            // Do NOT consume the click — the outside click should remain available
            // for other widgets or game code to handle.
            if engine.input.is_mouse_pressed(MouseButton::Left) && !engine.input.mouse_consumed {
                if !engine.input.is_mouse_over(x, y, w, th + list_h) {
                    self.is_open = false;
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

        let hovered = engine.input.is_mouse_over(x, y, w, th);
        let clicked_inside = engine.input.was_clicked(x, y, w, th)
            && !engine.input.mouse_consumed;
        let clicked_outside = engine.input.is_mouse_pressed(MouseButton::Left)
            && !engine.input.is_mouse_over(x, y, w, th)
            && !engine.input.mouse_consumed;

        // ── Focus management ──────────────────────────────────────────────────
        if clicked_inside {
            self.is_focused = true;
            engine.input.mouse_consumed = true;
        } else if clicked_outside {
            self.is_focused = false;
        }

        // ── Keyboard input (only while focused) ───────────────────────────────
        let mut changed = false;

        if self.is_focused {
            // Mark keys as consumed while we have focus
            engine.input.key_consumed = true;

            // Advance caret blink (period = 1 s → 0.5 s visible, 0.5 s hidden).
            self.cursor_blink = (self.cursor_blink + dt) % 1.0;

            // Drain and consume typed printable characters.
            let incoming: Vec<char> = engine.input.chars_typed.drain(..).collect();
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

        let left_hov  = engine.input.is_mouse_over(x, y, btn_w, th);
        let right_hov = engine.input.is_mouse_over(x + w - btn_w, y, btn_w, th);
        let left_click  = engine.input.was_clicked(x, y, btn_w, th)
            && !engine.input.mouse_consumed;
        let right_click = engine.input.was_clicked(x + w - btn_w, y, btn_w, th)
            && !engine.input.mouse_consumed;

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
            engine.input.mouse_consumed = true;
        }
        if right_click && can_cycle {
            self.selected = (self.selected + 1) % n;
            result = Some(self.selected);
            engine.input.mouse_consumed = true;
        }

        result
    }
}

use super::{Alignment, Padding, BorderStyle, Label, Rect};
use crate::renderer::text::text_width;

// ── Layout Engine ─────────────────────────────────────────────────────────────

/// Trait for UI elements that can be laid out and drawn.
pub trait Widget {
    /// Calculate the intrinsic pixel size of the widget.
    fn size(&self, engine: &jEngine) -> (f32, f32);
    /// Draw the widget at the given coordinates.
    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, available_w: f32, clip: Option<Rect>);
}

// ── Concrete Widgets ──────────────────────────────────────────────────────────

/// A simple line of text.
pub struct TextWidget {
    pub text: String,
    pub size: Option<f32>,
    pub color: Color,
}

impl Widget for TextWidget {
    fn size(&self, engine: &jEngine) -> (f32, f32) {
        let fs = self.size.unwrap_or(engine.tile_height() as f32);
        if let Some(font) = &engine.ui.text.font {
            (text_width(&self.text, font, fs), fs)
        } else {
            (self.text.chars().count() as f32 * engine.tile_width() as f32, fs)
        }
    }

    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, _available_w: f32, clip: Option<Rect>) {
        if let Some(c) = clip {
            let (_, h) = self.size(engine);
            if y + h < c.y || y > c.y + c.h {
                return; // Simple vertical culling
            }
        }
        engine.ui.ui_text(x, y, &self.text, self.color, TRANSP, self.size);
    }
}

/// A wrapper for a pre-existing Label.
pub struct LabelWidget<'a> {
    pub label: &'a mut Label,
}

impl<'a> Widget for LabelWidget<'a> {
    fn size(&self, engine: &jEngine) -> (f32, f32) {
        if let Some(font) = &engine.ui.text.font {
            (text_width(self.label.text(), font, self.label.font_size()), self.label.font_size())
        } else {
            (self.label.text().chars().count() as f32 * engine.tile_width() as f32, self.label.font_size())
        }
    }

    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, _available_w: f32, clip: Option<Rect>) {
        if let Some(c) = clip {
            let (_, h) = self.size(engine);
            if y + h < c.y || y > c.y + c.h {
                return;
            }
        }
        self.label.set_position([x, y]);
        self.label.draw(&mut engine.ui.text);
    }
}

/// A solid-colored rectangle.
pub struct RectWidget {
    pub w: f32,
    pub h: f32,
    pub color: Color,
}

impl Widget for RectWidget {
    fn size(&self, _engine: &jEngine) -> (f32, f32) { (self.w, self.h) }
    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, _available_w: f32, clip: Option<Rect>) {
        if let Some(c) = clip {
            if y + self.h < c.y || y > c.y + c.h {
                return;
            }
        }
        engine.ui.ui_rect(x, y, self.w, self.h, self.color);
    }
}

/// Fixed-size gap.
pub struct Spacer {
    pub size: f32,
    pub horizontal: bool,
}

impl Widget for Spacer {
    fn size(&self, _engine: &jEngine) -> (f32, f32) {
        if self.horizontal { (self.size, 0.0) } else { (0.0, self.size) }
    }
    fn draw(&mut self, _engine: &mut jEngine, _x: f32, _y: f32, _available_w: f32, _clip: Option<Rect>) {}
}

// ── Layout Containers ─────────────────────────────────────────────────────────

/// Vertical stack layout container.
pub struct VStack<'a> {
    pub widgets: Vec<Box<dyn Widget + 'a>>,
    pub alignment: Alignment,
    pub padding: Padding,
    pub spacing: f32,
    pub min_width: f32,
    pub bg_color: Option<Color>,
    pub border: Option<(BorderStyle, Color)>,
}

impl<'a> VStack<'a> {
    pub fn new(alignment: Alignment) -> Self {
        Self {
            widgets: Vec::new(),
            alignment,
            padding: Padding::default(),
            spacing: 0.0,
            min_width: 0.0,
            bg_color: None,
            border: None,
        }
    }

    pub fn with_padding(mut self, padding: Padding) -> Self { self.padding = padding; self }
    pub fn with_spacing(mut self, spacing: f32) -> Self { self.spacing = spacing; self }
    pub fn with_min_width(mut self, min_width: f32) -> Self { self.min_width = min_width; self }
    pub fn with_bg(mut self, color: Color) -> Self { self.bg_color = Some(color); self }
    pub fn with_border(mut self, style: BorderStyle, color: Color) -> Self { self.border = Some((style, color)); self }

    pub fn add(mut self, widget: impl Widget + 'a) -> Self {
        self.widgets.push(Box::new(widget));
        self
    }
}

impl<'a> Widget for VStack<'a> {
    fn size(&self, engine: &jEngine) -> (f32, f32) {
        let mut max_w = self.min_width;
        let mut total_h = self.padding.top + self.padding.bottom;

        for (i, w) in self.widgets.iter().enumerate() {
            let (ww, wh) = w.size(engine);
            max_w = max_w.max(ww + self.padding.left + self.padding.right);
            total_h += wh;
            if i < self.widgets.len() - 1 {
                total_h += self.spacing;
            }
        }
        (max_w, total_h)
    }

    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, _available_w: f32, clip: Option<Rect>) {
        // Pre-compute child sizes once to avoid calling size() twice per child
        // (once for the stack dimensions, once for layout). Without this,
        // nested containers cause O(N²) size() calls.
        let sizes: Vec<(f32, f32)> = self.widgets.iter().map(|w| w.size(engine)).collect();

        // Recompute stack dimensions from the cached sizes.
        let mut stack_w = self.min_width;
        let mut stack_h = self.padding.top + self.padding.bottom;
        for (i, &(ww, wh)) in sizes.iter().enumerate() {
            stack_w = stack_w.max(ww + self.padding.left + self.padding.right);
            stack_h += wh;
            if i < sizes.len() - 1 { stack_h += self.spacing; }
        }

        // Draw background and border
        if let Some(bg) = self.bg_color {
            engine.ui.ui_rect(x, y, stack_w, stack_h, bg);
        }
        if let Some((style, color)) = self.border {
            engine.ui.ui_box(x, y, stack_w, stack_h, style, color, Color::TRANSPARENT);
        }

        let content_w = stack_w - self.padding.left - self.padding.right;
        let mut current_y = y + self.padding.top;

        for (w, &(ww, wh)) in self.widgets.iter_mut().zip(sizes.iter()) {
            let item_x = match self.alignment {
                Alignment::Start => x + self.padding.left,
                // Centre within the padded content area; padding always respected.
                Alignment::Center => x + self.padding.left + (content_w - ww) * 0.5,
                Alignment::End => x + stack_w - ww - self.padding.right,
            };
            w.draw(engine, item_x, current_y, content_w, clip);
            current_y += wh + self.spacing;
        }
    }
}

/// Horizontal stack layout container.
pub struct HStack<'a> {
    pub widgets: Vec<Box<dyn Widget + 'a>>,
    pub alignment: Alignment, // Vertical alignment within the row
    pub padding: Padding,
    pub spacing: f32,
}

impl<'a> HStack<'a> {
    pub fn new(alignment: Alignment) -> Self {
        Self {
            widgets: Vec::new(),
            alignment,
            padding: Padding::default(),
            spacing: 0.0,
        }
    }

    pub fn with_padding(mut self, padding: Padding) -> Self { self.padding = padding; self }
    pub fn with_spacing(mut self, spacing: f32) -> Self { self.spacing = spacing; self }

    pub fn add(mut self, widget: impl Widget + 'a) -> Self {
        self.widgets.push(Box::new(widget));
        self
    }
}

impl<'a> Widget for HStack<'a> {
    fn size(&self, engine: &jEngine) -> (f32, f32) {
        let mut total_w = self.padding.left + self.padding.right;
        let mut max_h = 0.0f32;

        for (i, w) in self.widgets.iter().enumerate() {
            let (ww, wh) = w.size(engine);
            total_w += ww;
            max_h = max_h.max(wh);
            if i < self.widgets.len() - 1 {
                total_w += self.spacing;
            }
        }
        (total_w, max_h + self.padding.top + self.padding.bottom)
    }

    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, _available_w: f32, clip: Option<Rect>) {
        // Pre-compute child sizes once to avoid calling size() twice per child.
        let sizes: Vec<(f32, f32)> = self.widgets.iter().map(|w| w.size(engine)).collect();

        // Recompute stack height from the cached sizes.
        let max_h = sizes.iter().map(|&(_, h)| h).fold(0.0f32, f32::max);
        let stack_h = max_h + self.padding.top + self.padding.bottom;

        let mut current_x = x + self.padding.left;

        for (w, &(ww, wh)) in self.widgets.iter_mut().zip(sizes.iter()) {
            let item_y = match self.alignment {
                Alignment::Start => y + self.padding.top,
                Alignment::Center => y + (stack_h - wh) * 0.5,
                Alignment::End => y + stack_h - wh - self.padding.bottom,
            };
            w.draw(engine, current_x, item_y, ww, clip);
            current_x += ww + self.spacing;
        }
    }
}
        
// ── ScrollContainer ───────────────────────────────────────────────────────────

/// A container that clips its content to a fixed height and allows scrolling.
pub struct ScrollContainer<'a> {
    pub inner: Box<dyn Widget + 'a>,
    pub max_height: f32,
    pub scroll_offset: &'a mut f32,
}

impl<'a> Widget for ScrollContainer<'a> {
    fn size(&self, engine: &jEngine) -> (f32, f32) {
        let (w, h) = self.inner.size(engine);
        // Only reserve scrollbar width when the content actually overflows.
        let scrollbar_w = if h > self.max_height { 10.0 } else { 0.0 };
        (w + scrollbar_w, h.min(self.max_height))
    }

    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, available_w: f32, _clip: Option<Rect>) {
        let (_inner_w, inner_h) = self.inner.size(engine);
        let visible_h = inner_h.min(self.max_height);

        // ── Handle Mouse Wheel ──
        let hovered = engine.input.is_mouse_over(x, y, available_w, visible_h);
        if hovered && engine.input.mouse_wheel != 0.0 {
            let speed = 100.0;
            *self.scroll_offset = (*self.scroll_offset - engine.input.mouse_wheel * speed)
                .clamp(0.0, (inner_h - visible_h).max(0.0));
        }

        // ── Draw Clipped Content ──
        let clip_rect = Rect::new(x, y, available_w, visible_h);
        engine.ui.push_scissor(clip_rect);
        self.inner.draw(engine, x, y - *self.scroll_offset, available_w - 10.0, Some(clip_rect));
        engine.ui.pop_scissor();

        // ── Draw Scroll Indicator ──
        if inner_h > visible_h {
            let bar_x = x + available_w - 6.0;
            let bar_w = 4.0;

            // Track (background)
            engine.ui.ui_rect(bar_x, y, bar_w, visible_h, Color([0.0, 0.0, 0.0, 0.2]));

            // Handle (foreground)
            let handle_h = (visible_h / inner_h) * visible_h;
            let handle_y = y + (*self.scroll_offset / inner_h) * visible_h;
            engine.ui.ui_rect(bar_x, handle_y, bar_w, handle_h, Color::CYAN);
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────


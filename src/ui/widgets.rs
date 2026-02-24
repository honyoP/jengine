use crate::engine::{Color, jEngine, KeyCode};
use crate::input::MouseButton;
use super::modern::Panel;
use super::{Alignment, Padding, BorderStyle, Label, Rect};
use crate::renderer::text::text_width;

// ── Dropdown ──────────────────────────────────────────────────────────────────

pub struct Dropdown {
    pub options: Vec<String>,
    pub selected: usize,
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

    pub fn selected_text(&self) -> &str {
        self.options.get(self.selected).map(String::as_str).unwrap_or("")
    }

    pub fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, w: f32) -> Option<usize> {
        let th = engine.tile_height() as f32;
        let tw = engine.tile_width() as f32;
        let n = self.options.len();
        let theme = engine.ui.theme.clone();

        if n == 0 || w < tw * 3.0 { return None; }

        let hov = engine.input.is_mouse_over(x, y, w, th);
        let clicked = engine.input.was_clicked(x, y, w, th) && !engine.input.mouse_consumed;

        let header_bg = if hov || self.is_open { theme.selection_bg } else { theme.surface };
        let border_col = if self.is_open { theme.border_focus } else { theme.border };

        Panel::new(x, y, w, th)
            .with_color(header_bg)
            .with_border(border_col, 1.0)
            .with_radius(4.0)
            .draw(engine);

        let label: String = self.selected_text().chars().take(((w / tw) as usize).saturating_sub(4)).collect();
        engine.ui.ui_text(x + tw, y + th * 0.1, &label, theme.text_normal, Color::TRANSPARENT, Some(th * 0.8));

        let arrow = if self.is_open { "^" } else { "v" };
        engine.ui.ui_text(x + w - tw * 1.5, y + th * 0.1, arrow, theme.text_dim, Color::TRANSPARENT, Some(th * 0.8));

        if clicked { self.is_open = !self.is_open; engine.input.mouse_consumed = true; }

        let mut result = None;
        if self.is_open {
            let list_y = y + th + 2.0;
            let list_h = th * n as f32;

            Panel::new(x, list_y, w, list_h)
                .with_color(theme.surface)
                .with_border(border_col, 1.0)
                .with_radius(4.0)
                .draw(engine);

            for (i, option) in self.options.iter().enumerate() {
                let oy = list_y + i as f32 * th;
                let is_sel = i == self.selected;
                let is_hov = engine.input.is_mouse_over(x, oy, w, th);
                
                if is_hov || is_sel {
                    Panel::new(x + 2.0, oy + 2.0, w - 4.0, th - 4.0)
                        .with_color(theme.selection_bg)
                        .with_radius(2.0)
                        .draw(engine);
                }

                let fg = if is_hov || is_sel { theme.text_accent } else { theme.text_normal };
                engine.ui.ui_text(x + tw, oy + th * 0.1, option, fg, Color::TRANSPARENT, Some(th * 0.8));

                if engine.input.was_clicked(x, oy, w, th) && !engine.input.mouse_consumed {
                    if i != self.selected { result = Some(i); self.selected = i; }
                    self.is_open = false;
                    engine.input.mouse_consumed = true;
                }
            }

            if engine.input.is_mouse_pressed(MouseButton::Left) && !engine.input.mouse_consumed {
                if !engine.input.is_mouse_over(x, y, w, th + list_h + 2.0) { self.is_open = false; }
            }
        }
        result
    }
}

// ── InputBox ──────────────────────────────────────────────────────────────────

pub struct InputBox {
    pub value: String,
    pub max_chars: usize,
    pub is_focused: bool,
    cursor_blink: f32,
}

impl InputBox {
    pub fn new(max_chars: usize) -> Self {
        Self { value: String::new(), max_chars: max_chars.max(1), is_focused: false, cursor_blink: 0.0 }
    }

    pub fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, w: f32) -> bool {
        let th = engine.tile_height() as f32;
        let tw = engine.tile_width() as f32;
        let theme = engine.ui.theme.clone();

        let hovered = engine.input.is_mouse_over(x, y, w, th);
        if engine.input.was_clicked(x, y, w, th) && !engine.input.mouse_consumed {
            self.is_focused = true; engine.input.mouse_consumed = true;
        } else if engine.input.is_mouse_pressed(MouseButton::Left) && !engine.input.is_mouse_over(x, y, w, th) {
            // Only defocus when clicking outside the input box area.
            self.is_focused = false;
        }

        let mut changed = false;
        if self.is_focused {
            engine.input.key_consumed = true;
            self.cursor_blink = (self.cursor_blink + engine.dt()) % 1.0;
            let incoming: Vec<char> = engine.input.chars_typed.drain(..).collect();
            for ch in incoming {
                if self.value.chars().count() < self.max_chars { self.value.push(ch); changed = true; }
            }
            if engine.is_key_pressed(KeyCode::Backspace) && !self.value.is_empty() { self.value.pop(); changed = true; }
        }

        let border_col = if self.is_focused { theme.border_focus } else if hovered { theme.border } else { theme.text_dim };
        Panel::new(x, y, w, th)
            .with_color(if self.is_focused { theme.selection_bg } else { theme.surface })
            .with_border(border_col, 1.0)
            .with_radius(4.0)
            .draw(engine);

        let caret = if self.is_focused && self.cursor_blink < 0.5 { "|" } else { "" };
        let display = self.value.clone() + caret;
        engine.ui.ui_text(x + tw * 0.5, y + th * 0.1, &display, theme.text_normal, Color::TRANSPARENT, Some(th * 0.8));
        changed
    }
}

// ── ToggleSelector ────────────────────────────────────────────────────────────

pub struct ToggleSelector {
    pub options: Vec<String>,
    pub selected: usize,
}

impl ToggleSelector {
    pub fn new(options: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self { options: options.into_iter().map(Into::into).collect(), selected: 0 }
    }

    pub fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, w: f32) -> Option<usize> {
        let th = engine.tile_height() as f32;
        let tw = engine.tile_width() as f32;
        let theme = engine.ui.theme.clone();
        if self.options.is_empty() || w < tw * 6.0 { return None; }

        Panel::new(x, y, w, th).with_color(theme.surface).with_border(theme.border, 1.0).with_radius(4.0).draw(engine);

        let btn_w = tw * 2.0;
        if engine.input.was_clicked(x, y, btn_w, th) { self.selected = if self.selected == 0 { self.options.len() - 1 } else { self.selected - 1 }; return Some(self.selected); }
        if engine.input.was_clicked(x + w - btn_w, y, btn_w, th) { self.selected = (self.selected + 1) % self.options.len(); return Some(self.selected); }

        engine.ui.ui_text(x + tw * 0.5, y + th * 0.1, "<", theme.text_accent, Color::TRANSPARENT, Some(th * 0.8));
        engine.ui.ui_text(x + w - tw * 1.5, y + th * 0.1, ">", theme.text_accent, Color::TRANSPARENT, Some(th * 0.8));
        let label = self.options.get(self.selected).map(|s| s.as_str()).unwrap_or("");
        engine.ui.ui_text(x + w * 0.5 - (label.len() as f32 * tw * 0.3), y + th * 0.1, label, theme.text_normal, Color::TRANSPARENT, Some(th * 0.8));
        None
    }
}

// ── Layout Engine ─────────────────────────────────────────────────────────────

pub trait Widget {
    fn size(&self, engine: &jEngine) -> (f32, f32);
    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, available_w: f32, clip: Option<Rect>);
}

pub struct TextWidget {
    pub text: String,
    pub size: Option<f32>,
    pub color: Option<Color>,
}

impl Widget for TextWidget {
    fn size(&self, engine: &jEngine) -> (f32, f32) {
        let fs = self.size.unwrap_or(engine.tile_height() as f32);
        let stripped = self.text.replace("[/c]", "").split("[c:").map(|s| if let Some(pos) = s.find(']') { &s[pos+1..] } else { s }).collect::<String>();
        if let Some(font) = &engine.ui.text.font { (text_width(&stripped, font, fs), fs) } else { (stripped.len() as f32 * tw_factor(fs), fs) }
    }
    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, _available_w: f32, _clip: Option<Rect>) {
        let color = self.color.unwrap_or(engine.ui.theme.text_normal);
        engine.ui.ui_text(x, y, &self.text, color, Color::TRANSPARENT, self.size);
    }
}

fn tw_factor(fs: f32) -> f32 { fs * 0.6 }

pub struct IconText<'a> {
    pub sprite: &'a str,
    pub text: String,
    pub color: Option<Color>,
    pub spacing: f32,
}

impl<'a> Widget for IconText<'a> {
    fn size(&self, engine: &jEngine) -> (f32, f32) {
        let th = engine.tile_height() as f32;
        let tw = engine.tile_width() as f32;
        let text_w = if let Some(font) = &engine.ui.text.font { text_width(&self.text, font, th) } else { self.text.len() as f32 * tw };
        (tw + self.spacing + text_w, th)
    }
    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, _available_w: f32, _clip: Option<Rect>) {
        let color = self.color.unwrap_or(engine.ui.theme.text_normal);
        engine.draw_sprite((x / engine.tile_width() as f32) as u32, (y / engine.tile_height() as f32) as u32, self.sprite, 1, color);
        engine.ui.ui_text(x + engine.tile_width() as f32 + self.spacing, y, &self.text, color, Color::TRANSPARENT, None);
    }
}

pub struct LabelWidget<'a> { pub label: &'a mut Label }
impl<'a> Widget for LabelWidget<'a> {
    fn size(&self, engine: &jEngine) -> (f32, f32) {
        let fs = self.label.font_size;
        if let Some(font) = &engine.ui.text.font { (text_width(self.label.text(), font, fs), fs) } else { (self.label.text().len() as f32 * tw_factor(fs), fs) }
    }
    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, _w: f32, _c: Option<Rect>) { self.label.set_position([x, y]); self.label.draw(&mut engine.ui.text); }
}

pub struct RectWidget { pub w: f32, pub h: f32, pub color: Color, pub radius: f32 }
impl Widget for RectWidget {
    fn size(&self, _: &jEngine) -> (f32, f32) { (self.w, self.h) }
    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, _w: f32, _c: Option<Rect>) {
        Panel::new(x, y, self.w, self.h).with_color(self.color).with_radius(self.radius).draw(engine);
    }
}

pub struct Spacer { pub size: f32, pub horizontal: bool }
impl Widget for Spacer {
    fn size(&self, _: &jEngine) -> (f32, f32) { if self.horizontal { (self.size, 0.0) } else { (0.0, self.size) } }
    fn draw(&mut self, _: &mut jEngine, _: f32, _: f32, _: f32, _: Option<Rect>) {}
}

pub struct VStack<'a> {
    pub widgets: Vec<Box<dyn Widget + 'a>>,
    pub alignment: Alignment,
    pub padding: Padding,
    pub spacing: f32,
    pub min_width: f32,
    pub bg_color: Option<Color>,
    pub border: Option<(BorderStyle, Color)>,
    pub radius: f32,
}

impl<'a> VStack<'a> {
    pub fn new(alignment: Alignment) -> Self { Self { widgets: vec![], alignment, padding: Padding::default(), spacing: 0.0, min_width: 0.0, bg_color: None, border: None, radius: 0.0 } }
    pub fn with_padding(mut self, p: Padding) -> Self { self.padding = p; self }
    pub fn with_spacing(mut self, s: f32) -> Self { self.spacing = s; self }
    pub fn with_min_width(mut self, w: f32) -> Self { self.min_width = w; self }
    pub fn with_bg(mut self, c: Color) -> Self { self.bg_color = Some(c); self }
    pub fn with_border(mut self, s: BorderStyle, c: Color) -> Self { self.border = Some((s, c)); self }
    pub fn with_radius(mut self, r: f32) -> Self { self.radius = r; self }
    pub fn add(mut self, w: impl Widget + 'a) -> Self { self.widgets.push(Box::new(w)); self }
}

impl<'a> Widget for VStack<'a> {
    fn size(&self, engine: &jEngine) -> (f32, f32) {
        let mut max_w = self.min_width;
        let mut total_h = self.padding.top + self.padding.bottom;
        for (i, w) in self.widgets.iter().enumerate() {
            let (ww, wh) = w.size(engine);
            max_w = max_w.max(ww + self.padding.left + self.padding.right);
            total_h += wh;
            if i < self.widgets.len() - 1 { total_h += self.spacing; }
        }
        (max_w, total_h)
    }
    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, _aw: f32, _clip: Option<Rect>) {
        // Pre-compute child sizes once to avoid O(N²) double-calls with self.size(engine).
        let sizes: Vec<(f32, f32)> = self.widgets.iter().map(|w| w.size(engine)).collect();
        let mut max_w = self.min_width;
        let mut total_h = self.padding.top + self.padding.bottom;
        for (i, &(ww, wh)) in sizes.iter().enumerate() {
            max_w = max_w.max(ww + self.padding.left + self.padding.right);
            total_h += wh;
            if i < sizes.len() - 1 { total_h += self.spacing; }
        }
        let stack_w = max_w;
        let stack_h = total_h;

        if let Some(bg) = self.bg_color {
            let mut p = Panel::new(x, y, stack_w, stack_h).with_color(bg).with_radius(self.radius);
            if let Some((style, col)) = self.border {
                let thick = match style { BorderStyle::None => 0.0, BorderStyle::Thin => 1.0, BorderStyle::Thick => 2.0 };
                p = p.with_border(col, thick);
            }
            p.draw(engine);
        }
        let mut cur_y = y + self.padding.top;
        for (w, &(ww, wh)) in self.widgets.iter_mut().zip(sizes.iter()) {
            let ix = match self.alignment {
                Alignment::Start  => x + self.padding.left,
                Alignment::Center => x + (stack_w - ww) * 0.5,
                Alignment::End    => x + stack_w - ww - self.padding.right,
            };
            w.draw(engine, ix, cur_y, stack_w - self.padding.left - self.padding.right, None);
            cur_y += wh + self.spacing;
        }
    }
}

pub struct HStack<'a> {
    pub widgets: Vec<Box<dyn Widget + 'a>>,
    pub alignment: Alignment,
    pub padding: Padding,
    pub spacing: f32,
    pub bg_color: Option<Color>,
    pub border: Option<(BorderStyle, Color)>,
    pub radius: f32,
}

impl<'a> HStack<'a> {
    pub fn new(alignment: Alignment) -> Self { Self { widgets: vec![], alignment, padding: Padding::default(), spacing: 0.0, bg_color: None, border: None, radius: 0.0 } }
    pub fn with_padding(mut self, p: Padding) -> Self { self.padding = p; self }
    pub fn with_spacing(mut self, s: f32) -> Self { self.spacing = s; self }
    pub fn with_bg(mut self, c: Color) -> Self { self.bg_color = Some(c); self }
    pub fn with_border(mut self, s: BorderStyle, c: Color) -> Self { self.border = Some((s, c)); self }
    pub fn with_radius(mut self, r: f32) -> Self { self.radius = r; self }
    pub fn add(mut self, w: impl Widget + 'a) -> Self { self.widgets.push(Box::new(w)); self }
}

impl<'a> Widget for HStack<'a> {
    fn size(&self, engine: &jEngine) -> (f32, f32) {
        let mut total_w = self.padding.left + self.padding.right;
        let mut max_h = 0.0f32;
        for (i, w) in self.widgets.iter().enumerate() {
            let (ww, wh) = w.size(engine);
            total_w += ww;
            max_h = max_h.max(wh);
            if i < self.widgets.len() - 1 { total_w += self.spacing; }
        }
        (total_w, max_h + self.padding.top + self.padding.bottom)
    }
    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, _aw: f32, _clip: Option<Rect>) {
        // Pre-compute child sizes once to avoid O(N²) double-calls with self.size(engine).
        let sizes: Vec<(f32, f32)> = self.widgets.iter().map(|w| w.size(engine)).collect();
        let mut total_w = self.padding.left + self.padding.right;
        let mut max_h = 0.0f32;
        for (i, &(ww, wh)) in sizes.iter().enumerate() {
            total_w += ww;
            max_h = max_h.max(wh);
            if i < sizes.len() - 1 { total_w += self.spacing; }
        }
        let sw = total_w;
        let sh = max_h + self.padding.top + self.padding.bottom;

        if let Some(bg) = self.bg_color {
            let mut p = Panel::new(x, y, sw, sh).with_color(bg).with_radius(self.radius);
            if let Some((style, col)) = self.border {
                let thick = match style { BorderStyle::None => 0.0, BorderStyle::Thin => 1.0, BorderStyle::Thick => 2.0 };
                p = p.with_border(col, thick);
            }
            p.draw(engine);
        }
        let mut cur_x = x + self.padding.left;
        for (w, &(ww, wh)) in self.widgets.iter_mut().zip(sizes.iter()) {
            let iy = match self.alignment {
                Alignment::Start  => y + self.padding.top,
                Alignment::Center => y + (sh - wh) * 0.5,
                Alignment::End    => y + sh - wh - self.padding.bottom,
            };
            w.draw(engine, cur_x, iy, ww, None);
            cur_x += ww + self.spacing;
        }
    }
}

pub struct TabHeader { pub tabs: Vec<String>, pub selected: usize }
impl TabHeader { pub fn new(tabs: Vec<String>, selected: usize) -> Self { Self { tabs, selected } } }
impl Widget for TabHeader {
    fn size(&self, engine: &jEngine) -> (f32, f32) { (self.tabs.iter().map(|t| (t.len() + 4) as f32 * engine.tile_width() as f32).sum(), engine.tile_height() as f32) }
    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, _: f32, _: Option<Rect>) {
        let mut cx = x;
        let th = engine.tile_height() as f32;
        let tw = engine.tile_width() as f32;
        let theme = engine.ui.theme.clone();
        for (i, tab) in self.tabs.iter().enumerate() {
            let is_sel = i == self.selected;
            let tab_w = (tab.len() + 4) as f32 * tw;
            if is_sel { Panel::new(cx, y, tab_w, th).with_color(theme.selection_bg).with_radius(4.0).draw(engine); }
            engine.ui.ui_text(cx + tw * 2.0, y + th * 0.1, tab, if is_sel { theme.text_accent } else { theme.text_dim }, Color::TRANSPARENT, Some(th * 0.8));
            if engine.input.was_clicked(cx, y, tab_w, th) { self.selected = i; engine.input.mouse_consumed = true; }
            cx += tab_w;
        }
    }
}

pub struct Grid<'a> { pub rows: usize, pub cols: usize, pub children: Vec<Option<Box<dyn Widget + 'a>>>, pub cell_w: f32, pub cell_h: f32, pub spacing: f32 }
impl<'a> Widget for Grid<'a> {
    fn size(&self, _: &jEngine) -> (f32, f32) { (self.cols as f32 * self.cell_w + (self.cols - 1) as f32 * self.spacing, self.rows as f32 * self.cell_h + (self.rows - 1) as f32 * self.spacing) }
    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, _: f32, _: Option<Rect>) {
        for r in 0..self.rows { for c in 0..self.cols {
            if let Some(Some(child)) = self.children.get_mut(r * self.cols + c) {
                child.draw(engine, x + c as f32 * (self.cell_w + self.spacing), y + r as f32 * (self.cell_h + self.spacing), self.cell_w, None);
            }
        }}
    }
}

pub struct ScrollContainer<'a> { pub inner: Box<dyn Widget + 'a>, pub max_height: f32, pub scroll_offset: &'a mut f32 }
impl<'a> Widget for ScrollContainer<'a> {
    fn size(&self, engine: &jEngine) -> (f32, f32) { let (w, h) = self.inner.size(engine); (w + 10.0, h.min(self.max_height)) }
    fn draw(&mut self, engine: &mut jEngine, x: f32, y: f32, aw: f32, _: Option<Rect>) {
        let (_, ih) = self.inner.size(engine);
        let vh = ih.min(self.max_height);
        if engine.input.is_mouse_over(x, y, aw, vh) && engine.input.mouse_wheel != 0.0 { *self.scroll_offset = (*self.scroll_offset - engine.input.mouse_wheel * 100.0).clamp(0.0, (ih - vh).max(0.0)); }
        let clip = Rect::new(x, y, aw, vh); engine.ui.push_scissor(clip);
        self.inner.draw(engine, x, y - *self.scroll_offset, aw - 10.0, Some(clip)); engine.ui.pop_scissor();
        if ih > vh { Panel::new(x + aw - 6.0, y + (*self.scroll_offset / ih) * vh, 4.0, (vh / ih) * vh).with_color(engine.ui.theme.primary).with_radius(2.0).draw(engine); }
    }
}

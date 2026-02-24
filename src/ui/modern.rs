use crate::engine::{Color, jEngine};

/// A modern UI panel with procedural rounded corners and borders.
#[derive(Clone, Debug)]
pub struct Panel {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: Color,
    pub border_color: Color,
    pub border_thickness: f32,
    pub radius: [f32; 4],
    pub mode: u32,
    pub mode_param: f32,
}

impl Panel {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            x, y, w, h,
            color: Color::WHITE,
            border_color: Color::TRANSPARENT,
            border_thickness: 0.0,
            radius: [0.0; 4],
            mode: 0,
            mode_param: 0.0,
        }
    }

    pub fn with_color(mut self, color: Color) -> Self { self.color = color; self }
    pub fn with_border(mut self, color: Color, thickness: f32) -> Self {
        self.border_color = color;
        self.border_thickness = thickness;
        self
    }
    pub fn with_radius(mut self, r: f32) -> Self { self.radius = [r; 4]; self }
    pub fn with_rounded_corners(mut self, tl: f32, tr: f32, br: f32, bl: f32) -> Self {
        self.radius = [tl, tr, br, bl];
        self
    }
    
    /// Set a procedural pattern.
    /// mode 1 = crosshatch (mode_param is scale in pixels)
    pub fn with_pattern(mut self, mode: u32, param: f32) -> Self {
        self.mode = mode;
        self.mode_param = param;
        self
    }

    pub fn draw(&self, engine: &mut jEngine) {
        // Delegate through ui_panel so the active scissor clip is applied automatically.
        engine.ui.ui_panel(
            self.x, self.y, self.w, self.h,
            self.color, self.border_color, self.border_thickness,
            self.radius, self.mode, self.mode_param,
        );
    }
}

use glam::Vec2;

/// Camera uniform uploaded to the GPU — contains the combined view-projection matrix.
///
/// Layout (column-major, matching WGSL `mat4x4<f32>`):
/// ```text
/// col0: [sx,  0,   0,  0]
/// col1: [0,   sy,  0,  0]
/// col2: [0,   0,   1,  0]
/// col3: [tx,  ty,  0,  1]
/// ```
/// where `sx = 2z/w`, `sy = -2z/h`, `tx = -sx*cx`, `ty = -sy*cy`.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    /// Column-major 4×4 view-projection matrix sent to the vertex shader.
    pub view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    /// Plain orthographic projection (no camera transform).
    /// Maps pixel coords [0..w] × [0..h] directly to clip space.
    /// Used for the UI pass so that UI is always screen-fixed.
    pub fn identity_ortho(width: f32, height: f32) -> Self {
        let sx = 2.0 / width;
        let sy = -2.0 / height;
        Self {
            view_proj: [
                [sx,   0.0,  0.0, 0.0], // col0
                [0.0,  sy,   0.0, 0.0], // col1
                [0.0,  0.0,  1.0, 0.0], // col2
                [-1.0, 1.0,  0.0, 1.0], // col3
            ],
        }
    }
}

/// 2D camera: tracks a world-space position with smooth zoom and screen-shake.
pub struct Camera {
    /// World-space pixel position the camera is centered on.
    pub position: Vec2,
    /// Current zoom level (1.0 = 1:1, >1 zooms in, <1 zooms out).
    pub zoom: f32,
    /// Rotation in radians (reserved; not yet applied in the shader).
    pub rotation: f32,
    /// Smooth-zoom lerp target; `set_camera_zoom` writes here.
    pub(crate) target_zoom: f32,
    /// Remaining shake time in seconds.
    pub(crate) shake_timer: f32,
    /// Peak shake displacement in pixels.
    pub(crate) shake_intensity: f32,
    /// Current shake displacement offset (recomputed every tick).
    pub(crate) shake_offset: Vec2,
}

impl Camera {
    pub fn new(center_x: f32, center_y: f32) -> Self {
        Self {
            position: Vec2::new(center_x, center_y),
            zoom: 1.0,
            rotation: 0.0,
            target_zoom: 1.0,
            shake_timer: 0.0,
            shake_intensity: 0.0,
            shake_offset: Vec2::ZERO,
        }
    }

    /// Advance camera animations by `dt` seconds:
    /// - Smooth zoom lerps toward `target_zoom`.
    /// - Shake displacement decays and oscillates.
    pub fn tick(&mut self, dt: f32) {
        // Smooth zoom interpolation (converges at ~8× per second).
        let speed = 8.0_f32;
        self.zoom += (self.target_zoom - self.zoom) * (speed * dt).min(1.0);

        // Camera shake: high-frequency sinusoidal offset with linear decay.
        if self.shake_timer > 0.0 {
            self.shake_timer -= dt;
            let duration = 0.5_f32;
            let decay = (self.shake_timer / duration).max(0.0);
            let t = self.shake_timer;
            use std::f32::consts::TAU;
            self.shake_offset = Vec2::new(
                (t * 47.0 * TAU).sin() * self.shake_intensity * decay,
                (t * 37.0 * TAU + 1.1).sin() * self.shake_intensity * decay,
            );
            if self.shake_timer <= 0.0 {
                self.shake_timer = 0.0;
                self.shake_offset = Vec2::ZERO;
            }
        }
    }

    /// Trigger a camera shake.  `intensity` is the peak displacement in pixels.
    /// Shake lasts 0.5 seconds and decays linearly.
    pub fn shake(&mut self, intensity: f32) {
        self.shake_timer = 0.5;
        self.shake_intensity = intensity;
    }

    /// Build the GPU-ready `CameraUniform` for the given viewport dimensions.
    ///
    /// The resulting matrix maps world-space pixel coordinates so that
    /// `self.position` (plus any shake offset) lands at screen center, with
    /// the visible region scaled by `self.zoom`.
    ///
    /// Derivation (y-down pixel space → NDC):
    /// ```text
    /// x_ndc = sx * world_x + tx    (sx = 2z/w,  tx = -sx*cx)
    /// y_ndc = sy * world_y + ty    (sy = -2z/h, ty = -sy*cy)
    /// ```
    /// At the camera center (cx, cy): x_ndc = 0, y_ndc = 0 ✓
    pub fn build_view_proj(&self, width: f32, height: f32) -> CameraUniform {
        let cx = self.position.x + self.shake_offset.x;
        let cy = self.position.y + self.shake_offset.y;
        let z = self.zoom.max(0.01);

        let sx = 2.0 * z / width;
        let sy = -2.0 * z / height;
        let tx = -sx * cx;
        let ty = -sy * cy;

        CameraUniform {
            view_proj: [
                [sx,  0.0, 0.0, 0.0], // col0
                [0.0, sy,  0.0, 0.0], // col1
                [0.0, 0.0, 1.0, 0.0], // col2
                [tx,  ty,  0.0, 1.0], // col3
            ],
        }
    }
}
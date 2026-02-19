// ── Letterbox viewport math ───────────────────────────────────────────────────
//
// Computes the largest axis-uniform scaled rectangle that fits the logical
// (game) resolution inside the physical (window) resolution, centred on both
// axes.  The resulting `Viewport` describes the scissor / viewport rectangle
// to pass to the GPU.

use crate::window::WindowConfig;

// ── Viewport ──────────────────────────────────────────────────────────────────

/// Axis-aligned rectangle in physical pixels that centres the game view while
/// preserving its aspect ratio (letterbox / pillarbox).
///
/// Use `x`, `y` as the top-left origin and `width`, `height` as the extent.
/// All values are in physical pixels, ready for a GPU scissor or viewport call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    /// Horizontal offset from the left edge of the window in physical pixels.
    pub x: f32,
    /// Vertical offset from the top edge of the window in physical pixels.
    pub y: f32,
    /// Width of the game view in physical pixels.
    pub width: f32,
    /// Height of the game view in physical pixels.
    pub height: f32,
}

// ── letterbox_viewport ────────────────────────────────────────────────────────

/// Calculate the letterbox `Viewport` for `config`.
///
/// The uniform scale factor is:
/// ```text
/// scale = min(physical_width  / logical_width,
///             physical_height / logical_height)
/// ```
/// The centring offsets are:
/// ```text
/// x = (physical_width  - logical_width  * scale) / 2
/// y = (physical_height - logical_height * scale) / 2
/// ```
///
/// Returns a zero-sized `Viewport` at the origin when either logical dimension
/// is zero (avoids a division-by-zero and produces a safe no-op rectangle).
pub fn letterbox_viewport(config: &WindowConfig) -> Viewport {
    if config.logical_width == 0 || config.logical_height == 0 {
        return Viewport { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };
    }

    let pw = config.physical_width  as f32;
    let ph = config.physical_height as f32;
    let lw = config.logical_width   as f32;
    let lh = config.logical_height  as f32;

    let scale = (pw / lw).min(ph / lh);

    let width  = lw * scale;
    let height = lh * scale;
    let x      = (pw - width)  / 2.0;
    let y      = (ph - height) / 2.0;

    Viewport { x, y, width, height }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::WindowConfig;

    fn cfg(pw: u32, ph: u32, lw: u32, lh: u32) -> WindowConfig {
        WindowConfig { physical_width: pw, physical_height: ph,
                       logical_width: lw,  logical_height: lh,
                       mode: crate::window::WindowMode::Windowed }
    }

    // ── exact-fit (no bars) ───────────────────────────────────────────────────

    #[test]
    fn exact_fit_no_offset() {
        // Physical == logical → scale = 1, no bars.
        let v = letterbox_viewport(&cfg(1280, 720, 1280, 720));
        assert_eq!(v.x, 0.0);
        assert_eq!(v.y, 0.0);
        assert_eq!(v.width,  1280.0);
        assert_eq!(v.height, 720.0);
    }

    #[test]
    fn integer_scale_2x_no_offset() {
        // 2× upscale — still fills perfectly.
        let v = letterbox_viewport(&cfg(2560, 1440, 1280, 720));
        assert_eq!(v.x, 0.0);
        assert_eq!(v.y, 0.0);
        assert_eq!(v.width,  2560.0);
        assert_eq!(v.height, 1440.0);
    }

    // ── letterbox (horizontal bars) ───────────────────────────────────────────

    #[test]
    fn letterbox_4x3_in_16x9() {
        // 800×600 logical in 1280×720 physical → scale = min(1.6, 1.2) = 1.2
        // scaled: 960×720, bars: (1280-960)/2 = 160 each side, y = 0.
        let v = letterbox_viewport(&cfg(1280, 720, 800, 600));
        assert!((v.x - 160.0).abs() < 1e-3, "x={}", v.x);
        assert!((v.y -   0.0).abs() < 1e-3, "y={}", v.y);
        assert!((v.width  - 960.0).abs() < 1e-3, "w={}", v.width);
        assert!((v.height - 720.0).abs() < 1e-3, "h={}", v.height);
    }

    // ── pillarbox (vertical bars) ─────────────────────────────────────────────

    #[test]
    fn pillarbox_16x9_in_4x3() {
        // 1280×720 logical in 800×600 physical → scale = min(0.625, 0.833) = 0.625
        // scaled: 800×450, bars: y = (600-450)/2 = 75, x = 0.
        let v = letterbox_viewport(&cfg(800, 600, 1280, 720));
        assert!((v.x -   0.0).abs() < 1e-3, "x={}", v.x);
        assert!((v.y -  75.0).abs() < 1e-3, "y={}", v.y);
        assert!((v.width  - 800.0).abs() < 1e-3, "w={}", v.width);
        assert!((v.height - 450.0).abs() < 1e-3, "h={}", v.height);
    }

    // ── square viewport ───────────────────────────────────────────────────────

    #[test]
    fn square_logical_in_wide_physical() {
        // 512×512 logical in 1024×600 physical → scale = min(2.0, 1.171…) = 1.171…
        // scaled: ~600×600, bars: x = (1024-600)/2 = 212, y = 0.
        let v = letterbox_viewport(&cfg(1024, 600, 512, 512));
        let expected_w = 512.0 * (600.0f32 / 512.0);
        let expected_x = (1024.0 - expected_w) / 2.0;
        assert!((v.x - expected_x).abs() < 1e-3, "x={}", v.x);
        assert!((v.y - 0.0).abs() < 1e-3, "y={}", v.y);
        assert!((v.width  - expected_w).abs() < 1e-3, "w={}", v.width);
        assert!((v.height - 600.0).abs() < 1e-3, "h={}", v.height);
    }

    // ── edge cases ────────────────────────────────────────────────────────────

    #[test]
    fn zero_logical_width_returns_zero_viewport() {
        let v = letterbox_viewport(&cfg(1280, 720, 0, 720));
        assert_eq!(v, Viewport { x: 0.0, y: 0.0, width: 0.0, height: 0.0 });
    }

    #[test]
    fn zero_logical_height_returns_zero_viewport() {
        let v = letterbox_viewport(&cfg(1280, 720, 1280, 0));
        assert_eq!(v, Viewport { x: 0.0, y: 0.0, width: 0.0, height: 0.0 });
    }

    #[test]
    fn zero_physical_size_gives_zero_sized_viewport() {
        // Physical 0×0 → scale = 0 → width/height = 0, offsets = 0.
        let v = letterbox_viewport(&cfg(0, 0, 1280, 720));
        assert_eq!(v.x, 0.0);
        assert_eq!(v.y, 0.0);
        assert_eq!(v.width,  0.0);
        assert_eq!(v.height, 0.0);
    }

    // ── viewport is always contained within physical bounds ───────────────────

    #[test]
    fn viewport_never_exceeds_physical_bounds() {
        for (pw, ph, lw, lh) in [
            (1920u32, 1080u32, 1280u32, 720u32),
            (800, 600, 1920, 1080),
            (1280, 1024, 1280, 720),
            (3840, 2160, 1280, 720),
        ] {
            let v = letterbox_viewport(&cfg(pw, ph, lw, lh));
            assert!(v.x >= 0.0,                           "x negative  ({pw}x{ph}/{lw}x{lh})");
            assert!(v.y >= 0.0,                           "y negative  ({pw}x{ph}/{lw}x{lh})");
            assert!(v.x + v.width  <= pw as f32 + 1e-3,  "overflows x ({pw}x{ph}/{lw}x{lh})");
            assert!(v.y + v.height <= ph as f32 + 1e-3,  "overflows y ({pw}x{ph}/{lw}x{lh})");
        }
    }
}
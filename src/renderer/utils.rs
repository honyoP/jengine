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


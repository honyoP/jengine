// ── WindowMode ────────────────────────────────────────────────────────────────

/// Controls how the OS window is presented.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WindowMode {
    /// Standard decorated window at the configured resolution.
    Windowed,
    /// Exclusive hardware fullscreen at the configured resolution.
    Fullscreen,
    /// Borderless window sized to match the monitor's native resolution.
    Borderless,
}

// ── WindowConfig ──────────────────────────────────────────────────────────────

/// Window configuration snapshot.
///
/// - **physical** dimensions are the actual pixel size of the OS window
///   (accounts for HiDPI scaling and fullscreen mode).
/// - **logical** dimensions are the internal game resolution used for
///   rendering and UI layout (e.g. 1280 × 720 regardless of DPI).
#[derive(Clone, Debug, PartialEq)]
pub struct WindowConfig {
    /// Actual window width in physical pixels.
    pub physical_width: u32,
    /// Actual window height in physical pixels.
    pub physical_height: u32,
    /// Internal game / render resolution width.
    pub logical_width: u32,
    /// Internal game / render resolution height.
    pub logical_height: u32,
    /// Active window mode.
    pub mode: WindowMode,
}

impl WindowConfig {
    /// Returns a `WindowConfig` initialised to 1280 × 720 in `Windowed` mode.
    pub fn default() -> Self {
        Self {
            physical_width:  1280,
            physical_height: 720,
            logical_width:   1280,
            logical_height:  720,
            mode:            WindowMode::Windowed,
        }
    }

    /// Aspect ratio of the **logical** resolution (`logical_width / logical_height`).
    ///
    /// Returns `0.0` when `logical_height` is zero to avoid division by zero.
    pub fn aspect_ratio(&self) -> f32 {
        if self.logical_height == 0 {
            return 0.0;
        }
        self.logical_width as f32 / self.logical_height as f32
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_1280x720_windowed() {
        let cfg = WindowConfig::default();
        assert_eq!(cfg.physical_width,  1280);
        assert_eq!(cfg.physical_height, 720);
        assert_eq!(cfg.logical_width,   1280);
        assert_eq!(cfg.logical_height,  720);
        assert_eq!(cfg.mode, WindowMode::Windowed);
    }

    #[test]
    fn aspect_ratio_1280x720() {
        let cfg = WindowConfig::default();
        let ratio = cfg.aspect_ratio();
        assert!((ratio - 16.0 / 9.0).abs() < 1e-5, "expected 16/9, got {ratio}");
    }

    #[test]
    fn aspect_ratio_zero_height_returns_zero() {
        let cfg = WindowConfig {
            physical_width: 0, physical_height: 0,
            logical_width: 1920, logical_height: 0,
            mode: WindowMode::Windowed,
        };
        assert_eq!(cfg.aspect_ratio(), 0.0);
    }

    #[test]
    fn aspect_ratio_square() {
        let cfg = WindowConfig {
            physical_width: 512, physical_height: 512,
            logical_width: 512, logical_height: 512,
            mode: WindowMode::Windowed,
        };
        assert!((cfg.aspect_ratio() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn aspect_ratio_4x3() {
        let cfg = WindowConfig {
            physical_width: 800, physical_height: 600,
            logical_width: 800, logical_height: 600,
            mode: WindowMode::Borderless,
        };
        let ratio = cfg.aspect_ratio();
        assert!((ratio - 4.0 / 3.0).abs() < 1e-5, "expected 4/3, got {ratio}");
    }

    #[test]
    fn mode_variants_are_distinct() {
        assert_ne!(WindowMode::Windowed,    WindowMode::Fullscreen);
        assert_ne!(WindowMode::Windowed,    WindowMode::Borderless);
        assert_ne!(WindowMode::Fullscreen,  WindowMode::Borderless);
    }

    #[test]
    fn physical_and_logical_can_differ() {
        // Typical HiDPI setup: 2× physical but 1× logical resolution.
        let cfg = WindowConfig {
            physical_width:  2560,
            physical_height: 1440,
            logical_width:   1280,
            logical_height:  720,
            mode: WindowMode::Fullscreen,
        };
        assert_eq!(cfg.physical_width  / cfg.logical_width,  2);
        assert_eq!(cfg.physical_height / cfg.logical_height, 2);
    }
}
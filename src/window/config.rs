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


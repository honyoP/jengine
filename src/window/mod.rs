pub mod config;

pub use config::{WindowConfig, WindowMode};

use winit::dpi::PhysicalSize;
use winit::window::{Fullscreen, Window};

/// Apply `config` to `window`, updating decorations, fullscreen state, and size.
///
/// # Windowed
/// Removes any active fullscreen mode, restores decorations and resizability,
/// and requests the window be resized to match `config.physical_width × physical_height`.
///
/// # Fullscreen
/// Attempts exclusive hardware fullscreen by searching the current monitor's
/// available video modes for the best resolution match against
/// `config.physical_width × physical_height`.  Falls back to
/// `Borderless` on the current monitor if no monitor handle is available.
///
/// # Borderless
/// Enters a borderless fullscreen window on the current monitor
/// (`Fullscreen::Borderless(None)`).  The monitor's native resolution becomes
/// the physical window size; the logical game resolution is left unchanged.
pub fn apply_window_settings(window: &Window, config: &WindowConfig) {
    match config.mode {
        WindowMode::Windowed => {
            // Exit any fullscreen mode first so decorations and size take effect.
            window.set_fullscreen(None);
            window.set_decorations(true);
            window.set_resizable(false);

            // Request the physical pixel size stored in the config.
            // `request_inner_size` returns `Some(size)` when the OS applies the
            // resize synchronously, or `None` when it will arrive later as a
            // `WindowEvent::Resized`.  We ignore the return value here because
            // the engine's resize handler will reconcile the final size.
            let _ = window.request_inner_size(PhysicalSize::new(
                config.physical_width,
                config.physical_height,
            ));
        }

        WindowMode::Fullscreen => {
            let fullscreen = match window.current_monitor() {
                Some(monitor) => {
                    // Pick the video mode whose resolution is closest (by
                    // pixel area) to the requested physical dimensions.
                    let target_area =
                        config.physical_width as u64 * config.physical_height as u64;

                    let best = monitor.video_modes().min_by_key(|vm| {
                        let s = vm.size();
                        let area = s.width as u64 * s.height as u64;
                        area.abs_diff(target_area)
                    });

                    match best {
                        Some(vm) => Fullscreen::Exclusive(vm),
                        // No video modes available — fall back to borderless.
                        None => Fullscreen::Borderless(Some(monitor)),
                    }
                }
                // No monitor handle (headless / Wayland edge case) — use
                // borderless on the current monitor.
                None => Fullscreen::Borderless(None),
            };

            window.set_fullscreen(Some(fullscreen));
        }

        WindowMode::Borderless => {
            // Borderless(None) targets the monitor the window is currently on.
            // The OS sizes the window to the monitor's native resolution.
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }
    }
}

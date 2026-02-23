use jengine::window::*;

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

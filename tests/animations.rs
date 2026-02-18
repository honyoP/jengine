/// Unit tests for the engine-side animation system.
///
/// `compute_offset` is a pure function so tests require no GPU or window.
use jengine::engine::{AnimationType, compute_offset};

const BASH_DUR: f32 = 0.18;
const SHIVER_DUR: f32 = 0.45;

// ── Bash tests ───────────────────────────────────────────────────────────────

/// At t = 0 the sine term is sin(0) = 0, so offset must be zero.
#[test]
fn bash_offset_zero_at_start() {
    let anim = AnimationType::Bash { direction: [1.0, 0.0], magnitude: 6.0 };
    let off = compute_offset(&anim, 0.0, BASH_DUR);
    assert!(off[0].abs() < 1e-6, "x offset at t=0 should be 0, got {}", off[0]);
    assert!(off[1].abs() < 1e-6, "y offset at t=0 should be 0, got {}", off[1]);
}

/// At t = duration/2 progress = 0.5, sin(0.5 * π) = 1.0 → offset == magnitude.
#[test]
fn bash_offset_peaks_at_midpoint() {
    let mag = 6.0_f32;
    let anim = AnimationType::Bash { direction: [1.0, 0.0], magnitude: mag };
    let off = compute_offset(&anim, BASH_DUR * 0.5, BASH_DUR);
    assert!((off[0] - mag).abs() < 1e-4, "x offset at midpoint should be {mag}, got {}", off[0]);
    assert!(off[1].abs() < 1e-6, "y offset should remain 0 for horizontal bash");
}

/// At t = duration progress = 1.0, sin(π) ≈ 0 → offset must be near zero.
#[test]
fn bash_offset_zero_at_end() {
    let anim = AnimationType::Bash { direction: [1.0, 0.0], magnitude: 6.0 };
    let off = compute_offset(&anim, BASH_DUR, BASH_DUR);
    assert!(off[0].abs() < 1e-4, "x offset at t=duration should be ≈0, got {}", off[0]);
}

/// Bash should work for diagonal directions as well.
#[test]
fn bash_diagonal_direction() {
    let dir = [1.0_f32 / 2.0_f32.sqrt(), 1.0 / 2.0_f32.sqrt()];
    let mag = 4.0_f32;
    let anim = AnimationType::Bash { direction: dir, magnitude: mag };
    let off = compute_offset(&anim, BASH_DUR * 0.5, BASH_DUR);
    // Each component should be dir * magnitude at midpoint.
    let expected = dir[0] * mag;
    assert!((off[0] - expected).abs() < 1e-4);
    assert!((off[1] - expected).abs() < 1e-4);
}

/// Clamped progress: elapsed > duration should not produce growing offsets.
#[test]
fn bash_clamped_past_end() {
    let anim = AnimationType::Bash { direction: [0.0, 1.0], magnitude: 5.0 };
    let off = compute_offset(&anim, BASH_DUR * 2.0, BASH_DUR);
    // progress is clamped to 1.0 → sin(π) ≈ 0
    assert!(off[1].abs() < 1e-4);
}

// ── Shiver tests ─────────────────────────────────────────────────────────────

/// Bell envelope is sin(0) = 0 at t = 0, so shiver is suppressed at birth.
#[test]
fn shiver_envelope_zero_at_start() {
    let anim = AnimationType::Shiver { magnitude: 4.0 };
    let off = compute_offset(&anim, 0.0, SHIVER_DUR);
    assert!(off[0].abs() < 1e-5, "shiver x at t=0 should be 0, got {}", off[0]);
    assert!(off[1].abs() < 1e-5, "shiver y at t=0 should be 0, got {}", off[1]);
}

/// Bell envelope is sin(π) ≈ 0 at t = duration, so shiver is suppressed at death.
#[test]
fn shiver_envelope_zero_at_end() {
    let anim = AnimationType::Shiver { magnitude: 4.0 };
    let off = compute_offset(&anim, SHIVER_DUR, SHIVER_DUR);
    assert!(off[0].abs() < 1e-4, "shiver x at t=duration should be ≈0, got {}", off[0]);
    assert!(off[1].abs() < 1e-4, "shiver y at t=duration should be ≈0, got {}", off[1]);
}

/// Near midpoint the envelope reaches its maximum, so the offset magnitude
/// should not exceed `magnitude` in either axis.
#[test]
fn shiver_bounded_by_magnitude() {
    let mag = 3.0_f32;
    let anim = AnimationType::Shiver { magnitude: mag };
    // Sample several points near midpoint.
    for i in 0..20 {
        let t = SHIVER_DUR * 0.3 + (i as f32 / 20.0) * SHIVER_DUR * 0.4;
        let off = compute_offset(&anim, t, SHIVER_DUR);
        assert!(off[0].abs() <= mag + 1e-4, "shiver x out of bounds at t={t}: {}", off[0]);
        assert!(off[1].abs() <= mag + 1e-4, "shiver y out of bounds at t={t}: {}", off[1]);
    }
}

// ── AnimationType helpers ─────────────────────────────────────────────────────

#[test]
fn bash_duration_is_reasonable() {
    let dur = AnimationType::Bash { direction: [1.0, 0.0], magnitude: 4.0 }.duration();
    assert!(dur > 0.0 && dur < 1.0, "Bash duration should be a sub-second value");
}

#[test]
fn shiver_duration_is_reasonable() {
    let dur = AnimationType::Shiver { magnitude: 2.0 }.duration();
    assert!(dur > 0.0 && dur < 2.0, "Shiver duration should be a sub-two-second value");
}

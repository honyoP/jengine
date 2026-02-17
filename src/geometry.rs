// =============================================================================
// GEOMETRY.RS — Geometric primitives for roguelikes
//
// Common geometric operations needed in roguelikes:
// - Distance calculations (for FOV radius, attack range, etc.)
// - Line drawing (for line-of-sight, projectiles)
// - Circle/radius operations (for area effects)
// =============================================================================

/// Calculate Manhattan distance between two points.
/// Also known as "taxicab distance" - the distance traveling only
/// along grid axes (no diagonals).
///
/// Use for: 4-directional movement costs, simple range checks.
#[inline]
pub fn distance_manhattan(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    (x1 - x2).abs() + (y1 - y2).abs()
}

/// Calculate Chebyshev distance between two points.
/// Also known as "chessboard distance" - a king's move distance
/// where diagonals count the same as orthogonals.
///
/// Use for: 8-directional movement where diagonals cost 1.
#[inline]
pub fn distance_chebyshev(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    (x1 - x2).abs().max((y1 - y2).abs())
}

/// Calculate Euclidean distance between two points.
/// The "real" straight-line distance.
///
/// Use for: Circular FOV, realistic range calculations.
#[inline]
pub fn distance_euclidean(x1: i32, y1: i32, x2: i32, y2: i32) -> f32 {
    let dx = (x1 - x2) as f32;
    let dy = (y1 - y2) as f32;
    (dx * dx + dy * dy).sqrt()
}

/// Calculate squared Euclidean distance (avoids sqrt).
/// Useful when you only need to compare distances.
#[inline]
pub fn distance_squared(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    let dx = x1 - x2;
    let dy = y1 - y2;
    dx * dx + dy * dy
}

// =============================================================================
// BRESENHAM'S LINE ALGORITHM
// =============================================================================

/// Generate all points along a line from (x1, y1) to (x2, y2).
///
/// Uses Bresenham's line algorithm, which produces a line with no gaps.
/// This is the standard algorithm for roguelike line-of-sight and projectiles.
///
/// The returned Vec includes both endpoints.
pub fn line(x1: i32, y1: i32, x2: i32, y2: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();

    let dx = (x2 - x1).abs();
    let dy = (y2 - y1).abs();
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };

    let mut x = x1;
    let mut y = y1;
    let mut err = dx - dy;

    loop {
        points.push((x, y));

        if x == x2 && y == y2 {
            break;
        }

        let e2 = 2 * err;

        if e2 > -dy {
            err -= dy;
            x += sx;
        }

        if e2 < dx {
            err += dx;
            y += sy;
        }
    }

    points
}

/// Check if there's a clear line of sight from (x1, y1) to (x2, y2).
///
/// The `is_blocking` function should return true for tiles that block sight.
/// Returns true if the line is clear (no blocking tiles between start and end).
/// The start and end points themselves are NOT checked.
pub fn line_of_sight(
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    is_blocking: impl Fn(i32, i32) -> bool,
) -> bool {
    let points = line(x1, y1, x2, y2);

    // Skip first and last point (the endpoints)
    for &(x, y) in points.iter().skip(1).take(points.len().saturating_sub(2)) {
        if is_blocking(x, y) {
            return false;
        }
    }

    true
}

/// Iterate along a line, calling a function for each point.
/// Stops early if the function returns false.
///
/// Returns true if the line completed, false if it was interrupted.
pub fn walk_line(
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    mut callback: impl FnMut(i32, i32) -> bool,
) -> bool {
    let dx = (x2 - x1).abs();
    let dy = (y2 - y1).abs();
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };

    let mut x = x1;
    let mut y = y1;
    let mut err = dx - dy;

    loop {
        if !callback(x, y) {
            return false;
        }

        if x == x2 && y == y2 {
            break;
        }

        let e2 = 2 * err;

        if e2 > -dy {
            err -= dy;
            x += sx;
        }

        if e2 < dx {
            err += dx;
            y += sy;
        }
    }

    true
}

// =============================================================================
// CIRCLE/RADIUS OPERATIONS
// =============================================================================

/// Get all points within a given radius of (cx, cy) using Euclidean distance.
///
/// This produces a filled circle. Points are returned in no particular order.
pub fn points_in_radius(cx: i32, cy: i32, radius: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();
    let r_sq = radius * radius;

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= r_sq {
                points.push((cx + dx, cy + dy));
            }
        }
    }

    points
}

/// Get all points within a given radius using Chebyshev distance.
///
/// This produces a filled square (diamond rotated 45°).
pub fn points_in_radius_chebyshev(cx: i32, cy: i32, radius: i32) -> Vec<(i32, i32)> {
    let mut points = Vec::new();

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            points.push((cx + dx, cy + dy));
        }
    }

    points
}

/// Get all points at exactly a given radius (circle outline).
pub fn circle_outline(cx: i32, cy: i32, radius: i32) -> Vec<(i32, i32)> {
    if radius <= 0 {
        return vec![(cx, cy)];
    }

    let mut points = Vec::new();

    // Use midpoint circle algorithm
    let mut x = radius;
    let mut y = 0;
    let mut err = 1 - radius;

    while x >= y {
        // Add points in all 8 octants
        points.push((cx + x, cy + y));
        points.push((cx - x, cy + y));
        points.push((cx + x, cy - y));
        points.push((cx - x, cy - y));
        points.push((cx + y, cy + x));
        points.push((cx - y, cy + x));
        points.push((cx + y, cy - x));
        points.push((cx - y, cy - x));

        y += 1;

        if err < 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x + 1);
        }
    }

    // Remove duplicates (happens at octant boundaries)
    points.sort();
    points.dedup();
    points
}

// =============================================================================
// DIRECTION HELPERS
// =============================================================================

/// The 4 cardinal directions as (dx, dy) offsets.
pub const CARDINALS: [(i32, i32); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

/// The 4 diagonal directions as (dx, dy) offsets.
pub const DIAGONALS: [(i32, i32); 4] = [(-1, -1), (1, -1), (-1, 1), (1, 1)];

/// All 8 directions (cardinals + diagonals).
pub const ALL_DIRECTIONS: [(i32, i32); 8] = [
    (0, -1),  // N
    (1, -1),  // NE
    (1, 0),   // E
    (1, 1),   // SE
    (0, 1),   // S
    (-1, 1),  // SW
    (-1, 0),  // W
    (-1, -1), // NW
];

/// Get the direction from (x1, y1) toward (x2, y2) as a unit vector.
/// Returns (0, 0) if the points are the same.
pub fn direction_toward(x1: i32, y1: i32, x2: i32, y2: i32) -> (i32, i32) {
    let dx = (x2 - x1).signum();
    let dy = (y2 - y1).signum();
    (dx, dy)
}

/// Normalize a direction to one of the 8 compass directions.
/// Useful for converting arbitrary movement to grid movement.
pub fn normalize_direction(dx: i32, dy: i32) -> (i32, i32) {
    (dx.signum(), dy.signum())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manhattan_distance() {
        assert_eq!(distance_manhattan(0, 0, 3, 4), 7);
        assert_eq!(distance_manhattan(0, 0, 0, 0), 0);
        assert_eq!(distance_manhattan(-1, -1, 1, 1), 4);
    }

    #[test]
    fn test_chebyshev_distance() {
        assert_eq!(distance_chebyshev(0, 0, 3, 4), 4);
        assert_eq!(distance_chebyshev(0, 0, 3, 3), 3);
    }

    #[test]
    fn test_euclidean_distance() {
        let dist = distance_euclidean(0, 0, 3, 4);
        assert!((dist - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_line_horizontal() {
        let points = line(0, 0, 5, 0);
        assert_eq!(points, vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0)]);
    }

    #[test]
    fn test_line_vertical() {
        let points = line(0, 0, 0, 3);
        assert_eq!(points, vec![(0, 0), (0, 1), (0, 2), (0, 3)]);
    }

    #[test]
    fn test_line_diagonal() {
        let points = line(0, 0, 3, 3);
        assert_eq!(points.len(), 4);
        assert!(points.contains(&(0, 0)));
        assert!(points.contains(&(3, 3)));
    }

    #[test]
    fn test_points_in_radius() {
        let points = points_in_radius(0, 0, 1);
        assert!(points.contains(&(0, 0)));
        assert!(points.contains(&(1, 0)));
        assert!(points.contains(&(0, 1)));
        assert!(points.contains(&(-1, 0)));
        assert!(points.contains(&(0, -1)));
        // Corners should NOT be included (distance > 1)
        assert!(!points.contains(&(1, 1)));
    }

    #[test]
    fn test_line_of_sight_clear() {
        assert!(line_of_sight(0, 0, 5, 0, |_, _| false));
    }

    #[test]
    fn test_line_of_sight_blocked() {
        assert!(!line_of_sight(0, 0, 5, 0, |x, _| x == 3));
    }
}

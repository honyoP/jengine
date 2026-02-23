use jengine::geometry::*;

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

#[test]
fn test_circle_outline_radius_zero() {
    let pts = circle_outline(5, 5, 0);
    assert_eq!(pts, vec![(5, 5)]);
}

#[test]
fn test_circle_outline_radius_one() {
    let pts = circle_outline(0, 0, 1);
    // Midpoint circle: radius 1 should include the 4 cardinal points.
    assert!(pts.contains(&(1, 0)));
    assert!(pts.contains(&(-1, 0)));
    assert!(pts.contains(&(0, 1)));
    assert!(pts.contains(&(0, -1)));
    // No duplicates.
    let mut sorted = pts.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), pts.len());
}

#[test]
fn test_walk_line_completes() {
    let mut visited = Vec::new();
    let completed = walk_line(0, 0, 3, 0, |x, y| { visited.push((x, y)); true });
    assert!(completed);
    assert_eq!(visited, vec![(0, 0), (1, 0), (2, 0), (3, 0)]);
}

#[test]
fn test_walk_line_stops_early() {
    let mut count = 0;
    let completed = walk_line(0, 0, 5, 0, |_, _| { count += 1; count < 3 });
    assert!(!completed);
    assert_eq!(count, 3);
}

#[test]
fn test_direction_toward_cardinal() {
    assert_eq!(direction_toward(0, 0, 5, 0), (1, 0));
    assert_eq!(direction_toward(0, 0, 0, 5), (0, 1));
    assert_eq!(direction_toward(5, 0, 0, 0), (-1, 0));
}

#[test]
fn test_direction_toward_diagonal() {
    assert_eq!(direction_toward(0, 0, 3, 3), (1, 1));
    assert_eq!(direction_toward(5, 5, 0, 0), (-1, -1));
}

#[test]
fn test_direction_toward_same_point() {
    assert_eq!(direction_toward(2, 2, 2, 2), (0, 0));
}

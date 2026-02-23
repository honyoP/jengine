use jengine::pathfinding::prelude::*;

// ── A* ────────────────────────────────────────────────────────────────────────

#[test]
fn astar_trivial_same_start_and_goal() {
    let result = astar((2, 2), (2, 2), 5, 5, |_, _| true, 100);
    assert_eq!(result, Some(vec![(2, 2)]));
}

#[test]
fn astar_straight_line() {
    let path = astar((0, 0), (4, 0), 10, 10, |_, _| true, 200).unwrap();
    assert_eq!(path.first(), Some(&(0, 0)));
    assert_eq!(path.last(), Some(&(4, 0)));
    assert_eq!(path.len(), 5);
}

#[test]
fn astar_blocked_returns_none() {
    // Wall across x=2 — no path exists.
    let result = astar((0, 0), (4, 0), 10, 5, |x, _| x != 2, 500);
    assert!(result.is_none());
}

#[test]
fn astar_navigates_around_wall() {
    // Wall blocks direct route; path must go around.
    let path = astar((0, 2), (4, 2), 5, 5, |x, y| !(x == 2 && y == 2), 200).unwrap();
    assert_eq!(path.first(), Some(&(0, 2)));
    assert_eq!(path.last(), Some(&(4, 2)));
    // Should not pass through the wall.
    assert!(!path.contains(&(2, 2)));
}

#[test]
fn astar_out_of_bounds_start_returns_none() {
    let result = astar((-1, 0), (3, 3), 5, 5, |_, _| true, 100);
    assert!(result.is_none());
}

#[test]
fn astar_out_of_bounds_goal_returns_none() {
    let result = astar((0, 0), (10, 0), 5, 5, |_, _| true, 100);
    assert!(result.is_none());
}

#[test]
fn astar_negative_dimensions_returns_none() {
    let result = astar((0, 0), (1, 1), -1, 5, |_, _| true, 100);
    assert!(result.is_none());
    let result = astar((0, 0), (1, 1), 5, 0, |_, _| true, 100);
    assert!(result.is_none());
}

#[test]
fn astar_max_iterations_limit() {
    // Large open grid, tiny iteration cap — should return None.
    let result = astar((0, 0), (99, 99), 100, 100, |_, _| true, 1);
    assert!(result.is_none());
}

// ── A* 8-directional ─────────────────────────────────────────────────────────

#[test]
fn astar_8dir_diagonal_path() {
    let path = astar_8dir((0, 0), (3, 3), 5, 5, |_, _| true, 200).unwrap();
    assert_eq!(path.first(), Some(&(0, 0)));
    assert_eq!(path.last(), Some(&(3, 3)));
    // Diagonal movement: path length should be <= 4 (3 diagonal steps + start).
    assert!(path.len() <= 4);
}

#[test]
fn astar_8dir_oob_returns_none() {
    let result = astar_8dir((0, 0), (0, -1), 5, 5, |_, _| true, 100);
    assert!(result.is_none());
}

// ── astar_next_step ───────────────────────────────────────────────────────────

#[test]
fn astar_next_step_moves_toward_goal() {
    let next = astar_next_step((0, 0), (4, 0), 10, 10, |_, _| true, 200);
    assert_eq!(next, Some((1, 0)));
}

#[test]
fn astar_next_step_at_goal_returns_none() {
    // Already AT goal — path has length 1 so no "next" step.
    let next = astar_next_step((3, 3), (3, 3), 5, 5, |_, _| true, 100);
    assert!(next.is_none());
}

// ── DijkstraMap ───────────────────────────────────────────────────────────────

#[test]
fn dijkstra_single_goal_at_origin() {
    let map = DijkstraMap::new(5, 5, &[(0, 0)], |_, _| true);
    assert_eq!(map.get(0, 0), 0.0);
    assert_eq!(map.get(1, 0), 1.0);
    assert_eq!(map.get(0, 1), 1.0);
    assert_eq!(map.get(4, 4), 8.0);
}

#[test]
fn dijkstra_out_of_bounds_returns_max() {
    let map = DijkstraMap::new(5, 5, &[(0, 0)], |_, _| true);
    assert_eq!(map.get(-1, 0), f32::MAX);
    assert_eq!(map.get(0, 10), f32::MAX);
}

#[test]
fn dijkstra_blocked_cell_unreachable() {
    // Wall fully surrounds (4,4); it should be unreachable.
    let map = DijkstraMap::new(5, 5, &[(0, 0)], |x, y| !(x == 3 && y == 4) && !(x == 4 && y == 3));
    assert_eq!(map.get(4, 4), f32::MAX);
}

#[test]
fn dijkstra_direction_to_goal() {
    let map = DijkstraMap::new(5, 5, &[(0, 0)], |_, _| true);
    // From (2, 0), the best direction toward (0, 0) is (-1, 0).
    assert_eq!(map.direction_to_goal(2, 0), (-1, 0));
}

#[test]
fn dijkstra_direction_away() {
    let map = DijkstraMap::new(5, 5, &[(0, 0)], |_, _| true);
    // From (0, 0) (the goal), moving away goes toward higher values.
    let dir = map.direction_away(0, 0);
    // Should be one of the cardinal directions (any is fine since they're equidistant).
    assert!(dir == (1, 0) || dir == (0, 1) || dir == (-1, 0) || dir == (0, -1));
}

#[test]
fn dijkstra_zero_dimensions_returns_empty() {
    let map = DijkstraMap::new(0, 5, &[(0, 0)], |_, _| true);
    assert_eq!(map.get(0, 0), f32::MAX);
}

#[test]
fn dijkstra_multiple_goals() {
    let map = DijkstraMap::new(10, 1, &[(0, 0), (9, 0)], |_, _| true);
    // Center cell (4,0) is 4 steps from left goal, 5 from right goal.
    assert_eq!(map.get(4, 0), 4.0);
    // Cell (5,0) is 5 from left, 4 from right.
    assert_eq!(map.get(5, 0), 4.0);
}

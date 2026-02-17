use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use crate::geometry::distance_manhattan;

// =============================================================================
// A* PATHFINDING
// =============================================================================
///
/// A* pathfinding: find the shortest path from start to goal.
///
/// Returns the full path as a Vec of (x, y) coordinates, including
/// start and goal. Returns None if no path exists.
///
/// # Arguments
/// * `start` - Starting position (x, y)
/// * `goal` - Target position (x, y)
/// * `is_passable` - Function that returns true if a tile can be walked on
/// * `max_iterations` - Maximum nodes to explore (prevents infinite loops)
pub fn astar(
    start: (i32, i32),
    goal: (i32, i32),
    is_passable: impl Fn(i32, i32) -> bool,
    max_iterations: usize,
) -> Option<Vec<(i32, i32)>> {
    if start == goal {
        return Some(vec![start]);
    }

    // Priority queue: (f_score, x, y) - use Reverse for min-heap
    let mut open: BinaryHeap<Reverse<(i32, i32, i32)>> = BinaryHeap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();

    g_score.insert(start, 0);
    let h = distance_manhattan(start.0, start.1, goal.0, goal.1);
    open.push(Reverse((h, start.0, start.1)));

    let mut iterations = 0;

    while let Some(Reverse((_, cx, cy))) = open.pop() {
        iterations += 1;
        if iterations > max_iterations {
            return None;
        }

        let current = (cx, cy);

        // Reached goal — reconstruct path
        if current == goal {
            return Some(reconstruct_path(&came_from, start, goal));
        }

        let current_g = g_score[&current];

        // Explore 4 cardinal neighbors
        for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
            let next = (cx + dx, cy + dy);

            // Allow moving to goal even if "impassable" (it's often the target entity)
            let is_goal = next == goal;
            if !is_goal && !is_passable(next.0, next.1) {
                continue;
            }

            let new_g = current_g + 1;
            let existing_g = g_score.get(&next).copied().unwrap_or(i32::MAX);

            if new_g < existing_g {
                g_score.insert(next, new_g);
                came_from.insert(next, current);
                let f = new_g + distance_manhattan(next.0, next.1, goal.0, goal.1);
                open.push(Reverse((f, next.0, next.1)));
            }
        }
    }

    None // No path found
}

/// A* pathfinding with 8-directional movement (including diagonals).
pub fn astar_8dir(
    start: (i32, i32),
    goal: (i32, i32),
    is_passable: impl Fn(i32, i32) -> bool,
    max_iterations: usize,
) -> Option<Vec<(i32, i32)>> {
    if start == goal {
        return Some(vec![start]);
    }

    let mut open: BinaryHeap<Reverse<(i32, i32, i32)>> = BinaryHeap::new();
    let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
    let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();

    g_score.insert(start, 0);
    let h = distance_manhattan(start.0, start.1, goal.0, goal.1);
    open.push(Reverse((h, start.0, start.1)));

    let mut iterations = 0;

    // 8 directions including diagonals
    let directions = [
        (0, -1),
        (1, -1),
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
    ];

    while let Some(Reverse((_, cx, cy))) = open.pop() {
        iterations += 1;
        if iterations > max_iterations {
            return None;
        }

        let current = (cx, cy);

        if current == goal {
            return Some(reconstruct_path(&came_from, start, goal));
        }

        let current_g = g_score[&current];

        for (dx, dy) in directions {
            let next = (cx + dx, cy + dy);

            let is_goal = next == goal;
            if !is_goal && !is_passable(next.0, next.1) {
                continue;
            }

            // Diagonal movement costs slightly more (approximation of sqrt(2))
            let cost = if dx != 0 && dy != 0 { 14 } else { 10 };
            let new_g = current_g + cost;
            let existing_g = g_score.get(&next).copied().unwrap_or(i32::MAX);

            if new_g < existing_g {
                g_score.insert(next, new_g);
                came_from.insert(next, current);
                // Use Chebyshev distance as heuristic for 8-dir
                let h = (next.0 - goal.0).abs().max((next.1 - goal.1).abs()) * 10;
                let f = new_g + h;
                open.push(Reverse((f, next.0, next.1)));
            }
        }
    }

    None
}

/// Reconstruct path from came_from map.
fn reconstruct_path(
    came_from: &HashMap<(i32, i32), (i32, i32)>,
    start: (i32, i32),
    goal: (i32, i32),
) -> Vec<(i32, i32)> {
    let mut path = vec![goal];
    let mut current = goal;

    while current != start {
        current = came_from[&current];
        path.push(current);
    }

    path.reverse();
    path
}

/// Returns None if already at goal or no path exists.
pub fn astar_next_step(
    start: (i32, i32),
    goal: (i32, i32),
    is_passable: impl Fn(i32, i32) -> bool,
    max_iterations: usize,
) -> Option<(i32, i32)> {
    let path = astar(start, goal, is_passable, max_iterations)?;
    if path.len() > 1 { Some(path[1]) } else { None }
}

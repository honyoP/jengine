use std::cmp::Reverse;
use std::collections::BinaryHeap;

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
    width: i32,
    height: i32,
    is_passable: impl Fn(i32, i32) -> bool,
    max_iterations: usize,
) -> Option<Vec<(i32, i32)>> {
    if width <= 0 || height <= 0 { return None; }
    if start.0 < 0 || start.0 >= width || start.1 < 0 || start.1 >= height { return None; }
    if goal.0  < 0 || goal.0  >= width || goal.1  < 0 || goal.1  >= height { return None; }

    if start == goal {
        return Some(vec![start]);
    }

    let size = (width * height) as usize;
    // Priority queue: (f_score, x, y) - use Reverse for min-heap
    let mut open: BinaryHeap<Reverse<(i32, i32, i32)>> = BinaryHeap::new();
    let mut came_from: Vec<i32> = vec![-1; size];
    let mut g_score: Vec<i32> = vec![i32::MAX; size];

    let start_idx = (start.1 * width + start.0) as usize;
    g_score[start_idx] = 0;
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
            return Some(reconstruct_path(&came_from, start, goal, width));
        }

        let current_idx = (cy * width + cx) as usize;
        let current_g = g_score[current_idx];

        // Explore 4 cardinal neighbors
        for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
            let nx = cx + dx;
            let ny = cy + dy;
            if nx < 0 || nx >= width || ny < 0 || ny >= height { continue; }
            let next = (nx, ny);

            // Allow moving to goal even if "impassable" (it's often the target entity)
            let is_goal = next == goal;
            if !is_goal && !is_passable(nx, ny) {
                continue;
            }

            let next_idx = (ny * width + nx) as usize;
            let new_g = current_g + 1;

            if new_g < g_score[next_idx] {
                g_score[next_idx] = new_g;
                came_from[next_idx] = current_idx as i32;
                let f = new_g + distance_manhattan(nx, ny, goal.0, goal.1);
                open.push(Reverse((f, nx, ny)));
            }
        }
    }

    None // No path found
}

/// A* pathfinding with 8-directional movement (including diagonals).
pub fn astar_8dir(
    start: (i32, i32),
    goal: (i32, i32),
    width: i32,
    height: i32,
    is_passable: impl Fn(i32, i32) -> bool,
    max_iterations: usize,
) -> Option<Vec<(i32, i32)>> {
    if width <= 0 || height <= 0 { return None; }
    if start.0 < 0 || start.0 >= width || start.1 < 0 || start.1 >= height { return None; }
    if goal.0  < 0 || goal.0  >= width || goal.1  < 0 || goal.1  >= height { return None; }

    if start == goal {
        return Some(vec![start]);
    }

    let size = (width * height) as usize;
    let mut open: BinaryHeap<Reverse<(i32, i32, i32)>> = BinaryHeap::new();
    let mut came_from: Vec<i32> = vec![-1; size];
    let mut g_score: Vec<i32> = vec![i32::MAX; size];

    let start_idx = (start.1 * width + start.0) as usize;
    g_score[start_idx] = 0;
    let h = distance_manhattan(start.0, start.1, goal.0, goal.1);
    open.push(Reverse((h, start.0, start.1)));

    let mut iterations = 0;

    // 8 directions including diagonals
    let directions = [
        (0, -1), (1, -1), (1, 0), (1, 1),
        (0, 1), (-1, 1), (-1, 0), (-1, -1),
    ];

    while let Some(Reverse((_, cx, cy))) = open.pop() {
        iterations += 1;
        if iterations > max_iterations {
            return None;
        }

        let current = (cx, cy);

        if current == goal {
            return Some(reconstruct_path(&came_from, start, goal, width));
        }

        let current_idx = (cy * width + cx) as usize;
        let current_g = g_score[current_idx];

        for (dx, dy) in directions {
            let nx = cx + dx;
            let ny = cy + dy;
            if nx < 0 || nx >= width || ny < 0 || ny >= height { continue; }
            let next = (nx, ny);

            let is_goal = next == goal;
            if !is_goal && !is_passable(nx, ny) {
                continue;
            }

            // Diagonal movement costs slightly more (approximation of sqrt(2))
            let cost = if dx != 0 && dy != 0 { 14 } else { 10 };
            let new_g = current_g + cost;
            let next_idx = (ny * width + nx) as usize;

            if new_g < g_score[next_idx] {
                g_score[next_idx] = new_g;
                came_from[next_idx] = current_idx as i32;
                // Use Chebyshev distance as heuristic for 8-dir
                let h = (nx - goal.0).abs().max((ny - goal.1).abs()) * 10;
                let f = new_g + h;
                open.push(Reverse((f, nx, ny)));
            }
        }
    }

    None
}

/// Reconstruct path from came_from map.
fn reconstruct_path(
    came_from: &[i32],
    start: (i32, i32),
    goal: (i32, i32),
    width: i32,
) -> Vec<(i32, i32)> {
    let mut path = vec![goal];
    let start_idx = (start.1 * width + start.0) as i32;
    let mut current_idx = (goal.1 * width + goal.0) as i32;

    while current_idx != start_idx {
        current_idx = came_from[current_idx as usize];
        if current_idx == -1 { break; }
        let x = current_idx % width;
        let y = current_idx / width;
        path.push((x, y));
    }

    path.reverse();
    path
}

/// Returns None if already at goal or no path exists.
pub fn astar_next_step(
    start: (i32, i32),
    goal: (i32, i32),
    width: i32,
    height: i32,
    is_passable: impl Fn(i32, i32) -> bool,
    max_iterations: usize,
) -> Option<(i32, i32)> {
    let path = astar(start, goal, width, height, is_passable, max_iterations)?;
    if path.len() > 1 { Some(path[1]) } else { None }
}

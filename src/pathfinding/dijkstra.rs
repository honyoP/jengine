use std::collections::VecDeque;

// =============================================================================
// DIJKSTRA MAPS
// =============================================================================
///
/// A Dijkstra map stores the distance from every cell to the nearest goal.
///
/// Used for:
/// - Flee behavior: move AWAY from low values (toward high)
/// - Approach behavior: move TOWARD low values
/// - Combining influences: add multiple Dijkstra maps together
///
/// Convention:
/// - 0.0 = goal
/// - Higher values = farther from goal
/// - f32::MAX = unreachable
pub struct DijkstraMap {
    pub width: i32,
    pub height: i32,
    values: Vec<f32>,
}

impl DijkstraMap {
    /// Create a new Dijkstra map from goal points.
    ///
    /// # Arguments
    /// * `width`, `height` - Map dimensions
    /// * `goals` - Goal positions (will have value 0.0)
    /// * `is_passable` - Function returning true for walkable tiles
    pub fn new(
        width: i32,
        height: i32,
        goals: &[(i32, i32)],
        is_passable: impl Fn(i32, i32) -> bool,
    ) -> Self {
        if width <= 0 || height <= 0 {
            return Self { width, height, values: Vec::new() };
        }

        let size = (width * height) as usize;
        let mut values = vec![f32::MAX; size];

        // BFS from all goal cells simultaneously — O(cells) instead of O(cells²).
        let mut queue: VecDeque<(i32, i32)> = VecDeque::new();
        for &(gx, gy) in goals {
            if gx >= 0 && gy >= 0 && gx < width && gy < height {
                let idx = (gy * width + gx) as usize;
                if values[idx] == f32::MAX {
                    values[idx] = 0.0;
                    queue.push_back((gx, gy));
                }
            }
        }

        while let Some((x, y)) = queue.pop_front() {
            let current = values[(y * width + x) as usize];
            for (dx, dy) in [(0i32, -1i32), (0, 1), (-1, 0), (1, 0)] {
                let nx = x + dx;
                let ny = y + dy;
                if nx < 0 || ny < 0 || nx >= width || ny >= height { continue; }
                if !is_passable(nx, ny) { continue; }
                let nidx = (ny * width + nx) as usize;
                if values[nidx] == f32::MAX {
                    values[nidx] = current + 1.0;
                    queue.push_back((nx, ny));
                }
            }
        }

        Self { width, height, values }
    }

    /// Get the value at (x, y). Returns f32::MAX for out of bounds.
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> f32 {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            return f32::MAX;
        }
        self.values[(y * self.width + x) as usize]
    }

    /// Find the direction to move to get closer to goals (toward lower values).
    pub fn direction_to_goal(&self, x: i32, y: i32) -> (i32, i32) {
        let current = self.get(x, y);
        if current == 0.0 || current == f32::MAX {
            return (0, 0);
        }

        let mut best_dir = (0, 0);
        let mut best_value = current;

        for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
            let value = self.get(x + dx, y + dy);
            if value < best_value {
                best_value = value;
                best_dir = (dx, dy);
            }
        }

        best_dir
    }

    /// Find the direction to move to get away from goals (toward higher values).
    pub fn direction_away(&self, x: i32, y: i32) -> (i32, i32) {
        let current = self.get(x, y);
        if current == f32::MAX {
            return (0, 0);
        }

        let mut best_dir = (0, 0);
        let mut best_value = current;

        for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
            let value = self.get(x + dx, y + dy);
            if value != f32::MAX && value > best_value {
                best_value = value;
                best_dir = (dx, dy);
            }
        }

        best_dir
    }

    /// Invert the map (for flee behavior).
    pub fn invert(&mut self) {
        let max_val = self
            .values
            .iter()
            .filter(|&&v| v != f32::MAX)
            .fold(0.0f32, |a, &b| a.max(b));

        for v in &mut self.values {
            if *v != f32::MAX {
                *v = max_val - *v;
            }
        }
    }

    /// Multiply all values by a factor.
    pub fn multiply(&mut self, factor: f32) {
        for v in &mut self.values {
            if *v != f32::MAX {
                *v *= factor;
            }
        }
    }

    /// Add another Dijkstra map to this one.
    pub fn add(&mut self, other: &DijkstraMap) {
        assert_eq!(self.width, other.width);
        assert_eq!(self.height, other.height);

        for (a, b) in self.values.iter_mut().zip(other.values.iter()) {
            if *a != f32::MAX && *b != f32::MAX {
                *a += *b;
            }
        }
    }
}

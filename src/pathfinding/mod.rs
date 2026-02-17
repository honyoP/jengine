mod astar;
mod dijkstra;

pub mod prelude {
    pub use crate::pathfinding::astar::*;
    pub use crate::pathfinding::dijkstra::*;
}

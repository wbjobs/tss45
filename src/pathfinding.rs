use priority_queue::PriorityQueue;
use std::collections::{HashMap, HashSet};

use crate::components::Position;
use crate::resources::{TileLocks, TileMap};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Node {
    position: Position,
    g_cost: i32,
    h_cost: i32,
    parent: Option<Position>,
}

impl Node {
    fn new(position: Position, g_cost: i32, h_cost: i32, parent: Option<Position>) -> Self {
        Self {
            position,
            g_cost,
            h_cost,
            parent,
        }
    }

    fn f_cost(&self) -> i32 {
        self.g_cost + self.h_cost
    }
}

fn heuristic(a: &Position, b: &Position) -> i32 {
    let dx = (a.x - b.x).abs();
    let dy = (a.y - b.y).abs();
    let straight = (dx - dy).abs();
    let diagonal = dx.min(dy);
    straight * 10 + diagonal * 14
}

fn get_distance(a: &Position, b: &Position) -> i32 {
    let dx = (a.x - b.x).abs();
    let dy = (a.y - b.y).abs();
    if dx == 1 && dy == 1 {
        14
    } else {
        10
    }
}

pub fn find_path(
    tile_map: &TileMap,
    start: &Position,
    goal: &Position,
) -> Option<Vec<Position>> {
    find_path_with_locks(tile_map, start, goal, None)
}

pub fn find_path_with_locks(
    tile_map: &TileMap,
    start: &Position,
    goal: &Position,
    tile_locks: Option<&TileLocks>,
) -> Option<Vec<Position>> {
    if start == goal {
        return Some(vec![*start]);
    }

    if !tile_map.is_passable(goal.x, goal.y) {
        return None;
    }

    let mut open_set: PriorityQueue<Position, std::cmp::Reverse<i32>> = PriorityQueue::new();
    let mut nodes: HashMap<Position, Node> = HashMap::new();
    let mut closed_set: HashSet<Position> = HashSet::new();

    let start_h = heuristic(start, goal);
    let start_node = Node::new(*start, 0, start_h, None);

    nodes.insert(*start, start_node);
    open_set.push(*start, std::cmp::Reverse(start_h));

    while let Some((current_pos, _)) = open_set.pop() {
        closed_set.insert(current_pos);

        if current_pos == *goal {
            return Some(reconstruct_path(&nodes, current_pos));
        }

        let current_g = nodes.get(&current_pos).map(|n| n.g_cost).unwrap_or(0);
        let neighbors = get_neighbors_with_locks(tile_map, &current_pos, goal, tile_locks);

        for neighbor_pos in neighbors {
            if closed_set.contains(&neighbor_pos) {
                continue;
            }

            let move_cost = get_distance(&current_pos, &neighbor_pos);
            let new_g = current_g + move_cost;

            let existing_node = nodes.get(&neighbor_pos);
            if existing_node.is_none() || new_g < existing_node.unwrap().g_cost {
                let h = heuristic(&neighbor_pos, goal);
                let f = new_g + h;

                nodes.insert(
                    neighbor_pos,
                    Node::new(neighbor_pos, new_g, h, Some(current_pos)),
                );

                if !open_set.iter().any(|(p, _)| *p == neighbor_pos) {
                    open_set.push(neighbor_pos, std::cmp::Reverse(f));
                } else {
                    open_set.change_priority(&neighbor_pos, std::cmp::Reverse(f));
                }
            }
        }
    }

    None
}

fn get_neighbors_with_locks(
    tile_map: &TileMap,
    pos: &Position,
    goal: &Position,
    tile_locks: Option<&TileLocks>,
) -> Vec<Position> {
    let mut neighbors = Vec::new();
    let directions = [
        (0, -1),
        (1, 0),
        (0, 1),
        (-1, 0),
        (1, -1),
        (1, 1),
        (-1, 1),
        (-1, -1),
    ];

    for (dx, dy) in directions.iter() {
        let nx = pos.x + dx;
        let ny = pos.y + dy;

        if !tile_map.is_passable(nx, ny) {
            continue;
        }

        let neighbor_pos = Position::new(nx, ny);

        if let Some(locks) = tile_locks {
            if neighbor_pos != *goal && locks.is_locked(nx, ny) {
                continue;
            }
        }

        neighbors.push(neighbor_pos);
    }

    neighbors
}

fn reconstruct_path(nodes: &HashMap<Position, Node>, mut current: Position) -> Vec<Position> {
    let mut path = vec![current];
    while let Some(node) = nodes.get(&current) {
        if let Some(parent) = node.parent {
            path.push(parent);
            current = parent;
        } else {
            break;
        }
    }
    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{ResourceType, Tile};
    use crate::resources::TileLocks;

    #[test]
    fn test_simple_path() {
        let mut map = TileMap::new(10, 10);
        let start = Position::new(0, 0);
        let goal = Position::new(3, 0);

        let path = find_path(&map, &start, &goal);
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path[0], start);
        assert_eq!(path[path.len() - 1], goal);
    }

    #[test]
    fn test_path_with_obstacle() {
        let mut map = TileMap::new(10, 10);
        map.set_tile(1, 0, Tile::wall());

        let start = Position::new(0, 0);
        let goal = Position::new(3, 0);

        let path = find_path(&map, &start, &goal);
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path[0], start);
        assert_eq!(path[path.len() - 1], goal);
        for pos in &path {
            assert_ne!(*pos, Position::new(1, 0));
        }
    }

    #[test]
    fn test_no_path() {
        let mut map = TileMap::new(5, 5);
        for x in 0..5 {
            map.set_tile(x, 2, Tile::wall());
        }

        let start = Position::new(0, 0);
        let goal = Position::new(4, 4);

        let path = find_path(&map, &start, &goal);
        assert!(path.is_none());
    }

    #[test]
    fn test_same_start_goal() {
        let map = TileMap::new(10, 10);
        let start = Position::new(5, 5);

        let path = find_path(&map, &start, &start);
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path.len(), 1);
        assert_eq!(path[0], start);
    }

    #[test]
    fn test_diagonal_path() {
        let map = TileMap::new(10, 10);
        let start = Position::new(0, 0);
        let goal = Position::new(2, 2);

        let path = find_path(&map, &start, &goal);
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.len() >= 3);
        assert_eq!(path[0], start);
        assert_eq!(path[path.len() - 1], goal);
    }

    #[test]
    fn test_path_with_locks() {
        let map = TileMap::new(10, 10);
        let start = Position::new(0, 0);
        let goal = Position::new(3, 0);

        let mut locks = TileLocks::new();
        locks.try_lock(1, 0, bevy_ecs::entity::Entity::PLACEHOLDER, ResourceType::Food);

        let path = find_path_with_locks(&map, &start, &goal, Some(&locks));
        assert!(path.is_some());
        let path = path.unwrap();
        for pos in &path {
            assert_ne!(*pos, Position::new(1, 0));
        }
    }

    #[test]
    fn test_path_goal_can_be_locked() {
        let map = TileMap::new(10, 10);
        let start = Position::new(0, 0);
        let goal = Position::new(3, 0);

        let mut locks = TileLocks::new();
        locks.try_lock(3, 0, bevy_ecs::entity::Entity::PLACEHOLDER, ResourceType::Food);

        let path = find_path_with_locks(&map, &start, &goal, Some(&locks));
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path[path.len() - 1], goal);
    }
}

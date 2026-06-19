use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::components::{Position, ResourceType, Tile};

#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct TileMap {
    pub width: i32,
    pub height: i32,
    pub tiles: HashMap<(i32, i32), Tile>,
}

impl TileMap {
    pub fn new(width: i32, height: i32) -> Self {
        let mut tiles = HashMap::new();
        for x in 0..width {
            for y in 0..height {
                tiles.insert((x, y), Tile::empty());
            }
        }
        Self {
            width,
            height,
            tiles,
        }
    }

    pub fn get_tile(&self, x: i32, y: i32) -> Option<&Tile> {
        if x < 0 || x >= self.width || y < 0 || y >= self.height {
            return None;
        }
        self.tiles.get(&(x, y))
    }

    pub fn get_tile_mut(&mut self, x: i32, y: i32) -> Option<&mut Tile> {
        if x < 0 || x >= self.width || y < 0 || y >= self.height {
            return None;
        }
        self.tiles.get_mut(&(x, y))
    }

    pub fn is_passable(&self, x: i32, y: i32) -> bool {
        self.get_tile(x, y).map(|t| t.passable).unwrap_or(false)
    }

    pub fn get_resource_type(&self, x: i32, y: i32) -> ResourceType {
        self.get_tile(x, y)
            .map(|t| t.resource_type)
            .unwrap_or(ResourceType::None)
    }

    pub fn set_tile(&mut self, x: i32, y: i32, tile: Tile) {
        if x >= 0 && x < self.width && y >= 0 && y < self.height {
            self.tiles.insert((x, y), tile);
        }
    }

    pub fn find_nearest_resource(
        &self,
        from: &Position,
        resource_type: ResourceType,
    ) -> Option<Position> {
        let mut nearest: Option<(Position, i32)> = None;

        for ((x, y), tile) in &self.tiles {
            if tile.resource_type == resource_type && tile.resource_amount > 0 {
                let pos = Position::new(*x, *y);
                let dist = from.manhattan_to(&pos);
                match nearest {
                    None => nearest = Some((pos, dist)),
                    Some((_, current_dist)) if dist < current_dist => {
                        nearest = Some((pos, dist));
                    }
                    _ => {}
                }
            }
        }

        nearest.map(|(pos, _)| pos)
    }

    pub fn gather_resource(&mut self, x: i32, y: i32, amount: u32) -> u32 {
        if let Some(tile) = self.get_tile_mut(x, y) {
            let gathered = tile.resource_amount.min(amount);
            tile.resource_amount -= gathered;
            if tile.resource_amount == 0 {
                tile.resource_type = ResourceType::None;
            }
            return gathered;
        }
        0
    }

    pub fn get_neighbors(&self, pos: &Position) -> Vec<Position> {
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
            if self.is_passable(nx, ny) {
                neighbors.push(Position::new(nx, ny));
            }
        }

        neighbors
    }
}

#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct GameTime {
    pub tick: u64,
    pub elapsed_seconds: f64,
}

impl GameTime {
    pub fn new() -> Self {
        Self {
            tick: 0,
            elapsed_seconds: 0.0,
        }
    }

    pub fn tick(&mut self, delta_seconds: f64) {
        self.tick += 1;
        self.elapsed_seconds += delta_seconds;
    }
}

impl Default for GameTime {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Resource, Clone, Debug, Default)]
pub struct SaveLoadState {
    pub pending_save: Option<String>,
    pub pending_load: Option<String>,
}

#[derive(Resource, Clone, Debug)]
pub struct NetworkChannels {
    pub event_sender: crossbeam_channel::Sender<NetworkEvent>,
    pub command_receiver: crossbeam_channel::Receiver<NetworkCommand>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NetworkEvent {
    PawnMoved {
        entity_id: u64,
        position: Position,
    },
    ResourceChanged {
        position: Position,
        resource_type: ResourceType,
        amount: u32,
    },
    InventoryChanged {
        entity_id: u64,
        inventory: crate::components::Inventory,
    },
    TaskChanged {
        entity_id: u64,
        task: crate::components::Task,
    },
    HungerChanged {
        entity_id: u64,
        value: f32,
        max: f32,
    },
    StaminaChanged {
        entity_id: u64,
        value: f32,
        max: f32,
    },
    HappinessChanged {
        entity_id: u64,
        value: f32,
        max: f32,
    },
    SkillsChanged {
        entity_id: u64,
        mining: f32,
        farming: f32,
        building: f32,
    },
    BuildingCompleted {
        position: Position,
        building_type: crate::components::BuildingType,
    },
    WorldState {
        tick: u64,
        map_size: (i32, i32),
    },
}

#[derive(Clone, Debug, Deserialize)]
pub enum NetworkCommand {
    Subscribe,
    Unsubscribe,
    SaveGame(String),
    LoadGame(String),
    SpawnPawn(Position),
    SetTile(Position, Tile),
}

#[derive(Resource, Clone, Debug, Default)]
pub struct TileLocks {
    pub locks: std::collections::HashMap<(i32, i32), (bevy_ecs::entity::Entity, ResourceType)>,
}

impl TileLocks {
    pub fn new() -> Self {
        Self {
            locks: std::collections::HashMap::new(),
        }
    }

    pub fn is_locked(&self, x: i32, y: i32) -> bool {
        self.locks.contains_key(&(x, y))
    }

    pub fn is_locked_by(&self, x: i32, y: i32, entity: bevy_ecs::entity::Entity) -> bool {
        self.locks.get(&(x, y)).map(|(e, _)| *e == entity).unwrap_or(false)
    }

    pub fn try_lock(
        &mut self,
        x: i32,
        y: i32,
        entity: bevy_ecs::entity::Entity,
        resource_type: ResourceType,
    ) -> bool {
        if self.is_locked(x, y) {
            return false;
        }
        self.locks.insert((x, y), (entity, resource_type));
        true
    }

    pub fn unlock(&mut self, x: i32, y: i32) {
        self.locks.remove(&(x, y));
    }

    pub fn unlock_entity(&mut self, entity: bevy_ecs::entity::Entity) {
        self.locks.retain(|_, (e, _)| *e != entity);
    }

    pub fn get_locked_positions(&self) -> Vec<(i32, i32)> {
        self.locks.keys().copied().collect()
    }

    pub fn get_locked_count(&self) -> usize {
        self.locks.len()
    }
}

#[derive(Resource, Clone, Debug, Default)]
pub struct TaskScheduler {
    pub pending_requests: Vec<(bevy_ecs::entity::Entity, Position, crate::components::TaskCategory)>,
    pub assignments: std::collections::HashMap<bevy_ecs::entity::Entity, (Position, crate::components::TaskCategory)>,
}

impl TaskScheduler {
    pub fn new() -> Self {
        Self {
            pending_requests: Vec::new(),
            assignments: std::collections::HashMap::new(),
        }
    }

    pub fn request_task(
        &mut self,
        entity: bevy_ecs::entity::Entity,
        position: Position,
        category: crate::components::TaskCategory,
    ) {
        if !self.assignments.contains_key(&entity)
            && !self.pending_requests.iter().any(|(e, _, _)| *e == entity)
        {
            self.pending_requests.push((entity, position, category));
        }
    }

    pub fn complete_task(&mut self, entity: bevy_ecs::entity::Entity) {
        self.assignments.remove(&entity);
    }

    pub fn get_assignment(&self, entity: bevy_ecs::entity::Entity) -> Option<(Position, crate::components::TaskCategory)> {
        self.assignments.get(&entity).cloned()
    }

    pub fn has_assignment(&self, entity: bevy_ecs::entity::Entity) -> bool {
        self.assignments.contains_key(&entity)
    }
}

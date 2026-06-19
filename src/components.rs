use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn distance_to(&self, other: &Position) -> f32 {
        let dx = (self.x - other.x) as f32;
        let dy = (self.y - other.y) as f32;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn manhattan_to(&self, other: &Position) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    Iron,
    Wood,
    Food,
    None,
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Tile {
    pub resource_type: ResourceType,
    pub passable: bool,
    pub resource_amount: u32,
}

impl Tile {
    pub fn new(resource_type: ResourceType, passable: bool, resource_amount: u32) -> Self {
        Self {
            resource_type,
            passable,
            resource_amount,
        }
    }

    pub fn empty() -> Self {
        Self::new(ResourceType::None, true, 0)
    }

    pub fn resource(resource_type: ResourceType, amount: u32) -> Self {
        Self::new(resource_type, true, amount)
    }

    pub fn wall() -> Self {
        Self::new(ResourceType::None, false, 0)
    }
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Hunger {
    pub value: f32,
    pub max: f32,
    pub decay_rate: f32,
}

impl Hunger {
    pub fn new(max: f32, decay_rate: f32) -> Self {
        Self {
            value: max,
            max,
            decay_rate,
        }
    }

    pub fn is_hungry(&self) -> bool {
        self.value < self.max * 0.3
    }

    pub fn is_starving(&self) -> bool {
        self.value <= 0.0
    }

    pub fn eat(&mut self, amount: f32) {
        self.value = (self.value + amount).min(self.max);
    }

    pub fn decay(&mut self, delta: f32) {
        self.value = (self.value - self.decay_rate * delta).max(0.0);
    }
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Stamina {
    pub value: f32,
    pub max: f32,
    pub regeneration_rate: f32,
    pub move_cost: f32,
    pub gather_cost: f32,
}

impl Stamina {
    pub fn new(max: f32, regeneration_rate: f32, move_cost: f32, gather_cost: f32) -> Self {
        Self {
            value: max,
            max,
            regeneration_rate,
            move_cost,
            gather_cost,
        }
    }

    pub fn can_move(&self) -> bool {
        self.value >= self.move_cost
    }

    pub fn can_gather(&self) -> bool {
        self.value >= self.gather_cost
    }

    pub fn consume_move(&mut self) {
        self.value = (self.value - self.move_cost).max(0.0);
    }

    pub fn consume_gather(&mut self) {
        self.value = (self.value - self.gather_cost).max(0.0);
    }

    pub fn regenerate(&mut self, delta: f32) {
        self.value = (self.value + self.regeneration_rate * delta).min(self.max);
    }
}

#[derive(Component, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Task {
    Idle,
    FindFood,
    FindResource(ResourceType),
    MoveTo(Position),
    Gather(Position, ResourceType),
    ReturnToBase,
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Path {
    pub waypoints: Vec<Position>,
    pub current_index: usize,
}

impl Path {
    pub fn new(waypoints: Vec<Position>) -> Self {
        Self {
            waypoints,
            current_index: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.waypoints.is_empty()
    }

    pub fn current_target(&self) -> Option<&Position> {
        self.waypoints.get(self.current_index)
    }

    pub fn advance(&mut self) {
        if self.current_index < self.waypoints.len() {
            self.current_index += 1;
        }
    }

    pub fn is_complete(&self) -> bool {
        self.current_index >= self.waypoints.len()
    }

    pub fn clear(&mut self) {
        self.waypoints.clear();
        self.current_index = 0;
    }
}

#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Inventory {
    pub iron: u32,
    pub wood: u32,
    pub food: u32,
}

impl Inventory {
    pub fn add(&mut self, resource_type: ResourceType, amount: u32) {
        match resource_type {
            ResourceType::Iron => self.iron += amount,
            ResourceType::Wood => self.wood += amount,
            ResourceType::Food => self.food += amount,
            ResourceType::None => {}
        }
    }

    pub fn take(&mut self, resource_type: ResourceType, amount: u32) -> u32 {
        let taken = match resource_type {
            ResourceType::Iron => self.iron.min(amount),
            ResourceType::Wood => self.wood.min(amount),
            ResourceType::Food => self.food.min(amount),
            ResourceType::None => 0,
        };
        match resource_type {
            ResourceType::Iron => self.iron -= taken,
            ResourceType::Wood => self.wood -= taken,
            ResourceType::Food => self.food -= taken,
            ResourceType::None => {}
        }
        taken
    }

    pub fn count(&self, resource_type: ResourceType) -> u32 {
        match resource_type {
            ResourceType::Iron => self.iron,
            ResourceType::Wood => self.wood,
            ResourceType::Food => self.food,
            ResourceType::None => 0,
        }
    }
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Pawn;

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct NeedsPathfinding;

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct TileMarker;

#[derive(Component, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileLock {
    pub locked_by: Option<Entity>,
    pub resource_type: ResourceType,
}

impl TileLock {
    pub fn new(locked_by: Entity, resource_type: ResourceType) -> Self {
        Self {
            locked_by: Some(locked_by),
            resource_type,
        }
    }

    pub fn is_locked(&self) -> bool {
        self.locked_by.is_some()
    }

    pub fn is_locked_by(&self, entity: Entity) -> bool {
        self.locked_by == Some(entity)
    }
}

#[derive(Component, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestTask {
    pub resource_type: ResourceType,
}

impl RequestTask {
    pub fn new(resource_type: ResourceType) -> Self {
        Self { resource_type }
    }
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct AssignedTask {
    pub target_position: Position,
    pub resource_type: ResourceType,
}

impl AssignedTask {
    pub fn new(target_position: Position, resource_type: ResourceType) -> Self {
        Self {
            target_position,
            resource_type,
        }
    }
}

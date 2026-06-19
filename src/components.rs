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
    Build(Position),
    Entertain(Position),
    Socialize(Entity),
    ReturnToBase,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskCategory {
    GatherResource(ResourceType),
    BuildStructure,
    Entertainment,
    Socialize,
    Idle,
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
    pub category: TaskCategory,
}

impl RequestTask {
    pub fn new(category: TaskCategory) -> Self {
        Self { category }
    }

    pub fn gather(resource_type: ResourceType) -> Self {
        Self {
            category: TaskCategory::GatherResource(resource_type),
        }
    }

    pub fn build() -> Self {
        Self {
            category: TaskCategory::BuildStructure,
        }
    }

    pub fn entertain() -> Self {
        Self {
            category: TaskCategory::Entertainment,
        }
    }

    pub fn socialize() -> Self {
        Self {
            category: TaskCategory::Socialize,
        }
    }
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct AssignedTask {
    pub target_position: Position,
    pub category: TaskCategory,
}

impl AssignedTask {
    pub fn new(target_position: Position, category: TaskCategory) -> Self {
        Self {
            target_position,
            category,
        }
    }

    pub fn gather(target_position: Position, resource_type: ResourceType) -> Self {
        Self {
            target_position,
            category: TaskCategory::GatherResource(resource_type),
        }
    }

    pub fn build(target_position: Position) -> Self {
        Self {
            target_position,
            category: TaskCategory::BuildStructure,
        }
    }

    pub fn entertain(target_position: Position) -> Self {
        Self {
            target_position,
            category: TaskCategory::Entertainment,
        }
    }
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Skills {
    pub mining: f32,
    pub farming: f32,
    pub building: f32,
}

impl Skills {
    pub fn new(mining: f32, farming: f32, building: f32) -> Self {
        Self {
            mining,
            farming,
            building,
        }
    }

    pub fn default() -> Self {
        Self::new(1.0, 1.0, 1.0)
    }

    pub fn level_up(&mut self, skill: SkillType, amount: f32) {
        match skill {
            SkillType::Mining => self.mining = (self.mining + amount).min(10.0),
            SkillType::Farming => self.farming = (self.farming + amount).min(10.0),
            SkillType::Building => self.building = (self.building + amount).min(10.0),
        }
    }

    pub fn get_skill(&self, skill_type: SkillType) -> f32 {
        match skill_type {
            SkillType::Mining => self.mining,
            SkillType::Farming => self.farming,
            SkillType::Building => self.building,
        }
    }

    pub fn get_gather_bonus(&self, resource_type: ResourceType) -> f32 {
        match resource_type {
            ResourceType::Iron => self.mining,
            ResourceType::Wood => self.mining * 0.8 + self.farming * 0.2,
            ResourceType::Food => self.farming,
            ResourceType::None => 1.0,
        }
    }
}

impl Default for Skills {
    fn default() -> Self {
        Self::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SkillType {
    Mining,
    Farming,
    Building,
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Happiness {
    pub value: f32,
    pub max: f32,
    pub decay_rate: f32,
    pub food_memory: Vec<ResourceType>,
    pub food_memory_max: usize,
    pub entertainment_factor: f32,
    pub social_factor: f32,
}

impl Happiness {
    pub fn new(max: f32) -> Self {
        Self {
            value: max * 0.7,
            max,
            decay_rate: 0.5,
            food_memory: Vec::new(),
            food_memory_max: 5,
            entertainment_factor: 0.3,
            social_factor: 0.2,
        }
    }

    pub fn is_unhappy(&self) -> bool {
        self.value < self.max * 0.4
    }

    pub fn is_depressed(&self) -> bool {
        self.value < self.max * 0.2
    }

    pub fn refuses_to_work(&self) -> bool {
        self.value < self.max * 0.25
    }

    pub fn eat_food(&mut self, food_type: ResourceType) {
        if food_type == ResourceType::Food {
            self.food_memory.push(food_type);
            if self.food_memory.len() > self.food_memory_max {
                self.food_memory.remove(0);
            }

            let variety = self.calculate_food_variety();
            let happiness_change = if variety > 0.6 {
                5.0
            } else if variety > 0.3 {
                0.0
            } else {
                -3.0
            };
            self.value = (self.value + happiness_change).max(0.0).min(self.max);
        }
    }

    fn calculate_food_variety(&self) -> f32 {
        if self.food_memory.is_empty() {
            return 1.0;
        }

        let mut unique_types = std::collections::HashSet::new();
        for food in &self.food_memory {
            unique_types.insert(food);
        }

        unique_types.len() as f32 / self.food_memory.len() as f32
    }

    pub fn gain_entertainment(&mut self, amount: f32) {
        self.value = (self.value + amount * self.entertainment_factor).min(self.max);
    }

    pub fn gain_social(&mut self, amount: f32) {
        self.value = (self.value + amount * self.social_factor).min(self.max);
    }

    pub fn decay(&mut self, delta: f32) {
        self.value = (self.value - self.decay_rate * delta).max(0.0);
    }
}

impl Default for Happiness {
    fn default() -> Self {
        Self::new(100.0)
    }
}

#[derive(Component, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildingType {
    House,
    Farm,
    EntertainmentPark,
    Mine,
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Building {
    pub building_type: BuildingType,
    pub health: f32,
    pub max_health: f32,
    pub is_complete: bool,
    pub build_progress: f32,
    pub build_cost_wood: u32,
    pub build_cost_stone: u32,
}

impl Building {
    pub fn new(building_type: BuildingType) -> Self {
        let (max_health, wood_cost, stone_cost) = match building_type {
            BuildingType::House => (100.0, 20, 10),
            BuildingType::Farm => (50.0, 10, 0),
            BuildingType::EntertainmentPark => (150.0, 50, 20),
            BuildingType::Mine => (80.0, 15, 25),
        };

        Self {
            building_type,
            health: 0.0,
            max_health,
            is_complete: false,
            build_progress: 0.0,
            build_cost_wood: wood_cost,
            build_cost_stone: stone_cost,
        }
    }

    pub fn build(&mut self, amount: f32, building_skill: f32) -> bool {
        if self.is_complete {
            return true;
        }

        let build_speed = amount * (1.0 + building_skill * 0.2);
        self.build_progress += build_speed;
        self.health = self.build_progress.min(self.max_health);

        if self.build_progress >= self.max_health {
            self.is_complete = true;
            self.health = self.max_health;
            return true;
        }

        false
    }
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct EntertainmentFacility {
    pub radius: i32,
    pub happiness_per_second: f32,
    pub capacity: u32,
    pub current_visitors: u32,
}

impl EntertainmentFacility {
    pub fn new(radius: i32, happiness_per_second: f32, capacity: u32) -> Self {
        Self {
            radius,
            happiness_per_second,
            capacity,
            current_visitors: 0,
        }
    }

    pub fn can_visit(&self) -> bool {
        self.current_visitors < self.capacity
    }

    pub fn enter(&mut self) -> bool {
        if self.can_visit() {
            self.current_visitors += 1;
            true
        } else {
            false
        }
    }

    pub fn leave(&mut self) {
        if self.current_visitors > 0 {
            self.current_visitors -= 1;
        }
    }
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct Socializing {
    pub target_entity: Option<Entity>,
    pub social_duration: f32,
}

impl Socializing {
    pub fn new(target: Entity) -> Self {
        Self {
            target_entity: Some(target),
            social_duration: 0.0,
        }
    }

    pub fn idle() -> Self {
        Self {
            target_entity: None,
            social_duration: 0.0,
        }
    }
}

#[derive(Component, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeedEntertainment;

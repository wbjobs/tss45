use bevy_ecs::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::components::{
    Building, BuildingType, EntertainmentFacility, Position, ResourceType, SkillType, Skills,
    TaskCategory,
};
use crate::resources::{NetworkChannels, NetworkEvent, TileLocks, TileMap, TaskScheduler};

struct TaskSlot {
    position: Position,
    category: TaskCategory,
    skill_required: SkillType,
    priority: f32,
}

impl TaskSlot {
    fn gather(position: Position, resource_type: ResourceType) -> Self {
        let skill = match resource_type {
            ResourceType::Iron => SkillType::Mining,
            ResourceType::Wood => SkillType::Mining,
            ResourceType::Food => SkillType::Farming,
            ResourceType::None => SkillType::Farming,
        };
        Self {
            position,
            category: TaskCategory::GatherResource(resource_type),
            skill_required: skill,
            priority: 1.0,
        }
    }

    fn build(position: Position, building_type: BuildingType) -> Self {
        let priority = match building_type {
            BuildingType::EntertainmentPark => 1.5,
            BuildingType::House => 1.2,
            BuildingType::Farm => 1.0,
            BuildingType::Mine => 1.3,
        };
        Self {
            position,
            category: TaskCategory::BuildStructure,
            skill_required: SkillType::Building,
            priority,
        }
    }

    fn entertain(position: Position) -> Self {
        Self {
            position,
            category: TaskCategory::Entertainment,
            skill_required: SkillType::Building,
            priority: 2.0,
        }
    }

    fn socialize(position: Position) -> Self {
        Self {
            position,
            category: TaskCategory::Socialize,
            skill_required: SkillType::Building,
            priority: 1.8,
        }
    }
}

fn calculate_match_score(
    pawn_pos: &Position,
    skills: &Skills,
    slot: &TaskSlot,
) -> f32 {
    let distance = pawn_pos.manhattan_to(&slot.position) as f32;
    let distance_score = 1.0 / (distance + 1.0);

    let skill_level = skills.get_skill(slot.skill_required);
    let skill_score = skill_level / 10.0;

    let distance_weight = 0.4;
    let skill_weight = 0.5;
    let priority_weight = 0.1;

    distance_score * distance_weight + skill_score * skill_weight + slot.priority * priority_weight
}

pub fn task_scheduler_system(
    mut commands: Commands,
    tile_map: Res<TileMap>,
    buildings: Query<(Entity, &Position, &Building, Option<&EntertainmentFacility>)>,
    pawns: Query<(Entity, &Position, &Skills)>,
    mut tile_locks: ResMut<TileLocks>,
    mut task_scheduler: ResMut<TaskScheduler>,
    network_channels: Option<Res<NetworkChannels>>,
) {
    let pending: Vec<(Entity, Position, TaskCategory)> =
        task_scheduler.pending_requests.drain(..).collect();

    if pending.is_empty() {
        return;
    }

    let mut available_slots: Vec<TaskSlot> = Vec::new();
    collect_gather_tasks(&tile_map, &tile_locks, &mut available_slots);
    collect_build_tasks(&buildings, &mut available_slots);
    collect_entertainment_tasks(&buildings, &mut available_slots);
    collect_social_tasks(&pawns, &mut available_slots);

    if available_slots.is_empty() {
        for (entity, pos, category) in pending {
            task_scheduler.pending_requests.push((entity, pos, category));
        }
        return;
    }

    let pawn_skills: HashMap<Entity, (Position, Skills)> = pawns
        .iter()
        .map(|(e, pos, skills)| (e, (*pos, skills.clone())))
        .collect();

    let mut assignment_scores: Vec<(Entity, usize, f32)> = Vec::new();

    for (entity, pawn_pos, request_category) in &pending {
        if let Some((pos, skills)) = pawn_skills.get(entity) {
            for (slot_idx, slot) in available_slots.iter().enumerate() {
                if category_matches(&slot.category, request_category) {
                    let score = calculate_match_score(pos, skills, slot);
                    assignment_scores.push((*entity, slot_idx, score));
                }
            }
        }
    }

    assignment_scores.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    let mut assigned_pawns: HashSet<Entity> = HashSet::new();
    let mut assigned_slots: HashSet<usize> = HashSet::new();

    for (entity, slot_idx, _score) in &assignment_scores {
        if assigned_pawns.contains(entity) || assigned_slots.contains(slot_idx) {
            continue;
        }

        let slot = &available_slots[*slot_idx];

        if let TaskCategory::GatherResource(resource_type) = slot.category {
            if tile_locks.try_lock(slot.position.x, slot.position.y, *entity, resource_type) {
                assigned_pawns.insert(*entity);
                assigned_slots.insert(*slot_idx);

                task_scheduler
                    .assignments
                    .insert(*entity, (slot.position, slot.category.clone()));

                if let Some(ref channels) = network_channels {
                    let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                        entity_id: entity.index(),
                        task: crate::components::Task::MoveTo(slot.position),
                    });
                }
            }
        } else {
            assigned_pawns.insert(*entity);
            assigned_slots.insert(*slot_idx);

            task_scheduler
                .assignments
                .insert(*entity, (slot.position, slot.category.clone()));

            if let Some(ref channels) = network_channels {
                let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                    entity_id: entity.index(),
                    task: crate::components::Task::MoveTo(slot.position),
                });
            }
        }
    }

    for (entity, pos, category) in &pending {
        if !assigned_pawns.contains(entity) {
            task_scheduler.pending_requests.push((*entity, *pos, category.clone()));
        }
    }
}

fn category_matches(slot_category: &TaskCategory, request_category: &TaskCategory) -> bool {
    match (slot_category, request_category) {
        (TaskCategory::GatherResource(a), TaskCategory::GatherResource(b)) => a == b,
        (TaskCategory::BuildStructure, TaskCategory::BuildStructure) => true,
        (TaskCategory::Entertainment, TaskCategory::Entertainment) => true,
        (TaskCategory::Socialize, TaskCategory::Socialize) => true,
        _ => false,
    }
}

fn collect_gather_tasks(
    tile_map: &TileMap,
    tile_locks: &TileLocks,
    slots: &mut Vec<TaskSlot>,
) {
    for ((x, y), tile) in &tile_map.tiles {
        if tile.resource_type != ResourceType::None && tile.resource_amount > 0 {
            if !tile_locks.is_locked(*x, *y) {
                slots.push(TaskSlot::gather(
                    Position::new(*x, *y),
                    tile.resource_type,
                ));
            }
        }
    }
}

fn collect_build_tasks(
    buildings: &Query<(Entity, &Position, &Building, Option<&EntertainmentFacility>)>,
    slots: &mut Vec<TaskSlot>,
) {
    for (_entity, pos, building, _) in buildings.iter() {
        if !building.is_complete {
            slots.push(TaskSlot::build(*pos, building.building_type.clone()));
        }
    }
}

fn collect_entertainment_tasks(
    buildings: &Query<(Entity, &Position, &Building, Option<&EntertainmentFacility>)>,
    slots: &mut Vec<TaskSlot>,
) {
    for (_entity, pos, building, facility_opt) in buildings.iter() {
        if building.is_complete {
            if facility_opt.is_some() {
                slots.push(TaskSlot::entertain(*pos));
            }
        }
    }
}

fn collect_social_tasks(
    pawns: &Query<(Entity, &Position, &Skills)>,
    slots: &mut Vec<TaskSlot>,
) {
    for (_entity, pos, _skills) in pawns.iter() {
        slots.push(TaskSlot::socialize(*pos));
    }
}

pub fn cleanup_stale_locks_system(
    mut commands: Commands,
    mut tile_locks: ResMut<TileLocks>,
    mut task_scheduler: ResMut<TaskScheduler>,
    tile_map: Res<TileMap>,
    query: Query<(Entity, &Position, &crate::components::Task), With<crate::components::Pawn>>,
    network_channels: Option<Res<NetworkChannels>>,
) {
    let active_entities: HashSet<Entity> = query.iter().map(|(e, _, _)| e).collect();

    tile_locks.locks.retain(|pos, (entity, resource_type)| {
        if !active_entities.contains(entity) {
            task_scheduler.assignments.remove(entity);
            return false;
        }

        let tile = tile_map.get_tile(pos.0, pos.1);
        match tile {
            Some(t) if t.resource_type == *resource_type && t.resource_amount > 0 => true,
            _ => {
                task_scheduler.assignments.remove(entity);
                false
            }
        }
    });
}

pub fn release_lock_for_entity(
    tile_locks: &mut TileLocks,
    task_scheduler: &mut TaskScheduler,
    entity: Entity,
) {
    tile_locks.unlock_entity(entity);
    task_scheduler.complete_task(entity);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::Skills;

    #[test]
    fn test_calculate_match_score() {
        let pawn_pos = Position::new(0, 0);
        let skills = Skills::new(5.0, 2.0, 3.0);

        let slot = TaskSlot {
            position: Position::new(10, 0),
            category: TaskCategory::GatherResource(ResourceType::Iron),
            skill_required: SkillType::Mining,
            priority: 1.0,
        };

        let score = calculate_match_score(&pawn_pos, &skills, &slot);
        assert!(score > 0.0);
    }

    #[test]
    fn test_skill_affects_score() {
        let pawn_pos = Position::new(0, 0);

        let low_skill = Skills::new(1.0, 1.0, 1.0);
        let high_skill = Skills::new(10.0, 1.0, 1.0);

        let slot = TaskSlot {
            position: Position::new(5, 0),
            category: TaskCategory::GatherResource(ResourceType::Iron),
            skill_required: SkillType::Mining,
            priority: 1.0,
        };

        let low_score = calculate_match_score(&pawn_pos, &low_skill, &slot);
        let high_score = calculate_match_score(&pawn_pos, &high_skill, &slot);

        assert!(high_score > low_score);
    }

    #[test]
    fn test_distance_affects_score() {
        let skills = Skills::new(5.0, 5.0, 5.0);

        let near_pos = Position::new(1, 0);
        let far_pos = Position::new(10, 0);

        let slot = TaskSlot {
            position: Position::new(0, 0),
            category: TaskCategory::GatherResource(ResourceType::Food),
            skill_required: SkillType::Farming,
            priority: 1.0,
        };

        let near_score = calculate_match_score(&near_pos, &skills, &slot);
        let far_score = calculate_match_score(&far_pos, &skills, &slot);

        assert!(near_score > far_score);
    }
}

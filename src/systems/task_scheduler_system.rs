use bevy_ecs::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::components::{Position, ResourceType};
use crate::resources::{NetworkChannels, NetworkEvent, TileLocks, TileMap, TaskScheduler};

pub fn task_scheduler_system(
    mut commands: Commands,
    tile_map: Res<TileMap>,
    mut tile_locks: ResMut<TileLocks>,
    mut task_scheduler: ResMut<TaskScheduler>,
    network_channels: Option<Res<NetworkChannels>>,
) {
    let pending: Vec<(Entity, Position, ResourceType)> =
        task_scheduler.pending_requests.drain(..).collect();

    let mut grouped_by_resource: HashMap<ResourceType, Vec<(Entity, Position)>> = HashMap::new();
    for (entity, pos, resource_type) in &pending {
        grouped_by_resource
            .entry(*resource_type)
            .or_insert_with(Vec::new)
            .push((*entity, *pos));
    }

    let mut assigned_positions: HashSet<(i32, i32)> = HashSet::new();

    for (resource_type, requests) in grouped_by_resource.iter() {
        let available_resources = collect_available_resources(&tile_map, &tile_locks, *resource_type);

        if available_resources.is_empty() {
            for (entity, _) in requests {
                task_scheduler.pending_requests.push((*entity, Position::new(0, 0), *resource_type));
            }
            continue;
        }

        let mut resource_list: Vec<(Position, u32, i32)> = available_resources
            .iter()
            .map(|(pos, amount)| (*pos, *amount, 0))
            .collect();

        for (entity, pawn_pos) in requests {
            if resource_list.is_empty() {
                task_scheduler.pending_requests.push((*entity, *pawn_pos, *resource_type));
                continue;
            }

            let mut best_idx = 0;
            let mut best_dist = i32::MAX;

            for (idx, (res_pos, _, _)) in resource_list.iter().enumerate() {
                let dist = pawn_pos.manhattan_to(res_pos);
                if dist < best_dist {
                    best_dist = dist;
                    best_idx = idx;
                }
            }

            let (target_pos, _, _) = resource_list.remove(best_idx);
            assigned_positions.insert((target_pos.x, target_pos.y));

            if tile_locks.try_lock(target_pos.x, target_pos.y, *entity, *resource_type) {
                task_scheduler
                    .assignments
                    .insert(*entity, (target_pos, *resource_type));

                if let Some(ref channels) = network_channels {
                    let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                        entity_id: entity.index(),
                        task: crate::components::Task::MoveTo(target_pos),
                    });
                }
            } else {
                task_scheduler.pending_requests.push((*entity, *pawn_pos, *resource_type));
            }
        }
    }
}

fn collect_available_resources(
    tile_map: &TileMap,
    tile_locks: &TileLocks,
    resource_type: ResourceType,
) -> Vec<(Position, u32)> {
    let mut resources = Vec::new();

    for ((x, y), tile) in &tile_map.tiles {
        if tile.resource_type == resource_type && tile.resource_amount > 0 {
            if !tile_locks.is_locked(*x, *y) {
                resources.push((Position::new(*x, *y), tile.resource_amount));
            }
        }
    }

    resources
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

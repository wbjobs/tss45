use bevy_ecs::prelude::*;

use crate::components::{
    AssignedTask, Hunger, Inventory, Pawn, Path, Position, ResourceType, Stamina, Task,
};
use crate::pathfinding::find_path_with_locks;
use crate::resources::{NetworkChannels, NetworkEvent, TileLocks, TileMap, TaskScheduler};
use crate::systems::task_scheduler_system::release_lock_for_entity;

pub fn movement_system(
    mut commands: Commands,
    mut tile_map: ResMut<TileMap>,
    mut tile_locks: ResMut<TileLocks>,
    mut task_scheduler: ResMut<TaskScheduler>,
    mut query: Query<(
        Entity,
        &mut Position,
        &mut Path,
        &mut Task,
        &mut Stamina,
        &mut Hunger,
        &mut Inventory,
        Option<&AssignedTask>,
        With<Pawn>,
    )>,
    network_channels: Option<Res<NetworkChannels>>,
) {
    for (
        entity,
        mut position,
        mut path,
        mut task,
        mut stamina,
        mut hunger,
        mut inventory,
        assigned_task,
        _,
    ) in query.iter_mut()
    {
        match &*task {
            Task::MoveTo(target_pos) => {
                if path.is_complete() {
                    if let Some(current_target) = path.current_target() {
                        *position = *current_target;
                    }

                    let is_at_target = assigned_task
                        .map(|a| a.target_position == *position)
                        .unwrap_or(false);

                    if is_at_target || &*position == target_pos {
                        let resource_type = assigned_task
                            .map(|a| a.resource_type)
                            .unwrap_or_else(|| tile_map.get_resource_type(position.x, position.y));

                        if resource_type != ResourceType::None {
                            *task = Task::Gather(*position, resource_type);

                            if let Some(ref channels) = network_channels {
                                let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                                    entity_id: entity.index(),
                                    task: task.clone(),
                                });
                            }
                        } else {
                            release_lock_for_entity(&mut tile_locks, &mut task_scheduler, entity);
                            commands
                                .entity(entity)
                                .remove::<AssignedTask>()
                                .remove::<Path>();
                            path.clear();
                            *task = Task::Idle;

                            if let Some(ref channels) = network_channels {
                                let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                                    entity_id: entity.index(),
                                    task: task.clone(),
                                });
                            }
                        }
                    }
                    continue;
                }

                if stamina.can_move() {
                    if let Some(target) = path.current_target() {
                        if *position != *target {
                            stamina.consume_move();
                            *position = *target;

                            if let Some(ref channels) = network_channels {
                                let _ = channels.event_sender.send(NetworkEvent::PawnMoved {
                                    entity_id: entity.index(),
                                    position: *position,
                                });
                                let _ = channels.event_sender.send(NetworkEvent::StaminaChanged {
                                    entity_id: entity.index(),
                                    value: stamina.value,
                                    max: stamina.max,
                                });
                            }
                        }
                        path.advance();
                    }
                }
            }

            Task::Gather(gather_pos, resource_type) => {
                if position.manhattan_to(gather_pos) <= 1 {
                    let still_locked = tile_locks.is_locked_by(gather_pos.x, gather_pos.y, entity)
                        || !tile_locks.is_locked(gather_pos.x, gather_pos.y);

                    if !still_locked {
                        release_lock_for_entity(&mut tile_locks, &mut task_scheduler, entity);
                        commands
                            .entity(entity)
                            .remove::<AssignedTask>()
                            .remove::<Path>();
                        path.clear();
                        *task = Task::Idle;

                        if let Some(ref channels) = network_channels {
                            let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                                entity_id: entity.index(),
                                task: task.clone(),
                            });
                        }
                        continue;
                    }

                    let tile_exists = tile_map
                        .get_tile(gather_pos.x, gather_pos.y)
                        .map(|t| t.resource_type == *resource_type && t.resource_amount > 0)
                        .unwrap_or(false);

                    if !tile_exists {
                        release_lock_for_entity(&mut tile_locks, &mut task_scheduler, entity);
                        commands
                            .entity(entity)
                            .remove::<AssignedTask>()
                            .remove::<Path>();
                        path.clear();
                        *task = Task::Idle;

                        if let Some(ref channels) = network_channels {
                            let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                                entity_id: entity.index(),
                                task: task.clone(),
                            });
                        }
                        continue;
                    }

                    if stamina.can_gather() {
                        stamina.consume_gather();

                        let gathered = tile_map.gather_resource(gather_pos.x, gather_pos.y, 1);

                        if gathered > 0 {
                            inventory.add(*resource_type, gathered);

                            if let Some(ref channels) = network_channels {
                                let _ = channels.event_sender.send(NetworkEvent::InventoryChanged {
                                    entity_id: entity.index(),
                                    inventory: inventory.clone(),
                                });

                                if let Some(tile) = tile_map.get_tile(gather_pos.x, gather_pos.y) {
                                    let _ = channels.event_sender.send(NetworkEvent::ResourceChanged {
                                        position: *gather_pos,
                                        resource_type: tile.resource_type,
                                        amount: tile.resource_amount,
                                    });
                                }

                                let _ = channels.event_sender.send(NetworkEvent::StaminaChanged {
                                    entity_id: entity.index(),
                                    value: stamina.value,
                                    max: stamina.max,
                                });
                            }

                            if *resource_type == ResourceType::Food {
                                let food_eaten = inventory.take(ResourceType::Food, 1);
                                if food_eaten > 0 {
                                    hunger.eat(30.0);

                                    if let Some(ref channels) = network_channels {
                                        let _ =
                                            channels.event_sender.send(NetworkEvent::HungerChanged {
                                                entity_id: entity.index(),
                                                value: hunger.value,
                                                max: hunger.max,
                                            });
                                        let _ = channels.event_sender.send(
                                            NetworkEvent::InventoryChanged {
                                                entity_id: entity.index(),
                                                inventory: inventory.clone(),
                                            },
                                        );
                                    }
                                }
                            }

                            let tile_depleted = tile_map
                                .get_tile(gather_pos.x, gather_pos.y)
                                .map(|t| t.resource_amount == 0)
                                .unwrap_or(true);

                            if tile_depleted {
                                release_lock_for_entity(
                                    &mut tile_locks,
                                    &mut task_scheduler,
                                    entity,
                                );
                                commands
                                    .entity(entity)
                                    .remove::<AssignedTask>()
                                    .remove::<Path>();
                                path.clear();
                                *task = Task::Idle;

                                if let Some(ref channels) = network_channels {
                                    let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                                        entity_id: entity.index(),
                                        task: task.clone(),
                                    });
                                }
                            }
                        } else {
                            release_lock_for_entity(&mut tile_locks, &mut task_scheduler, entity);
                            commands
                                .entity(entity)
                                .remove::<AssignedTask>()
                                .remove::<Path>();
                            path.clear();
                            *task = Task::Idle;

                            if let Some(ref channels) = network_channels {
                                let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                                    entity_id: entity.index(),
                                    task: task.clone(),
                                });
                            }
                        }
                    }
                } else {
                    *task = Task::MoveTo(*gather_pos);
                    commands.entity(entity).insert(crate::components::NeedsPathfinding);

                    if let Some(ref channels) = network_channels {
                        let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                            entity_id: entity.index(),
                            task: task.clone(),
                        });
                    }
                }
            }

            _ => {}
        }
    }
}

pub fn pathfinding_executor_system(
    mut commands: Commands,
    tile_map: Res<TileMap>,
    tile_locks: Res<TileLocks>,
    mut query: Query<(
        Entity,
        &Position,
        &Task,
        Option<&AssignedTask>,
        With<crate::components::NeedsPathfinding>,
    )>,
    network_channels: Option<Res<NetworkChannels>>,
) {
    for (entity, position, task, assigned_task, _) in query.iter_mut() {
        let target = if let Some(assigned) = assigned_task {
            Some(assigned.target_position)
        } else {
            match &*task {
                Task::MoveTo(target_pos) => Some(*target_pos),
                Task::Gather(target_pos, _) => Some(*target_pos),
                _ => None,
            }
        };

        if let Some(target_pos) = target {
            if let Some(path_waypoints) =
                find_path_with_locks(&tile_map, position, &target_pos, Some(&tile_locks))
            {
                let new_path = Path::new(path_waypoints);

                commands
                    .entity(entity)
                    .remove::<crate::components::NeedsPathfinding>()
                    .insert(new_path);
            } else {
                commands.entity(entity).remove::<crate::components::NeedsPathfinding>();

                if let Some(ref channels) = network_channels {
                    let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                        entity_id: entity.index(),
                        task: Task::Idle,
                    });
                }
            }
        } else {
            commands.entity(entity).remove::<crate::components::NeedsPathfinding>();
        }
    }
}

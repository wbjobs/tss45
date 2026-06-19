use bevy_ecs::prelude::*;

use crate::components::{Hunger, Inventory, Pawn, Path, Position, ResourceType, Stamina, Task};
use crate::resources::{NetworkChannels, NetworkEvent, TileMap};

pub fn movement_system(
    mut commands: Commands,
    mut tile_map: ResMut<TileMap>,
    mut query: Query<(
        Entity,
        &mut Position,
        &mut Path,
        &mut Task,
        &mut Stamina,
        &mut Hunger,
        &mut Inventory,
        With<Pawn>,
    )>,
    network_channels: Option<Res<NetworkChannels>>,
) {
    for (entity, mut position, mut path, mut task, mut stamina, mut hunger, mut inventory, _) in
        query.iter_mut()
    {
        match &*task {
            Task::MoveTo(_) => {
                if path.is_complete() {
                    if let Some(current_target) = path.current_target() {
                        *position = *current_target;
                    }

                    if let ResourceType::Food = tile_map.get_resource_type(position.x, position.y)
                    {
                        *task = Task::Gather(*position, ResourceType::Food);

                        if let Some(ref channels) = network_channels {
                            let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                                entity_id: entity.index(),
                                task: task.clone(),
                            });
                        }
                    } else {
                        let resource_type = tile_map.get_resource_type(position.x, position.y);
                        if resource_type != ResourceType::None {
                            *task = Task::Gather(*position, resource_type);

                            if let Some(ref channels) = network_channels {
                                let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                                    entity_id: entity.index(),
                                    task: task.clone(),
                                });
                            }
                        } else {
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

                                let tile = tile_map.get_tile(gather_pos.x, gather_pos.y).unwrap();
                                let _ = channels.event_sender.send(NetworkEvent::ResourceChanged {
                                    position: *gather_pos,
                                    resource_type: tile.resource_type,
                                    amount: tile.resource_amount,
                                });

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
                                        let _ = channels
                                            .event_sender
                                            .send(NetworkEvent::HungerChanged {
                                                entity_id: entity.index(),
                                                value: hunger.value,
                                                max: hunger.max,
                                            });
                                        let _ = channels
                                            .event_sender
                                            .send(NetworkEvent::InventoryChanged {
                                                entity_id: entity.index(),
                                                inventory: inventory.clone(),
                                            });
                                    }
                                }
                            }

                            if tile_map.get_resource_type(gather_pos.x, gather_pos.y)
                                == ResourceType::None
                            {
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

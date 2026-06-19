use bevy_ecs::prelude::*;

use crate::components::{NeedsPathfinding, Path, Position, ResourceType, Task};
use crate::pathfinding::find_path;
use crate::resources::{NetworkChannels, NetworkEvent, TileMap};

pub fn pathfinding_system(
    mut commands: Commands,
    tile_map: Res<TileMap>,
    mut query: Query<(
        Entity,
        &Position,
        &Task,
        Option<&Path>,
        With<NeedsPathfinding>,
    )>,
    network_channels: Option<Res<NetworkChannels>>,
) {
    for (entity, position, task, existing_path, _) in query.iter_mut() {
        let target = match &*task {
            Task::FindResource(resource_type) => {
                tile_map.find_nearest_resource(position, *resource_type)
            }
            Task::MoveTo(target_pos) => Some(*target_pos),
            Task::Gather(target_pos, _) => Some(*target_pos),
            _ => None,
        };

        if let Some(target_pos) = target {
            if let Some(path_waypoints) = find_path(&tile_map, position, &target_pos) {
                let new_path = Path::new(path_waypoints);

                if let Some(ref channels) = network_channels {
                    let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                        entity_id: entity.index(),
                        task: Task::MoveTo(target_pos),
                    });
                }

                commands
                    .entity(entity)
                    .remove::<NeedsPathfinding>()
                    .insert(new_path)
                    .insert(Task::MoveTo(target_pos));
            } else {
                commands.entity(entity).remove::<NeedsPathfinding>();

                if let Some(ref channels) = network_channels {
                    let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                        entity_id: entity.index(),
                        task: Task::Idle,
                    });
                }

                commands.entity(entity).insert(Task::Idle);
            }
        } else {
            commands.entity(entity).remove::<NeedsPathfinding>();

            if let Some(ref channels) = network_channels {
                let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                    entity_id: entity.index(),
                    task: Task::Idle,
                });
            }

            commands.entity(entity).insert(Task::Idle);
        }
    }
}

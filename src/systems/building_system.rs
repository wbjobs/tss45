use bevy_ecs::prelude::*;
use bevy_time::Time;

use crate::components::{
    AssignedTask, Building, BuildingType, Pawn, Path, Position, Skills, Stamina, Task, TaskCategory,
};
use crate::resources::{NetworkChannels, NetworkEvent, TileLocks, TaskScheduler};
use crate::systems::task_scheduler_system::release_lock_for_entity;

pub fn building_system(
    mut commands: Commands,
    time: Res<Time>,
    mut pawns: Query<
        (
            Entity,
            &Position,
            &mut Task,
            &mut Stamina,
            &Skills,
            &mut Path,
            Option<&AssignedTask>,
        ),
        With<Pawn>,
    >,
    mut buildings: Query<(Entity, &Position, &mut Building)>,
    mut tile_locks: ResMut<TileLocks>,
    mut task_scheduler: ResMut<TaskScheduler>,
    network_channels: Option<Res<NetworkChannels>>,
) {
    let delta = time.delta_seconds();

    for (
        pawn_entity,
        pawn_pos,
        mut pawn_task,
        mut stamina,
        skills,
        mut path,
        assigned_task,
    ) in pawns.iter_mut()
    {
        let target_building_pos = match &*pawn_task {
            Task::Build(pos) => Some(*pos),
            _ => None,
        };

        if target_building_pos.is_none() {
            continue;
        }

        let target_pos = target_building_pos.unwrap();

        let at_target = pawn_pos.manhattan_to(&target_pos) <= 1;
        if !at_target {
            continue;
        }

        let building_entity = find_building_at(&buildings, &target_pos);
        if building_entity.is_none() {
            release_lock_for_entity(&mut tile_locks, &mut task_scheduler, pawn_entity);
            commands
                .entity(pawn_entity)
                .remove::<AssignedTask>()
                .remove::<Path>();
            path.clear();
            *pawn_task = Task::Idle;
            continue;
        }

        let building_ent = building_entity.unwrap();

        if let Ok((_, _, mut building)) = buildings.get_mut(building_ent) {
            if building.is_complete {
                release_lock_for_entity(&mut tile_locks, &mut task_scheduler, pawn_entity);
                commands
                    .entity(pawn_entity)
                    .remove::<AssignedTask>()
                    .remove::<Path>();
                path.clear();
                *pawn_task = Task::Idle;

                if let Some(ref channels) = network_channels {
                    let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                        entity_id: pawn_entity.index(),
                        task: pawn_task.clone(),
                    });
                }
                continue;
            }

            if stamina.can_gather() {
                stamina.consume_gather();

                let build_amount = 5.0 * delta;
                let was_complete = building.build(build_amount, skills.building);

                if let Some(ref channels) = network_channels {
                    let _ = channels.event_sender.send(NetworkEvent::StaminaChanged {
                        entity_id: pawn_entity.index(),
                        value: stamina.value,
                        max: stamina.max,
                    });
                }

                if was_complete {
                    release_lock_for_entity(&mut tile_locks, &mut task_scheduler, pawn_entity);
                    commands
                        .entity(pawn_entity)
                        .remove::<AssignedTask>()
                        .remove::<Path>();
                    path.clear();
                    *pawn_task = Task::Idle;

                    if let Some(ref channels) = network_channels {
                        let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                            entity_id: pawn_entity.index(),
                            task: pawn_task.clone(),
                        });
                    }

                    if let Some(ref channels) = network_channels {
                        let _ = channels.event_sender.send(NetworkEvent::BuildingCompleted {
                            position: target_pos,
                            building_type: building.building_type.clone(),
                        });
                    }
                }
            }
        }
    }
}

fn find_building_at(
    buildings: &Query<(Entity, &Position, &mut Building)>,
    pos: &Position,
) -> Option<Entity> {
    for (entity, b_pos, _) in buildings.iter() {
        if b_pos == pos {
            return Some(entity);
        }
    }
    None
}

pub fn spawn_building(
    commands: &mut Commands,
    position: Position,
    building_type: BuildingType,
) -> Entity {
    let building = Building::new(building_type.clone());
    let entity = commands.spawn_empty().id();

    commands
        .entity(entity)
        .insert(building)
        .insert(position);

    if building_type == BuildingType::EntertainmentPark {
        commands
            .entity(entity)
            .insert(crate::components::EntertainmentFacility::new(5, 2.0, 10));
    }

    entity
}

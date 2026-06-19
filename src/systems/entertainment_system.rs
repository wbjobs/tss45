use bevy_ecs::prelude::*;
use bevy_time::Time;

use crate::components::{
    AssignedTask, Building, EntertainmentFacility, Happiness, Pawn, Path, Position,
    Skills, Stamina, Task, TaskCategory,
};
use crate::resources::{NetworkChannels, NetworkEvent, TileLocks, TaskScheduler};
use crate::systems::task_scheduler_system::release_lock_for_entity;

pub fn entertainment_system(
    mut commands: Commands,
    time: Res<Time>,
    mut pawns: Query<
        (
            Entity,
            &Position,
            &mut Task,
            &mut Happiness,
            &mut Stamina,
            &mut Path,
            Option<&AssignedTask>,
        ),
        With<Pawn>,
    >,
    facilities: Query<(Entity, &Position, &Building, &EntertainmentFacility)>,
    mut tile_locks: ResMut<TileLocks>,
    mut task_scheduler: ResMut<TaskScheduler>,
    network_channels: Option<Res<NetworkChannels>>,
) {
    let delta = time.delta_seconds();

    for (
        pawn_entity,
        pawn_pos,
        mut pawn_task,
        mut happiness,
        mut stamina,
        mut path,
        assigned_task,
    ) in pawns.iter_mut()
    {
        let target_facility_pos = match &*pawn_task {
            Task::Entertain(pos) => Some(*pos),
            _ => None,
        };

        if target_facility_pos.is_none() {
            continue;
        }

        let target_pos = target_facility_pos.unwrap();

        let at_target = pawn_pos.manhattan_to(&target_pos) <= 2;
        if !at_target {
            continue;
        }

        let facility_entity = find_facility_at(&facilities, &target_pos);
        if facility_entity.is_none() {
            release_lock_for_entity(&mut tile_locks, &mut task_scheduler, pawn_entity);
            commands
                .entity(pawn_entity)
                .remove::<AssignedTask>()
                .remove::<crate::components::NeedEntertainment>()
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

        let happiness_gain = 5.0 * delta;
        let old_happiness = happiness.value;
        happiness.gain_entertainment(happiness_gain);

        if (happiness.value - old_happiness).abs() > 0.01 {
            if let Some(ref channels) = network_channels {
                let _ = channels.event_sender.send(NetworkEvent::HappinessChanged {
                    entity_id: pawn_entity.index(),
                    value: happiness.value,
                    max: happiness.max,
                });
            }
        }

        if happiness.value >= happiness.max * 0.8 {
            release_lock_for_entity(&mut tile_locks, &mut task_scheduler, pawn_entity);
            commands
                .entity(pawn_entity)
                .remove::<AssignedTask>()
                .remove::<crate::components::NeedEntertainment>()
                .remove::<Path>();
            path.clear();
            *pawn_task = Task::Idle;

            if let Some(ref channels) = network_channels {
                let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                    entity_id: pawn_entity.index(),
                    task: pawn_task.clone(),
                });
            }
        }
    }
}

fn find_facility_at(
    facilities: &Query<(Entity, &Position, &Building, &EntertainmentFacility)>,
    pos: &Position,
) -> Option<Entity> {
    for (entity, f_pos, building, _) in facilities.iter() {
        if f_pos == pos && building.is_complete {
            return Some(entity);
        }
    }
    None
}

pub fn entertainment_passive_benefit_system(
    time: Res<Time>,
    facilities: Query<(&Position, &Building, &EntertainmentFacility)>,
    mut pawns: Query<(&Position, &mut Happiness), With<Pawn>>,
) {
    let delta = time.delta_seconds();
    let benefit_rate = 0.3;

    let facility_positions: Vec<(Position, i32, f32)> = facilities
        .iter()
        .filter(|(_, building, _)| building.is_complete)
        .map(|(pos, _, facility)| (*pos, facility.radius, facility.happiness_per_second))
        .collect();

    if facility_positions.is_empty() {
        return;
    }

    for (pawn_pos, mut happiness) in pawns.iter_mut() {
        let mut total_benefit = 0.0;

        for (facility_pos, radius, hap_per_sec) in &facility_positions {
            let dist = pawn_pos.manhattan_to(facility_pos);
            if dist <= *radius {
                let falloff = 1.0 - (dist as f32 / *radius as f32) * 0.5;
                total_benefit += hap_per_sec * falloff;
            }
        }

        if total_benefit > 0.0 {
            happiness.gain_entertainment(total_benefit * delta * benefit_rate);
        }
    }
}

pub fn social_system(
    mut commands: Commands,
    time: Res<Time>,
    mut pawns: Query<
        (
            Entity,
            &Position,
            &mut Task,
            &mut Happiness,
            &mut Stamina,
            &mut Path,
            Option<&AssignedTask>,
        ),
        With<Pawn>,
    >,
    mut tile_locks: ResMut<TileLocks>,
    mut task_scheduler: ResMut<TaskScheduler>,
    network_channels: Option<Res<NetworkChannels>>,
) {
    let delta = time.delta_seconds();

    let pawn_positions: Vec<(Entity, Position)> = pawns
        .iter()
        .map(|(e, pos, _, _, _, _, _)| (e, *pos))
        .collect();

    for (
        pawn_entity,
        pawn_pos,
        mut pawn_task,
        mut happiness,
        mut _stamina,
        mut path,
        assigned_task,
    ) in pawns.iter_mut()
    {
        let target_social_pos = match &*pawn_task {
            Task::Socialize(pos) => Some(*pos),
            _ => None,
        };

        if target_social_pos.is_none() {
            continue;
        }

        let target_pos = target_social_pos.unwrap();
        let at_target = pawn_pos.manhattan_to(&target_pos) <= 2;

        if !at_target {
            continue;
        }

        let has_partner = pawn_positions.iter().any(|(other_e, other_pos)| {
            *other_e != pawn_entity && other_pos.manhattan_to(pawn_pos) <= 2
        });

        if has_partner {
            let happiness_gain = 3.0 * delta;
            let old_happiness = happiness.value;
            happiness.gain_social(happiness_gain);

            if (happiness.value - old_happiness).abs() > 0.01 {
                if let Some(ref channels) = network_channels {
                    let _ = channels.event_sender.send(NetworkEvent::HappinessChanged {
                        entity_id: pawn_entity.index(),
                        value: happiness.value,
                        max: happiness.max,
                    });
                }
            }

            if happiness.value >= happiness.max * 0.7 {
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
            }
        }
    }
}

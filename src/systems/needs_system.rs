use bevy_ecs::prelude::*;
use bevy_time::Time;

use crate::components::{Hunger, Pawn, Position, RequestTask, ResourceType, Stamina, Task};
use crate::resources::{NetworkChannels, NetworkEvent, TaskScheduler};

pub fn needs_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &Position, &mut Hunger, &mut Stamina, &Task, Option<&RequestTask>), With<Pawn>>,
    mut task_scheduler: ResMut<TaskScheduler>,
    network_channels: Option<Res<NetworkChannels>>,
) {
    let delta = time.delta_seconds();

    for (entity, position, mut hunger, mut stamina, task, has_request) in query.iter_mut() {
        let hunger_was = hunger.value;
        let stamina_was = stamina.value;

        hunger.decay(delta);
        stamina.regenerate(delta);

        if let Some(ref channels) = network_channels {
            if (hunger.value - hunger_was).abs() > 0.01 {
                let _ = channels.event_sender.send(NetworkEvent::HungerChanged {
                    entity_id: entity.index(),
                    value: hunger.value,
                    max: hunger.max,
                });
            }
            if (stamina.value - stamina_was).abs() > 0.01 {
                let _ = channels.event_sender.send(NetworkEvent::StaminaChanged {
                    entity_id: entity.index(),
                    value: stamina.value,
                    max: stamina.max,
                });
            }
        }

        let needs_task = match &*task {
            Task::Idle | Task::FindFood => true,
            Task::FindResource(_) => true,
            _ => false,
        };

        if needs_task && has_request.is_none() && !task_scheduler.has_assignment(entity) {
            if hunger.is_hungry() || hunger.is_starving() {
                commands.entity(entity).insert(RequestTask::new(ResourceType::Food));
                task_scheduler.request_task(entity, *position, ResourceType::Food);

                if let Some(ref channels) = network_channels {
                    let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                        entity_id: entity.index(),
                        task: Task::FindFood,
                    });
                }
            }
        }
    }
}

pub fn task_assignment_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Task, &Hunger, &RequestTask), With<Pawn>>,
    task_scheduler: Res<TaskScheduler>,
) {
    for (entity, mut task, hunger, request) in query.iter_mut() {
        if let Some((target_pos, resource_type)) = task_scheduler.get_assignment(entity) {
            commands
                .entity(entity)
                .remove::<RequestTask>()
                .insert(crate::components::NeedsPathfinding)
                .insert(crate::components::AssignedTask::new(target_pos, resource_type));

            *task = Task::MoveTo(target_pos);
        } else if hunger.is_hungry() {
            if request.resource_type != ResourceType::Food {
                commands.entity(entity).insert(RequestTask::new(ResourceType::Food));
            }
        } else {
            if !task_scheduler.has_assignment(entity) {
                commands.entity(entity).remove::<RequestTask>();
                *task = Task::Idle;
            }
        }
    }
}

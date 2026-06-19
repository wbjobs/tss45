use bevy_ecs::prelude::*;
use bevy_time::Time;

use crate::components::{Hunger, Pawn, ResourceType, Stamina, Task};
use crate::resources::{NetworkChannels, NetworkEvent};

pub fn needs_system(
    time: Res<Time>,
    mut query: Query<(Entity, &mut Hunger, &mut Stamina, &mut Task), With<Pawn>>,
    network_channels: Option<Res<NetworkChannels>>,
) {
    let delta = time.delta_seconds();

    for (entity, mut hunger, mut stamina, mut task) in query.iter_mut() {
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

        if *task == Task::Idle {
            if hunger.is_hungry() {
                *task = Task::FindFood;

                if let Some(ref channels) = network_channels {
                    let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                        entity_id: entity.index(),
                        task: task.clone(),
                    });
                }
            }
        }

        if hunger.is_starving() {
            *task = Task::FindFood;

            if let Some(ref channels) = network_channels {
                let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                    entity_id: entity.index(),
                    task: task.clone(),
                });
            }
        }
    }
}

pub fn task_assignment_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Task, &Hunger), (With<Pawn>, Without<crate::components::NeedsPathfinding>)>,
) {
    for (entity, mut task, hunger) in query.iter_mut() {
        match &*task {
            Task::FindFood => {
                if hunger.is_hungry() {
                    *task = Task::FindResource(ResourceType::Food);
                    commands.entity(entity).insert(crate::components::NeedsPathfinding);
                } else {
                    *task = Task::Idle;
                }
            }
            Task::FindResource(_) => {
                commands.entity(entity).insert(crate::components::NeedsPathfinding);
            }
            _ => {}
        }
    }
}

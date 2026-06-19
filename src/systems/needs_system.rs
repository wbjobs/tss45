use bevy_ecs::prelude::*;
use bevy_time::Time;

use crate::components::{
    Happiness, Hunger, Pawn, Position, RequestTask, ResourceType, SkillType, Skills, Stamina,
    Task, TaskCategory,
};
use crate::resources::{NetworkChannels, NetworkEvent, TaskScheduler};

pub fn needs_system(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &Position,
            &mut Hunger,
            &mut Stamina,
            &mut Happiness,
            &Skills,
            &Task,
            Option<&RequestTask>,
        ),
        With<Pawn>,
    >,
    mut task_scheduler: ResMut<TaskScheduler>,
    network_channels: Option<Res<NetworkChannels>>,
) {
    let delta = time.delta_seconds();

    for (entity, position, mut hunger, mut stamina, mut happiness, skills, task, has_request) in
        query.iter_mut()
    {
        let hunger_was = hunger.value;
        let stamina_was = stamina.value;
        let happiness_was = happiness.value;

        hunger.decay(delta);
        stamina.regenerate(delta);
        happiness.decay(delta);

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
            if (happiness.value - happiness_was).abs() > 0.01 {
                let _ = channels.event_sender.send(NetworkEvent::HappinessChanged {
                    entity_id: entity.index(),
                    value: happiness.value,
                    max: happiness.max,
                });
            }
        }

        let needs_task = match &*task {
            Task::Idle | Task::FindFood => true,
            _ => false,
        };

        if needs_task && has_request.is_none() && !task_scheduler.has_assignment(entity) {
            if happiness.refuses_to_work() {
                commands
                    .entity(entity)
                    .insert(RequestTask::entertain())
                    .insert(crate::components::NeedEntertainment);
                task_scheduler.request_task(entity, *position, TaskCategory::Entertainment);

                if let Some(ref channels) = network_channels {
                    let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                        entity_id: entity.index(),
                        task: Task::Entertain(Position::new(0, 0)),
                    });
                }
            } else if hunger.is_hungry() || hunger.is_starving() {
                commands
                    .entity(entity)
                    .insert(RequestTask::gather(ResourceType::Food));
                task_scheduler
                    .request_task(entity, *position, TaskCategory::GatherResource(ResourceType::Food));

                if let Some(ref channels) = network_channels {
                    let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                        entity_id: entity.index(),
                        task: Task::FindFood,
                    });
                }
            } else if happiness.is_unhappy() {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let use_social: bool = rng.gen_bool(0.4);

                if use_social {
                    commands
                        .entity(entity)
                        .insert(RequestTask::socialize());
                    task_scheduler.request_task(entity, *position, TaskCategory::Socialize);

                    if let Some(ref channels) = network_channels {
                        let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                            entity_id: entity.index(),
                            task: Task::Socialize(Position::new(0, 0)),
                        });
                    }
                } else {
                    commands
                        .entity(entity)
                        .insert(RequestTask::entertain())
                        .insert(crate::components::NeedEntertainment);
                    task_scheduler.request_task(entity, *position, TaskCategory::Entertainment);

                    if let Some(ref channels) = network_channels {
                        let _ = channels.event_sender.send(NetworkEvent::TaskChanged {
                            entity_id: entity.index(),
                            task: Task::Entertain(Position::new(0, 0)),
                        });
                    }
                }
            }
        }
    }
}

pub fn task_assignment_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Task, &RequestTask), With<Pawn>>,
    task_scheduler: Res<TaskScheduler>,
) {
    for (entity, mut task, request) in query.iter_mut() {
        if let Some((target_pos, category)) = task_scheduler.get_assignment(entity) {
            commands
                .entity(entity)
                .remove::<RequestTask>()
                .remove::<crate::components::NeedEntertainment>()
                .insert(crate::components::NeedsPathfinding)
                .insert(crate::components::AssignedTask::new(target_pos, category.clone()));

            match category {
                TaskCategory::GatherResource(_) => {
                    *task = Task::MoveTo(target_pos);
                }
                TaskCategory::BuildStructure => {
                    *task = Task::Build(target_pos);
                }
                TaskCategory::Entertainment => {
                    *task = Task::Entertain(target_pos);
                }
                TaskCategory::Socialize => {
                    *task = Task::Socialize(target_pos);
                }
                _ => {
                    *task = Task::Idle;
                }
            }
        } else if !task_scheduler.has_assignment(entity) {
            match request.category {
                TaskCategory::Entertainment => {}
                _ => {
                    commands
                        .entity(entity)
                        .remove::<RequestTask>()
                        .remove::<crate::components::NeedEntertainment>();
                    *task = Task::Idle;
                }
            }
        }
    }
}

pub fn skill_gain_system(
    mut query: Query<(&Task, &mut Skills, &Stamina), With<Pawn>>,
    time: Res<Time>,
) {
    let delta = time.delta_seconds();

    for (task, mut skills, stamina) in query.iter_mut() {
        if stamina.value < 1.0 {
            continue;
        }

        let gain_rate = 0.05 * delta;

        match task {
            Task::Gather(_, resource_type) => match resource_type {
                ResourceType::Iron => {
                    skills.level_up(SkillType::Mining, gain_rate);
                }
                ResourceType::Wood => {
                    skills.level_up(SkillType::Mining, gain_rate * 0.6);
                    skills.level_up(SkillType::Farming, gain_rate * 0.2);
                }
                ResourceType::Food => {
                    skills.level_up(SkillType::Farming, gain_rate);
                }
                ResourceType::None => {}
            },
            Task::Build(_) => {
                skills.level_up(SkillType::Building, gain_rate * 1.5);
            }
            _ => {}
        }
    }
}

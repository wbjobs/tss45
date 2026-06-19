use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::components::{AssignedTask, Hunger, Inventory, Path, Position, Stamina, Task};
use crate::resources::{GameTime, SaveLoadState, TileLocks, TileMap, TaskScheduler};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PawnSaveData {
    pub position: Position,
    pub hunger: Hunger,
    pub stamina: Stamina,
    pub task: Task,
    pub path: Option<Path>,
    pub inventory: Inventory,
    pub assigned_task: Option<AssignedTask>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GameSaveData {
    pub tile_map: TileMap,
    pub game_time: GameTime,
    pub pawns: Vec<PawnSaveData>,
}

pub fn save_game_system(world: &mut World) {
    let save_state = world.get_resource::<SaveLoadState>().unwrap();
    let pending_save = save_state.pending_save.clone();

    if let Some(save_path) = pending_save {
        let tile_map = world.get_resource::<TileMap>().unwrap().clone();
        let game_time = world.get_resource::<GameTime>().unwrap().clone();

        let mut pawns = Vec::new();
        let mut query = world.query::<(
            &Position,
            &Hunger,
            &Stamina,
            &Task,
            Option<&Path>,
            &Inventory,
            Option<&AssignedTask>,
        )>();

        for (pos, hunger, stamina, task, path, inventory, assigned_task) in query.iter(world) {
            pawns.push(PawnSaveData {
                position: *pos,
                hunger: hunger.clone(),
                stamina: stamina.clone(),
                task: task.clone(),
                path: path.cloned(),
                inventory: inventory.clone(),
                assigned_task: assigned_task.cloned(),
            });
        }

        let save_data = GameSaveData {
            tile_map,
            game_time,
            pawns,
        };

        match serde_json::to_string_pretty(&save_data) {
            Ok(json) => {
                let path = PathBuf::from(&save_path);
                if let Some(parent) = path.parent() {
                    if !parent.exists() {
                        let _ = fs::create_dir_all(parent);
                    }
                }
                match fs::write(&save_path, json) {
                    Ok(_) => {
                        println!("游戏已保存到: {}", save_path);
                    }
                    Err(e) => {
                        eprintln!("保存游戏失败: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("序列化游戏状态失败: {}", e);
            }
        }

        if let Some(mut save_state) = world.get_resource_mut::<SaveLoadState>() {
            save_state.pending_save = None;
        }
    }
}

pub fn load_game_system(world: &mut World) {
    let save_state = world.get_resource::<SaveLoadState>().unwrap();
    let pending_load = save_state.pending_load.clone();

    if let Some(load_path) = pending_load {
        match fs::read_to_string(&load_path) {
            Ok(json) => match serde_json::from_str::<GameSaveData>(&json) {
                Ok(save_data) => {
                    world.insert_resource(save_data.tile_map);
                    world.insert_resource(save_data.game_time);
                    world.insert_resource(TileLocks::new());
                    world.insert_resource(TaskScheduler::new());

                    let mut to_despawn = Vec::new();
                    let mut query = world.query::<Entity>();
                    for entity in query.iter(world) {
                        to_despawn.push(entity);
                    }
                    for entity in to_despawn {
                        world.despawn(entity);
                    }

                    for pawn_data in save_data.pawns {
                        let mut entity = world.spawn_empty();
                        entity
                            .insert(crate::components::Pawn)
                            .insert(pawn_data.position)
                            .insert(pawn_data.hunger)
                            .insert(pawn_data.stamina)
                            .insert(pawn_data.task)
                            .insert(pawn_data.inventory);

                        if let Some(path) = pawn_data.path {
                            entity.insert(path);
                        }
                        if let Some(assigned) = pawn_data.assigned_task {
                            entity.insert(assigned);
                            entity.insert(crate::components::NeedsPathfinding);
                        }
                    }

                    println!("游戏已从 {} 加载", load_path);
                }
                Err(e) => {
                    eprintln!("反序列化游戏状态失败: {}", e);
                }
            },
            Err(e) => {
                eprintln!("读取存档文件失败: {}", e);
            }
        }

        if let Some(mut save_state) = world.get_resource_mut::<SaveLoadState>() {
            save_state.pending_load = None;
        }
    }
}

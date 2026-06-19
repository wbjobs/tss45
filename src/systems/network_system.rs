use bevy_ecs::prelude::*;

use crate::components::{Inventory, Pawn, Position, Task, Tile};
use crate::resources::{NetworkChannels, NetworkCommand, SaveLoadState, TileMap};

pub fn network_command_system(
    mut commands: Commands,
    mut tile_map: ResMut<TileMap>,
    mut save_load_state: ResMut<SaveLoadState>,
    network_channels: Option<Res<NetworkChannels>>,
    mut pawn_query: Query<(
        &Position, &mut Task),
        With<Pawn>,
    )>,
) {
    if let Some(ref channels) = network_channels {
        while let Ok(command) = channels.command_receiver.try_recv() {
            match command {
                NetworkCommand::Subscribe => {}
                NetworkCommand::Unsubscribe => {}
                NetworkCommand::SaveGame(path) => {
                    save_load_state.pending_save = Some(path);
                }
                NetworkCommand::LoadGame(path) => {
                    save_load_state.pending_load = Some(path);
                }
                NetworkCommand::SpawnPawn(position) => {
                    spawn_pawn(&mut commands, position);
                }
                NetworkCommand::SetTile(position, tile) => {
                    tile_map.set_tile(position.x, position.y, tile);
                }
            }
        }
    }
}

fn spawn_pawn(commands: &mut Commands, position: Position) {
    use crate::components::{Hunger, Inventory, Stamina, Task};

    commands
        .spawn_empty()
        .insert(Pawn)
        .insert(position)
        .insert(Hunger::new(100.0, 2.0))
        .insert(Stamina::new(100.0, 5.0, 5.0, 15.0))
        .insert(Task::Idle)
        .insert(Inventory::default());
}

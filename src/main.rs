use bevy_ecs::prelude::*;
use bevy_time::Time;
use rand::Rng;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Receiver, Sender};

mod components;
mod networking;
mod pathfinding;
mod resources;
mod systems;

use components::{Hunger, Inventory, Pawn, Position, ResourceType, Stamina, Task, Tile};
use resources::{GameTime, NetworkChannels, NetworkCommand, NetworkEvent, SaveLoadState, TileMap};
use systems::{
    movement_system::movement_system,
    needs_system::{needs_system, task_assignment_system},
    network_system::network_command_system,
    pathfinding_system::pathfinding_system,
    save_load_system::{load_game_system, save_game_system},
};

const TICK_RATE: f64 = 60.0;
const TICK_DURATION: Duration = Duration::from_nanos((1_000_000_000.0 / TICK_RATE) as u64);
const MAP_WIDTH: i32 = 100;
const MAP_HEIGHT: i32 = 100;

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    let _guard = rt.enter();

    let (event_sender, event_receiver): (Sender<NetworkEvent>, Receiver<NetworkEvent>) = unbounded();
    let (command_sender, command_receiver): (Sender<NetworkCommand>, Receiver<NetworkCommand>) = unbounded();

    let addr: SocketAddr = "127.0.0.1:9001".parse().unwrap();
    let _network_handle = networking::start_network_server(addr, event_receiver, command_sender.clone());

    let mut world = World::new();

    world.insert_resource(Time::default());
    world.insert_resource(GameTime::new());
    world.insert_resource(SaveLoadState::default());
    world.insert_resource(TileMap::new(MAP_WIDTH, MAP_HEIGHT));
    world.insert_resource(NetworkChannels {
        event_sender: event_sender.clone(),
        command_receiver,
    });

    let mut schedule = Schedule::default();

    schedule.add_systems((
        network_command_system,
        load_game_system,
        needs_system,
        task_assignment_system,
        pathfinding_system,
        movement_system,
        save_game_system,
    ));

    initialize_map(&mut world);
    spawn_pawns(&mut world, 5);

    println!("ECS游戏服务器已启动");
    println!("地图大小: {}x{}", MAP_WIDTH, MAP_HEIGHT);
    println!("Tick速率: {} Ticks/s", TICK_RATE);
    println!("网络监听: ws://{}", addr);
    println!("按 Ctrl+C 停止服务器");

    let mut last_tick = Instant::now();
    let mut tick_count: u64 = 0;
    let mut fps_timer = Instant::now();
    let mut fps_ticks = 0;

    loop {
        let now = Instant::now();
        let elapsed = now.duration_since(last_tick);

        if elapsed >= TICK_DURATION {
            last_tick = now;

            let delta_secs = elapsed.as_secs_f64();
            if let Some(mut time) = world.get_resource_mut::<Time>() {
                time.update();
            }
            if let Some(mut game_time) = world.get_resource_mut::<GameTime>() {
                game_time.tick(delta_secs);
            }

            schedule.run(&mut world);

            if let Some(ref channels) = world.get_resource::<NetworkChannels>() {
                if let Some(game_time) = world.get_resource::<GameTime>() {
                    if let Some(tile_map) = world.get_resource::<TileMap>() {
                        let _ = channels.event_sender.send(NetworkEvent::WorldState {
                            tick: game_time.tick,
                            map_size: (tile_map.width, tile_map.height),
                        });
                    }
                }
            }

            tick_count += 1;
            fps_ticks += 1;

            if fps_timer.elapsed() >= Duration::from_secs(1) {
                println!("FPS: {}, Tick: {}", fps_ticks, tick_count);
                fps_timer = Instant::now();
                fps_ticks = 0;
            }
        } else {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

fn initialize_map(world: &mut World) {
    if let Some(mut tile_map) = world.get_resource_mut::<TileMap>() {
        let mut rng = rand::thread_rng();

        for x in 0..MAP_WIDTH {
            for y in 0..MAP_HEIGHT {
                if x == 0 || x == MAP_WIDTH - 1 || y == 0 || y == MAP_HEIGHT - 1 {
                    tile_map.set_tile(x, y, Tile::wall());
                    continue;
                }

                let roll: f64 = rng.gen();
                if roll < 0.05 {
                    tile_map.set_tile(x, y, Tile::wall());
                } else if roll < 0.12 {
                    tile_map.set_tile(x, y, Tile::resource(ResourceType::Food, rng.gen_range(20..50)));
                } else if roll < 0.18 {
                    tile_map.set_tile(x, y, Tile::resource(ResourceType::Wood, rng.gen_range(20..50)));
                } else if roll < 0.23 {
                    tile_map.set_tile(x, y, Tile::resource(ResourceType::Iron, rng.gen_range(20..50)));
                }
            }
        }

        for x in 45..55 {
            for y in 45..55 {
                tile_map.set_tile(x, y, Tile::empty());
            }
        }

        println!("地图初始化完成");
        println!("  - 边界墙: 已设置");
        println!("  - 随机障碍物: 5%");
        println!("  - 食物资源: 7%");
        println!("  - 木头资源: 6%");
        println!("  - 铁矿资源: 5%");
        println!("  - 中心安全区: 10x10");
    }
}

fn spawn_pawns(world: &mut World, count: usize) {
    let mut rng = rand::thread_rng();
    let center_x = MAP_WIDTH / 2;
    let center_y = MAP_HEIGHT / 2;

    for i in 0..count {
        let offset_x = rng.gen_range(-5..5);
        let offset_y = rng.gen_range(-5..5);
        let x = center_x + offset_x;
        let y = center_y + offset_y;

        world.spawn((
            Pawn,
            Position::new(x, y),
            Hunger::new(100.0, 2.0),
            Stamina::new(100.0, 5.0, 5.0, 15.0),
            Task::Idle,
            Inventory::default(),
        ));

        println!("Pawn {} 生成于 ({}, {})", i, x, y);
    }

    println!("共生成 {} 个Pawn", count);
}

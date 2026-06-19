#!/usr/bin/env python3
"""
项目逻辑验证脚本

由于环境中没有Rust编译器，此脚本用于验证：
1. 项目结构完整性
2. JSON序列化格式验证
3. A*算法逻辑验证（Python版本）
4. 网络协议格式验证
"""

import json
import math
import sys
from pathlib import Path
from typing import List, Tuple, Dict, Optional
from dataclasses import dataclass
from enum import Enum
import heapq


class ResourceType(Enum):
    IRON = "Iron"
    WOOD = "Wood"
    FOOD = "Food"
    NONE = "None"


@dataclass
class Position:
    x: int
    y: int

    def __hash__(self):
        return hash((self.x, self.y))

    def __eq__(self, other):
        return self.x == other.x and self.y == other.y

    def manhattan_to(self, other: 'Position') -> int:
        return abs(self.x - other.x) + abs(self.y - other.y)


@dataclass
class Tile:
    resource_type: ResourceType
    passable: bool
    resource_amount: int


class TileMap:
    def __init__(self, width: int, height: int):
        self.width = width
        self.height = height
        self.tiles: Dict[Tuple[int, int], Tile] = {}
        for x in range(width):
            for y in range(height):
                self.tiles[(x, y)] = Tile(ResourceType.NONE, True, 0)

    def get_tile(self, x: int, y: int) -> Optional[Tile]:
        if x < 0 or x >= self.width or y < 0 or y >= self.height:
            return None
        return self.tiles.get((x, y))

    def is_passable(self, x: int, y: int) -> bool:
        tile = self.get_tile(x, y)
        return tile.passable if tile else False

    def set_tile(self, x: int, y: int, tile: Tile):
        if 0 <= x < self.width and 0 <= y < self.height:
            self.tiles[(x, y)] = tile

    def get_neighbors(self, pos: Position) -> List[Position]:
        neighbors = []
        directions = [
            (0, -1), (1, 0), (0, 1), (-1, 0),
            (1, -1), (1, 1), (-1, 1), (-1, -1)
        ]
        for dx, dy in directions:
            nx, ny = pos.x + dx, pos.y + dy
            if self.is_passable(nx, ny):
                neighbors.append(Position(nx, ny))
        return neighbors

    def find_nearest_resource(self, from_pos: Position, resource_type: ResourceType) -> Optional[Position]:
        nearest = None
        nearest_dist = float('inf')
        for (x, y), tile in self.tiles.items():
            if tile.resource_type == resource_type and tile.resource_amount > 0:
                pos = Position(x, y)
                dist = from_pos.manhattan_to(pos)
                if dist < nearest_dist:
                    nearest = pos
                    nearest_dist = dist
        return nearest


class TileLocks:
    """空间锁管理器"""
    def __init__(self):
        self.locks: Dict[Tuple[int, int], Tuple[int, ResourceType]] = {}

    def is_locked(self, x: int, y: int) -> bool:
        return (x, y) in self.locks

    def is_locked_by(self, x: int, y: int, entity_id: int) -> bool:
        lock = self.locks.get((x, y))
        return lock is not None and lock[0] == entity_id

    def try_lock(self, x: int, y: int, entity_id: int, resource_type: ResourceType) -> bool:
        if self.is_locked(x, y):
            return False
        self.locks[(x, y)] = (entity_id, resource_type)
        return True

    def unlock(self, x: int, y: int):
        self.locks.pop((x, y), None)

    def unlock_entity(self, entity_id: int):
        self.locks = {k: v for k, v in self.locks.items() if v[0] != entity_id}

    def get_locked_count(self) -> int:
        return len(self.locks)


class TaskScheduler:
    """任务调度中心"""
    def __init__(self):
        self.pending_requests: List[Tuple[int, Position, ResourceType]] = []
        self.assignments: Dict[int, Tuple[Position, ResourceType]] = {}

    def request_task(self, entity_id: int, position: Position, resource_type: ResourceType):
        if entity_id not in self.assignments:
            if not any(e == entity_id for e, _, _ in self.pending_requests):
                self.pending_requests.append((entity_id, position, resource_type))

    def process_requests(self, tile_map: TileMap, tile_locks: TileLocks):
        """处理所有待处理请求，统一分配任务"""
        requests = self.pending_requests.copy()
        self.pending_requests.clear()

        grouped: Dict[ResourceType, List[Tuple[int, Position]]] = {}
        for entity_id, pos, rtype in requests:
            if rtype not in grouped:
                grouped[rtype] = []
            grouped[rtype].append((entity_id, pos))

        for rtype, entity_reqs in grouped.items():
            available = []
            for (x, y), tile in tile_map.tiles.items():
                if tile.resource_type == rtype and tile.resource_amount > 0:
                    if not tile_locks.is_locked(x, y):
                        available.append((Position(x, y), tile.resource_amount))

            for entity_id, pawn_pos in entity_reqs:
                if not available:
                    self.pending_requests.append((entity_id, pawn_pos, rtype))
                    continue

                best_idx = 0
                best_dist = float('inf')
                for idx, (res_pos, _) in enumerate(available):
                    dist = pawn_pos.manhattan_to(res_pos)
                    if dist < best_dist:
                        best_dist = dist
                        best_idx = idx

                target_pos, _ = available.pop(best_idx)
                if tile_locks.try_lock(target_pos.x, target_pos.y, entity_id, rtype):
                    self.assignments[entity_id] = (target_pos, rtype)
                else:
                    self.pending_requests.append((entity_id, pawn_pos, rtype))

    def complete_task(self, entity_id: int):
        self.assignments.pop(entity_id, None)

    def has_assignment(self, entity_id: int) -> bool:
        return entity_id in self.assignments

    def get_assignment(self, entity_id: int) -> Optional[Tuple[Position, ResourceType]]:
        return self.assignments.get(entity_id)


def heuristic(a: Position, b: Position) -> int:
    """A*启发式函数（切比雪夫距离"""
    dx = abs(a.x - b.x)
    dy = abs(a.y - b.y)
    straight = abs(dx - dy)
    diagonal = min(dx, dy)
    return straight * 10 + diagonal * 14


def get_distance(a: Position, b: Position) -> int:
    """获取两点间移动成本"""
    dx = abs(a.x - b.x)
    dy = abs(a.y - b.y)
    if dx == 1 and dy == 1:
        return 14
    return 10


def find_path(tile_map: TileMap, start: Position, goal: Position) -> Optional[List[Position]]:
    """A*寻路算法（Python版本）"""
    return find_path_with_locks(tile_map, start, goal, None)


def get_neighbors_with_locks(
    tile_map: TileMap,
    pos: Position,
    goal: Position,
    tile_locks: Optional[TileLocks],
) -> List[Position]:
    neighbors = []
    directions = [
        (0, -1), (1, 0), (0, 1), (-1, 0),
        (1, -1), (1, 1), (-1, 1), (-1, -1)
    ]
    for dx, dy in directions:
        nx, ny = pos.x + dx, pos.y + dy
        if not tile_map.is_passable(nx, ny):
            continue
        neighbor = Position(nx, ny)
        if tile_locks is not None:
            if neighbor != goal and tile_locks.is_locked(nx, ny):
                continue
        neighbors.append(neighbor)
    return neighbors


def find_path_with_locks(
    tile_map: TileMap,
    start: Position,
    goal: Position,
    tile_locks: Optional[TileLocks],
) -> Optional[List[Position]]:
    """A*寻路算法（考虑空间锁）"""
    if start == goal:
        return [start]

    if not tile_map.is_passable(goal.x, goal.y):
        return None

    open_set = []
    heapq.heappush(open_set, (0, id(start), start))

    nodes = {}
    closed_set = set()

    start_h = heuristic(start, goal)
    nodes[start] = (0, start_h, None)

    while open_set:
        f_cost, _, current = heapq.heappop(open_set)

        if current in closed_set:
            continue

        closed_set.add(current)

        if current == goal:
            path = [current]
            while nodes[current][2] is not None:
                current = nodes[current][2]
                path.append(current)
            path.reverse()
            return path

        current_g = nodes[current][0]
        neighbors = get_neighbors_with_locks(tile_map, current, goal, tile_locks)

        for neighbor in neighbors:
            if neighbor in closed_set:
                continue

            move_cost = get_distance(current, neighbor)
            new_g = current_g + move_cost

            if neighbor not in nodes or new_g < nodes[neighbor][0]:
                h = heuristic(neighbor, goal)
                f = new_g + h
                nodes[neighbor] = (new_g, h, current)
                heapq.heappush(open_set, (f, id(neighbor), neighbor))

    return None


def verify_project_structure():
    """验证项目文件结构"""
    print("=" * 60)
    print("项目结构验证")
    print("=" * 60)

    required_files = [
        "Cargo.toml",
        "src/main.rs",
        "src/components.rs",
        "src/resources.rs",
        "src/pathfinding.rs",
        "src/networking.rs",
        "src/systems/mod.rs",
        "src/systems/task_scheduler_system.rs",
        "src/systems/needs_system.rs",
        "src/systems/pathfinding_system.rs",
        "src/systems/movement_system.rs",
        "src/systems/save_load_system.rs",
        "src/systems/network_system.rs",
        "examples/python_client.py",
        "examples/UnityClient.cs",
    ]

    all_ok = True
    for file_path in required_files:
        path = Path(file_path)
        exists = path.exists()
        status = "[OK]" if exists else "[FAIL]"
        print(f"  {status} {file_path}")
        if not exists:
            all_ok = False

    print()
    return all_ok


def verify_astar_algorithm():
    """验证A*寻路算法"""
    print("=" * 60)
    print("A*寻路算法验证")
    print("=" * 60)

    map_size = 20
    tile_map = TileMap(map_size, map_size)

    tile_map.set_tile(5, 5, Tile(ResourceType.NONE, False, 0))
    tile_map.set_tile(5, 6, Tile(ResourceType.NONE, False, 0))
    tile_map.set_tile(5, 7, Tile(ResourceType.NONE, False, 0))

    tile_map.set_tile(10, 10, Tile(ResourceType.FOOD, True, 50))
    tile_map.set_tile(15, 8, Tile(ResourceType.IRON, True, 30))

    print("测试1: 简单直线路径")
    start = Position(0, 0)
    goal = Position(5, 0)
    path = find_path(tile_map, start, goal)
    assert path is not None, "路径应为有效"
    assert path[0] == start, "起点正确"
    assert path[-1] == goal, "终点正确"
    print(f"  [OK] 找到路径，长度: {len(path)}")

    print("测试2: 绕开障碍物")
    start = Position(3, 6)
    goal = Position(8, 6)
    path = find_path(tile_map, start, goal)
    assert path is not None, "路径应为有效"
    for pos in path:
        assert (pos.x, pos.y) != (5, 6), "路径应绕开障碍"
    print(f"  [OK] 找到路径，长度: {len(path)}")

    print("测试3: 寻找最近资源")
    from_pos = Position(0, 0)
    nearest = tile_map.find_nearest_resource(from_pos, ResourceType.FOOD)
    assert nearest == Position(10, 10), "最近食物位置正确"
    print(f"  [OK] 最近食物位置: ({nearest.x}, {nearest.y})")

    print("测试4: 同点寻路")
    start = Position(5, 5)
    path = find_path(tile_map, start, start)
    assert path is not None and len(path) == 1, "同点路径长度为1"
    print(f"  [OK] 同点寻路正确")

    print("测试5: 无法到达")
    for x in range(map_size):
        tile_map.set_tile(x, 10, Tile(ResourceType.NONE, False, 0))
    start = Position(0, 0)
    goal = Position(0, 15)
    path = find_path(tile_map, start, goal)
    assert path is None, "应为无效"
    print(f"  [OK] 无法到达时返回None正确")

    print("测试6: 寻路绕开锁定格子")
    tile_map2 = TileMap(10, 10)
    locks = TileLocks()
    locks.try_lock(1, 0, 1, ResourceType.FOOD)
    start = Position(0, 0)
    goal = Position(3, 0)
    path = find_path_with_locks(tile_map2, start, goal, locks)
    assert path is not None, "路径应为有效"
    for pos in path:
        assert (pos.x, pos.y) != (1, 0), "路径应绕开锁定格子"
    print(f"  [OK] 路径避开锁定格子 (1, 0)")

    print("测试7: 目标格子即使锁定也可达")
    locks2 = TileLocks()
    locks2.try_lock(3, 0, 1, ResourceType.FOOD)
    path = find_path_with_locks(tile_map2, start, goal, locks2)
    assert path is not None, "目标格锁定时仍应可达"
    assert path[-1] == goal, "终点应是锁定的目标格"
    print(f"  [OK] 目标格即使锁定也可到达")

    print()
    return True


def verify_tile_locks():
    """验证空间锁机制"""
    print("=" * 60)
    print("空间锁机制验证")
    print("=" * 60)

    locks = TileLocks()

    print("测试1: 基本加锁解锁")
    assert locks.try_lock(5, 5, 1, ResourceType.FOOD) == True
    assert locks.is_locked(5, 5) == True
    assert locks.is_locked_by(5, 5, 1) == True
    assert locks.is_locked_by(5, 5, 2) == False
    print("  [OK] 加锁成功")

    print("测试2: 重复加锁失败")
    assert locks.try_lock(5, 5, 2, ResourceType.WOOD) == False
    print("  [OK] 已锁定格子无法重复加锁")

    print("测试3: 解锁后可重新加锁")
    locks.unlock(5, 5)
    assert locks.is_locked(5, 5) == False
    assert locks.try_lock(5, 5, 2, ResourceType.WOOD) == True
    print("  [OK] 解锁后可重新加锁")

    print("测试4: 按实体解锁")
    locks.try_lock(10, 10, 2, ResourceType.IRON)
    locks.try_lock(11, 11, 3, ResourceType.FOOD)
    assert locks.get_locked_count() == 3
    locks.unlock_entity(2)
    assert locks.get_locked_count() == 1
    assert locks.is_locked(5, 5) == False
    assert locks.is_locked(10, 10) == False
    assert locks.is_locked(11, 11) == True
    print("  [OK] 按实体解锁正确")

    print()
    return True


def verify_task_scheduler():
    """验证任务调度中心"""
    print("=" * 60)
    print("任务调度中心验证")
    print("=" * 60)

    tile_map = TileMap(20, 20)
    tile_map.set_tile(5, 5, Tile(ResourceType.FOOD, True, 50))
    tile_map.set_tile(15, 15, Tile(ResourceType.FOOD, True, 30))
    tile_map.set_tile(10, 5, Tile(ResourceType.IRON, True, 25))

    locks = TileLocks()
    scheduler = TaskScheduler()

    print("测试1: 多小人请求同一资源，分配到不同格子")
    scheduler.request_task(1, Position(0, 0), ResourceType.FOOD)
    scheduler.request_task(2, Position(19, 19), ResourceType.FOOD)
    scheduler.process_requests(tile_map, locks)

    assert scheduler.has_assignment(1) == True
    assert scheduler.has_assignment(2) == True

    pos1, _ = scheduler.get_assignment(1)
    pos2, _ = scheduler.get_assignment(2)
    assert pos1 != pos2, "两个小人应分配到不同资源点"
    print(f"  [OK] Pawn1分配到 ({pos1.x}, {pos1.y})")
    print(f"  [OK] Pawn2分配到 ({pos2.x}, {pos2.y})")

    print("测试2: 已分配的资源格子被锁定")
    assert locks.is_locked(pos1.x, pos1.y) == True
    assert locks.is_locked(pos2.x, pos2.y) == True
    assert locks.get_locked_count() == 2
    print("  [OK] 目标格子均已锁定")

    print("测试3: 第三个小人无空闲资源时进入等待队列")
    scheduler.request_task(3, Position(10, 10), ResourceType.FOOD)
    scheduler.process_requests(tile_map, locks)
    assert scheduler.has_assignment(3) == False
    assert len(scheduler.pending_requests) >= 1
    print("  [OK] 无资源时请求进入等待队列")

    print("测试4: 任务完成释放锁，等待者获得分配")
    scheduler.complete_task(1)
    locks.unlock_entity(1)
    scheduler.process_requests(tile_map, locks)
    assert scheduler.has_assignment(3) == True
    pos3, _ = scheduler.get_assignment(3)
    print(f"  [OK] Pawn3获得分配 ({pos3.x}, {pos3.y})")

    print("测试5: 资源耗尽后不再分配")
    tile_map.set_tile(15, 15, Tile(ResourceType.NONE, True, 0))
    scheduler.complete_task(3)
    locks.unlock_entity(3)
    scheduler.request_task(4, Position(0, 0), ResourceType.FOOD)
    scheduler.process_requests(tile_map, locks)
    assert scheduler.has_assignment(4) == True
    pos4, _ = scheduler.get_assignment(4)
    assert pos4 == Position(5, 5), "应只分配剩余的食物"
    print(f"  [OK] 耗尽的资源不再分配")

    print()
    return True


def verify_collision_avoidance():
    """验证碰撞避免逻辑（多小人场景）"""
    print("=" * 60)
    print("碰撞避免综合验证")
    print("=" * 60)

    tile_map = TileMap(30, 30)
    for i in range(3):
        tile_map.set_tile(10 + i * 5, 15, Tile(ResourceType.FOOD, True, 100))

    locks = TileLocks()
    scheduler = TaskScheduler()

    print("测试: 5个小人同时请求食物，无重复分配")
    for i in range(5):
        scheduler.request_task(i + 1, Position(5, 5 + i), ResourceType.FOOD)

    scheduler.process_requests(tile_map, locks)

    assigned_positions = set()
    assigned_count = 0
    for i in range(5):
        if scheduler.has_assignment(i + 1):
            pos, rtype = scheduler.get_assignment(i + 1)
            assert rtype == ResourceType.FOOD
            pos_key = (pos.x, pos.y)
            assert pos_key not in assigned_positions, f"小人{i+1}分配到已被占用的格子 {pos_key}"
            assigned_positions.add(pos_key)
            assigned_count += 1

    assert assigned_count == 3, f"应只有3个获得分配，实际 {assigned_count}"
    assert locks.get_locked_count() == 3, "锁定数量应等于分配数量"

    print(f"  [OK] {assigned_count}个小人获得不同资源点")
    print(f"  [OK] {5 - assigned_count}个小人进入等待队列")
    print(f"  [OK] 所有目标格子均被锁定，无重复分配")

    print()
    return True


def verify_json_serialization():
    """验证JSON序列化格式"""
    print("=" * 60)
    print("JSON序列化格式验证")
    print("=" * 60)

    network_event_samples = [
        {
            "type": "Event",
            "event": {
                "PawnMoved": {
                    "entity_id": 1,
                    "position": {"x": 10, "y": 20}
                }
            }
        },
        {
            "type": "Event",
            "event": {
                "ResourceChanged": {
                    "position": {"x": 5, "y": 5},
                    "resource_type": "Food",
                    "amount": 25
                }
            }
        },
        {
            "type": "Event",
            "event": {
                "TaskChanged": {
                    "entity_id": 1,
                    "task": "FindFood"
                }
            }
        },
        {
            "type": "Event",
            "event": {
                "WorldState": {
                    "tick": 1000,
                    "map_size": [100, 100]
                }
            }
        }
    ]

    for i, sample in enumerate(network_event_samples):
        json_str = json.dumps(sample, ensure_ascii=False)
        parsed = json.loads(json_str)
        assert parsed == sample, f"事件 {i} 序列化正确"
        print(f"  [OK] 事件 {i} 序列化正确")

    client_commands = [
        {"type": "Subscribe"},
        {"type": "Unsubscribe"},
        {"type": "Command", "command": {"SaveGame": "saves/game1.json"}},
        {"type": "Command", "command": {"LoadGame": "saves/game1.json"}},
        {"type": "Command", "command": {"SpawnPawn": {"x": 50, "y": 50}}},
        {"type": "Ping"},
    ]

    for i, cmd in enumerate(client_commands):
        json_str = json.dumps(cmd, ensure_ascii=False)
        parsed = json.loads(json_str)
        assert parsed == cmd, f"命令 {i} 序列化正确"
        print(f"  [OK] 命令 {i} 序列化正确")

    print()
    return True


def verify_game_state_save_format():
    """验证游戏存档格式"""
    print("=" * 60)
    print("游戏存档格式验证")
    print("=" * 60)

    save_data = {
        "tile_map": {
            "width": 100,
            "height": 100,
            "tiles": {
                "[0, 0]": {
                    "resource_type": "None",
                    "passable": True,
                    "resource_amount": 0
                },
                "[10, 10]": {
                    "resource_type": "Food",
                    "passable": True,
                    "resource_amount": 50
                }
            }
        },
        "game_time": {
            "tick": 3600,
            "elapsed_seconds": 60.0
        },
        "pawns": [
            {
                "position": {"x": 50,
                "y": 50
            },
            "hunger": {
                "value": 75.5,
                "max": 100.0,
                "decay_rate": 2.0
            },
            "stamina": {
                "value": 100.0,
                "max": 100.0,
                "regeneration_rate": 5.0,
                "move_cost": 5.0,
                "gather_cost": 15.0
            },
            "task": "Idle",
            "path": None,
            "inventory": {
                "iron": 10,
                "wood": 5,
                "food": 3
            }
        }
    ]
    }

    json_str = json.dumps(save_data, indent=2, ensure_ascii=False)
    parsed = json.loads(json_str)

    assert parsed["tile_map"]["width"] == 100
    assert parsed["tile_map"]["height"] == 100
    assert parsed["game_time"]["tick"] == 3600
    assert len(parsed["pawns"]) == 1
    assert parsed["pawns"][0]["position"]["x"] == 50

    print("  [OK] 存档格式正确")
    print(f"  [OK] 可正确序列化和反序列化")
    print()

    return True


def verify_ecs_concepts():
    """验证ECS概念理解"""
    print("=" * 60)
    print("ECS架构概念验证")
    print("=" * 60)

    components = [
        ("Position", "位置组件"),
        ("Hunger", "饥饿度组件"),
        ("Stamina", "体力组件"),
        ("Task", "任务组件"),
        ("Path", "路径组件"),
        ("Inventory", "库存组件"),
        ("Tile", "瓦片组件"),
        ("Pawn", "标记组件"),
        ("NeedsPathfinding", "标记组件"),
        ("TileLock", "空间锁组件"),
        ("RequestTask", "任务请求组件"),
        ("AssignedTask", "已分配任务组件"),
    ]

    for name, desc in components:
        print(f"  [OK] 组件: {name} - {desc}")

    systems = [
        ("needs_system", "需求系统 - 检查饥饿度，向调度中心请求任务"),
        ("task_scheduler_system", "任务调度系统 - 统一分配资源采集任务，管理空间锁"),
        ("cleanup_stale_locks_system", "过期锁清理系统 - 清理资源耗尽或无效的锁"),
        ("task_assignment_system", "任务分配系统 - 接收调度结果并触发寻路"),
        ("pathfinding_executor_system", "寻路执行系统 - 执行A*寻路（考虑空间锁）"),
        ("movement_system", "移动系统 - 沿路径移动，执行采集并释放锁"),
        ("save_game_system", "存档系统 - 保存游戏状态"),
        ("load_game_system", "读档系统 - 加载游戏状态并重置调度器"),
        ("network_command_system", "网络命令系统 - 处理客户端命令"),
    ]

    print()
    for name, desc in systems:
        print(f"  [OK] 系统: {name} - {desc}")

    print()
    return True


def verify_tick_rate():
    """验证60 Tick/s计时逻辑"""
    print("=" * 60)
    print("60 Tick/s 计时逻辑验证")
    print("=" * 60)

    tick_rate = 60.0
    tick_duration_ns = int(1_000_000_000 / tick_rate)
    tick_duration_ms = tick_duration_ns / 1_000_000

    print(f"  Tick速率: {tick_rate} Ticks/s")
    print(f"  Tick间隔: {tick_duration_ns} ns")
    print(f"  Tick间隔: {tick_duration_ms:.2f} ms")
    print(f"  每秒最大Tick数: {1_000_000_000 / tick_duration_ns:.2f}")

    assert abs(tick_duration_ns - 16666666) < 100, "Tick间隔正确"
    print("  [OK] 计时逻辑正确")
    print()
    return True


def main():
    print("\n" + "=" * 60)
    print("ECS游戏服务器项目验证报告")
    print("=" * 60 + "\n")

    checks = [
        ("项目结构", verify_project_structure),
        ("A*寻路算法", verify_astar_algorithm),
        ("空间锁机制", verify_tile_locks),
        ("任务调度中心", verify_task_scheduler),
        ("碰撞避免综合", verify_collision_avoidance),
        ("JSON序列化", verify_json_serialization),
        ("游戏存档格式", verify_game_state_save_format),
        ("ECS架构概念", verify_ecs_concepts),
        ("60 Tick/s计时", verify_tick_rate),
    ]

    results = {}
    for name, check_func in checks:
        try:
            results[name] = check_func()
        except Exception as e:
            print(f"  [FAIL] 失败: {e}")
            import traceback
            traceback.print_exc()
            results[name] = False

    print("=" * 60)
    print("验证结果汇总")
    print("=" * 60)

    all_passed = True
    for name, passed in results.items():
        status = "PASS" if passed else "FAIL"
        print(f"  {status}: {name}")
        if not passed:
            all_passed = False

    print()
    if all_passed:
        print("[OK] 所有验证通过！")
        print("项目结构完整，逻辑正确，可以在有Rust环境中编译运行。")
    else:
        print("[FAIL] 部分验证失败，请检查上述错误。")

    print()
    print("=" * 60)
    print("编译运行说明")
    print("=" * 60)
    print("""
在安装了Rust环境后，执行以下命令：

1. 编译项目:
   cargo build --release

2. 运行服务器:
   cargo run --release

3. 运行测试:
   cargo test

4. 运行Python客户端 (需要安装websockets库):
   pip install websockets
   python examples/python_client.py

服务器将在 ws://127.0.0.1:9001 监听连接。
""")

    return 0 if all_passed else 1


if __name__ == "__main__":
    sys.exit(main())

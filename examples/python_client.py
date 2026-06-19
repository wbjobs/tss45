import json
import asyncio
import websockets
import argparse
from datetime import datetime


class GameClient:
    def __init__(self, server_url: str = "ws://127.0.0.1:9001"):
        self.server_url = server_url
        self.websocket = None
        self.subscribed = False
        self.event_count = 0
        self.last_print = datetime.now()

    async def connect(self):
        print(f"正在连接到服务器: {self.server_url}")
        self.websocket = await websockets.connect(self.server_url)
        print("连接成功!")

    async def disconnect(self):
        if self.websocket:
            await self.websocket.close()
            print("连接已关闭")

    async def send_message(self, message_type: str, **kwargs):
        message = {"type": message_type, **kwargs}
        await self.websocket.send(json.dumps(message))
        print(f"发送: {json.dumps(message, ensure_ascii=False)}")

    async def subscribe(self):
        await self.send_message("Subscribe")
        self.subscribed = True
        print("已订阅世界状态")

    async def unsubscribe(self):
        await self.send_message("Unsubscribe")
        self.subscribed = False
        print("已取消订阅")

    async def ping(self):
        await self.send_message("Ping")

    async def save_game(self, file_path: str):
        command = {
            "type": "Command",
            "command": {"SaveGame": file_path}
        }
        await self.websocket.send(json.dumps(command))
        print(f"已发送保存游戏命令: {file_path}")

    async def load_game(self, file_path: str):
        command = {
            "type": "Command",
            "command": {"LoadGame": file_path}
        }
        await self.websocket.send(json.dumps(command))
        print(f"已发送加载游戏命令: {file_path}")

    async def spawn_pawn(self, x: int, y: int):
        command = {
            "type": "Command",
            "command": {"SpawnPawn": {"x": x, "y": y}}
        }
        await self.websocket.send(json.dumps(command))
        print(f"已发送生成Pawn命令: ({x}, {y})")

    async def handle_event(self, event: dict):
        self.event_count += 1

        event_type = event.get("type", "Unknown")
        event_data = event.get("data", event)

        if isinstance(event_data, dict):
            msg_type = event_data.get("type", "")

            if msg_type == "Event":
                inner_event = event_data.get("event", {})
                event_name = next(iter(inner_event.keys())) if inner_event else "Unknown"

                now = datetime.now()
                if (now - self.last_print).total_seconds() >= 0.5:
                    print(f"\r收到事件 #{self.event_count}: {event_name}", end="")
                    self.last_print = now

                if event_name == "PawnMoved":
                    data = inner_event["PawnMoved"]
                    pass
                elif event_name == "ResourceChanged":
                    data = inner_event["ResourceChanged"]
                    pass
                elif event_name == "InventoryChanged":
                    data = inner_event["InventoryChanged"]
                    pass
                elif event_name == "TaskChanged":
                    data = inner_event["TaskChanged"]
                    print(f"\n任务变更: 实体#{data['entity_id']} -> {data['task']}")
                elif event_name == "HungerChanged":
                    data = inner_event["HungerChanged"]
                    pass
                elif event_name == "StaminaChanged":
                    data = inner_event["StaminaChanged"]
                    pass
                elif event_name == "WorldState":
                    data = inner_event["WorldState"]
                    print(f"\n世界状态: Tick={data['tick']}, 地图大小={data['map_size']}")

            elif msg_type == "Pong":
                print("\n收到Pong")

            elif msg_type == "Error":
                print(f"\n服务器错误: {event_data.get('message', '未知错误')}")

    async def listen(self):
        try:
            async for message in self.websocket:
                if isinstance(message, str):
                    try:
                        data = json.loads(message)
                        await self.handle_event(data)
                    except json.JSONDecodeError as e:
                        print(f"无法解析消息: {message}, 错误: {e}")
        except websockets.exceptions.ConnectionClosed:
            print("\n服务器连接已关闭")
        except Exception as e:
            print(f"\n监听出错: {e}")

    async def interactive_shell(self):
        await self.connect()
        await self.subscribe()

        print("\n可用命令:")
        print("  subscribe   - 订阅世界状态")
        print("  unsubscribe - 取消订阅")
        print("  ping        - 发送心跳")
        print("  save <path> - 保存游戏")
        print("  load <path> - 加载游戏")
        print("  spawn <x> <y> - 生成Pawn")
        print("  quit        - 退出")

        listen_task = asyncio.create_task(self.listen())

        try:
            while True:
                try:
                    user_input = await asyncio.to_thread(input, "\n> ")
                    user_input = user_input.strip()

                    if not user_input:
                        continue

                    parts = user_input.split()
                    cmd = parts[0].lower()

                    if cmd == "quit" or cmd == "exit":
                        break
                    elif cmd == "subscribe":
                        await self.subscribe()
                    elif cmd == "unsubscribe":
                        await self.unsubscribe()
                    elif cmd == "ping":
                        await self.ping()
                    elif cmd == "save":
                        if len(parts) >= 2:
                            await self.save_game(parts[1])
                        else:
                            print("用法: save <文件路径>")
                    elif cmd == "load":
                        if len(parts) >= 2:
                            await self.load_game(parts[1])
                        else:
                            print("用法: load <文件路径>")
                    elif cmd == "spawn":
                        if len(parts) >= 3:
                            try:
                                x = int(parts[1])
                                y = int(parts[2])
                                await self.spawn_pawn(x, y)
                            except ValueError:
                                print("坐标必须是整数")
                        else:
                            print("用法: spawn <x> <y>")
                    else:
                        print(f"未知命令: {cmd}")

                except EOFError:
                    break

        finally:
            listen_task.cancel()
            await self.disconnect()


async def main():
    parser = argparse.ArgumentParser(description="ECS游戏服务器WebSocket客户端")
    parser.add_argument("--server", default="ws://127.0.0.1:9001", help="服务器地址")
    parser.add_argument("--headless", action="store_true", help="无头模式，仅监听事件")
    args = parser.parse_args()

    client = GameClient(args.server)

    if args.headless:
        await client.connect()
        await client.subscribe()
        print("无头模式运行中，按 Ctrl+C 停止...")
        try:
            await client.listen()
        except KeyboardInterrupt:
            pass
        finally:
            await client.disconnect()
    else:
        await client.interactive_shell()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\n用户中断")
    except ConnectionRefusedError:
        print("无法连接到服务器，请确保服务器正在运行")

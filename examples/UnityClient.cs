using System;
using System.Collections;
using System.Collections.Generic;
using System.Text;
using System.Threading.Tasks;
using UnityEngine;
using WebSocketSharp;

public class GameClient : MonoBehaviour
{
    [SerializeField] private string serverUrl = "ws://127.0.0.1:9001";
    private WebSocket ws;
    private bool isConnected = false;
    private bool isSubscribed = false;

    public event Action<PawnMovedEvent> OnPawnMoved;
    public event Action<ResourceChangedEvent> OnResourceChanged;
    public event Action<InventoryChangedEvent> OnInventoryChanged;
    public event Action<TaskChangedEvent> OnTaskChanged;
    public event Action<HungerChangedEvent> OnHungerChanged;
    public event Action<StaminaChangedEvent> OnStaminaChanged;
    public event Action<WorldStateEvent> OnWorldState;

    private Queue<string> messageQueue = new Queue<string>();
    private readonly object queueLock = new object();

    void Start()
    {
        Connect();
    }

    void Update()
    {
        lock (queueLock)
        {
            while (messageQueue.Count > 0)
            {
                string message = messageQueue.Dequeue();
                ProcessMessage(message);
            }
        }
    }

    void OnDestroy()
    {
        Disconnect();
    }

    public void Connect()
    {
        try
        {
            ws = new WebSocket(serverUrl);
            ws.OnOpen += (sender, e) =>
            {
                isConnected = true;
                Debug.Log("已连接到游戏服务器");
                Subscribe();
            };

            ws.OnMessage += (sender, e) =>
            {
                if (e.IsText)
                {
                    lock (queueLock)
                    {
                        messageQueue.Enqueue(e.Data);
                    }
                }
            };

            ws.OnError += (sender, e) =>
            {
                Debug.LogError($"WebSocket错误: {e.Message}");
            };

            ws.OnClose += (sender, e) =>
            {
                isConnected = false;
                isSubscribed = false;
                Debug.Log("与游戏服务器断开连接");
            };

            ws.Connect();
        }
        catch (Exception ex)
        {
            Debug.LogError($"连接失败: {ex.Message}");
        }
    }

    public void Disconnect()
    {
        if (ws != null && ws.IsAlive)
        {
            Unsubscribe();
            ws.Close();
        }
    }

    private void SendMessage(string type, object data = null)
    {
        if (ws != null && ws.IsAlive)
        {
            var message = new Dictionary<string, object>
            {
                ["type"] = type
            };

            if (data != null)
            {
                foreach (var kvp in data as IDictionary<string, object>)
                {
                    message[kvp.Key] = kvp.Value;
                }
            }

            string json = JsonUtility.ToJson(message);
            ws.Send(json);
        }
    }

    public void Subscribe()
    {
        SendMessage("Subscribe");
        isSubscribed = true;
    }

    public void Unsubscribe()
    {
        SendMessage("Unsubscribe");
        isSubscribed = false;
    }

    public void SaveGame(string filePath)
    {
        var command = new Dictionary<string, object>
        {
            ["command"] = new Dictionary<string, object>
            {
                ["SaveGame"] = filePath
            }
        };
        SendMessage("Command", command);
    }

    public void LoadGame(string filePath)
    {
        var command = new Dictionary<string, object>
        {
            ["command"] = new Dictionary<string, object>
            {
                ["LoadGame"] = filePath
            }
        };
        SendMessage("Command", command);
    }

    public void SpawnPawn(int x, int y)
    {
        var command = new Dictionary<string, object>
        {
            ["command"] = new Dictionary<string, object>
            {
                ["SpawnPawn"] = new Dictionary<string, int>
                {
                    ["x"] = x,
                    ["y"] = y
                }
            }
        };
        SendMessage("Command", command);
    }

    private void ProcessMessage(string message)
    {
        try
        {
            var serverMsg = JsonUtility.FromJson<ServerMessage>(message);

            if (serverMsg.type == "Event")
            {
                DispatchEvent(serverMsg);
            }
            else if (serverMsg.type == "Pong")
            {
                Debug.Log("收到Pong");
            }
            else if (serverMsg.type == "Error")
            {
                Debug.LogError($"服务器错误: {serverMsg.message}");
            }
        }
        catch (Exception ex)
        {
            Debug.LogError($"处理消息失败: {ex.Message}");
        }
    }

    private void DispatchEvent(ServerMessage msg)
    {
        if (msg.PawnMoved != null)
        {
            OnPawnMoved?.Invoke(msg.PawnMoved);
        }
        else if (msg.ResourceChanged != null)
        {
            OnResourceChanged?.Invoke(msg.ResourceChanged);
        }
        else if (msg.InventoryChanged != null)
        {
            OnInventoryChanged?.Invoke(msg.InventoryChanged);
        }
        else if (msg.TaskChanged != null)
        {
            OnTaskChanged?.Invoke(msg.TaskChanged);
        }
        else if (msg.HungerChanged != null)
        {
            OnHungerChanged?.Invoke(msg.HungerChanged);
        }
        else if (msg.StaminaChanged != null)
        {
            OnStaminaChanged?.Invoke(msg.StaminaChanged);
        }
        else if (msg.WorldState != null)
        {
            OnWorldState?.Invoke(msg.WorldState);
        }
    }

    public bool IsConnected => isConnected;
    public bool IsSubscribed => isSubscribed;
}

[Serializable]
public class ServerMessage
{
    public string type;
    public string message;
    public PawnMovedEvent PawnMoved;
    public ResourceChangedEvent ResourceChanged;
    public InventoryChangedEvent InventoryChanged;
    public TaskChangedEvent TaskChanged;
    public HungerChangedEvent HungerChanged;
    public StaminaChangedEvent StaminaChanged;
    public WorldStateEvent WorldState;
}

[Serializable]
public class Position
{
    public int x;
    public int y;
}

[Serializable]
public class PawnMovedEvent
{
    public ulong entity_id;
    public Position position;
}

[Serializable]
public enum ResourceType
{
    Iron,
    Wood,
    Food,
    None
}

[Serializable]
public class ResourceChangedEvent
{
    public Position position;
    public ResourceType resource_type;
    public uint amount;
}

[Serializable]
public class Inventory
{
    public uint iron;
    public uint wood;
    public uint food;
}

[Serializable]
public class InventoryChangedEvent
{
    public ulong entity_id;
    public Inventory inventory;
}

[Serializable]
public enum TaskType
{
    Idle,
    FindFood,
    FindResource,
    MoveTo,
    Gather,
    ReturnToBase
}

[Serializable]
public class TaskChangedEvent
{
    public ulong entity_id;
    public TaskType task;
    public Position target_position;
    public ResourceType resource_type;
}

[Serializable]
public class HungerChangedEvent
{
    public ulong entity_id;
    public float value;
    public float max;
}

[Serializable]
public class StaminaChangedEvent
{
    public ulong entity_id;
    public float value;
    public float max;
}

[Serializable]
public class WorldStateEvent
{
    public ulong tick;
    public int[] map_size;
}

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

use crate::resources::{NetworkCommand, NetworkEvent};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    Subscribe,
    Unsubscribe,
    Command(NetworkCommand),
    Ping,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    Event(NetworkEvent),
    Pong,
    Error(String),
}

pub struct NetworkServer {
    address: SocketAddr,
    event_receiver: Receiver<NetworkEvent>,
    command_sender: Sender<NetworkCommand>,
}

impl NetworkServer {
    pub fn new(
        address: SocketAddr,
        event_receiver: Receiver<NetworkEvent>,
        command_sender: Sender<NetworkCommand>,
    ) -> Self {
        Self {
            address,
            event_receiver,
            command_sender,
        }
    }

    pub async fn run(self) {
        let listener = TcpListener::bind(self.address)
            .await
            .expect("Failed to bind TCP listener");
        println!("网络服务器已启动，监听地址: {}", self.address);

        let (event_tx, _event_rx) = broadcast::channel::<NetworkEvent>(1000);
        let subscribers = Arc::new(Mutex::new(HashSet::<SocketAddr>::new()));

        let event_sender = event_tx.clone();
        let event_receiver = self.event_receiver;
        tokio::spawn(async move {
            loop {
                if let Ok(event) = event_receiver.try_recv() {
                    let _ = event_sender.send(event);
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
            }
        });

        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    println!("新客户端连接: {}", addr);
                    let cmd_sender = self.command_sender.clone();
                    let subs = subscribers.clone();
                    let event_rx = event_tx.subscribe();
                    tokio::spawn(Self::handle_connection(stream, addr, cmd_sender, subs, event_rx));
                }
                Err(e) => {
                    eprintln!("接受连接失败: {}", e);
                }
            }
        }
    }

    async fn handle_connection(
        stream: TcpStream,
        addr: SocketAddr,
        command_sender: Sender<NetworkCommand>,
        subscribers: Arc<Mutex<HashSet<SocketAddr>>>,
        mut event_rx: broadcast::Receiver<NetworkEvent>,
    ) {
        let ws_stream = match tokio_tungstenite::accept_async(stream).await {
            Ok(ws) => ws,
            Err(e) => {
                eprintln!("WebSocket握手失败: {}", e);
                return;
            }
        };

        let (mut write, mut read) = ws_stream.split();
        let is_subscribed = Arc::new(Mutex::new(false));

        let write_task_is_subscribed = is_subscribed.clone();
        let write_task_subs = subscribers.clone();
        let write_task_addr = addr;

        let write_task = tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        let subscribed = *write_task_is_subscribed.lock().await;
                        if subscribed {
                            let msg = match serde_json::to_string(&ServerMessage::Event(event)) {
                                Ok(m) => m,
                                Err(e) => {
                                    eprintln!("序列化事件失败: {}", e);
                                    continue;
                                }
                            };
                            if let Err(e) = write.send(Message::Text(msg)).await {
                                eprintln!("发送消息失败: {}", e);
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });

        loop {
            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Err(e) = Self::handle_client_message(
                                &text,
                                addr,
                                &command_sender,
                                &subscribers,
                                &is_subscribed,
                            ).await {
                                eprintln!("处理客户端消息失败: {}", e);
                                break;
                            }
                        }
                        Some(Ok(Message::Binary(_))) => {
                            let _ = write.send(Message::Text(
                                serde_json::to_string(&ServerMessage::Error(
                                    "不支持二进制消息".to_string()
                                )).unwrap()
                            )).await;
                        }
                        Some(Ok(Message::Close(_))) => {
                            println!("客户端断开连接: {}", addr);
                            subscribers.lock().await.remove(&addr);
                            *is_subscribed.lock().await = false;
                            break;
                        }
                        Some(Err(e)) => {
                            eprintln!("WebSocket错误: {}", e);
                            subscribers.lock().await.remove(&addr);
                            *is_subscribed.lock().await = false;
                            break;
                        }
                        None => {
                            subscribers.lock().await.remove(&addr);
                            *is_subscribed.lock().await = false;
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        write_task.abort();
    }

    async fn handle_client_message(
        text: &str,
        addr: SocketAddr,
        command_sender: &Sender<NetworkCommand>,
        subscribers: &Arc<Mutex<HashSet<SocketAddr>>>,
        is_subscribed: &Arc<Mutex<bool>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match serde_json::from_str::<ClientMessage>(text) {
            Ok(ClientMessage::Subscribe) => {
                subscribers.lock().await.insert(addr);
                *is_subscribed.lock().await = true;
                println!("客户端订阅: {}", addr);
            }
            Ok(ClientMessage::Unsubscribe) => {
                subscribers.lock().await.remove(&addr);
                *is_subscribed.lock().await = false;
                println!("客户端取消订阅: {}", addr);
            }
            Ok(ClientMessage::Command(cmd)) => {
                let _ = command_sender.send(cmd);
            }
            Ok(ClientMessage::Ping) => {
                // Pong is handled by the message sender, but we don't have write access here
                // This would need to be sent back through the write half
            }
            Err(e) => {
                eprintln!("无效消息: {} from {}", e, addr);
            }
        }
        Ok(())
    }
}

pub fn start_network_server(
    address: SocketAddr,
    event_receiver: Receiver<NetworkEvent>,
    command_sender: Sender<NetworkCommand>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let server = NetworkServer::new(address, event_receiver, command_sender);
        server.run().await;
    })
}

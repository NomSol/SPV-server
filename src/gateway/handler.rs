use std::sync::Arc;

use crate::matchmaking::service::MatchService;
use crate::models::message::{ClientMessage, ServerMessage};
use crate::error::{Error, Result};
use axum::extract::ws::{Message, WebSocket};
use futures_util::{stream::StreamExt, SinkExt};
use serde_json::json;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::ConnectionManager;

pub struct WebSocketHandler {
    conn_manager: ConnectionManager,
    match_service: Arc<MatchService>,
}

impl WebSocketHandler {
    pub fn new(match_service: Arc<MatchService>) -> Self {
        Self {
            conn_manager: ConnectionManager::new(),
            match_service,
        }
    }

    async fn send_message(&self, conn_id: Uuid, message: &ServerMessage) -> Result<()> {
        let msg = serde_json::to_string(message)
            .map_err(|_| Error::InvalidMessage)?;
        
        // 获取连接对应的 sender
        if let Some(sender) = self.conn_manager.get_sender(&conn_id).await {
            sender.send(Message::Text(msg))
                .map_err(|e| Error::WsError(e.to_string()))?;
        }
        
        Ok(())
    }

    pub async fn handle_connection(
        self: Arc<Self>,
        socket: WebSocket,
        user_id: Uuid,
    ) {
        let conn_id = Uuid::new_v4();
        let (mut ws_sender, mut ws_receiver) = socket.split();
        let (tx, mut rx) = mpsc::unbounded_channel();
        
        // 创建发送任务
        let send_task = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if ws_sender.send(message).await.is_err() {
                    break;
                }
            }
        });
        
        // 添加到连接管理器
        self.conn_manager.add_connection(conn_id, user_id, tx.clone()).await;
    
        // 发送欢迎消息
        let welcome_msg = ServerMessage {
            msg_id: Uuid::new_v4(),
            code: 0,
            data: Some(json!({
                "conn_id": conn_id,
                "message": "Connected successfully"
            })),
            error: None,
        };
    
        let _ = self.send_message(conn_id, &welcome_msg).await;
    
        // 处理接收消息
        while let Some(Ok(message)) = ws_receiver.next().await {
            match message {
                Message::Text(text) => {
                    if let Err(e) = self.handle_message(conn_id, &text).await {
                        let error_msg = ServerMessage {
                            msg_id: Uuid::new_v4(),
                            code: e.code(),
                            data: None,
                            error: Some(e.to_string()),
                        };
                        let _ = self.send_message(conn_id, &error_msg).await;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    
        // 清理连接
        self.conn_manager.remove_connection(&conn_id).await;
        send_task.abort();
    }

    // 开始匹配
    async fn handle_match_start(&self, conn_id: Uuid, msg: ClientMessage) -> Result<()> {
        // 解析匹配类型 (1v1, 2v2, 5v5)
        let match_type: String = serde_json::from_value(msg.data)
            .map_err(|_| Error::InvalidMessage)?;
        
        let state = self.conn_manager.get_connection(&conn_id)
            .await
            .ok_or(Error::ConnectionNotFound)?;
        
        // 加入对应的匹配池
        let match_result = self.match_service.clone().join_match(
            state.user_id,
            &match_type
        ).await?;

        let response = ServerMessage {
            msg_id: msg.msg_id,
            code: 0,
            data: Some(json!({
                "match_id": match_result.match_id,
                "status": match_result.status,
                "type": match_type,
                "current_players": match_result.current_players,
                "required_players": match_result.required_players
            })),
            error: None,
        };
        
        self.send_message(conn_id, &response).await
    }

    // 取消匹配
    async fn handle_match_cancel(&self, conn_id: Uuid, msg: ClientMessage) -> Result<()> {
        let state = self.conn_manager.get_connection(&conn_id)
            .await
            .ok_or(Error::ConnectionNotFound)?;
        
        if let Some(match_id) = state.match_id {
            // 从匹配池中移除
            self.match_service.leave_match(state.user_id, match_id).await?;
        }

        let response = ServerMessage {
            msg_id: msg.msg_id,
            code: 0,
            data: Some(json!({
                "status": "cancelled"
            })),
            error: None,
        };
        
        self.send_message(conn_id, &response).await
    }

    // 处理心跳检测
    async fn handle_ping(&self, conn_id: Uuid, msg: ClientMessage) -> Result<()> {
        // 检查比赛状态
        let state = self.conn_manager.get_connection(&conn_id)
            .await
            .ok_or(Error::ConnectionNotFound)?;
        
        // 如果在比赛中，检查比赛进度
        let match_status = if let Some(match_id) = state.match_id {
            let status = self.match_service.get_match_status(match_id).await?;
            Some(status)
        } else {
            None
        };

        let response = ServerMessage {
            msg_id: msg.msg_id,
            code: 0,
            data: Some(json!({
                "time": chrono::Utc::now(),
                "match_status": match_status
            })),
            error: None,
        };
        
        self.send_message(conn_id, &response).await
    }

    async fn handle_message(&self, conn_id: Uuid, text: &str) -> Result<()> {
        let client_msg: ClientMessage = serde_json::from_str(text)
            .map_err(|_| Error::InvalidMessage)?;

        match client_msg.cmd.as_str() {
            "match.start" => self.handle_match_start(conn_id, client_msg).await,
            "match.cancel" => self.handle_match_cancel(conn_id, client_msg).await,
            "sys.ping" => self.handle_ping(conn_id, client_msg).await,
            _ => Err(Error::InvalidMessage),
        }
    }

}
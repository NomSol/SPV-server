use std::collections::HashMap;
use std::sync::Arc;
use axum::extract::ws::Message;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ClientState {
    pub user_id: Uuid,
    pub match_id: Option<Uuid>,
    pub sender: mpsc::UnboundedSender<Message>,
}

#[derive(Clone)]
pub struct ConnectionManager {
    connections: Arc<RwLock<HashMap<Uuid, ClientState>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_sender(&self, conn_id: &Uuid) -> Option<mpsc::UnboundedSender<Message>> {
        let connections = self.connections.read().await;
        connections.get(conn_id).map(|state| state.sender.clone())
    }

    pub async fn add_connection(&self, conn_id: Uuid, user_id: Uuid, sender: mpsc::UnboundedSender<Message>) {
        let state = ClientState {
            user_id,
            match_id: None,
            sender,
        };
        
        let mut connections = self.connections.write().await;
        connections.insert(conn_id, state);
    }

    pub async fn remove_connection(&self, conn_id: &Uuid) {
        let mut connections = self.connections.write().await;
        connections.remove(conn_id);
    }

    pub async fn get_connection(&self, conn_id: &Uuid) -> Option<ClientState> {
        let connections = self.connections.read().await;
        connections.get(conn_id).cloned()
    }

    // 添加按匹配ID查找连接的方法，为广播做准备
    pub async fn get_connections_by_match(&self, match_id: Uuid) -> Vec<Uuid> {
        let connections = self.connections.read().await;
        
        connections.iter()
            .filter_map(|(conn_id, state)| {
                if state.match_id == Some(match_id) {
                    Some(*conn_id)
                } else {
                    None
                }
            })
            .collect()
    }
    
    // 添加更新连接匹配ID的方法
    pub async fn update_match_id(&self, conn_id: &Uuid, match_id: Option<Uuid>) {
        let mut connections = self.connections.write().await;
        
        if let Some(state) = connections.get_mut(conn_id) {
            state.match_id = match_id;
        }
    }
}
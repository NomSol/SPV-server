use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientMessage {
    pub msg_id: Uuid,
    pub cmd: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ServerMessage {
    pub msg_id: Uuid,
    pub code: i32,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}
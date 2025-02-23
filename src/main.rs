use std::sync::Arc;
use axum::{
    Router,
    routing::get,
    extract::{WebSocketUpgrade, Query},
    response::Response,
};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

mod config;
mod models;
mod error;
mod db;
mod gateway;
mod matchmaking;

use gateway::WebSocketHandler;
use crate::matchmaking::service::MatchService;

#[tokio::main]
async fn main() {
    // 初始化日志
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("debug"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 创建匹配服务
    let match_service = MatchService::new();
    
    // 创建WebSocket处理器
    let ws_handler = Arc::new(WebSocketHandler::new(match_service.clone()));

    // 创建路由
    let app = Router::new()
        .route("/ws", get(move |ws, params| ws_handler_fn(ws, params, ws_handler.clone())));

    // 启动服务器
    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("Server running on http://127.0.0.1:3000");
    
    axum::serve(listener, app).await.unwrap();
}

async fn ws_handler_fn(
    ws: WebSocketUpgrade,
    Query(params): Query<std::collections::HashMap<String, String>>,
    ws_handler: Arc<WebSocketHandler>,
) -> Response {
    // 这里应该添加token验证
    // 临时使用一个测试用户ID
    let user_id = Uuid::new_v4();
    
    ws.on_upgrade(move |socket| async move {
        ws_handler.handle_connection(socket, user_id).await;
    })
}
use std::sync::Arc;
use axum::{
    Router,
    routing::{get, get_service},
    extract::{WebSocketUpgrade, Query, State, ws::Message},
    response::{Response, IntoResponse},
    http::{Request, StatusCode},
};
use tower_http::{
    services::ServeDir,
    cors::{CorsLayer, Any},
};
use uuid::Uuid;
use std::collections::HashMap;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use dotenv::dotenv;

mod config;
mod error;
mod models;
mod db;
mod gateway;
mod matchmaking;

use gateway::handler::WebSocketHandler;
use gateway::state::ConnectionManager;
use matchmaking::service::MatchService;

#[tokio::main]
async fn main() {
    // Load environment variables
    dotenv().ok();
    
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into())
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();
    
    // Create matchmaking service
    let match_service = MatchService::new();
    
    // Create WebSocket handler
    let ws_handler = Arc::new(WebSocketHandler::new(match_service.clone()));
    
    // Create connection manager
    let conn_manager = ConnectionManager::new();
    
    // Create a CORS layer
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);
    
    // Create app state
    let app_state = AppState {
        ws_handler: ws_handler.clone(),
        conn_manager: conn_manager.clone(),
    };
    
    // Build the router
    let app = Router::new()
        .route("/ws", get(ws_handler_fn))
        .nest_service("/test", get_service(ServeDir::new("static")))
        .layer(cors)
        .with_state(app_state);
    
    // Get port from environment or use default
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3000);
    
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    
    tracing::info!("Starting server on {}", addr);
    
    // Start the server
    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// App state for sharing handlers
#[derive(Clone)]
struct AppState {
    ws_handler: Arc<WebSocketHandler>,
    conn_manager: ConnectionManager,
}

// WebSocket handler function
async fn ws_handler_fn(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    // In a real app, you'd validate a token here
    // For testing, we'll use a simple user_id parameter
    let user_id = params
        .get("user_id")
        .map(|id| Uuid::parse_str(id).unwrap_or_else(|_| Uuid::new_v4()))
        .unwrap_or_else(Uuid::new_v4);
    
    tracing::info!("WebSocket connection from user: {}", user_id);
    
    // Upgrade the connection
    ws.on_upgrade(move |socket| async move {
        state.ws_handler.handle_connection(socket, user_id).await;
    })
}
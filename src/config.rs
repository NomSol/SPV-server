use std::sync::Arc;
use dotenv::dotenv;

#[derive(Debug, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub hasura: HasuraConfig,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct HasuraConfig {
    pub endpoint: String,
    pub admin_secret: String,
}

impl Config {
    pub fn load() -> Self {
        // Load .env file if present
        dotenv().ok();
        
        // Load server configuration
        let host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = std::env::var("SERVER_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()
            .unwrap_or(3000);
        
        // Load Hasura configuration
        let endpoint = std::env::var("NEXT_PUBLIC_HASURA_ENDPOINT")
            .expect("NEXT_PUBLIC_HASURA_ENDPOINT environment variable not set");
            
        let admin_secret = std::env::var("NEXT_PUBLIC_HASURA_ADMIN_SECRET")
            .expect("NEXT_PUBLIC_HASURA_ADMIN_SECRET environment variable not set");
        
        Self {
            server: ServerConfig { host, port },
            hasura: HasuraConfig { endpoint, admin_secret },
        }
    }
}
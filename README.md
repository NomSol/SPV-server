# PVP Mode-server
Demo Server to handle pvp mode

![serverTest](assets/serverTest.png)

## Architecture

```
NOM-server/
├── Cargo.toml
├── .env                            # Environment variables
├── src/
│   ├── main.rs                     # Program entry point
│   ├── config/
│   │   └── mod.rs                  # Configuration management
│   ├── error.rs                    # Error handling
│   ├── gateway/                    # WebSocket gateway
│   │   ├── mod.rs
│   │   ├── handler.rs              # Connection handler
│   │   └── connection_manager.rs   # Connection state management
│   ├── matchmaking/                # Matchmaking service
│   │   ├── mod.rs
│   │   └── service.rs              # Matchmaking logic
│   ├── models/                     # Data models
│   │   ├── mod.rs
│   │   ├── message.rs              # Message definitions
│   │   └── game.rs                 # Game-related structures
│   └── db/                         # Database interaction
│       ├── mod.rs
│       ├── hasura_client.rs        # Hasura GraphQL client
│       └── hasura_match_repository.rs # Match-related database operations
└── README.md
```

## Implemented Features

### WebSocket Gateway
	•	WebSocket connection management
	•	Message routing
	•	Connection state tracking
	•	Heartbeat detection

### Match System
	•	Multiple match modes (1v1, 2v2, 5v5)
	•	Room pool management
	•	Dynamic room creation and recycling
	•	Player join/leave management

### Message Protocol
```json
// Client to Server (C2S)
{
    "msg_id": "uuid-string",
    "cmd": "command-string",
    "data": "command-data"
}

// Server to Client (S2C)
{
    "msg_id": "uuid-string",
    "code": 0,
    "data": {},
    "error": null
}
```

## Development Guide

### Environment Requirements
- Rust 1.75+

### Dependency Configuration
```toml
[dependencies]
# Async runtime and WebSocket server; futures-util is used for handling WebSocket read/write streams
tokio = { version = "1.36.0", features = ["full"] }
axum = { version = "0.7.4", features = ["ws"] }
futures-util = "0.3.30"

# Database
sqlx = { version = "0.7.3", features = ["runtime-tokio-rustls", "postgres", "uuid"] }

# Utility Libraries
serde = { version = "1.0.196", features = ["derive"] }
serde_json = "1.0.113"
jsonwebtoken = "9.2.0"
chrono = { version = "0.4.33", features = ["serde"] }
uuid = { version = "1.7.0", features = ["v4", "serde"] }

# Error Handling and Logging
thiserror = "1.0.56"
tracing = "0.1.40"
tracing-subscriber = { version = "0.3.18", features = ["env-filter"] }
```

### Run the Server
```bash
cargo run
```

### Testing
	1.	Run the server.
	2.	Open test.html to test WebSocket functionality.
	3.	Use multiple clients to test matchmaking features.

## Upcoming Features
	1.	Database Integration
	•	User authentication
	•	Match records
	•	Score tracking
	2.	Game Logic
	•	Treasure generation
	•	Score calculation
	•	Match settlement
	3.	System Optimization
	•	Improved error handling
	•	Logging system
	•	Monitoring metrics

## Protocol Commands

Currently supported commands:
	•	match.start: Start matchmaking
	•	match.cancel: Cancel matchmaking
	•	sys.ping: Heartbeat check

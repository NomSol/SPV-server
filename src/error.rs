use axum::response::Response;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Authentication failed")]
    AuthError,
    #[error("Invalid message format")]
    InvalidMessage,
    #[error("WebSocket error: {0}")]
    WsError(String),
    #[error("Database error: {0}")]
    DbError(String),
    #[error("Connection not found")]
    ConnectionNotFound,
    #[error("We didn't find that match")]
    MatchNotFound,
    #[error("The match type is not right")]
    InvalidMatchType,
    #[error("The match is not ready")]
    MatchNotReady,
    #[error("You have already joined a match")]
    UserAlreadyInMatch,
    #[error("Your match has already started, so you can't leave")]
    MatchAlreadyStarted,
}

impl Error {
    pub fn code(&self) -> i32 {
        match self {
            Error::AuthError => 1001,
            Error::InvalidMessage => 1002,
            Error::WsError(_) => 1003,
            Error::DbError(_) => 1004,
            Error::ConnectionNotFound => 1005,
            Error::MatchNotFound => 1006,
            Error::InvalidMatchType => 1007,
            Error::MatchNotReady => 1008,
            Error::UserAlreadyInMatch => 1009,
            Error::MatchAlreadyStarted => 1010,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
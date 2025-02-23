use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchType {
    OneVsOne,
    TwoVsTwo,
    FiveVsFive,
}

impl MatchType {
    pub fn team_size(&self) -> i32 {
        match self {
            MatchType::OneVsOne => 1,
            MatchType::TwoVsTwo => 2,
            MatchType::FiveVsFive => 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerPosition {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchResult {
    pub match_id: Uuid,
    pub status: String,
    pub match_type: String,
    pub current_players: i32,
    pub required_players: i32,
}

#[derive(Debug, Clone)]
pub struct MatchRoom {
    pub id: Uuid,
    pub match_type: String,
    pub required_players: i32,
    pub current_players: i32,
    pub players: Vec<Uuid>,
    pub status: String,
}
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
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "1v1" => Some(MatchType::OneVsOne),
            "2v2" => Some(MatchType::TwoVsTwo),
            "5v5" => Some(MatchType::FiveVsFive),
            _ => None,
        }
    }
    
    pub fn to_str(&self) -> &'static str {
        match self {
            MatchType::OneVsOne => "1v1",
            MatchType::TwoVsTwo => "2v2",
            MatchType::FiveVsFive => "5v5",
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
    pub required_players: i32,
    pub current_players: i32,
    pub players: Vec<Uuid>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct MatchMember {
    pub user_id: Uuid,
    pub score: i32,
}

#[derive(Debug, Clone)]
pub struct MatchTeam {
    pub id: Uuid,
    pub team_number: i32,
    pub members: Vec<MatchMember>,
    pub total_score: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchDetails {
    pub id: Uuid,
    pub match_type: String,
    pub status: String,
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    pub teams: Vec<TeamDetails>,
    pub duration: Option<std::time::Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamDetails {
    pub id: Uuid,
    pub team_number: i32,
    pub members: Vec<MemberDetails>,
    pub total_score: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberDetails {
    pub user_id: Uuid,
    pub nickname: String,
    pub avatar_url: String,
    pub score: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasureDiscovery {
    pub match_id: Uuid,
    pub team_id: Uuid,
    pub user_id: Uuid,
    pub treasure_id: Uuid,
    pub score: i32,
}
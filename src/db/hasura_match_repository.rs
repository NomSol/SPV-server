use std::sync::Arc;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use chrono::{DateTime, Utc};

use crate::error::{Error, Result};
use crate::models::game::{MatchRoom, MatchTeam, MatchMember, MatchDetails, TeamDetails, MemberDetails};

use super::hasura_client::HasuraClient;

pub struct HasuraMatchRepository {
    client: Arc<HasuraClient>,
}

#[derive(Debug, Deserialize)]
struct MatchInsertResponse {
    insert_treasure_matches_one: MatchData,
}

#[derive(Debug, Deserialize)]
struct TeamInsertResponse {
    insert_match_teams_one: TeamData,
}

#[derive(Debug, Deserialize)]
struct MemberInsertResponse {
    insert_match_members_one: MemberData,
}

#[derive(Debug, Deserialize)]
struct DiscoveryInsertResponse {
    insert_match_discoveries_one: DiscoveryData,
}

#[derive(Debug, Deserialize)]
struct MatchUpdateResponse {
    update_treasure_matches_by_pk: Option<MatchData>,
}

#[derive(Debug, Deserialize)]
struct MatchQueryResponse {
    treasure_matches_by_pk: Option<MatchData>,
}

#[derive(Debug, Deserialize)]
struct TeamsQueryResponse {
    match_teams: Vec<TeamData>,
}

#[derive(Debug, Deserialize)]
struct UserInMatchResponse {
    match_members: Vec<UserMatchData>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MatchData {
    id: Uuid,
    match_type: String,
    status: String,
    required_players_per_team: i32,
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
    winner_team_id: Option<Uuid>,
    match_teams: Option<Vec<TeamData>>,
    match_members: Option<Vec<MemberData>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TeamData {
    id: Uuid,
    team_number: i32,
    current_players: i32,
    max_players: i32,
    total_score: i32,
    match_members: Option<Vec<MemberWithUserData>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MemberData {
    id: Uuid,
    user_id: Uuid,
    #[serde(default)]
    individual_score: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct MemberWithUserData {
    id: Uuid,
    user_id: Uuid,
    individual_score: i32,
    user: UserData,
}

#[derive(Debug, Serialize, Deserialize)]
struct UserData {
    id: Uuid,
    nickname: String,
    avatar_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiscoveryData {
    id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
struct UserMatchData {
    match_id: Uuid,
}

impl HasuraMatchRepository {
    pub async fn new() -> Result<Self> {
        let client = HasuraClient::get_instance().await?;
        Ok(Self { client })
    }
    
    // Create a new match in the database
    pub async fn create_match(&self, match_id: Uuid, match_type: &str, required_players_per_team: i32) -> Result<Uuid> {
        let mutation = r#"
            mutation CreateMatch($id: uuid!, $match_type: String!, $required_players: Int!) {
                insert_treasure_matches_one(object: {
                    id: $id,
                    match_type: $match_type,
                    status: "matching",
                    required_players_per_team: $required_players
                }) {
                    id
                    match_type
                    status
                    required_players_per_team
                }
            }
        "#;
        
        let variables = json!({
            "id": match_id,
            "match_type": match_type,
            "required_players": required_players_per_team
        });
        
        let response: MatchInsertResponse = self.client.mutate(mutation, variables).await?;
        Ok(response.insert_treasure_matches_one.id)
    }
    
    // Create a team for a match
    pub async fn create_team(&self, team_id: Uuid, match_id: Uuid, team_number: i32, max_players: i32) -> Result<Uuid> {
        let mutation = r#"
            mutation CreateTeam($id: uuid!, $match_id: uuid!, $team_number: Int!, $max_players: Int!) {
                insert_match_teams_one(object: {
                    id: $id,
                    match_id: $match_id,
                    team_number: $team_number,
                    max_players: $max_players,
                    current_players: 0
                    total_score: 0
                }) {
                    id
                    team_number
                    current_players
                    max_players
                    total_score
                }
            }
        "#;
        
        let variables = json!({
            "id": team_id,
            "match_id": match_id,
            "team_number": team_number,
            "max_players": max_players
        });
        
        let response: TeamInsertResponse = self.client.mutate(mutation, variables).await?;
        Ok(response.insert_match_teams_one.id)
    }
    
    // Add a player to a team
    pub async fn add_player_to_team(&self, match_id: Uuid, team_id: Uuid, user_id: Uuid) -> Result<Uuid> {
        // 插入队员记录
        let mutation = r#"
            mutation AddPlayerToTeam($match_id: uuid!, $team_id: uuid!, $user_id: uuid!) {
                insert_match_members_one(object: {
                    match_id: $match_id,
                    team_id: $team_id,
                    user_id: $user_id,
                    individual_score: 0
                }) {
                    id
                    user_id
                }
            }
        "#;
        
        let variables = json!({
            "match_id": match_id,
            "team_id": team_id,
            "user_id": user_id
        });
        
        let response: MemberInsertResponse = self.client.mutate(mutation, variables).await?;
        
        // 首先获取当前team信息
        let query = r#"
            query GetTeamInfo($team_id: uuid!) {
                match_teams_by_pk(id: $team_id) {
                    current_players
                    max_players
                }
            }
        "#;
        
        let query_variables = json!({
            "team_id": team_id
        });
        
        #[derive(Debug, Deserialize)]
        struct TeamInfoResponse {
            match_teams_by_pk: TeamInfo,
        }
        
        #[derive(Debug, Deserialize)]
        struct TeamInfo {
            current_players: i32,
            max_players: i32,
        }
        
        let team_info: TeamInfoResponse = self.client.query(query, query_variables).await?;
        
        // 计算新的玩家数量，确保不超过最大值
        let current = team_info.match_teams_by_pk.current_players;
        let max = team_info.match_teams_by_pk.max_players;
        let new_count = std::cmp::min(current + 1, max);
        
        // 使用直接设置的方式更新
        let update_mutation = r#"
            mutation UpdateTeamDirectly($team_id: uuid!, $current_players: Int!) {
                update_match_teams_by_pk(
                    pk_columns: {id: $team_id},
                    _set: {current_players: $current_players}
                ) {
                    id
                    current_players
                }
            }
        "#;
        
        let update_variables = json!({
            "team_id": team_id,
            "current_players": new_count
        });
        
        self.client.mutate::<Value>(update_mutation, update_variables).await?;
        
        Ok(response.insert_match_members_one.id)
    }

    // Start a match
    pub async fn start_match(&self, match_id: Uuid) -> Result<()> {
        let now = chrono::Utc::now();
        let now_iso = now.to_rfc3339();
        
        // 简化查询，确保语法正确
        let mutation = r#"
            mutation StartMatch($id: uuid!, $start_time: timestamptz!) {
                update_treasure_matches_by_pk(
                    pk_columns: {id: $id},
                    _set: {status: "playing", start_time: $start_time}
                ) {
                    id
                    status
                    match_type  # 确保包含所有需要的字段
                    required_players_per_team
                    start_time
                }
            }
        "#;
        
        let variables = json!({
            "id": match_id,
            "start_time": now_iso
        });
        
        println!("开始匹配: {} 状态设为playing, 时间: {}", match_id, now_iso);
        
        let response: MatchUpdateResponse = self.client.mutate(mutation, variables).await?;
        
        if response.update_treasure_matches_by_pk.is_none() {
            println!("错误: 匹配不存在");
            return Err(Error::MatchNotFound);
        }
        
        println!("匹配成功开始");
        
        Ok(())
    }
    
    // Record a treasure discovery
    pub async fn record_discovery(&self, match_id: Uuid, team_id: Uuid, user_id: Uuid, treasure_id: Uuid, score: i32) -> Result<Uuid> {
        // Create discovery record
        let mutation = r#"
            mutation RecordDiscovery($match_id: uuid!, $team_id: uuid!, $user_id: uuid!, $treasure_id: uuid!, $score: Int!) {
                insert_match_discoveries_one(object: {
                    match_id: $match_id,
                    team_id: $team_id,
                    user_id: $user_id,
                    treasure_id: $treasure_id,
                    score: $score
                }) {
                    id
                }
            }
        "#;
        
        let variables = json!({
            "match_id": match_id,
            "team_id": team_id,
            "user_id": user_id,
            "treasure_id": treasure_id,
            "score": score
        });
        
        let response: DiscoveryInsertResponse = self.client.mutate(mutation, variables).await?;
        
        // Update individual score
        let update_member_mutation = r#"
            mutation UpdateMemberScore($match_id: uuid!, $user_id: uuid!, $score: Int!) {
                update_match_members(
                    where: {
                        match_id: {_eq: $match_id},
                        user_id: {_eq: $user_id}
                    },
                    _inc: {individual_score: $score}
                ) {
                    affected_rows
                }
            }
        "#;
        
        let update_member_variables = json!({
            "match_id": match_id,
            "user_id": user_id,
            "score": score
        });
        
        self.client.mutate::<Value>(update_member_mutation, update_member_variables).await?;
        
        // Update team score
        let update_team_mutation = r#"
            mutation UpdateTeamScore($team_id: uuid!, $score: Int!) {
                update_match_teams_by_pk(
                    pk_columns: {id: $team_id},
                    _inc: {total_score: $score}
                ) {
                    id
                    total_score
                }
            }
        "#;
        
        let update_team_variables = json!({
            "team_id": team_id,
            "score": score
        });
        
        self.client.mutate::<Value>(update_team_mutation, update_team_variables).await?;
        
        Ok(response.insert_match_discoveries_one.id)
    }
    
    // End a match
    pub async fn end_match(&self, match_id: Uuid) -> Result<()> {
        // Find the winning team
        let query = r#"
            query GetWinningTeam($match_id: uuid!) {
                match_teams(
                    where: {match_id: {_eq: $match_id}},
                    order_by: {total_score: desc},
                    limit: 1
                ) {
                    id
                }
            }
        "#;
        
        let variables = json!({
            "match_id": match_id
        });
        
        println!("查找匹配 {} 的获胜队伍", match_id);
        
        let response: TeamsQueryResponse = self.client.query(query, variables).await?;
        
        if response.match_teams.is_empty() {
            println!("错误: 未找到队伍");
            return Err(Error::MatchNotFound);
        }
        
        let winner_id = response.match_teams[0].id;
        println!("获胜队伍: {}", winner_id);
        
        // 使用ISO格式时间
        let now = chrono::Utc::now();
        let now_iso = now.to_rfc3339();
        
        // Update match status and set winner
        let mutation = r#"
            mutation EndMatch($id: uuid!, $winner_id: uuid!, $end_time: timestamptz!) {
                update_treasure_matches_by_pk(
                    pk_columns: {id: $id},
                    _set: {
                        status: "finished",  // 修改为正确的状态值
                        end_time: $end_time,  // 使用ISO格式时间
                        is_finished: true,
                        winner_team_id: $winner_id
                    }
                ) {
                    id
                    status
                    end_time
                    winner_team_id
                }
            }
        "#;
        
        let variables = json!({
            "id": match_id,
            "winner_id": winner_id,
            "end_time": now_iso
        });
        
        println!("结束匹配 {}, 获胜队伍: {}", match_id, winner_id);
        
        let response: MatchUpdateResponse = self.client.mutate(mutation, variables).await?;
        
        if response.update_treasure_matches_by_pk.is_none() {
            println!("错误: 更新匹配状态时未找到匹配");
            return Err(Error::MatchNotFound);
        }
        
        println!("匹配成功结束, 结果: {:?}", response.update_treasure_matches_by_pk);
        
        Ok(())
    }
    
    // Get match details
    pub async fn get_match(&self, match_id: Uuid) -> Result<MatchRoom> {
        let query = r#"
            query GetMatch($id: uuid!) {
                treasure_matches_by_pk(id: $id) {
                    id
                    match_type
                    status
                    required_players_per_team
                    match_members {
                        user_id
                    }
                }
            }
        "#;
        
        let variables = json!({
            "id": match_id
        });
        
        let response: MatchQueryResponse = self.client.query(query, variables).await?;
        
        let match_data = response.treasure_matches_by_pk
            .ok_or(Error::MatchNotFound)?;
        
        // Extract player IDs
        let players = match_data.match_members.map_or_else(Vec::new, |members| {
            members.into_iter().map(|m| m.user_id).collect()
        });
        
        Ok(MatchRoom {
            id: match_data.id,
            required_players: match_data.required_players_per_team * 2,
            current_players: players.len() as i32,
            players,
            status: match_data.status,
        })
    }
    
    // Get teams for a match
    pub async fn get_match_teams(&self, match_id: Uuid) -> Result<Vec<MatchTeam>> {
        let query = r#"
            query GetMatchTeams($match_id: uuid!) {
                match_teams(
                    where: {match_id: {_eq: $match_id}},
                    order_by: {team_number: asc}
                ) {
                    id
                    team_number
                    current_players
                    total_score
                    match_members {
                        id
                        user_id
                        individual_score
                        user {
                            id
                            nickname
                            avatar_url
                        }
                    }
                }
            }
        "#;
        
        let variables = json!({
            "match_id": match_id
        });
        
        let response: TeamsQueryResponse = self.client.query(query, variables).await?;
        
        let teams = response.match_teams.into_iter().map(|team| {
            let members = team.match_members.unwrap_or_default().into_iter().map(|m| {
                MatchMember {
                    user_id: m.user_id,
                    score: m.individual_score,
                }
            }).collect();
            
            MatchTeam {
                id: team.id,
                team_number: team.team_number,
                members,
                total_score: team.total_score,
            }
        }).collect();
        
        Ok(teams)
    }
    
    // Get match details with user info
    pub async fn get_match_details(&self, match_id: Uuid) -> Result<MatchDetails> {
        let query = r#"
            query GetMatchDetails($id: uuid!) {
                treasure_matches_by_pk(id: $id) {
                    id
                    match_type
                    status
                    start_time
                    end_time
                    match_teams(order_by: {team_number: asc}) {
                        id
                        team_number
                        total_score
                        match_members {
                            id
                            user_id
                            individual_score
                            user {
                                id
                                nickname
                                avatar_url
                            }
                        }
                    }
                }
            }
        "#;
        
        let variables = json!({
            "id": match_id
        });
        
        let response: MatchQueryResponse = self.client.query(query, variables).await?;
        
        let match_data = response.treasure_matches_by_pk
            .ok_or(Error::MatchNotFound)?;
        
        // Calculate duration
        let duration = match (match_data.start_time, match_data.end_time) {
            (Some(start), Some(end)) => Some(std::time::Duration::from_secs(
                (end - start).num_seconds() as u64
            )),
            (Some(start), None) => Some(std::time::Duration::from_secs(
                (Utc::now() - start).num_seconds() as u64
            )),
            _ => None,
        };
        
        // Transform team data
        let teams = match_data.match_teams.unwrap_or_default().into_iter().map(|team| {
            let members = team.match_members.unwrap_or_default().into_iter().map(|m| {
                MemberDetails {
                    user_id: m.user_id,
                    nickname: m.user.nickname,
                    avatar_url: m.user.avatar_url,
                    score: m.individual_score,
                }
            }).collect();
            
            TeamDetails {
                id: team.id,
                team_number: team.team_number,
                members,
                total_score: team.total_score,
            }
        }).collect();
        
        Ok(MatchDetails {
            id: match_data.id,
            match_type: match_data.match_type,
            status: match_data.status,
            start_time: match_data.start_time,
            teams,
            duration,
        })
    }
    
    pub async fn is_user_in_match(&self, user_id: Uuid) -> Result<Option<Uuid>> {
        // First, get all match IDs for this user
        let query = r#"
            query IsUserInMatch($user_id: uuid!) {
                match_members(
                    where: {
                        user_id: {_eq: $user_id}
                    }
                ) {
                    match_id
                }
            }
        "#;
        
        let variables = json!({
            "user_id": user_id
        });
        
        let response: UserInMatchResponse = self.client.query(query, variables).await?;
        
        if response.match_members.is_empty() {
            return Ok(None);
        }
        
        // Get all match IDs
        let match_ids: Vec<Uuid> = response.match_members.iter().map(|m| m.match_id).collect();
        
        // Now check if any of these matches are active
        let active_match_query = r#"
            query GetActiveMatches($match_ids: [uuid!]) {
                treasure_matches(
                    where: {
                        id: {_in: $match_ids},
                        status: {_in: ["matching", "in_progress"]}
                    },
                    limit: 1
                ) {
                    id
                }
            }
        "#;
        
        #[derive(Debug, Deserialize)]
        struct ActiveMatchResponse {
            treasure_matches: Vec<MatchData>,
        }
        
        #[derive(Debug, Deserialize)]
        struct MatchData {
            id: Uuid,
        }
        
        let active_match_variables = json!({
            "match_ids": match_ids
        });
        
        let active_match_response: ActiveMatchResponse = self.client.query(active_match_query, active_match_variables).await?;
        
        if active_match_response.treasure_matches.is_empty() {
            return Ok(None);
        }
        
        Ok(Some(active_match_response.treasure_matches[0].id))
    }
}
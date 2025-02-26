use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use uuid::Uuid;
use rand::seq::SliceRandom;
use rand::thread_rng;

use crate::error::{Error, Result};
use crate::models::game::{MatchResult, MatchRoom};
use crate::db::hasura_match_repository::HasuraMatchRepository;

pub struct MatchService {
    match_pools: Arc<RwLock<HashMap<String, Vec<MatchRoom>>>>,
    min_room_count: HashMap<String, usize>,
    repo_cell: Arc<tokio::sync::OnceCell<Arc<HasuraMatchRepository>>>,
}

impl MatchService {
    pub fn new() -> Arc<Self> {
        // Create a shared repository
        let repo_cell = Arc::new(tokio::sync::OnceCell::new());
        let repo_cell_clone = repo_cell.clone();
        
        // Create the service
        let service = Arc::new(Self {
            match_pools: Arc::new(RwLock::new(HashMap::new())),
            min_room_count: HashMap::from([
                ("1v1".to_string(), 5),
                ("2v2".to_string(), 3),
                ("5v5".to_string(), 2),
            ]),
            repo_cell,
        });
        
        // Clone for init task
        let service_clone = service.clone();
        
        // Initialize in background
        tokio::spawn(async move {
            // Initialize DB connection
            match HasuraMatchRepository::new().await {
                Ok(repo) => {
                    let _ = repo_cell_clone.set(Arc::new(repo));
                }
                Err(e) => {
                    eprintln!("Failed to initialize match repository: {:?}", e);
                }
            }
            
            // Initialize match pools
            if let Err(e) = service_clone.initialize_pools().await {
                eprintln!("Failed to initialize pools: {:?}", e);
            }
        });
        
        service
    }

    fn get_repo(&self) -> Option<Arc<HasuraMatchRepository>> {
        self.repo_cell.get().cloned()
    }

    // Initialize match pools
    async fn initialize_pools(&self) -> Result<()> {
        let mut pools = self.match_pools.write().await;
        
        for (match_type, &min_count) in &self.min_room_count {
            let pool = pools.entry(match_type.clone())
                .or_insert_with(Vec::new);
            
            // Create initial rooms
            while pool.len() < min_count {
                pool.push(MatchRoom {
                    id: Uuid::new_v4(),
                    required_players: self.get_required_players(match_type)?,
                    current_players: 0,
                    players: Vec::new(),
                    status: "matching".to_string(),
                });
            }
        }
        
        Ok(())
    }

    // Get required players for a match type
    fn get_required_players(&self, match_type: &str) -> Result<i32> {
        match match_type {
            "1v1" => Ok(2),
            "2v2" => Ok(4),
            "5v5" => Ok(10),
            _ => Err(Error::InvalidMatchType),
        }
    }

    // Join a match
    pub async fn join_match(self: Arc<Self>, user_id: Uuid, match_type: &str) -> Result<MatchResult> {
        // Check if user is already in a match
        if let Some(repo) = &self.get_repo() {
            if let Some(_active_match) = repo.is_user_in_match(user_id).await? {
                return Err(Error::UserAlreadyInMatch);
            }
        }
        
        let mut pools = self.match_pools.write().await;
        
        // Get or create match pool
        let pool = pools.entry(match_type.to_string())
            .or_insert_with(Vec::new);
        
        // Get required players
        let required_players = self.get_required_players(match_type)?;

        // Find an available room
        if let Some(room) = pool.iter_mut().find(|r| 
            r.status == "matching" && 
            r.current_players < r.required_players && 
            !r.players.contains(&user_id)
        ) {
            room.players.push(user_id);
            room.current_players += 1;

            // Check if room is full
            if room.current_players == room.required_players {
                room.status = "ready".to_string();
                
                // Clone room ID for async call
                let match_id = room.id;
                
                // Clone the Arc for the background task
                let match_service = self.clone();
                tokio::spawn(async move {
                    if let Err(e) = match_service.start_match(match_id).await {
                        eprintln!("Failed to start match {}: {:?}", match_id, e);
                    }
                });
            }

            return Ok(MatchResult {
                match_id: room.id,
                status: room.status.clone(),
                match_type: match_type.to_string(),
                current_players: room.current_players,
                required_players: room.required_players,
            });
        }

        // Create new room if none available
        let new_room = MatchRoom {
            id: Uuid::new_v4(),
            required_players,
            current_players: 1,
            players: vec![user_id],
            status: "matching".to_string(),
        };

        let result = MatchResult {
            match_id: new_room.id,
            status: new_room.status.clone(),
            match_type: match_type.to_string(),
            current_players: new_room.current_players,
            required_players: new_room.required_players,
        };

        pool.push(new_room);
        Ok(result)
    }

    // Leave a match
    pub async fn leave_match(&self, user_id: Uuid, match_id: Uuid) -> Result<()> {
        let mut pools = self.match_pools.write().await;
        
        for (match_type, pool) in pools.iter_mut() {
            if let Some(index) = pool.iter().position(|r| r.id == match_id) {
                let room = &mut pool[index];
                
                // Only allow leaving if match hasn't started
                if room.status != "matching" {
                    return Err(Error::MatchAlreadyStarted);
                }
                
                if let Some(player_index) = room.players.iter().position(|&p| p == user_id) {
                    room.players.remove(player_index);
                    room.current_players -= 1;
                    
                    // Recycle empty rooms if above minimum count
                    if room.current_players == 0 {
                        let min_count = self.min_room_count.get(match_type).unwrap_or(&0);
                        let empty_rooms = pool.iter()
                            .filter(|r| r.current_players == 0)
                            .count();
                        
                        if empty_rooms > *min_count {
                            pool.remove(index);
                        }
                    }
                }
                return Ok(());
            }
        }
        
        Err(Error::MatchNotFound)
    }

    // Get match status
    pub async fn get_match_status(&self, match_id: Uuid) -> Result<String> {
        // First check in-memory pools
        let pools = self.match_pools.read().await;
        
        for pool in pools.values() {
            if let Some(room) = pool.iter().find(|r| r.id == match_id) {
                return Ok(room.status.clone());
            }
        }
        
        // If not found in memory, check database
        if let Some(repo) = &self.get_repo() {
            match repo.get_match(match_id).await {
                Ok(room) => return Ok(room.status),
                Err(Error::MatchNotFound) => return Err(Error::MatchNotFound),
                Err(_) => {} // Continue to next check if DB error
            }
        }
        
        Err(Error::MatchNotFound)
    }

    // Start a match
    pub async fn start_match(&self, match_id: Uuid) -> Result<()> {
        // Find match room
        let mut match_type = String::new();
        let mut match_room = None;
        
        {
            let pools = self.match_pools.read().await;
            
            for (type_name, pool) in pools.iter() {
                if let Some(room) = pool.iter().find(|r| r.id == match_id) {
                    match_type = type_name.clone();
                    match_room = Some(room.clone());
                    break;
                }
            }
        }
        
        // Room not found
        let room = match match_room {
            Some(r) => r,
            None => return Err(Error::MatchNotFound),
        };
        
        // Verify room is ready
        if room.status != "ready" {
            return Err(Error::MatchNotReady);
        }
        
        // Proceed with starting the match if we have a repository
        if let Some(repo) = &self.get_repo() {
            // Calculate players per team
            let players_per_team = room.required_players / 2;
            
            // 1. Create match record in database
            repo.create_match(match_id, &match_type, players_per_team).await?;
            
            // 2. Create teams
            let team1_id = Uuid::new_v4();
            let team2_id = Uuid::new_v4();
            
            repo.create_team(team1_id, match_id, 1, players_per_team).await?;
            repo.create_team(team2_id, match_id, 2, players_per_team).await?;
            
            // 3. Randomly assign players to teams
            let mut players = room.players.clone();
            players.shuffle(&mut thread_rng());
            
            // Split players into two teams
            let team1_players = &players[0..players_per_team as usize];
            let team2_players = &players[players_per_team as usize..];
            
            // Add Team 1 members
            for &player_id in team1_players {
                repo.add_player_to_team(match_id, team1_id, player_id).await?;
            }
            
            // Add Team 2 members
            for &player_id in team2_players {
                repo.add_player_to_team(match_id, team2_id, player_id).await?;
            }
            
            // 4. Start the match
            repo.start_match(match_id).await?;
        }
        
        // Update in-memory state
        {
            let mut pools = self.match_pools.write().await;
            if let Some(pool) = pools.get_mut(&match_type) {
                if let Some(room) = pool.iter_mut().find(|r| r.id == match_id) {
                    room.status = "in_progress".to_string();
                }
            }
        }
        
        Ok(())
    }
    
    // End a match
    pub async fn end_match(&self, match_id: Uuid) -> Result<()> {
        // Update in-memory state first
        {
            let pools = self.match_pools.read().await;
            for (match_type, pool) in pools.iter() {
                if let Some(_) = pool.iter().find(|r| r.id == match_id) {
                    // Found the match, remove it after updating DB
                    let mut pools = self.match_pools.write().await;
                    if let Some(pool) = pools.get_mut(match_type) {
                        pool.retain(|r| r.id != match_id);
                    }
                    break;
                }
            }
        }
        
        // Update database
        if let Some(repo) = &self.get_repo() {
            repo.end_match(match_id).await?;
        }
        
        Ok(())
    }
    
    // Record treasure discovery
    pub async fn record_discovery(&self, match_id: Uuid, team_id: Uuid, user_id: Uuid, treasure_id: Uuid, score: i32) -> Result<()> {
        if let Some(repo) = &self.get_repo() {
            repo.record_discovery(match_id, team_id, user_id, treasure_id, score).await?;
        }
        
        Ok(())
    }
    
    // Get full match details
    pub async fn get_match_details(&self, match_id: Uuid) -> Result<crate::models::game::MatchDetails> {
        if let Some(repo) = &self.get_repo() {
            // Use the repository method that already handles this
            return repo.get_match_details(match_id).await;
        }
        
        Err(Error::MatchNotFound)
    }
}
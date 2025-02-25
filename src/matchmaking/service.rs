use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use uuid::Uuid;
use crate::error::{Error, Result};
use crate::models::game::{MatchResult, MatchRoom};

pub struct MatchService {
    match_pools: Arc<RwLock<HashMap<String, Vec<MatchRoom>>>>,
    min_room_count: HashMap<String, usize>,  // 每种类型最少保持的房间数
}

impl MatchService {
    pub fn new() -> Arc<Self> {
        let service = Arc::new(Self {
            match_pools: Arc::new(RwLock::new(HashMap::new())),
            min_room_count: HashMap::from([
                ("1v1".to_string(), 5),  // 保持5个1v1房间
                ("2v2".to_string(), 3),  // 保持3个2v2房间
                ("5v5".to_string(), 2),  // 保持2个5v5房间
            ]),
        });

        // 克隆一份Arc用于异步任务
        let service_clone = service.clone();
        
        // 初始化房间池
        tokio::spawn(async move {
            if let Err(e) = service_clone.initialize_pools().await {
                eprintln!("Failed to initialize pools: {:?}", e);
            }
        });

        service
    }

    // 初始化房间池
    async fn initialize_pools(&self) -> Result<()> {
        let mut pools = self.match_pools.write().await;
        
        for (match_type, &min_count) in &self.min_room_count {
            let pool = pools.entry(match_type.clone())
                .or_insert_with(Vec::new);
            
            // 创建初始房间直到达到最小数量
            while pool.len() < min_count {
                pool.push(MatchRoom {
                    id: Uuid::new_v4(),
                    //match_type: match_type.clone(),
                    required_players: self.get_required_players(match_type)?,
                    current_players: 0,
                    players: Vec::new(),
                    status: "matching".to_string(),
                });
            }
        }
        
        Ok(())
    }

    // 维护房间池的大小
    async fn maintain_pool_size(&self, match_type: &str) -> Result<()> {
        let mut pools = self.match_pools.write().await;
        let pool = pools.get_mut(match_type).ok_or(Error::InvalidMatchType)?;
        
        if let Some(&min_count) = self.min_room_count.get(match_type) {
            // 统计空闲房间数量
            let empty_rooms = pool.iter()
                .filter(|r| r.current_players == 0)
                .count();
            
            // 如果空闲房间数量小于最小值，创建新房间
            while empty_rooms < min_count {
                pool.push(MatchRoom {
                    id: Uuid::new_v4(),
                    //match_type: match_type.to_string(),
                    required_players: self.get_required_players(match_type)?,
                    current_players: 0,
                    players: Vec::new(),
                    status: "matching".to_string(),
                });
            }
        }
        
        Ok(())
    }

    // 获取所需玩家数
    fn get_required_players(&self, match_type: &str) -> Result<i32> {
        match match_type {
            "1v1" => Ok(2),
            "2v2" => Ok(4),
            "5v5" => Ok(10),
            _ => Err(Error::InvalidMatchType),
        }
    }

    // 加入匹配
    pub async fn join_match(&self, user_id: Uuid, match_type: &str) -> Result<MatchResult> {
        let mut pools = self.match_pools.write().await;
        
        // 获取或创建对应类型的匹配池
        let pool = pools.entry(match_type.to_string())
            .or_insert_with(Vec::new);
        
        // 获取每个队伍需要的玩家数
        let required_players = match match_type {
            "1v1" => 2,
            "2v2" => 4,
            "5v5" => 10,
            _ => return Err(Error::InvalidMatchType),
        };

        // 查找可以加入的房间
        if let Some(room) = pool.iter_mut().find(|r| 
            r.status == "matching" && 
            r.current_players < r.required_players && 
            !r.players.contains(&user_id)
        ) {
            room.players.push(user_id);
            room.current_players += 1;

            // 检查是否可以开始比赛
            if room.current_players == room.required_players {
                room.status = "ready".to_string();
                // 调用数据库开始比赛
                self.start_match(room.id).await?;
            }

            return Ok(MatchResult {
                match_id: room.id,
                status: room.status.clone(),
                match_type: match_type.to_string(),
                current_players: room.current_players,
                required_players: room.required_players,
            });
        }

        // 如果没有合适的房间，创建新房间
        let new_room = MatchRoom {
            id: Uuid::new_v4(),
            //match_type: match_type.to_string(),
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

    // 离开匹配
    pub async fn leave_match(&self, user_id: Uuid, match_id: Uuid) -> Result<()> {
        let mut pools = self.match_pools.write().await;
        
        for (match_type, pool) in pools.iter_mut() {
            if let Some(index) = pool.iter().position(|r| r.id == match_id) {
                let room = &mut pool[index];
                if let Some(player_index) = room.players.iter().position(|&p| p == user_id) {
                    room.players.remove(player_index);
                    room.current_players -= 1;
                    
                    // 如果房间空了，并且超过最小房间数，才移除
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

    // 获取匹配状态
    pub async fn get_match_status(&self, match_id: Uuid) -> Result<String> {
        let pools = self.match_pools.read().await;
        
        for pool in pools.values() {
            if let Some(room) = pool.iter().find(|r| r.id == match_id) {
                return Ok(room.status.clone());
            }
        }
        
        Err(Error::MatchNotFound)
    }

    // 开始比赛
    async fn start_match(&self, match_id: Uuid) -> Result<()> {
        // 这里需要调用数据库操作来创建比赛记录
        // TODO: 实现数据库操作
        Ok(())
    }
}
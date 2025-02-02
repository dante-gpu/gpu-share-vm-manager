use std::collections::HashMap;
use uuid::Uuid;
use anyhow::{Result, anyhow};

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub credits: f64,
    pub allocated_gpus: Vec<u32>,
}

pub struct UserManager {
    pub users: HashMap<String, User>, 
}

impl UserManager {
    pub fn new() -> Self {
        Self {
            users: HashMap::new(),
        }
    }
    
    pub fn create_user(&mut self, username: &str) -> Result<&User> {
        if self.users.contains_key(username) {
            return Err(anyhow!("User already exists"));
        }
        
        let user = User {
            id: Uuid::new_v4(),
            credits: 1000000.0, // 100 token for new user
            allocated_gpus: Vec::new(),
        };
        
        self.users.insert(username.to_string(), user);
        Ok(self.users.get(username).unwrap())
    }
    
    pub fn get_user(&mut self, username: &str) -> anyhow::Result<&mut User> {
        if !self.users.contains_key(username) {
            let user = User {
                id: Uuid::new_v4(),
                credits: 1000000.0,
                allocated_gpus: Vec::new(),
            };
            self.users.insert(username.to_string(), user);
        }
        Ok(self.users.get_mut(username).unwrap())
    }
    pub fn deduct_credits(&mut self, username: &str, amount: f64) -> Result<()> {
        let user = self.get_user(username)?;
        if user.credits < amount {
            return Err(anyhow!("Insufficient credits: {:.2} needed, {:.2} available", amount, user.credits));
        }
        user.credits -= amount;
        Ok(())
    }
}
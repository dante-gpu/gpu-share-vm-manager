use chrono::{DateTime, Utc};
use std::time::Duration;
use uuid::Uuid;
// use anyhow::Result;

#[derive(Debug, Clone)]
pub struct BillingSystem {
    transactions: Vec<Transaction>,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub user_id: Uuid,
    pub gpu_id: u32,
    pub start_time: DateTime<Utc>,
    pub duration: Duration,
    pub cost: f64,
}

impl BillingSystem {
    pub fn new() -> Self {
        Self {
            transactions: Vec::new(),
        }
    }
    
    pub fn add_transaction(&mut self, transaction: Transaction) {
        self.transactions.push(transaction);
    }
    
    pub fn get_user_balance(&self, user_id: Uuid) -> f64 {
        self.transactions
            .iter()
            .filter(|t| t.user_id == user_id)
            .map(|t| t.cost)
            .sum()
    }
}
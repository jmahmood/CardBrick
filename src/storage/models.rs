// src/storage/models.rs
// Data models for database rows

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardRow {
    pub id: i64,
    pub front: String,
    pub back: String,
    pub media_id: Option<i64>,
    pub similarity_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrsRow {
    pub card_id: i64,
    pub next_due_ts: Option<i64>,
    pub interval: Option<i64>,
    pub ease: Option<f64>,
    pub lapses: Option<i64>,
    pub reps: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BanditRow {
    pub param_id: String,
    pub arm_value: i64,
    pub alpha: i64,
    pub beta: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyLogRow {
    pub date: String,
    pub pack_sz: Option<i64>,
    pub rev_coef: Option<f64>,
    pub fail_k: Option<i64>,
    pub cards_studied: Option<i64>,
    pub points: Option<i64>,
    pub reward_scaled: Option<f64>,
    pub reward_bin: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaRow {
    pub key: String,
    pub value: String,
}

pub type CardId = i64;

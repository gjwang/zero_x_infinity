use super::types::AccountType;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Internal transfer record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Transfer {
    pub transfer_id: i64,
    pub user_id: i64,
    pub asset_id: i32,
    #[sqlx(try_from = "i16")]
    pub from_account: AccountType,
    #[sqlx(try_from = "i16")]
    pub to_account: AccountType,
    pub amount: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct TransferRequest {
    pub from: String,
    pub to: String,
    pub asset: String,
    pub amount: String, // String to avoid float precision issues JSON
}

#[derive(Debug, Serialize)]
pub struct TransferResponse {
    pub transfer_id: String,
    pub status: String,
    pub from: String,
    pub to: String,
    pub asset: String,
    pub amount: String,
    pub timestamp: i64,
}

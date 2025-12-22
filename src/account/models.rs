//! Data models for account management

use chrono::{DateTime, Utc};

/// User status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i16)]
pub enum UserStatus {
    Disabled = 0,
    Active = 1,
}

impl From<i16> for UserStatus {
    fn from(v: i16) -> Self {
        match v {
            0 => UserStatus::Disabled,
            _ => UserStatus::Active,
        }
    }
}

/// User account
#[derive(Debug, Clone)]
pub struct User {
    pub user_id: i64,
    pub username: String,
    pub email: Option<String>,
    pub status: UserStatus,
    pub created_at: DateTime<Utc>,
}

/// Asset definition (BTC, USDT, etc.)
#[derive(Debug, Clone)]
pub struct Asset {
    pub asset_id: i32,
    pub asset: String,      // 资产代码: BTC, USDT
    pub name: String,
    pub decimals: i16,
    pub status: i16,
}

/// Trading pair (symbol)
#[derive(Debug, Clone)]
pub struct Symbol {
    pub symbol_id: i32,
    pub symbol: String,     // 交易对: BTC_USDT
    pub base_asset_id: i32,
    pub quote_asset_id: i32,
    pub price_decimals: i16,
    pub qty_decimals: i16,
    pub min_qty: i64,
    pub status: i16,
}

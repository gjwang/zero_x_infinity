//! Data models for account management

use chrono::{DateTime, Utc};

// ============================================================================
// User Flags (bitmask)
// ============================================================================
pub mod user_flags {
    pub const CAN_LOGIN: i32 = 0x01;
    pub const CAN_TRADE: i32 = 0x02;
    pub const CAN_WITHDRAW: i32 = 0x04;
    pub const CAN_API_ACCESS: i32 = 0x08;
    pub const IS_VIP: i32 = 0x10;
    pub const IS_KYC_VERIFIED: i32 = 0x20;
    pub const DEFAULT: i32 = 0x0F; // login + trade + withdraw + api
}

// ============================================================================
// Asset Flags (bitmask)
// ============================================================================
pub mod asset_flags {
    pub const CAN_DEPOSIT: i32 = 0x01;
    pub const CAN_WITHDRAW: i32 = 0x02;
    pub const CAN_TRADE: i32 = 0x04;
    pub const IS_STABLE_COIN: i32 = 0x08;
    pub const DEFAULT: i32 = 0x07; // deposit + withdraw + trade
}

// ============================================================================
// Symbol Flags (bitmask)
// ============================================================================
pub mod symbol_flags {
    pub const IS_TRADABLE: i32 = 0x01;
    pub const IS_VISIBLE: i32 = 0x02;
    pub const ALLOW_MARKET: i32 = 0x04;
    pub const ALLOW_LIMIT: i32 = 0x08;
    pub const DEFAULT: i32 = 0x0F; // all features
}

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
    pub flags: i32,
    pub created_at: DateTime<Utc>,
}

impl User {
    pub fn can_login(&self) -> bool {
        self.flags & user_flags::CAN_LOGIN != 0
    }
    pub fn can_trade(&self) -> bool {
        self.flags & user_flags::CAN_TRADE != 0
    }
    pub fn can_withdraw(&self) -> bool {
        self.flags & user_flags::CAN_WITHDRAW != 0
    }
}

/// Asset definition (BTC, USDT, etc.)
#[derive(Debug, Clone)]
pub struct Asset {
    pub asset_id: i32,
    pub asset: String,
    pub name: String,
    pub decimals: i16,
    pub status: i16,
    pub flags: i32,
}

impl Asset {
    pub fn can_deposit(&self) -> bool {
        self.flags & asset_flags::CAN_DEPOSIT != 0
    }
    pub fn can_withdraw(&self) -> bool {
        self.flags & asset_flags::CAN_WITHDRAW != 0
    }
    pub fn can_trade(&self) -> bool {
        self.flags & asset_flags::CAN_TRADE != 0
    }
}

/// Trading pair (symbol)
#[derive(Debug, Clone)]
pub struct Symbol {
    pub symbol_id: i32,
    pub symbol: String,
    pub base_asset_id: i32,
    pub quote_asset_id: i32,
    pub price_decimals: i16,
    pub qty_decimals: i16,
    pub min_qty: i64,
    pub status: i16,
    pub flags: i32,
}

impl Symbol {
    pub fn is_tradable(&self) -> bool {
        self.flags & symbol_flags::IS_TRADABLE != 0
    }
    pub fn is_visible(&self) -> bool {
        self.flags & symbol_flags::IS_VISIBLE != 0
    }
}

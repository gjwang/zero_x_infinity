//! Data models for user account management

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
    pub user_flags: i32,
    pub created_at: DateTime<Utc>,
}

impl User {
    pub fn can_login(&self) -> bool {
        self.user_flags & user_flags::CAN_LOGIN != 0
    }
    pub fn can_trade(&self) -> bool {
        self.user_flags & user_flags::CAN_TRADE != 0
    }
    pub fn can_withdraw(&self) -> bool {
        self.user_flags & user_flags::CAN_WITHDRAW != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_status_from_i16() {
        assert_eq!(UserStatus::from(0), UserStatus::Disabled);
        assert_eq!(UserStatus::from(1), UserStatus::Active);
        assert_eq!(UserStatus::from(99), UserStatus::Active); // default to Active
    }

    #[test]
    fn test_user_flags() {
        let user = User {
            user_id: 1,
            username: "test".to_string(),
            email: None,
            status: UserStatus::Active,
            user_flags: user_flags::CAN_LOGIN | user_flags::CAN_TRADE,
            created_at: Utc::now(),
        };

        assert!(user.can_login());
        assert!(user.can_trade());
        assert!(!user.can_withdraw());
    }
}

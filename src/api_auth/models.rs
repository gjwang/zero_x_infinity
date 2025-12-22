//! API Key models and types.
//!
//! Defines the data structures for API Key authentication.

/// Key type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i16)]
pub enum KeyType {
    /// Ed25519 asymmetric signature (recommended)
    Ed25519 = 1,
    /// HMAC-SHA256 symmetric signature
    HmacSha256 = 2,
    /// RSA asymmetric signature
    Rsa = 3,
}

impl KeyType {
    /// Convert from database i16 value.
    pub fn from_i16(value: i16) -> Option<Self> {
        match value {
            1 => Some(Self::Ed25519),
            2 => Some(Self::HmacSha256),
            3 => Some(Self::Rsa),
            _ => None,
        }
    }
}

/// Permission flags for API Keys.
pub mod permissions {
    /// Read-only access (query orders, balances, etc.)
    pub const READ: i32 = 0x01;
    /// Trading access (create/cancel orders)
    pub const TRADE: i32 = 0x02;
    /// Withdrawal access
    pub const WITHDRAW: i32 = 0x04;
    /// Internal transfer access
    pub const TRANSFER: i32 = 0x08;
    /// Full access (all permissions)
    pub const FULL: i32 = READ | TRADE | WITHDRAW | TRANSFER;
}

/// Check if a permission is granted.
pub fn has_permission(perms: i32, required: i32) -> bool {
    (perms & required) == required
}

/// API Key record from database.
#[derive(Debug, Clone)]
pub struct ApiKeyRecord {
    /// Primary key
    pub key_id: i32,
    /// Owner user ID
    pub user_id: i64,
    /// API Key string (AK_ + 16 hex)
    pub api_key: String,
    /// Key type (1=Ed25519, 2=HMAC, 3=RSA)
    pub key_type: i16,
    /// Public key or secret hash (32 bytes for Ed25519/HMAC)
    pub key_data: Vec<u8>,
    /// User-defined label
    pub label: Option<String>,
    /// Permission bitmask
    pub permissions: i32,
    /// Status (0=disabled, 1=active)
    pub status: i16,
    /// Last seen ts_nonce (for replay protection)
    pub last_ts_nonce: i64,
}

impl ApiKeyRecord {
    /// Check if the API Key is active.
    pub fn is_active(&self) -> bool {
        self.status == 1
    }

    /// Check if the API Key has the required permission.
    pub fn has_permission(&self, required: i32) -> bool {
        has_permission(self.permissions, required)
    }

    /// Get the key type enum.
    pub fn key_type_enum(&self) -> Option<KeyType> {
        KeyType::from_i16(self.key_type)
    }
}

/// Authenticated user info extracted from middleware.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    /// User ID from the API Key
    pub user_id: i64,
    /// API Key used for authentication
    pub api_key: String,
    /// Permission bitmask
    pub permissions: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_type_from_i16() {
        assert_eq!(KeyType::from_i16(1), Some(KeyType::Ed25519));
        assert_eq!(KeyType::from_i16(2), Some(KeyType::HmacSha256));
        assert_eq!(KeyType::from_i16(3), Some(KeyType::Rsa));
        assert_eq!(KeyType::from_i16(99), None);
    }

    #[test]
    fn test_permissions() {
        let read_only = permissions::READ;
        assert!(has_permission(read_only, permissions::READ));
        assert!(!has_permission(read_only, permissions::TRADE));

        let trader = permissions::READ | permissions::TRADE;
        assert!(has_permission(trader, permissions::READ));
        assert!(has_permission(trader, permissions::TRADE));
        assert!(!has_permission(trader, permissions::WITHDRAW));

        let full = permissions::FULL;
        assert!(has_permission(full, permissions::READ));
        assert!(has_permission(full, permissions::TRADE));
        assert!(has_permission(full, permissions::WITHDRAW));
        assert!(has_permission(full, permissions::TRANSFER));
    }

    #[test]
    fn test_api_key_record() {
        let record = ApiKeyRecord {
            key_id: 1,
            user_id: 1001,
            api_key: "AK_7F3D8E2A1B5C9F04".to_string(),
            key_type: 1,
            key_data: vec![0u8; 32],
            label: Some("Test Key".to_string()),
            permissions: permissions::READ | permissions::TRADE,
            status: 1,
            last_ts_nonce: 0,
        };

        assert!(record.is_active());
        assert!(record.has_permission(permissions::READ));
        assert!(record.has_permission(permissions::TRADE));
        assert!(!record.has_permission(permissions::WITHDRAW));
        assert_eq!(record.key_type_enum(), Some(KeyType::Ed25519));
    }
}

//! Asset models and flags

use sqlx::FromRow;

// ============================================================================
// Asset Flags (bitmask)
// ============================================================================
pub mod asset_flags {
    pub const CAN_DEPOSIT: i32 = 0x01;
    pub const CAN_WITHDRAW: i32 = 0x02;
    pub const CAN_TRADE: i32 = 0x04;
    pub const IS_STABLE_COIN: i32 = 0x08;
    /// Allow internal transfers between accounts (Phase 0x0B-a)
    pub const CAN_INTERNAL_TRANSFER: i32 = 0x10;
    /// Default flags: deposit + withdraw + trade + internal_transfer
    pub const DEFAULT: i32 = 0x17;
}

/// Asset definition (BTC, USDT, etc.)
#[derive(Debug, Clone, FromRow)]
pub struct Asset {
    pub asset_id: i32,
    pub asset: String,
    pub name: String,
    pub decimals: i16,
    pub status: i16,
    pub asset_flags: i32,
}

impl Asset {
    pub fn can_deposit(&self) -> bool {
        self.asset_flags & asset_flags::CAN_DEPOSIT != 0
    }
    pub fn can_withdraw(&self) -> bool {
        self.asset_flags & asset_flags::CAN_WITHDRAW != 0
    }
    pub fn can_trade(&self) -> bool {
        self.asset_flags & asset_flags::CAN_TRADE != 0
    }
    /// Check if internal transfers are allowed for this asset (Phase 0x0B-a)
    pub fn can_internal_transfer(&self) -> bool {
        self.asset_flags & asset_flags::CAN_INTERNAL_TRANSFER != 0
    }
    /// Check if asset is active (status = 1)
    pub fn is_active(&self) -> bool {
        self.status == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_flags_all_enabled() {
        let asset = Asset {
            asset_id: 1,
            asset: "BTC".to_string(),
            name: "Bitcoin".to_string(),
            decimals: 8,
            status: 1,
            asset_flags: asset_flags::CAN_DEPOSIT
                | asset_flags::CAN_WITHDRAW
                | asset_flags::CAN_TRADE,
        };

        assert!(asset.can_deposit());
        assert!(asset.can_withdraw());
        assert!(asset.can_trade());
    }

    #[test]
    fn test_asset_flags_partial() {
        let asset = Asset {
            asset_id: 2,
            asset: "USDT".to_string(),
            name: "Tether".to_string(),
            decimals: 6,
            status: 1,
            asset_flags: asset_flags::CAN_DEPOSIT | asset_flags::CAN_TRADE, // No withdraw
        };

        assert!(asset.can_deposit());
        assert!(!asset.can_withdraw());
        assert!(asset.can_trade());
    }

    #[test]
    fn test_asset_flags_none() {
        let asset = Asset {
            asset_id: 3,
            asset: "DISABLED".to_string(),
            name: "Disabled Asset".to_string(),
            decimals: 8,
            status: 0,
            asset_flags: 0, // No flags
        };

        assert!(!asset.can_deposit());
        assert!(!asset.can_withdraw());
        assert!(!asset.can_trade());
    }

    #[test]
    fn test_asset_flags_default() {
        let asset = Asset {
            asset_id: 4,
            asset: "ETH".to_string(),
            name: "Ethereum".to_string(),
            decimals: 18,
            status: 1,
            asset_flags: asset_flags::DEFAULT,
        };

        assert!(asset.can_deposit());
        assert!(asset.can_withdraw());
        assert!(asset.can_trade());
    }

    #[test]
    fn test_asset_flags_stable_coin() {
        let asset = Asset {
            asset_id: 5,
            asset: "USDC".to_string(),
            name: "USD Coin".to_string(),
            decimals: 6,
            status: 1,
            asset_flags: asset_flags::DEFAULT | asset_flags::IS_STABLE_COIN,
        };

        assert!(asset.can_deposit());
        assert!(asset.can_withdraw());
        assert!(asset.can_trade());
        assert_eq!(
            asset.asset_flags & asset_flags::IS_STABLE_COIN,
            asset_flags::IS_STABLE_COIN
        );
    }

    #[test]
    fn test_asset_flags_deposit_only() {
        let asset = Asset {
            asset_id: 6,
            asset: "LOCKED".to_string(),
            name: "Locked Asset".to_string(),
            decimals: 8,
            status: 1,
            asset_flags: asset_flags::CAN_DEPOSIT, // Only deposit
        };

        assert!(asset.can_deposit());
        assert!(!asset.can_withdraw());
        assert!(!asset.can_trade());
    }

    #[test]
    fn test_asset_flags_trade_only() {
        let asset = Asset {
            asset_id: 7,
            asset: "INTERNAL".to_string(),
            name: "Internal Token".to_string(),
            decimals: 8,
            status: 1,
            asset_flags: asset_flags::CAN_TRADE, // Only trade
        };

        assert!(!asset.can_deposit());
        assert!(!asset.can_withdraw());
        assert!(asset.can_trade());
    }
}

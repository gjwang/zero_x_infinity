//! Asset models and flags

use sqlx::FromRow;

// ============================================================================
// Asset Flags (bitmask)
// ============================================================================
pub mod flags {
    pub const CAN_DEPOSIT: i32 = 0x01;
    pub const CAN_WITHDRAW: i32 = 0x02;
    pub const CAN_TRADE: i32 = 0x04;
    pub const IS_STABLE_COIN: i32 = 0x08;
    pub const DEFAULT: i32 = 0x07; // deposit + withdraw + trade
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
        self.asset_flags & flags::CAN_DEPOSIT != 0
    }
    pub fn can_withdraw(&self) -> bool {
        self.asset_flags & flags::CAN_WITHDRAW != 0
    }
    pub fn can_trade(&self) -> bool {
        self.asset_flags & flags::CAN_TRADE != 0
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
            asset_flags: flags::CAN_DEPOSIT | flags::CAN_WITHDRAW | flags::CAN_TRADE,
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
            asset_flags: flags::CAN_DEPOSIT | flags::CAN_TRADE, // No withdraw
        };

        assert!(asset.can_deposit());
        assert!(!asset.can_withdraw());
        assert!(asset.can_trade());
    }
}

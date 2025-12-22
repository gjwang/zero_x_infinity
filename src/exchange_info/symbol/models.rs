//! Symbol (trading pair) models and flags

use sqlx::FromRow;

// ============================================================================
// Symbol Flags (bitmask)
// ============================================================================
pub mod flags {
    pub const IS_TRADABLE: i32 = 0x01;
    pub const IS_VISIBLE: i32 = 0x02;
    pub const ALLOW_MARKET: i32 = 0x04;
    pub const ALLOW_LIMIT: i32 = 0x08;
    pub const DEFAULT: i32 = 0x0F; // all features
}

/// Trading pair (symbol)
#[derive(Debug, Clone, FromRow)]
pub struct Symbol {
    pub symbol_id: i32,
    pub symbol: String,
    pub base_asset_id: i32,
    pub quote_asset_id: i32,
    pub price_decimals: i16,
    pub qty_decimals: i16,
    pub min_qty: i64,
    pub status: i16,
    pub symbol_flags: i32,
}

impl Symbol {
    pub fn is_tradable(&self) -> bool {
        self.symbol_flags & flags::IS_TRADABLE != 0
    }
    pub fn is_visible(&self) -> bool {
        self.symbol_flags & flags::IS_VISIBLE != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_flags() {
        let symbol = Symbol {
            symbol_id: 1,
            symbol: "BTC_USDT".to_string(),
            base_asset_id: 1,
            quote_asset_id: 2,
            price_decimals: 2,
            qty_decimals: 6,
            min_qty: 1000,
            status: 1,
            symbol_flags: flags::IS_TRADABLE | flags::IS_VISIBLE,
        };

        assert!(symbol.is_tradable());
        assert!(symbol.is_visible());
    }
}

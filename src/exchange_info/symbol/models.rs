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
    fn test_symbol_flags_all_enabled() {
        let symbol = Symbol {
            symbol_id: 1,
            symbol: "BTC_USDT".to_string(),
            base_asset_id: 1,
            quote_asset_id: 2,
            price_decimals: 2,
            qty_decimals: 6,
            min_qty: 1000,
            status: 1,
            symbol_flags: flags::DEFAULT,
        };

        assert!(symbol.is_tradable());
        assert!(symbol.is_visible());
    }

    #[test]
    fn test_symbol_flags_partial() {
        let symbol = Symbol {
            symbol_id: 2,
            symbol: "ETH_BTC".to_string(),
            base_asset_id: 3,
            quote_asset_id: 1,
            price_decimals: 6,
            qty_decimals: 4,
            min_qty: 100,
            status: 1,
            symbol_flags: flags::IS_TRADABLE, // Tradable but not visible
        };

        assert!(symbol.is_tradable());
        assert!(!symbol.is_visible());
    }

    #[test]
    fn test_symbol_flags_none() {
        let symbol = Symbol {
            symbol_id: 3,
            symbol: "DISABLED_PAIR".to_string(),
            base_asset_id: 1,
            quote_asset_id: 2,
            price_decimals: 2,
            qty_decimals: 6,
            min_qty: 1000,
            status: 0,
            symbol_flags: 0, // No flags
        };

        assert!(!symbol.is_tradable());
        assert!(!symbol.is_visible());
    }

    #[test]
    fn test_symbol_flags_visible_only() {
        let symbol = Symbol {
            symbol_id: 4,
            symbol: "COMING_SOON".to_string(),
            base_asset_id: 1,
            quote_asset_id: 2,
            price_decimals: 2,
            qty_decimals: 6,
            min_qty: 1000,
            status: 1,
            symbol_flags: flags::IS_VISIBLE, // Visible but not tradable
        };

        assert!(!symbol.is_tradable());
        assert!(symbol.is_visible());
    }

    #[test]
    fn test_symbol_flags_market_orders() {
        let symbol = Symbol {
            symbol_id: 5,
            symbol: "BTC_USD".to_string(),
            base_asset_id: 1,
            quote_asset_id: 4,
            price_decimals: 2,
            qty_decimals: 8,
            min_qty: 1,
            status: 1,
            symbol_flags: flags::IS_TRADABLE | flags::IS_VISIBLE | flags::ALLOW_MARKET,
        };

        assert!(symbol.is_tradable());
        assert!(symbol.is_visible());
        assert_eq!(
            symbol.symbol_flags & flags::ALLOW_MARKET,
            flags::ALLOW_MARKET
        );
    }

    #[test]
    fn test_symbol_flags_limit_orders_only() {
        let symbol = Symbol {
            symbol_id: 6,
            symbol: "ALT_USDT".to_string(),
            base_asset_id: 5,
            quote_asset_id: 2,
            price_decimals: 4,
            qty_decimals: 2,
            min_qty: 100,
            status: 1,
            symbol_flags: flags::IS_TRADABLE | flags::IS_VISIBLE | flags::ALLOW_LIMIT,
        };

        assert!(symbol.is_tradable());
        assert!(symbol.is_visible());
        assert_eq!(symbol.symbol_flags & flags::ALLOW_LIMIT, flags::ALLOW_LIMIT);
        assert_eq!(symbol.symbol_flags & flags::ALLOW_MARKET, 0);
    }
}

//! Symbol (trading pair) models and flags

use sqlx::FromRow;

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

/// Trading pair (symbol)
///
/// Field naming follows precision terminology (See money-type-safety.md Section 2.5):
/// - price_scale/qty_scale: for internal Decimal↔u64 conversion
/// - price_precision/qty_precision: for API input validation and output formatting
#[derive(Debug, Clone, FromRow)]
pub struct Symbol {
    pub symbol_id: i32,
    pub symbol: String,
    pub base_asset_id: i32,
    pub quote_asset_id: i32,
    /// Internal price scale factor
    pub price_scale: i16,
    /// API price precision for input/output
    pub price_precision: i16,
    /// Internal quantity scale factor
    pub qty_scale: i16,
    /// API quantity precision for input/output  
    pub qty_precision: i16,
    pub min_qty: i64,
    pub status: i16,
    pub symbol_flags: i32,
    /// Base maker fee rate (10^6 precision: 1000 = 0.10%)
    #[sqlx(default)]
    pub base_maker_fee: i32,
    /// Base taker fee rate (10^6 precision: 2000 = 0.20%)
    #[sqlx(default)]
    pub base_taker_fee: i32,
}

impl Symbol {
    pub fn is_tradable(&self) -> bool {
        self.symbol_flags & symbol_flags::IS_TRADABLE != 0
    }
    pub fn is_visible(&self) -> bool {
        self.symbol_flags & symbol_flags::IS_VISIBLE != 0
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
            price_scale: 2,
            price_precision: 2,
            qty_scale: 6,
            qty_precision: 6,
            min_qty: 1000,
            status: 1,
            symbol_flags: symbol_flags::DEFAULT,
            base_maker_fee: 1000,
            base_taker_fee: 2000,
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
            price_scale: 6,
            price_precision: 6,
            qty_scale: 4,
            qty_precision: 4,
            min_qty: 100,
            status: 1,
            symbol_flags: symbol_flags::IS_TRADABLE,
            base_maker_fee: 1000,
            base_taker_fee: 2000, // Tradable but not visible
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
            price_scale: 2,
            price_precision: 2,
            qty_scale: 6,
            qty_precision: 6,
            min_qty: 1000,
            status: 0,
            symbol_flags: 0,
            base_maker_fee: 1000,
            base_taker_fee: 2000, // No flags
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
            price_scale: 2,
            price_precision: 2,
            qty_scale: 6,
            qty_precision: 6,
            min_qty: 1000,
            status: 1,
            symbol_flags: symbol_flags::IS_VISIBLE,
            base_maker_fee: 1000,
            base_taker_fee: 2000, // Visible but not tradable
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
            price_scale: 2,
            price_precision: 2,
            qty_scale: 8,
            qty_precision: 8,
            min_qty: 1,
            status: 1,
            symbol_flags: symbol_flags::IS_TRADABLE
                | symbol_flags::IS_VISIBLE
                | symbol_flags::ALLOW_MARKET,
            base_maker_fee: 1000,
            base_taker_fee: 2000,
        };

        assert!(symbol.is_tradable());
        assert!(symbol.is_visible());
        assert_eq!(
            symbol.symbol_flags & symbol_flags::ALLOW_MARKET,
            symbol_flags::ALLOW_MARKET
        );
    }

    #[test]
    fn test_symbol_flags_limit_orders_only() {
        let symbol = Symbol {
            symbol_id: 6,
            symbol: "ALT_USDT".to_string(),
            base_asset_id: 5,
            quote_asset_id: 2,
            price_scale: 4,
            price_precision: 4,
            qty_scale: 2,
            qty_precision: 2,
            min_qty: 100,
            status: 1,
            symbol_flags: symbol_flags::IS_TRADABLE
                | symbol_flags::IS_VISIBLE
                | symbol_flags::ALLOW_LIMIT,
            base_maker_fee: 1000,
            base_taker_fee: 2000,
        };

        assert!(symbol.is_tradable());
        assert!(symbol.is_visible());
        assert_eq!(
            symbol.symbol_flags & symbol_flags::ALLOW_LIMIT,
            symbol_flags::ALLOW_LIMIT
        );
        assert_eq!(symbol.symbol_flags & symbol_flags::ALLOW_MARKET, 0);
    }
}

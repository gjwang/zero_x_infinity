//! Trading configuration types and loaders
//!
//! This module defines the configuration structures for assets, symbols,
//! and the complete trading environment.

use rustc_hash::FxHashMap;

// Re-export types for backwards compatibility
pub use crate::core_types::{AssetId, UserId};

/// Asset configuration from assets_config.csv
///
/// # Decimal Precision Design
///
/// | Field | Mutable | Purpose |
/// |-------|---------|---------|
/// | `decimals` | ⚠️ NEVER change | Internal storage precision |
/// | `display_decimals` | ✅ Can adjust | Client-facing display precision |
#[derive(Debug, Clone)]
pub struct AssetConfig {
    pub asset_id: AssetId,
    pub asset: String,
    /// Internal storage precision (e.g., 8 for BTC = satoshi)
    /// WARNING: Never change after initial setup!
    pub decimals: u32,
    /// Client-facing display precision (can be adjusted)
    pub display_decimals: u32,
}

/// Symbol (trading pair) configuration from symbols_config.csv
#[derive(Debug, Clone)]
pub struct SymbolConfig {
    pub symbol_id: u32,
    pub symbol: String,
    pub base_asset_id: AssetId,
    pub quote_asset_id: AssetId,
    /// Internal price precision
    pub price_decimal: u32,
    /// Client-facing price display precision
    pub price_display_decimal: u32,
}

/// Complete trading configuration
///
/// Loaded from CSV files and provides all precision/config info
/// needed for order processing.
#[derive(Debug)]
pub struct TradingConfig {
    /// All assets indexed by asset_id
    pub assets: FxHashMap<AssetId, AssetConfig>,
    /// All trading symbols
    pub symbols: Vec<SymbolConfig>,
    /// Currently active symbol for this test run
    pub active_symbol: SymbolConfig,
    // Internal storage decimals
    pub base_decimals: u32,
    pub quote_decimals: u32,
    // Client-facing display decimals
    pub qty_display_decimals: u32,
    pub price_display_decimals: u32,
}

impl TradingConfig {
    /// Get internal unit for base asset (e.g., 10^8 for BTC)
    #[inline]
    pub fn qty_unit(&self) -> u64 {
        10u64.pow(self.base_decimals)
    }

    /// Get internal unit for quote asset (e.g., 10^6 for USDT)
    #[inline]
    pub fn price_unit(&self) -> u64 {
        10u64.pow(self.quote_decimals)
    }

    /// Get base asset ID from active symbol
    #[inline]
    pub fn base_asset_id(&self) -> AssetId {
        self.active_symbol.base_asset_id
    }

    /// Get quote asset ID from active symbol
    #[inline]
    pub fn quote_asset_id(&self) -> AssetId {
        self.active_symbol.quote_asset_id
    }
}

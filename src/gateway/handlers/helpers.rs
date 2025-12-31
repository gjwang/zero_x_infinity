//! Handler helper functions and formatters
//!
//! This module contains shared utilities used by multiple handlers.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::money;
use crate::symbol_manager::SymbolManager;

// ============================================================================
// Depth Formatter - Encapsulated to prevent parameter errors
// ============================================================================

/// Depth data formatter that encapsulates conversion logic
///
/// This prevents parameter errors by internally fetching the correct
/// decimals and display_decimals from symbol_mgr.
pub struct DepthFormatter<'a> {
    symbol_mgr: &'a SymbolManager,
}

impl<'a> DepthFormatter<'a> {
    pub fn new(symbol_mgr: &'a SymbolManager) -> Self {
        Self { symbol_mgr }
    }

    /// Format quantity for a given symbol
    ///
    /// Automatically fetches base_asset.decimals and display_decimals
    /// from symbol_mgr, preventing parameter errors.
    pub fn format_qty(&self, value: u64, symbol_id: u32) -> Result<String, String> {
        let symbol = self
            .symbol_mgr
            .get_symbol_info_by_id(symbol_id)
            .ok_or_else(|| format!("Symbol {} not found", symbol_id))?;

        let asset = self
            .symbol_mgr
            .assets
            .get(&symbol.base_asset_id)
            .ok_or_else(|| format!("Asset {} not found", symbol.base_asset_id))?;

        Ok(format_qty_internal(
            value,
            asset.decimals,
            asset.display_decimals,
        ))
    }

    /// Format price for a given symbol
    pub fn format_price(&self, value: u64, symbol_id: u32) -> Result<String, String> {
        let symbol = self
            .symbol_mgr
            .get_symbol_info_by_id(symbol_id)
            .ok_or_else(|| format!("Symbol {} not found", symbol_id))?;

        Ok(format_price_internal(value, symbol.price_display_decimal))
    }

    /// Format depth data (bids/asks) for a symbol
    #[allow(clippy::type_complexity)]
    pub fn format_depth_data(
        &self,
        bids: &[(u64, u64)],
        asks: &[(u64, u64)],
        symbol_id: u32,
    ) -> Result<(Vec<[String; 2]>, Vec<[String; 2]>), String> {
        let formatted_bids: Vec<[String; 2]> = bids
            .iter()
            .map(|(p, q)| {
                Ok([
                    self.format_price(*p, symbol_id)?,
                    self.format_qty(*q, symbol_id)?,
                ])
            })
            .collect::<Result<_, String>>()?;

        let formatted_asks: Vec<[String; 2]> = asks
            .iter()
            .map(|(p, q)| {
                Ok([
                    self.format_price(*p, symbol_id)?,
                    self.format_qty(*q, symbol_id)?,
                ])
            })
            .collect::<Result<_, String>>()?;

        Ok((formatted_bids, formatted_asks))
    }
}

// ============================================================================
// Internal Format Helpers
// ============================================================================

/// Format price with display decimals (internal use only)
/// Uses crate::money for unified Decimal-based implementation
pub(crate) fn format_price_internal(value: u64, display_decimals: u32) -> String {
    money::format_amount(value, display_decimals, display_decimals)
}

/// Format quantity with display decimals (internal use only)
/// Uses crate::money for unified Decimal-based implementation
pub(crate) fn format_qty_internal(value: u64, decimals: u32, display_decimals: u32) -> String {
    money::format_amount(value, decimals, display_decimals)
}

// ============================================================================
// Time Helpers
// ============================================================================

/// Get current time in nanoseconds
pub fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

/// Get current time in milliseconds
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

// ============================================================================
// Tests (format_qty and format_price)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_qty_normal_cases() {
        // BTC: decimals=8, display_decimals=6
        assert_eq!(format_qty_internal(100000000, 8, 6), "1.000000");
        assert_eq!(format_qty_internal(50000000, 8, 6), "0.500000");
        assert_eq!(format_qty_internal(123456789, 8, 6), "1.234567"); // Decimal truncation (not f64 rounding)

        // ETH: decimals=8, display_decimals=4
        assert_eq!(format_qty_internal(100000000, 8, 4), "1.0000");
        assert_eq!(format_qty_internal(50000000, 8, 4), "0.5000");

        // USDT: decimals=8, display_decimals=4
        assert_eq!(format_qty_internal(1000000000, 8, 4), "10.0000");
    }

    #[test]
    fn test_format_qty_boundary_cases() {
        // Zero value
        assert_eq!(format_qty_internal(0, 8, 6), "0.000000");
        assert_eq!(format_qty_internal(0, 8, 4), "0.0000");

        // Minimum value (1 unit)
        assert_eq!(format_qty_internal(1, 8, 6), "0.000000"); // less than display_decimals, shows as 0
        assert_eq!(format_qty_internal(1, 8, 8), "0.00000001");

        // Large value
        assert_eq!(format_qty_internal(1000000000000, 8, 6), "10000.000000");
        assert_eq!(format_qty_internal(u64::MAX, 8, 6), "184467440737.095516"); // Decimal precision (not f64)
    }

    #[test]
    fn test_format_qty_precision_edge_cases() {
        // Values at the edge of display precision
        assert_eq!(format_qty_internal(100, 8, 6), "0.000001"); // Exactly at display precision
        assert_eq!(format_qty_internal(99, 8, 6), "0.000000"); // Below display precision (truncated)

        // Large value with all display decimals used
        assert_eq!(format_qty_internal(123456789012, 8, 6), "1234.567890");
    }

    #[test]
    fn test_format_qty_truncation() {
        // Verify truncation behavior (not rounding)
        // 1.23456789 -> with display_decimals=4 should show 1.2345 (not 1.2346)
        assert_eq!(format_qty_internal(123456789, 8, 4), "1.2345");
        // 1.99999999 -> should show 1.9999 (not 2.0000)
        assert_eq!(format_qty_internal(199999999, 8, 4), "1.9999");
    }

    #[test]
    fn test_format_qty_different_decimals() {
        // USDT: decimals=2
        assert_eq!(format_qty_internal(10000, 2, 2), "100.00");
        assert_eq!(format_qty_internal(123, 2, 2), "1.23");

        // Asset with decimals=6
        assert_eq!(format_qty_internal(1000000, 6, 4), "1.0000");
        assert_eq!(format_qty_internal(123456, 6, 4), "0.1234");
    }

    #[test]
    fn test_format_qty_real_world_scenarios() {
        // 1 BTC (8 decimals, display 6)
        assert_eq!(format_qty_internal(100_000_000, 8, 6), "1.000000");

        // 0.001 BTC
        assert_eq!(format_qty_internal(100_000, 8, 6), "0.001000");

        // 1000 USDT (2 decimals)
        assert_eq!(format_qty_internal(100_000, 2, 2), "1000.00");

        // Minimum tradeable: 0.000001 BTC
        assert_eq!(format_qty_internal(100, 8, 6), "0.000001");
    }

    #[test]
    fn test_format_price_normal_cases() {
        // BTC_USDT price: $43000.50
        assert_eq!(format_price_internal(4300050, 2), "43000.50");

        // ETH_USDT price: $2150.25
        assert_eq!(format_price_internal(215025, 2), "2150.25");

        // Large price
        assert_eq!(format_price_internal(10000000, 2), "100000.00");
    }

    #[test]
    fn test_format_price_boundary_cases() {
        // Zero price
        assert_eq!(format_price_internal(0, 2), "0.00");

        // Minimum price (1 cent)
        assert_eq!(format_price_internal(1, 2), "0.01");

        // Very large price
        assert_eq!(format_price_internal(u64::MAX, 2), "184467440737095516.15");
    }
}

//! Money Conversion Module
//!
//! Unified conversion between internal u64 representation and client-facing
//! string/Decimal representation. All conversions MUST go through this module.
//!
//! ## Design Principles
//! 1. Single Source of Truth: SymbolManager provides all decimal configurations
//! 2. Explicit Error Handling: No silent truncation
//! 3. Type Safety: Use wrapper types where possible
//!
//! ## Internal Representation
//! - All amounts are stored as `u64` (or `i64` for signed balances)
//! - The scale factor is `10^decimals` (e.g., 10^8 for BTC = satoshi)
//! - The authoritative source for decimals is `SymbolManager`
//!
//! ## Usage
//! (Internal utilities for money handling. Use `SymbolManager` for public API.)

use crate::symbol_manager::{AssetInfo, SymbolManager};
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;
use thiserror::Error;

// ============================================================================
// Core Money Types (Newtype Wrappers)
// ============================================================================

/// Represents an unsigned monetary amount scaled by 10^decimals.
/// Internal value is private to force construction through audited money logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScaledAmount(u64);

/// Represents a signed monetary amount scaled by 10^decimals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ScaledAmountSigned(i64);

impl ScaledAmount {
    pub fn to_raw(&self) -> u64 {
        self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }
}

impl ScaledAmountSigned {
    pub fn to_raw(&self) -> i64 {
        self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub fn abs(self) -> ScaledAmount {
        ScaledAmount(self.0.unsigned_abs())
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }
}

impl From<u64> for ScaledAmount {
    fn from(v: u64) -> Self {
        Self(v)
    }
}

impl From<i64> for ScaledAmountSigned {
    fn from(v: i64) -> Self {
        Self(v)
    }
}

impl Deref for ScaledAmount {
    type Target = u64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for ScaledAmountSigned {
    type Target = i64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for ScaledAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for ScaledAmountSigned {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ============================================================================
// Error Types
// ============================================================================

/// Money conversion errors
#[derive(Debug, Error)]
pub enum MoneyError {
    #[error("Precision overflow: provided {provided} decimals, max allowed {max}")]
    PrecisionOverflow { provided: u32, max: u32 },

    #[error("Amount must be positive")]
    InvalidAmount,

    #[error("Amount too large, would overflow")]
    Overflow,

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Asset not found: {0}")]
    AssetNotFound(u32),

    #[error("Symbol not found: {0}")]
    SymbolNotFound(u32),
}

// ============================================================================
// Parse: Client → Internal (String/Decimal → ScaledAmount)
// ============================================================================

/// Converts a decimal string to internal ScaledAmount using provided decimals.
/// (Low-level: Preferred usage via SymbolManager::parse_qty)
///
/// # Arguments
/// * `amount_str` - Client-provided amount string (e.g., "1.5", "100")
/// * `decimals` - Asset's internal decimal places
///
/// # Returns
/// * Internal u64 scaled value
///
/// # Errors
/// * `PrecisionOverflow` - If input has more decimal places than allowed
/// * `InvalidAmount` - If amount is zero or negative
/// * `Overflow` - If result would overflow u64
/// * `InvalidFormat` - If string format is invalid
///
/// # Example
/// parse_amount("1.5", 8) -> 150_000_000
pub(crate) fn parse_amount(amount_str: &str, decimals: u32) -> Result<ScaledAmount, MoneyError> {
    let amount_str = amount_str.trim();
    if amount_str.is_empty() {
        return Err(MoneyError::InvalidFormat("empty string".into()));
    }

    // Check for negative sign
    if amount_str.starts_with('-') || amount_str.starts_with('+') {
        return Err(MoneyError::InvalidAmount);
    }

    if amount_str.is_empty() {
        return Err(MoneyError::InvalidFormat("empty string".into()));
    }

    let parts: Vec<&str> = amount_str.split('.').collect();
    let (whole, frac) = match parts.len() {
        1 => (parts[0], ""),
        2 => {
            // Strict check: Require both sides of the dot to be non-empty
            // This prevents ambiguous formats like ".5" or "5."
            if parts[0].is_empty() {
                return Err(MoneyError::InvalidFormat(
                    "missing leading zero (e.g., use 0.5 instead of .5)".into(),
                ));
            }
            if parts[1].is_empty() {
                return Err(MoneyError::InvalidFormat(
                    "missing fractional part (e.g., use 5.0 instead of 5.)".into(),
                ));
            }
            if decimals == 0 {
                return Err(MoneyError::InvalidFormat(
                    "decimals is 0, but dot provided".into(),
                ));
            }
            (parts[0], parts[1])
        }
        _ => return Err(MoneyError::InvalidFormat("multiple decimal points".into())),
    };

    // Precision validation: REJECT if too many decimals (no silent truncation!)
    if frac.len() > decimals as usize {
        return Err(MoneyError::PrecisionOverflow {
            provided: frac.len() as u32,
            max: decimals,
        });
    }

    // Parse whole part with explicit error for overflow
    let whole_num: u64 = whole.parse::<u64>().map_err(|e| {
        let err_str = e.to_string();
        if err_str.contains("too large") || err_str.contains("overflow") {
            MoneyError::Overflow
        } else {
            MoneyError::InvalidFormat(format!("invalid character in whole part: {}", whole))
        }
    })?;

    let frac_num: u64 = if decimals == 0 || frac.is_empty() {
        0
    } else {
        // Pad fractional part to decimals
        let frac_padded = format!("{:0<width$}", frac, width = decimals as usize);
        frac_padded[..decimals as usize]
            .parse::<u64>()
            .map_err(|_| {
                // This should ideally never happen given the digits check, but safety first
                MoneyError::InvalidFormat("invalid fractional part".into())
            })?
    };

    let multiplier = 10u64.pow(decimals);
    let amount = whole_num
        .checked_mul(multiplier)
        .and_then(|v: u64| v.checked_add(frac_num))
        .ok_or(MoneyError::Overflow)?;

    if amount == 0 {
        return Err(MoneyError::InvalidAmount);
    }

    Ok(ScaledAmount(amount))
}

/// Converts a Decimal to internal ScaledAmount. Checks scale limit.
///
/// This is used at the Gateway API boundary where `rust_decimal::Decimal`
/// is used for JSON deserialization.
///
/// # Arguments
/// * `amount` - Validated Decimal value
/// * `decimals` - Target decimal places
///
/// # Returns
/// * Internal u64 scaled value
pub(crate) fn parse_decimal(amount: Decimal, decimals: u32) -> Result<ScaledAmount, MoneyError> {
    if amount.is_sign_negative() || amount.is_zero() {
        return Err(MoneyError::InvalidAmount);
    }

    // Force strict precision check
    if amount.scale() > decimals {
        return Err(MoneyError::PrecisionOverflow {
            provided: amount.scale(),
            max: decimals,
        });
    }

    let multiplier = Decimal::from(10u64.pow(decimals));
    let scaled = (amount * multiplier).to_u64().ok_or(MoneyError::Overflow)?;

    Ok(ScaledAmount(scaled))
}

// ============================================================================
// Format: Internal → Client (ScaledAmount → String)
// ============================================================================

/// Formats internal ScaledAmount to string with truncation at display_decimals.
///
/// # Arguments
/// * `amount` - Internal u64 scaled value
/// * `asset_decimals` - Asset's internal decimal places (for division)
/// * `display_decimals` - Number of decimals to show in output
///
/// # Example
/// format_amount(150_000_000, 8, 4) -> "1.5000"
pub(crate) fn format_amount(amount: u64, asset_decimals: u32, display_decimals: u32) -> String {
    let decimal_value = Decimal::from(amount) / Decimal::from(10u64.pow(asset_decimals));
    format!("{:.prec$}", decimal_value, prec = display_decimals as usize)
}

/// Formats with full precision of the asset.
///
/// This version preserves all decimal places, useful for internal data
/// exchange where precision loss is unacceptable.
pub(crate) fn format_amount_full(amount: u64, asset_decimals: u32) -> String {
    format_amount(amount, asset_decimals, asset_decimals)
}

/// Formats internal i64 to string with truncation.
pub(crate) fn format_amount_signed(
    amount: i64,
    asset_decimals: u32,
    display_decimals: u32,
) -> String {
    let abs_value = amount.unsigned_abs();
    let formatted = format_amount(abs_value, asset_decimals, display_decimals);
    if amount < 0 {
        format!("-{}", formatted)
    } else {
        formatted
    }
}

// ============================================================================
// SymbolManager-Aware Helpers
// ============================================================================

/// Format quantity for a symbol (uses base_asset decimals)
///
/// Automatically looks up the correct decimals from SymbolManager,
/// preventing parameter errors.
pub fn format_qty(
    value: ScaledAmount,
    symbol_id: u32,
    symbol_mgr: &SymbolManager,
) -> Result<String, MoneyError> {
    let symbol = symbol_mgr
        .get_symbol_info_by_id(symbol_id)
        .ok_or(MoneyError::SymbolNotFound(symbol_id))?;

    let asset = symbol_mgr
        .assets
        .get(&symbol.base_asset_id)
        .ok_or(MoneyError::AssetNotFound(symbol.base_asset_id))?;

    Ok(format_amount(
        *value,
        asset.decimals,
        asset.display_decimals,
    ))
}

/// Format price for a symbol
pub fn format_price(
    value: ScaledAmount,
    symbol_id: u32,
    symbol_mgr: &SymbolManager,
) -> Result<String, MoneyError> {
    let symbol = symbol_mgr
        .get_symbol_info_by_id(symbol_id)
        .ok_or(MoneyError::SymbolNotFound(symbol_id))?;

    Ok(format_amount(
        *value,
        symbol.price_decimal,
        symbol.price_display_decimal,
    ))
}

/// Parse quantity string for a symbol
pub fn parse_qty(
    amount_str: &str,
    symbol_id: u32,
    symbol_mgr: &SymbolManager,
) -> Result<ScaledAmount, MoneyError> {
    let symbol = symbol_mgr
        .get_symbol_info_by_id(symbol_id)
        .ok_or(MoneyError::SymbolNotFound(symbol_id))?;

    let asset = symbol_mgr
        .assets
        .get(&symbol.base_asset_id)
        .ok_or(MoneyError::AssetNotFound(symbol.base_asset_id))?;

    parse_amount(amount_str, asset.decimals)
}

/// Parse price string for a symbol
pub fn parse_price(
    price_str: &str,
    symbol_id: u32,
    symbol_mgr: &SymbolManager,
) -> Result<ScaledAmount, MoneyError> {
    let symbol = symbol_mgr
        .get_symbol_info_by_id(symbol_id)
        .ok_or(MoneyError::SymbolNotFound(symbol_id))?;

    parse_amount(price_str, symbol.price_decimal)
}

// ============================================================================
// Asset-Aware Helpers (for Funding module)
// ============================================================================

/// Format amount using asset decimals directly
///
/// This is useful in the Funding module where we work with AssetInfo
/// directly rather than through SymbolManager.
pub fn format_asset_amount(
    value: ScaledAmountSigned,
    decimals: u32,
    display_decimals: u32,
) -> String {
    format_amount_signed(*value, decimals, display_decimals)
}

/// Parse amount string using asset decimals
pub fn parse_asset_amount(amount_str: &str, decimals: u32) -> Result<ScaledAmount, MoneyError> {
    parse_amount(amount_str, decimals)
}

/// Format asset amount using AssetInfo
pub fn format_with_asset_info(value: ScaledAmountSigned, asset: &AssetInfo) -> String {
    format_amount_signed(*value, asset.decimals, asset.display_decimals)
}

// ============================================================================
// Layer 3: MoneyFormatter - Batch Conversion Entry Point
// ============================================================================

/// Formatter for batch money conversions
///
/// Use this when you need to format multiple prices/quantities for the same symbol.
/// This avoids repeated lookups and ensures consistent formatting.
pub struct MoneyFormatter<'a> {
    pub(crate) symbol_mgr: &'a SymbolManager,
    pub(crate) symbol_id: u32,
    // Cached values for performance
    pub(crate) price_decimals: u32,
    pub(crate) price_display_decimals: u32,
    pub(crate) qty_decimals: u32,
    pub(crate) qty_display_decimals: u32,
}

impl<'a> MoneyFormatter<'a> {
    /// Create a new formatter for a symbol
    pub fn new(symbol_mgr: &'a SymbolManager, symbol_id: u32) -> Option<Self> {
        let symbol = symbol_mgr.get_symbol_info_by_id(symbol_id)?;
        let base_asset = symbol_mgr.assets.get(&symbol.base_asset_id)?;

        Some(Self {
            symbol_mgr,
            symbol_id,
            price_decimals: symbol.price_decimal,
            price_display_decimals: symbol.price_display_decimal,
            qty_decimals: base_asset.decimals,
            qty_display_decimals: base_asset.display_decimals,
        })
    }

    /// Format a single price
    #[inline]
    pub fn format_price(&self, value: ScaledAmount) -> String {
        format_amount(*value, self.price_decimals, self.price_display_decimals)
    }

    /// Format a single quantity
    #[inline]
    pub fn format_qty(&self, value: ScaledAmount) -> String {
        format_amount(*value, self.qty_decimals, self.qty_display_decimals)
    }

    /// Format depth level [price, qty] as [String; 2]
    #[inline]
    pub fn format_depth_level(&self, price: ScaledAmount, qty: ScaledAmount) -> [String; 2] {
        [self.format_price(price), self.format_qty(qty)]
    }

    /// Format multiple depth levels
    pub fn format_depth_levels(&self, levels: &[(ScaledAmount, ScaledAmount)]) -> Vec<[String; 2]> {
        levels
            .iter()
            .map(|(p, q)| self.format_depth_level(*p, *q))
            .collect()
    }

    /// Format bids and asks together
    pub fn format_depth(
        &self,
        bids: &[(ScaledAmount, ScaledAmount)],
        asks: &[(ScaledAmount, ScaledAmount)],
    ) -> (Vec<[String; 2]>, Vec<[String; 2]>) {
        (
            self.format_depth_levels(bids),
            self.format_depth_levels(asks),
        )
    }

    /// Get symbol_id this formatter is configured for
    pub fn symbol_id(&self) -> u32 {
        self.symbol_id
    }

    /// Get reference to underlying SymbolManager
    pub fn symbol_mgr(&self) -> &'a SymbolManager {
        self.symbol_mgr
    }
}

// ============================================================================
// Legacy Compatibility (to be removed after migration)
// ============================================================================

/// Legacy decimal_to_u64 (redirects to scale_up_decimal)
#[deprecated(note = "Use parse_decimal instead")]
pub fn decimal_to_u64(decimal: Decimal, decimals: u32) -> Result<u64, &'static str> {
    parse_decimal(decimal, decimals)
        .map(|s| *s)
        .map_err(|_| "Conversion failed")
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // ========================================================================
    // parse_amount Tests
    // ========================================================================

    // ========================================================================
    // QA COMPREHENSIVE TEST SUITE
    // ========================================================================

    #[test]
    fn qa_parse_amount_variations() {
        // Normal cases
        assert_eq!(*parse_amount("1.23", 2).unwrap(), 123);
        assert_eq!(*parse_amount("1.23", 8).unwrap(), 123_000_000);

        // Leading/Trailing zeros
        assert_eq!(*parse_amount("001.23", 2).unwrap(), 123);
        assert_eq!(*parse_amount("1.2300", 8).unwrap(), 123_000_000);
        assert_eq!(*parse_amount("0.0001", 4).unwrap(), 1);

        // Zero representations (All rejected as we expect positive non-zero amounts)
        assert!(parse_amount("0", 2).is_err());
        assert!(parse_amount("0.00", 2).is_err());
    }

    #[test]
    fn qa_parse_amount_invalid_formats() {
        let cases = vec![
            "1,000.00", // Commas not allowed
            "1.2.3",    // Multiple dots
            "1. 23",    // Spaces inside
            "+1.23",    // Explicit plus rejected
            "1e2",      // Scientific notation rejected
            "0x12",     // Hex rejected
            ".",        // Just a dot rejected
            "1..",      // Multiple dots at end rejected
            ".5",       // Missing leading zero rejected (STRICT)
            "5.",       // Missing fractional part rejected (STRICT)
            "100.0",    // Dot with scale 0 rejected (STRICT)
        ];

        // Test scale 8 cases
        for case in &cases[..cases.len() - 1] {
            assert!(
                parse_amount(case, 8).is_err(),
                "Should reject invalid format: {}",
                case
            );
        }

        // Test scale 0 case
        assert!(parse_amount("100.0", 0).is_err());
    }

    #[test]
    fn qa_parse_amount_precision_limits() {
        // Exact limit
        assert!(parse_amount("1.234", 3).is_ok());

        // Overflow 1 unit
        let res = parse_amount("1.2345", 3);
        assert!(matches!(
            res,
            Err(MoneyError::PrecisionOverflow {
                provided: 4,
                max: 3
            })
        ));

        // No decimals allowed (Scale 0)
        assert_eq!(*parse_amount("100", 0).unwrap(), 100);
    }

    #[test]
    fn qa_parse_amount_u64_boundary() {
        // Max u64 is 18,446,744,073,709,551,615
        // Scale 8: 184,467,440,737.09551615
        let max_s8 = "184467440737.09551615";
        assert_eq!(*parse_amount(max_s8, 8).unwrap(), u64::MAX);

        // Overflow
        let too_big = "184467440737.09551616";
        assert!(matches!(
            parse_amount(too_big, 8),
            Err(MoneyError::Overflow)
        ));

        // High integer part before scaling
        let way_too_big = "999999999999999999999";
        assert!(matches!(
            parse_amount(way_too_big, 0),
            Err(MoneyError::Overflow)
        ));
    }

    #[test]
    fn qa_parse_decimal_edge_cases() {
        // Decimal with high scale but trailing zeros
        let d = Decimal::from_str("1.23000").unwrap(); // scale is 5
        assert!(parse_decimal(d, 2).is_err());

        // Normal conversion
        let d = Decimal::from_str("1.23").unwrap();
        assert_eq!(*parse_decimal(d, 8).unwrap(), 123_000_000);
    }

    #[test]
    fn qa_format_amount_truncation() {
        let val = 199_900_000;
        assert_eq!(format_amount(val, 8, 2), "1.99");
        assert_eq!(format_amount(val, 8, 1), "1.9");
        assert_eq!(format_amount(val, 8, 0), "1");
        assert_eq!(format_amount(val, 8, 8), "1.99900000");
    }

    #[test]
    fn qa_format_amount_signed_extremes() {
        assert_eq!(format_amount_signed(i64::MAX, 8, 2), "92233720368.54");
        assert_eq!(format_amount_signed(i64::MIN, 8, 2), "-92233720368.54");
        assert_eq!(format_amount_signed(-1, 8, 8), "-0.00000001");
        assert_eq!(format_amount_signed(1, 8, 8), "0.00000001");
    }

    #[test]
    fn qa_roundtrip_consistency() {
        let scales = vec![0, 2, 6, 8, 12, 17]; // 18 is too tight for u64 roundtrips with large numbers
        let values = vec!["1", "1.5", "0.00000001", "1234.5678", "999999.999999"];

        for scale in scales {
            for val_str in &values {
                if let Some(dot_pos) = val_str.find('.') {
                    if val_str.len() - dot_pos - 1 > scale as usize {
                        continue;
                    }
                } else if scale == 0 && val_str.contains('.') {
                    continue;
                }

                if let Ok(internal) = parse_amount(val_str, scale) {
                    let formatted = format_amount_full(*internal, scale);
                    let internal_back = parse_amount(&formatted, scale).unwrap();
                    assert_eq!(
                        internal, internal_back,
                        "Roundtrip failed for {} at scale {}",
                        val_str, scale
                    );
                }
            }
        }
    }

    #[test]
    fn qa_eth_precision_limits() {
        // ETH 18 decimals
        // u64::MAX at scale 18 is ~18.44 ETH
        let limit_eth = "18.446744073709551615";
        assert_eq!(*parse_amount(limit_eth, 18).unwrap(), u64::MAX);

        let overflow_eth = "18.446744073709551616";
        assert!(matches!(
            parse_amount(overflow_eth, 18),
            Err(MoneyError::Overflow)
        ));

        // Safe ETH amount
        let ten_eth = "10.000000000000000000";
        let internal = parse_amount("10.0", 18).unwrap();
        assert_eq!(format_amount_full(*internal, 18), ten_eth);
    }
}

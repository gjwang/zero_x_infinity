//! Input validation for asset and symbol names
//!
//! This module provides validated types that enforce uppercase naming rules.
//! All fields are private to force validation through the public API.

use std::fmt;

// ============================================================================
// Validation Errors
// ============================================================================

/// Validation errors for asset and symbol names
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ValidationError {
    #[error("Asset name must be uppercase: got '{got}', expected '{expected}'")]
    AssetNotUppercase { got: String, expected: String },

    #[error("Symbol name must be uppercase: got '{got}', expected '{expected}'")]
    SymbolNotUppercase { got: String, expected: String },

    #[error("Invalid length for {field}: expected {min}-{max}, got {actual}")]
    InvalidLength {
        field: &'static str,
        min: usize,
        max: usize,
        actual: usize,
    },

    #[error("Invalid format for {field}: '{value}' (expected: {expected})")]
    InvalidFormat {
        field: &'static str,
        value: String,
        expected: &'static str,
    },

    #[error("Symbol must contain underscore separator: got '{got}'")]
    MissingUnderscoreSeparator { got: String },
}

// ============================================================================
// AssetName - Validated Asset Name (Private Fields)
// ============================================================================

/// Validated asset name (guaranteed uppercase, valid format)
///
/// Fields are private to force validation through `new()`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetName(String);

impl AssetName {
    /// Create a new validated AssetName
    ///
    /// # Validation Rules
    /// - Must be uppercase (A-Z, 0-9, _)
    /// - Length: 1-16 characters
    /// - Regex: ^[A-Z0-9_]{1,16}$
    ///
    /// # Errors
    /// Returns `ValidationError` if validation fails
    ///
    /// # Examples
    /// ```
    /// use zero_x_infinity::account::validation::AssetName;
    ///
    /// let btc = AssetName::new("BTC").unwrap();
    /// assert_eq!(btc.as_str(), "BTC");
    ///
    /// let err = AssetName::new("btc");
    /// assert!(err.is_err()); // lowercase rejected
    /// ```
    pub fn new(name: &str) -> Result<Self, ValidationError> {
        let name = name.trim();

        // Check length
        if name.is_empty() || name.len() > 16 {
            return Err(ValidationError::InvalidLength {
                field: "asset",
                min: 1,
                max: 16,
                actual: name.len(),
            });
        }

        // Check uppercase
        let expected = name.to_uppercase();
        if name != expected {
            return Err(ValidationError::AssetNotUppercase {
                got: name.to_string(),
                expected,
            });
        }

        // Check format: only A-Z, 0-9, _
        if !name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(ValidationError::InvalidFormat {
                field: "asset",
                value: name.to_string(),
                expected: "uppercase letters, numbers, underscore only",
            });
        }

        Ok(Self(name.to_string()))
    }

    /// Get the validated asset name as &str
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert into owned String
    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for AssetName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for AssetName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// ============================================================================
// SymbolName - Validated Symbol Name (Private Fields)
// ============================================================================

/// Validated symbol name (guaranteed uppercase, BASE_QUOTE format)
///
/// Fields are private to force validation through `new()`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolName(String);

impl SymbolName {
    /// Create a new validated SymbolName
    ///
    /// # Validation Rules
    /// - Must be uppercase (A-Z, 0-9, _)
    /// - Must contain exactly one underscore separator
    /// - Length: 3-32 characters
    /// - Format: BASE_QUOTE
    /// - Regex: ^[A-Z0-9]+_[A-Z0-9]+$
    ///
    /// # Errors
    /// Returns `ValidationError` if validation fails
    ///
    /// # Examples
    /// ```
    /// use zero_x_infinity::account::validation::SymbolName;
    ///
    /// let symbol = SymbolName::new("BTC_USDT").unwrap();
    /// assert_eq!(symbol.as_str(), "BTC_USDT");
    ///
    /// let err = SymbolName::new("BTCUSDT");
    /// assert!(err.is_err()); // missing underscore
    /// ```
    pub fn new(name: &str) -> Result<Self, ValidationError> {
        let name = name.trim();

        // Check length
        if name.len() < 3 || name.len() > 32 {
            return Err(ValidationError::InvalidLength {
                field: "symbol",
                min: 3,
                max: 32,
                actual: name.len(),
            });
        }

        // Check uppercase
        let expected = name.to_uppercase();
        if name != expected {
            return Err(ValidationError::SymbolNotUppercase {
                got: name.to_string(),
                expected,
            });
        }

        // Check format: only A-Z, 0-9, _
        if !name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(ValidationError::InvalidFormat {
                field: "symbol",
                value: name.to_string(),
                expected: "uppercase letters, numbers, underscore only",
            });
        }

        // Check underscore separator
        let underscore_count = name.chars().filter(|&c| c == '_').count();
        if underscore_count == 0 {
            return Err(ValidationError::MissingUnderscoreSeparator {
                got: name.to_string(),
            });
        }

        // Check no double underscore, leading/trailing underscore
        if name.contains("__") || name.starts_with('_') || name.ends_with('_') {
            return Err(ValidationError::InvalidFormat {
                field: "symbol",
                value: name.to_string(),
                expected: "single underscore separator, no leading/trailing underscore",
            });
        }

        Ok(Self(name.to_string()))
    }

    /// Get the validated symbol name as &str
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Convert into owned String
    pub fn into_string(self) -> String {
        self.0
    }

    /// Split into base and quote asset names
    ///
    /// # Examples
    /// ```
    /// use zero_x_infinity::account::validation::SymbolName;
    ///
    /// let symbol = SymbolName::new("BTC_USDT").unwrap();
    /// let (base, quote) = symbol.split_base_quote();
    /// assert_eq!(base, "BTC");
    /// assert_eq!(quote, "USDT");
    /// ```
    pub fn split_base_quote(&self) -> (&str, &str) {
        let parts: Vec<&str> = self.0.split('_').collect();
        (parts[0], parts[1])
    }
}

impl fmt::Display for SymbolName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for SymbolName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // AssetName Tests
    // ========================================================================

    #[test]
    fn test_asset_name_valid() {
        assert!(AssetName::new("BTC").is_ok());
        assert!(AssetName::new("USDT").is_ok());
        assert!(AssetName::new("ETH").is_ok());
        assert!(AssetName::new("BTC2").is_ok());
        assert!(AssetName::new("STABLE_COIN").is_ok());
        assert!(AssetName::new("A").is_ok()); // single char allowed
    }

    #[test]
    fn test_asset_name_uppercase_required() {
        let err = AssetName::new("btc").unwrap_err();
        assert!(matches!(err, ValidationError::AssetNotUppercase { .. }));

        let err = AssetName::new("Btc").unwrap_err();
        assert!(matches!(err, ValidationError::AssetNotUppercase { .. }));
    }

    #[test]
    fn test_asset_name_invalid_length() {
        let err = AssetName::new("").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidLength { .. }));

        let err = AssetName::new("VERYLONGASSETCODE").unwrap_err(); // 17 chars
        assert!(matches!(err, ValidationError::InvalidLength { .. }));
    }

    #[test]
    fn test_asset_name_invalid_chars() {
        let err = AssetName::new("BTC-USD").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidFormat { .. }));

        let err = AssetName::new("BTC!").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidFormat { .. }));

        let err = AssetName::new("BTC USD").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidFormat { .. }));
    }

    #[test]
    fn test_asset_name_as_str() {
        let asset = AssetName::new("BTC").unwrap();
        assert_eq!(asset.as_str(), "BTC");
        assert_eq!(asset.to_string(), "BTC");
    }

    // ========================================================================
    // SymbolName Tests
    // ========================================================================

    #[test]
    fn test_symbol_name_valid() {
        assert!(SymbolName::new("BTC_USDT").is_ok());
        assert!(SymbolName::new("ETH_BTC").is_ok());
        assert!(SymbolName::new("USDT_USD").is_ok());
        assert!(SymbolName::new("A_B").is_ok()); // minimum length

        // Symbols starting with numbers (allowed)
        assert!(SymbolName::new("1000SHIB_USDT").is_ok());
        assert!(SymbolName::new("1INCH_USDT").is_ok());
        assert!(SymbolName::new("2KEY_BTC").is_ok());
    }

    #[test]
    fn test_symbol_name_uppercase_required() {
        let err = SymbolName::new("btc_usdt").unwrap_err();
        assert!(matches!(err, ValidationError::SymbolNotUppercase { .. }));

        let err = SymbolName::new("Btc_Usdt").unwrap_err();
        assert!(matches!(err, ValidationError::SymbolNotUppercase { .. }));
    }

    #[test]
    fn test_symbol_name_missing_underscore() {
        let err = SymbolName::new("BTCUSDT").unwrap_err();
        assert!(matches!(
            err,
            ValidationError::MissingUnderscoreSeparator { .. }
        ));
    }

    #[test]
    fn test_symbol_name_invalid_underscore() {
        let err = SymbolName::new("BTC__USDT").unwrap_err(); // double underscore
        assert!(matches!(err, ValidationError::InvalidFormat { .. }));

        let err = SymbolName::new("_BTCUSDT").unwrap_err(); // leading underscore
        assert!(matches!(err, ValidationError::InvalidFormat { .. }));

        let err = SymbolName::new("BTCUSDT_").unwrap_err(); // trailing underscore
        assert!(matches!(err, ValidationError::InvalidFormat { .. }));
    }

    #[test]
    fn test_symbol_name_invalid_length() {
        let err = SymbolName::new("AB").unwrap_err(); // too short
        assert!(matches!(err, ValidationError::InvalidLength { .. }));

        let err = SymbolName::new("VERYLONGBASENAME_VERYLONGQUOTE123").unwrap_err(); // 33 chars
        assert!(matches!(err, ValidationError::InvalidLength { .. }));
    }

    #[test]
    fn test_symbol_name_invalid_chars() {
        let err = SymbolName::new("BTC-USDT").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidFormat { .. }));

        let err = SymbolName::new("BTC!USDT").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidFormat { .. }));
    }

    #[test]
    fn test_symbol_name_split_base_quote() {
        let symbol = SymbolName::new("BTC_USDT").unwrap();
        let (base, quote) = symbol.split_base_quote();
        assert_eq!(base, "BTC");
        assert_eq!(quote, "USDT");
    }

    #[test]
    fn test_symbol_name_as_str() {
        let symbol = SymbolName::new("BTC_USDT").unwrap();
        assert_eq!(symbol.as_str(), "BTC_USDT");
        assert_eq!(symbol.to_string(), "BTC_USDT");
    }
}

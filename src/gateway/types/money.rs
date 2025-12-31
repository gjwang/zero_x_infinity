//! Money types for API boundary enforcement
//!
//! - `StrictDecimal`: Format-validated input type
//! - `DisplayAmount`: Type-safe output formatting

use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};

// ============================================================================
// StrictDecimal: Format-Validated Decimal at Serde Layer
// ============================================================================

/// Strict format Decimal - validates format during deserialization
///
/// This type provides format validation at the Serde layer:
/// - Rejects `.5` (must be `0.5`)
/// - Rejects `5.` (must be `5.0` or `5`)
/// - Rejects negative numbers
/// - Rejects empty strings
/// - Rejects scientific notation
///
/// Business validation (precision, range) happens later in SymbolManager.
#[derive(Debug, Clone, Copy)]
pub struct StrictDecimal(Decimal);

impl StrictDecimal {
    /// Get the inner Decimal value
    pub fn inner(self) -> Decimal {
        self.0
    }

    /// Create from Decimal (for testing)
    #[cfg(test)]
    pub fn from_decimal(d: Decimal) -> Self {
        Self(d)
    }
}

impl std::ops::Deref for StrictDecimal {
    type Target = Decimal;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de> Deserialize<'de> for StrictDecimal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        // Only accept JSON strings for strict format control
        // JSON numbers bypass our format validation, so we reject them
        let s = String::deserialize(deserializer)?;

        // Strict format validation
        if s.is_empty() {
            return Err(D::Error::custom("Amount cannot be empty"));
        }

        // Reject .5 format (must be 0.5)
        if s.starts_with('.') {
            return Err(D::Error::custom("Invalid format: use 0.5 not .5"));
        }

        // Reject 5. format (must be 5.0 or 5)
        if s.ends_with('.') {
            return Err(D::Error::custom("Invalid format: use 5.0 not 5."));
        }

        // Reject scientific notation (1.5e8, 1E10, etc.)
        if s.contains('e') || s.contains('E') {
            return Err(D::Error::custom(
                "Invalid format: scientific notation not allowed",
            ));
        }

        // Reject + prefix (should be implicit)
        if s.starts_with('+') {
            return Err(D::Error::custom("Invalid format: + prefix not allowed"));
        }

        // Parse using Decimal library
        let d = Decimal::from_str(&s)
            .map_err(|e| D::Error::custom(format!("Invalid decimal: {}", e)))?;

        // Reject negative numbers
        if d.is_sign_negative() {
            return Err(D::Error::custom("Amount cannot be negative"));
        }

        Ok(StrictDecimal(d))
    }
}

impl Serialize for StrictDecimal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as string to preserve precision
        serializer.serialize_str(&self.0.to_string())
    }
}

// ============================================================================
// DisplayAmount: Type-Safe Output for API Responses
// ============================================================================

/// Display amount for API responses - ensures all monetary output goes through
/// controlled formatting.
///
/// **Design Principles:**
/// 1. No public constructor - only SymbolManager can create instances
/// 2. Always serializes as JSON string (preserves precision)
/// 3. Formatting includes display_decimals truncation
#[derive(Debug, Clone)]
pub struct DisplayAmount(String);

impl DisplayAmount {
    /// Create a new DisplayAmount from a formatted string.
    ///
    /// This is `pub(crate)` to restrict construction to SymbolManager.
    /// External code cannot bypass the formatting layer.
    pub(crate) fn new(s: String) -> Self {
        Self(s)
    }

    /// Get the inner string value (for display/logging)
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Create DisplayAmount for testing only
    #[cfg(test)]
    pub fn from_str_test(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl std::fmt::Display for DisplayAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for DisplayAmount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Always serialize as string to preserve precision
        serializer.serialize_str(&self.0)
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strict_decimal_valid_string() {
        let json = r#""1.5""#;
        let d: StrictDecimal = serde_json::from_str(json).unwrap();
        assert_eq!(*d, Decimal::from_str("1.5").unwrap());
    }

    #[test]
    fn test_strict_decimal_rejects_json_number() {
        let json = r#"1.5"#;
        let result: Result<StrictDecimal, _> = serde_json::from_str(json);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("expected a string")
        );
    }

    #[test]
    fn test_strict_decimal_rejects_dot_prefix() {
        let json = r#"".5""#;
        let result: Result<StrictDecimal, _> = serde_json::from_str(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("use 0.5 not .5"));
    }

    #[test]
    fn test_strict_decimal_rejects_dot_suffix() {
        let json = r#""5.""#;
        let result: Result<StrictDecimal, _> = serde_json::from_str(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("use 5.0 not 5."));
    }

    #[test]
    fn test_strict_decimal_rejects_negative_string() {
        let json = r#""-1.5""#;
        let result: Result<StrictDecimal, _> = serde_json::from_str(json);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("cannot be negative")
        );
    }

    #[test]
    fn test_strict_decimal_rejects_scientific_notation() {
        let json = r#""1.5e8""#;
        let result: Result<StrictDecimal, _> = serde_json::from_str(json);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("scientific notation")
        );
    }

    #[test]
    fn test_strict_decimal_rejects_empty() {
        let json = r#""""#;
        let result: Result<StrictDecimal, _> = serde_json::from_str(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_display_amount_serializes_as_string() {
        let amount = DisplayAmount::new("123.45".to_string());
        let json = serde_json::to_string(&amount).unwrap();
        assert_eq!(json, r#""123.45""#);
    }
}

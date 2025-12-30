use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::models::{InternalOrder, OrderStatus, OrderType, Side, TimeInForce};
use crate::symbol_manager::SymbolManager;

/// Custom deserializer for non-empty strings
fn deserialize_non_empty_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.is_empty() {
        return Err(serde::de::Error::custom("string cannot be empty"));
    }
    Ok(s)
}

/// Client order (HTTP request deserialization)
///
/// This struct is used for HTTP API deserialization only.
/// Validation and conversion happen in separate functions.
#[derive(Debug, Clone, Deserialize)]
pub struct ClientOrder {
    /// Client order ID (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
    /// Trading symbol (must not be empty)
    #[serde(deserialize_with = "deserialize_non_empty_string")]
    pub symbol: String,
    /// Side: "BUY" | "SELL" (SCREAMING_CASE)
    pub side: Side,
    /// Order type: "LIMIT" | "MARKET" (SCREAMING_CASE)
    pub order_type: OrderType,
    /// Time in force: "GTC" | "IOC" (optional, defaults to GTC)
    #[serde(default)]
    pub time_in_force: TimeInForce,
    /// Price (required for LIMIT orders)
    pub price: Option<Decimal>,
    /// Quantity (unified field name)
    pub qty: Decimal,
}

/// Validate and parse ClientOrder to ValidatedClientOrder
///
/// This function performs business validation only.
/// Type validation is handled by serde during deserialization.
pub fn validate_client_order(
    req: ClientOrder,
    symbol_mgr: &SymbolManager,
) -> Result<ValidatedClientOrder, &'static str> {
    // 1. Resolve symbol_id from symbol name
    let symbol_id = symbol_mgr
        .get_symbol_id(&req.symbol)
        .ok_or("Symbol not found")?;

    // 2. Get symbol info for precision
    let symbol_info = symbol_mgr
        .get_symbol_info_by_id(symbol_id)
        .ok_or("Symbol info not found")?;

    // 3. Validate price for LIMIT orders
    let price = if req.order_type == OrderType::Limit {
        match req.price {
            Some(p) => {
                if p.is_zero() {
                    return Err("Price must be greater than zero");
                }
                if p.is_sign_negative() {
                    return Err("Price cannot be negative");
                }
                // Check decimal places
                if p.scale() > symbol_info.price_decimal {
                    return Err("Too many decimal places in price");
                }
                p
            }
            None => return Err("Price is required for LIMIT orders"),
        }
    } else {
        Decimal::ZERO // Market order
    };

    // 4. Validate quantity
    if req.qty.is_zero() {
        return Err("Quantity must be greater than zero");
    }
    if req.qty.is_sign_negative() {
        return Err("Quantity cannot be negative");
    }

    let base_asset = symbol_mgr
        .assets
        .get(&symbol_info.base_asset_id)
        .ok_or("Base asset not found")?;

    // Check decimal places
    if req.qty.scale() > base_asset.decimals {
        return Err("Too many decimal places in quantity");
    }

    Ok(ValidatedClientOrder {
        cid: req.cid,
        symbol_id,
        side: req.side,
        order_type: req.order_type,
        time_in_force: req.time_in_force,
        price,
        qty: req.qty,
        price_decimals: symbol_info.price_decimal,
        qty_decimals: base_asset.decimals,
    })
}

/// Validated client order with typed fields
///
/// After validation, all fields are properly typed and Decimal values are validated
#[derive(Debug)]
pub struct ValidatedClientOrder {
    pub cid: Option<String>,
    pub symbol_id: u32,
    pub side: Side,
    pub order_type: OrderType,
    pub time_in_force: TimeInForce,
    pub price: Decimal,      // Validated Decimal (0 for MARKET orders)
    pub qty: Decimal,        // Validated Decimal
    pub price_decimals: u32, // For conversion
    pub qty_decimals: u32,   // For conversion
}

impl ValidatedClientOrder {
    /// Convert to InternalOrder
    ///
    /// This is where Decimal -> u64 conversion happens
    pub fn into_internal_order(
        self,
        order_id: u64,
        user_id: u64,
        ingested_at_ns: u64,
    ) -> Result<InternalOrder, &'static str> {
        // Convert Decimal to u64
        let price_u64 = decimal_to_u64(self.price, self.price_decimals)?;
        let qty_u64 = decimal_to_u64(self.qty, self.qty_decimals)?;

        Ok(InternalOrder {
            order_id,
            user_id,
            symbol_id: self.symbol_id,
            price: price_u64,
            qty: qty_u64,
            filled_qty: 0,
            side: self.side,
            order_type: self.order_type,
            time_in_force: self.time_in_force,
            status: OrderStatus::NEW,
            lock_version: 0,
            seq_id: 0,
            ingested_at_ns,
            cid: self.cid,
        })
    }
}

/// Convert Decimal to u64
///
/// Multiplies by 10^decimals and converts to u64
pub fn decimal_to_u64(decimal: Decimal, decimals: u32) -> Result<u64, &'static str> {
    let multiplier = Decimal::from(10u64.pow(decimals));
    let result = decimal * multiplier;

    // Should not have fractional part after multiplication
    if !result.fract().is_zero() {
        return Err("Unexpected fractional part after scaling");
    }

    result.to_u64().ok_or("Number too large")
}

/// Cancel order request
#[derive(Debug, Deserialize, ToSchema)]
pub struct CancelOrderRequest {
    pub order_id: u64,
}

/// Reduce order request
#[derive(Debug, Deserialize, ToSchema)]
pub struct ReduceOrderRequest {
    pub order_id: u64,
    #[schema(value_type = String)]
    pub reduce_qty: Decimal,
}

/// Move order request
#[derive(Debug, Deserialize, ToSchema)]
pub struct MoveOrderRequest {
    pub order_id: u64,
    #[schema(value_type = String)]
    pub new_price: Decimal,
}

// ============================================================================
// Unified API Response Format
// ============================================================================

/// Unified API response wrapper
///
/// All API responses follow this structure:
/// - code: 0 = success, non-zero = error code
/// - msg: short message description
/// - data: actual data (success) or null (error)
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiResponse<T> {
    /// Response code: 0 for success, non-zero for errors
    #[schema(example = 0)]
    pub code: i32,
    /// Response message
    #[schema(example = "ok")]
    pub msg: String,
    /// Response data (only present when code == 0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    /// Create success response
    pub fn success(data: T) -> Self {
        Self {
            code: 0,
            msg: "ok".to_string(),
            data: Some(data),
        }
    }

    /// Create error response
    pub fn error(code: i32, msg: impl Into<String>) -> ApiResponse<()> {
        ApiResponse {
            code,
            msg: msg.into(),
            data: None,
        }
    }
}

/// Order response data
#[derive(Debug, Serialize)]
pub struct OrderResponseData {
    pub order_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
    pub order_status: String,
    pub accepted_at: u64,
}

/// Account response data (for legacy /account endpoint)
#[derive(Debug, Serialize, ToSchema)]
pub struct AccountResponseData {
    pub balances: Vec<crate::funding::service::BalanceInfo>,
}

/// Market depth API response data
#[derive(Debug, Serialize, ToSchema)]
pub struct DepthApiData {
    /// Trading symbol name
    #[schema(example = "BTC_USDT")]
    pub symbol: String,
    /// Bid levels [[price, qty], ...]
    #[schema(example = json!([["85000.00", "0.5"], ["84999.00", "1.2"]]))]
    pub bids: Vec<[String; 2]>,
    /// Ask levels [[price, qty], ...]
    #[schema(example = json!([["85001.00", "0.3"], ["85002.00", "0.8"]]))]
    pub asks: Vec<[String; 2]>,
    /// Last update sequence ID
    #[schema(example = 12345)]
    pub last_update_id: u64,
}

/// Error codes
pub mod error_codes {
    // Success
    pub const SUCCESS: i32 = 0;

    // Client errors (1xxx)
    pub const INVALID_PARAMETER: i32 = 1001;
    pub const INSUFFICIENT_BALANCE: i32 = 1002;
    pub const INVALID_PRICE_QTY: i32 = 1003;

    // Auth errors (2xxx)
    pub const MISSING_AUTH: i32 = 2001;
    pub const AUTH_FAILED: i32 = 2002;

    // Resource errors (4xxx)
    pub const ORDER_NOT_FOUND: i32 = 4001;
    pub const RATE_LIMITED: i32 = 4291;

    // Server errors (5xxx)
    pub const INTERNAL_ERROR: i32 = 5000;
    pub const SERVICE_UNAVAILABLE: i32 = 5001;
}

// Legacy types for backward compatibility (to be removed)
#[deprecated(note = "Use ApiResponse<OrderResponseData> instead")]
#[derive(Debug, Serialize)]
pub struct OrderResponse {
    pub order_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
    pub status: String,
    pub accepted_at: u64,
}

#[allow(deprecated)]
#[deprecated(note = "Use ApiResponse::error() instead")]
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[deprecated(note = "Use ApiResponse::error() instead")]
#[derive(Debug, Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[allow(deprecated)]
impl ErrorResponse {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: ErrorDetail {
                code: code.into(),
                message: message.into(),
                details: None,
            },
        }
    }

    pub fn with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self {
            error: ErrorDetail {
                code: code.into(),
                message: message.into(),
                details: Some(details),
            },
        }
    }
}

// Unit tests for gateway types and validation

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv_io::load_symbol_manager;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    #[test]
    fn test_deserialize_client_order_limit() {
        let json = r#"{"symbol":"BTC_USDT","side":"BUY","order_type":"LIMIT","price":85000.00,"qty":0.001}"#;
        let order: ClientOrder = serde_json::from_str(json).unwrap();
        assert_eq!(order.symbol, "BTC_USDT");
        assert_eq!(order.side, Side::Buy);
        assert_eq!(order.order_type, OrderType::Limit);
        assert!(order.price.is_some());
    }

    #[test]
    fn test_deserialize_client_order_market() {
        let json = r#"{"symbol":"BTC_USDT","side":"SELL","order_type":"MARKET","qty":0.002}"#;
        let order: ClientOrder = serde_json::from_str(json).unwrap();
        assert_eq!(order.side, Side::Sell);
        assert_eq!(order.order_type, OrderType::Market);
        assert!(order.price.is_none());
    }

    #[test]
    fn test_deserialize_empty_symbol_fails() {
        let json = r#"{"symbol":"","side":"BUY","order_type":"LIMIT","price":85000,"qty":0.001}"#;
        let result: Result<ClientOrder, _> = serde_json::from_str(json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_deserialize_invalid_side_fails() {
        let json = r#"{"symbol":"BTC_USDT","side":"INVALID","order_type":"LIMIT","price":85000,"qty":0.001}"#;
        let result: Result<ClientOrder, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_limit_order_success() {
        let (symbol_mgr, _) = load_symbol_manager().unwrap();
        let order = ClientOrder {
            cid: Some("test-001".to_string()),
            symbol: "BTC_USDT".to_string(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            price: Some(Decimal::from_str("85000.50").unwrap()),
            qty: Decimal::from_str("0.001").unwrap(),
            time_in_force: TimeInForce::GTC,
        };
        let result = validate_client_order(order, &symbol_mgr);
        assert!(result.is_ok());
        let validated = result.unwrap();
        assert_eq!(validated.side, Side::Buy);
        assert_eq!(validated.order_type, OrderType::Limit);
    }

    #[test]
    fn test_validate_market_order_success() {
        let (symbol_mgr, _) = load_symbol_manager().unwrap();
        let order = ClientOrder {
            cid: None,
            symbol: "BTC_USDT".to_string(),
            side: Side::Sell,
            order_type: OrderType::Market,
            price: None,
            qty: Decimal::from_str("0.002").unwrap(),
            time_in_force: TimeInForce::GTC,
        };
        let result = validate_client_order(order, &symbol_mgr);
        assert!(result.is_ok());
        let validated = result.unwrap();
        assert_eq!(validated.price, Decimal::ZERO);
    }

    #[test]
    fn test_validate_limit_missing_price_fails() {
        let (symbol_mgr, _) = load_symbol_manager().unwrap();
        let order = ClientOrder {
            cid: None,
            symbol: "BTC_USDT".to_string(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            price: None,
            qty: Decimal::from_str("0.001").unwrap(),
            time_in_force: TimeInForce::GTC,
        };
        let result = validate_client_order(order, &symbol_mgr);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Price is required for LIMIT orders");
    }

    #[test]
    fn test_validate_zero_price_fails() {
        let (symbol_mgr, _) = load_symbol_manager().unwrap();
        let order = ClientOrder {
            cid: None,
            symbol: "BTC_USDT".to_string(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            price: Some(Decimal::ZERO),
            qty: Decimal::from_str("0.001").unwrap(),
            time_in_force: TimeInForce::GTC,
        };
        let result = validate_client_order(order, &symbol_mgr);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Price must be greater than zero");
    }

    #[test]
    fn test_validate_negative_price_fails() {
        let (symbol_mgr, _) = load_symbol_manager().unwrap();
        let order = ClientOrder {
            cid: None,
            symbol: "BTC_USDT".to_string(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            price: Some(Decimal::from_str("-100").unwrap()),
            qty: Decimal::from_str("0.001").unwrap(),
            time_in_force: TimeInForce::GTC,
        };
        let result = validate_client_order(order, &symbol_mgr);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Price cannot be negative");
    }

    #[test]
    fn test_validate_zero_qty_fails() {
        let (symbol_mgr, _) = load_symbol_manager().unwrap();
        let order = ClientOrder {
            cid: None,
            symbol: "BTC_USDT".to_string(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            price: Some(Decimal::from_str("85000").unwrap()),
            qty: Decimal::ZERO,
            time_in_force: TimeInForce::GTC,
        };
        let result = validate_client_order(order, &symbol_mgr);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Quantity must be greater than zero");
    }

    #[test]
    fn test_validate_unknown_symbol_fails() {
        let (symbol_mgr, _) = load_symbol_manager().unwrap();
        let order = ClientOrder {
            cid: None,
            symbol: "UNKNOWN_SYMBOL".to_string(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            price: Some(Decimal::from_str("1000").unwrap()),
            qty: Decimal::from_str("1.0").unwrap(),
            time_in_force: TimeInForce::GTC,
        };
        let result = validate_client_order(order, &symbol_mgr);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Symbol not found");
    }

    #[test]
    fn test_into_internal_order() {
        let validated = ValidatedClientOrder {
            cid: Some("test-123".to_string()),
            symbol_id: 1,
            side: Side::Buy,
            order_type: OrderType::Limit,
            price: Decimal::from_str("85000.50").unwrap(),
            qty: Decimal::from_str("0.001").unwrap(),
            time_in_force: TimeInForce::GTC,
            price_decimals: 2,
            qty_decimals: 8,
        };
        let result = validated.into_internal_order(100, 1001, 1234567890);
        assert!(result.is_ok());
        let internal = result.unwrap();
        assert_eq!(internal.order_id, 100);
        assert_eq!(internal.user_id, 1001);
        assert_eq!(internal.price, 8500050); // 85000.50 * 100
        assert_eq!(internal.qty, 100000); // 0.001 * 100000000
        assert_eq!(internal.status, OrderStatus::NEW);
    }

    /// Test that IOC time_in_force correctly propagates from ClientOrder to InternalOrder
    #[test]
    fn test_ioc_time_in_force_passthrough() {
        let (symbol_mgr, _) = load_symbol_manager().unwrap();

        // Create a client order with IOC time_in_force
        let order = ClientOrder {
            cid: Some("ioc-test-001".to_string()),
            symbol: "BTC_USDT".to_string(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            time_in_force: TimeInForce::IOC, // Explicitly set IOC
            price: Some(Decimal::from_str("85000.00").unwrap()),
            qty: Decimal::from_str("0.001").unwrap(),
        };

        // Validate
        let validated = validate_client_order(order, &symbol_mgr).unwrap();
        assert_eq!(validated.time_in_force, TimeInForce::IOC);

        // Convert to internal order
        let internal = validated
            .into_internal_order(101, 1001, 1234567890)
            .unwrap();

        // Verify IOC is preserved in InternalOrder
        assert_eq!(internal.time_in_force, TimeInForce::IOC);
        assert_eq!(internal.order_id, 101);
    }

    /// Test that time_in_force defaults to GTC when not specified (via JSON)
    #[test]
    fn test_time_in_force_defaults_to_gtc() {
        // Simulate JSON without time_in_force field
        let json = r#"{
            "symbol": "BTC_USDT",
            "side": "BUY",
            "order_type": "LIMIT",
            "price": "85000.00",
            "qty": "0.001"
        }"#;

        let order: ClientOrder = serde_json::from_str(json).unwrap();

        // Should default to GTC
        assert_eq!(order.time_in_force, TimeInForce::GTC);
    }

    /// Test that IOC can be specified via JSON
    #[test]
    fn test_ioc_via_json() {
        let json = r#"{
            "symbol": "BTC_USDT",
            "side": "BUY",
            "order_type": "LIMIT",
            "time_in_force": "IOC",
            "price": "85000.00",
            "qty": "0.001"
        }"#;

        let order: ClientOrder = serde_json::from_str(json).unwrap();

        assert_eq!(order.time_in_force, TimeInForce::IOC);
    }
}

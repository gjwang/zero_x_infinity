//! Order types and validation for trading API
//!
//! - `ClientOrder`: HTTP request deserialization
//! - `ValidatedClientOrder`: Validated order ready for conversion
//! - `ValidatedOrderExtractor`: Axum extractor for framework-level validation

use rust_decimal::prelude::*;
use serde::Deserialize;
use std::sync::Arc;
use utoipa::ToSchema;

use axum::{
    Json,
    body::Body,
    extract::{FromRequest, Request},
    http::StatusCode,
};

use crate::models::{InternalOrder, OrderStatus, OrderType, Side, TimeInForce};
use crate::symbol_manager::SymbolManager;

use super::money::StrictDecimal;
use super::response::{ApiResponse, error_codes};
use crate::gateway::state::AppState;

// ============================================================================
// ClientOrder: HTTP Request Deserialization
// ============================================================================

/// Custom deserializer for non-empty strings
fn deserialize_non_empty_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
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
///
/// Note: price and qty now use StrictDecimal which validates format at Serde layer.
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
    /// Price (required for LIMIT orders) - format validated by StrictDecimal
    pub price: Option<StrictDecimal>,
    /// Quantity (unified field name) - format validated by StrictDecimal
    pub qty: StrictDecimal,
}

// ============================================================================
// ValidatedClientOrder: Business-Validated Order
// ============================================================================

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

/// Validate and parse ClientOrder to ValidatedClientOrder
///
/// This function performs business validation only.
/// Format validation (negative, .5, 5.) is handled by StrictDecimal during deserialization.
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
                let price_decimal = p.inner();
                if price_decimal.is_zero() {
                    return Err("Price must be greater than zero");
                }
                if price_decimal.scale() > symbol_info.price_decimal {
                    return Err("Too many decimal places in price");
                }
                price_decimal
            }
            None => return Err("Price is required for LIMIT orders"),
        }
    } else {
        Decimal::ZERO // Market order
    };

    // 4. Validate quantity
    let qty_decimal = req.qty.inner();
    if qty_decimal.is_zero() {
        return Err("Quantity must be greater than zero");
    }

    let base_asset = symbol_mgr
        .assets
        .get(&symbol_info.base_asset_id)
        .ok_or("Base asset not found")?;

    if qty_decimal.scale() > base_asset.decimals {
        return Err("Too many decimal places in quantity");
    }

    Ok(ValidatedClientOrder {
        cid: req.cid,
        symbol_id,
        side: req.side,
        order_type: req.order_type,
        time_in_force: req.time_in_force,
        price,
        qty: qty_decimal,
        price_decimals: symbol_info.price_decimal,
        qty_decimals: base_asset.decimals,
    })
}

impl ValidatedClientOrder {
    /// Convert to InternalOrder
    ///
    /// Uses SymbolManager intent-based API for Decimal → u64 conversion.
    /// This ensures compliance with money-type-safety.md Section 2.4.
    pub fn into_internal_order(
        self,
        order_id: u64,
        user_id: u64,
        ingested_at_ns: u64,
        symbol_mgr: &SymbolManager,
    ) -> Result<InternalOrder, &'static str> {
        // Use SymbolManager intent-based API (money-type-safety.md compliance)
        let price_u64 = if self.price.is_zero() {
            0 // Market order
        } else {
            symbol_mgr.decimal_to_price(self.price, self.symbol_id)?
        };
        let qty_u64 = symbol_mgr.decimal_to_qty(self.qty, self.symbol_id)?;

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

// NOTE: decimal_to_u64 has been REMOVED.
// All conversions must use SymbolManager intent-based API.
// This enforces money-type-safety.md Section 2.4.

// ============================================================================
// ValidatedOrderExtractor: Axum Framework Integration
// ============================================================================

/// Validated Order Extractor - Framework-level validation.
///
/// This extractor performs order validation at the Axum framework level,
/// preventing handlers from ever receiving invalid data.
#[derive(Debug)]
pub struct ValidatedOrderExtractor(pub ValidatedClientOrder);

/// Rejection type for ValidatedOrderExtractor
pub struct OrderValidationRejection {
    pub status: StatusCode,
    pub message: String,
}

impl axum::response::IntoResponse for OrderValidationRejection {
    fn into_response(self) -> axum::response::Response {
        let body = Json(ApiResponse::<()>::error(
            error_codes::INVALID_PARAMETER,
            &self.message,
        ));
        (self.status, body).into_response()
    }
}

impl FromRequest<Arc<AppState>, Body> for ValidatedOrderExtractor {
    type Rejection = OrderValidationRejection;

    async fn from_request(
        req: Request<Body>,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        // 1. Extract JSON body as ClientOrder
        let Json(client_order): Json<ClientOrder> =
            Json::from_request(req, state)
                .await
                .map_err(|e| OrderValidationRejection {
                    status: StatusCode::BAD_REQUEST,
                    message: format!("Invalid JSON: {}", e),
                })?;

        // 2. Validate using existing validation function
        let validated = validate_client_order(client_order, &state.symbol_mgr).map_err(|e| {
            OrderValidationRejection {
                status: StatusCode::BAD_REQUEST,
                message: e.to_string(),
            }
        })?;

        Ok(ValidatedOrderExtractor(validated))
    }
}

// ============================================================================
// Other Order-Related Request Types
// ============================================================================

/// Cancel order request
#[derive(Debug, Deserialize, ToSchema)]
pub struct CancelOrderRequest {
    pub order_id: u64,
}

/// Reduce order request
#[derive(Debug, Deserialize, ToSchema)]
pub struct ReduceOrderRequest {
    pub order_id: u64,
    /// Quantity to reduce - format validated by StrictDecimal
    #[schema(value_type = String)]
    pub reduce_qty: StrictDecimal,
}

/// Move order request
#[derive(Debug, Deserialize, ToSchema)]
pub struct MoveOrderRequest {
    pub order_id: u64,
    /// New price - format validated by StrictDecimal
    #[schema(value_type = String)]
    pub new_price: StrictDecimal,
}

//! Gateway types module
//!
//! This module provides type-safe types for API boundary enforcement:
//!
//! ## Input Types
//! - [`StrictDecimal`]: Format-validated decimal for API input
//! - [`ClientOrder`]: Order deserialization from HTTP requests
//! - [`ValidatedClientOrder`]: Business-validated order
//! - [`ValidatedOrderExtractor`]: Axum extractor for framework-level validation
//!
//! ## Output Types
//! - [`DisplayAmount`]: Type-safe formatted amount for API responses
//! - [`ApiResponse<T>`]: Unified API response wrapper
//!
//! ## Submodules
//! - [`money`]: Money types (StrictDecimal, DisplayAmount)
//! - [`order`]: Order types and validation
//! - [`response`]: Response types and error codes

pub mod money;
pub mod order;
pub mod response;

// Re-export commonly used types at module root
pub use money::{DisplayAmount, StrictDecimal};
pub use order::{
    CancelOrderRequest, ClientOrder, MoveOrderRequest, OrderValidationRejection,
    ReduceOrderRequest, ValidatedClientOrder, ValidatedOrderExtractor, decimal_to_u64,
    validate_client_order,
};
pub use response::{
    AccountResponseData, ApiResponse, DepthApiData, OrderResponseData, error_codes,
};

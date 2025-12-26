//! OpenAPI / Swagger UI Documentation
//!
//! This module provides auto-generated OpenAPI 3.0 documentation for the Zero X Infinity API.
//!
//! - Swagger UI: `http://localhost:8080/docs`
//! - OpenAPI JSON: `http://localhost:8080/api-docs/openapi.json`

use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};

// Import handler types for schema registration
use crate::gateway::handlers::{AssetApiData, ExchangeInfoData, HealthResponse, SymbolApiData};
use crate::gateway::types::DepthApiData;

/// Ed25519 signature-based authentication security scheme
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "ed25519_auth",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                    "Authorization",
                    r#"Ed25519 signature auth: Bearer {api_key}:{timestamp_nonce}:{signature}

Signature payload format:
- POST: {method}\n{path}\n{timestamp_nonce}\n{sha256(body)}
- GET: {method}\n{path}\n{timestamp_nonce}\n

Example: Bearer abc123:1703494800000_r1:base64_signature"#,
                ))),
            );
        }
    }
}

/// Main API Documentation struct
#[derive(OpenApi)]
#[openapi(
    info(
        title = "Zero X Infinity Exchange API",
        version = "1.0.0",
        description = "High-performance cryptocurrency exchange API achieving 1.3M orders/sec on a single core.",
        contact(
            name = "API Support",
            url = "https://github.com/gjwang/zero_x_infinity"
        ),
        license(
            name = "MIT"
        )
    ),
    servers(
        (url = "http://localhost:8080", description = "Development"),
    ),
    paths(
        // Public endpoints (Phase 2)
        crate::gateway::handlers::health_check,
        crate::gateway::handlers::get_depth,
        crate::gateway::handlers::get_klines,
        crate::gateway::handlers::get_assets,
        crate::gateway::handlers::get_symbols,
        crate::gateway::handlers::get_exchange_info,
        // Private endpoints (Phase 3)
        crate::gateway::handlers::create_order,
        crate::gateway::handlers::cancel_order,
        crate::gateway::handlers::get_order,
        crate::gateway::handlers::get_orders,
        crate::gateway::handlers::get_trades,
        crate::gateway::handlers::get_balances,
        crate::gateway::handlers::get_all_balances,
        crate::gateway::handlers::create_transfer,
        crate::gateway::handlers::get_transfer,
    ),
    components(
        schemas(
            HealthResponse,
            DepthApiData,
            AssetApiData,
            SymbolApiData,
            ExchangeInfoData,
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "Market Data", description = "Public market data endpoints (no auth required)"),
        (name = "Trading", description = "Order placement and management (auth required)"),
        (name = "Account", description = "Balance and account queries (auth required)"),
        (name = "Transfer", description = "Internal fund transfers (auth required)"),
        (name = "System", description = "Health checks and system info")
    )
)]
pub struct ApiDoc;

#[cfg(test)]
mod tests {
    use super::*;
    use utoipa::OpenApi;

    #[test]
    fn test_openapi_spec_generates() {
        let spec = ApiDoc::openapi();
        assert_eq!(spec.info.title, "Zero X Infinity Exchange API");
        assert_eq!(spec.info.version, "1.0.0");
    }

    #[test]
    fn test_openapi_json_serializable() {
        let spec = ApiDoc::openapi();
        let json = spec.to_json();
        assert!(json.is_ok());
        let json_str = json.unwrap();
        assert!(json_str.contains("Zero X Infinity Exchange API"));
    }

    #[test]
    fn test_public_endpoints_registered() {
        let spec = ApiDoc::openapi();
        let paths = spec.paths;
        // Verify public endpoints
        assert!(paths.paths.contains_key("/api/v1/health"));
        assert!(paths.paths.contains_key("/api/v1/public/depth"));
        assert!(paths.paths.contains_key("/api/v1/public/assets"));
    }

    #[test]
    fn test_security_scheme_registered() {
        let spec = ApiDoc::openapi();
        let components = spec.components.expect("should have components");
        assert!(components.security_schemes.contains_key("ed25519_auth"));
    }
}

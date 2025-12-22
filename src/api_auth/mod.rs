//! Authentication module for API security.
//!
//! This module provides Ed25519 signature-based authentication for private API endpoints.
//!
//! ## Components
//! - `base62`: Base62 encoding for signatures
//! - `signature`: Ed25519 signature verification
//! - `error`: Authentication error types (4001-4008)
//! - `ts_store`: Timestamp nonce store for replay protection
//! - `models`: API Key models and types
//! - `middleware`: Axum authentication middleware
//! - `repository`: Database operations for API Keys

pub mod base62;
pub mod error;
pub mod middleware;
pub mod models;
pub mod repository;
pub mod signature;
pub mod ts_store;

// Re-export for convenience
pub use base62::{decode as base62_decode, encode as base62_encode};
pub use error::{AuthError, AuthErrorCode};
pub use middleware::{
    AuthState, auth_middleware, extract_auth_header, parse_authorization, validate_ts_nonce,
    verify_signature,
};
pub use models::{ApiKeyRecord, AuthenticatedUser, KeyType, has_permission, permissions};
pub use repository::ApiKeyRepository;
pub use signature::verify_ed25519;
pub use ts_store::TsStore;

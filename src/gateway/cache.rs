//! TTL-based cache for config endpoints (Assets, Symbols)
//!
//! Uses the `cached` crate for automatic TTL expiration.
//! This enables hot-reload: Admin Dashboard changes are visible
//! within TTL_SECONDS without restarting Gateway.

use cached::proc_macro::cached;
use sqlx::PgPool;
use std::sync::Arc;

use crate::account::{Asset, AssetManager, Symbol, SymbolManager};

/// TTL for config cache in seconds
pub const TTL_SECONDS: u64 = 5;

/// Load all assets from database with caching
///
/// Results are cached for TTL_SECONDS. After expiration,
/// the next call will refresh from the database.
#[cached(
    time = 5,
    key = "String",
    convert = r#"{ "assets".to_string() }"#,
    result = true
)]
pub async fn load_assets_cached(pool: Arc<PgPool>) -> Result<Vec<Asset>, String> {
    tracing::debug!("[cache] Loading assets from database");
    AssetManager::load_all(&pool)
        .await
        .map_err(|e| format!("Failed to load assets: {}", e))
}

/// Load all symbols from database with caching
///
/// Results are cached for TTL_SECONDS. After expiration,
/// the next call will refresh from the database.
#[cached(
    time = 5,
    key = "String",
    convert = r#"{ "symbols".to_string() }"#,
    result = true
)]
pub async fn load_symbols_cached(pool: Arc<PgPool>) -> Result<Vec<Symbol>, String> {
    tracing::debug!("[cache] Loading symbols from database");
    SymbolManager::load_all(&pool)
        .await
        .map_err(|e| format!("Failed to load symbols: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttl_constant() {
        assert_eq!(TTL_SECONDS, 5);
    }
}

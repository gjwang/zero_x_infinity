//! Asset manager for loading and querying assets

use super::models::Asset;
use crate::exchange_info::validation;
use sqlx::PgPool;

/// Asset manager for loading and caching assets
pub struct AssetManager;

impl AssetManager {
    /// Load all active assets
    pub async fn load_all(pool: &PgPool) -> Result<Vec<Asset>, sqlx::Error> {
        let rows: Vec<Asset> = sqlx::query_as(
            r#"SELECT asset_id, asset, name, internal_scale, asset_precision, status, asset_flags 
               FROM assets_tb WHERE status = 1"#,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// Get asset by ID
    pub async fn get_by_id(pool: &PgPool, asset_id: i32) -> Result<Option<Asset>, sqlx::Error> {
        let row: Option<Asset> = sqlx::query_as(
            r#"SELECT asset_id, asset, name, internal_scale, asset_precision, status, asset_flags 
               FROM assets_tb WHERE asset_id = $1"#,
        )
        .bind(asset_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// Get asset by asset name (e.g., "BTC", "USDT")
    ///
    /// # Validation
    /// Input must be uppercase and match format ^[A-Z0-9_]{1,16}$
    pub async fn get_by_asset(pool: &PgPool, asset: &str) -> Result<Option<Asset>, sqlx::Error> {
        // Validate input using AssetName
        let asset_name = validation::AssetName::new(asset)
            .map_err(|e| sqlx::Error::Protocol(format!("Invalid asset name: {}", e)))?;

        let row: Option<Asset> = sqlx::query_as(
            r#"SELECT asset_id, asset, name, internal_scale, asset_precision, status, asset_flags 
               FROM assets_tb WHERE asset = $1"#,
        )
        .bind(asset_name.as_str())
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }
}

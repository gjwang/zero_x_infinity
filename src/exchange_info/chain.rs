use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Result};

/// Chain configuration from chains_tb
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub chain_slug: String,
    pub chain_name: String,
    pub rpc_urls: Vec<String>,
    pub network_id: Option<String>,
    pub scan_start_height: i64,
    pub confirmation_blocks: i32,
    pub is_active: Option<bool>,
}

/// Asset binding from chain_assets_tb
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainAsset {
    pub id: i32,
    pub chain_slug: String,
    pub asset_id: i32,
    pub asset_symbol: String, // Joined from assets_tb
    pub contract_address: Option<String>,
    pub decimals: i16,
    pub min_deposit: Option<Decimal>,
    pub min_withdraw: Option<Decimal>,
    pub withdraw_fee: Option<Decimal>,
    pub is_active: Option<bool>,
}

pub struct ChainManager {
    pool: PgPool,
}

impl ChainManager {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Fetch all active chains
    pub async fn get_active_chains(&self) -> Result<Vec<ChainConfig>> {
        sqlx::query_as!(
            ChainConfig,
            r#"
            SELECT 
                chain_slug, chain_name, rpc_urls, network_id, 
                scan_start_height, confirmation_blocks, is_active
            FROM chains_tb
            WHERE is_active = TRUE
            "#
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Fetch specific chain config
    pub async fn get_chain_config(&self, chain_slug: &str) -> Result<Option<ChainConfig>> {
        sqlx::query_as!(
            ChainConfig,
            r#"
            SELECT 
                chain_slug, chain_name, rpc_urls, network_id, 
                scan_start_height, confirmation_blocks, is_active
            FROM chains_tb
            WHERE chain_slug = $1
            "#,
            chain_slug
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Fetch all assets for a specific chain (Active only)
    pub async fn get_assets_by_chain(&self, chain_slug: &str) -> Result<Vec<ChainAsset>> {
        // Join with assets_tb to get the symbol
        sqlx::query_as!(
            ChainAsset,
            r#"
            SELECT 
                ca.id, ca.chain_slug, ca.asset_id, 
                a.asset as asset_symbol,
                ca.contract_address, ca.decimals, 
                ca.min_deposit, ca.min_withdraw, ca.withdraw_fee,
                ca.is_active
            FROM chain_assets_tb ca
            JOIN assets_tb a ON ca.asset_id = a.asset_id
            WHERE ca.chain_slug = $1 AND ca.is_active = TRUE
            "#,
            chain_slug
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Lookup user_id by chain address (ADR-006 Dual-Lookup: Address → User)
    pub async fn get_user_by_address(
        &self,
        chain_slug: &str,
        address: &str,
    ) -> Result<Option<i64>> {
        let result = sqlx::query_scalar!(
            r#"
            SELECT user_id
            FROM user_chain_addresses
            WHERE chain_slug = $1 AND LOWER(address) = LOWER($2)
            "#,
            chain_slug,
            address
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    /// Get all watched addresses for a chain (for Sentinel startup)
    pub async fn get_watched_addresses(&self, chain_slug: &str) -> Result<Vec<String>> {
        let addresses = sqlx::query_scalar!(
            r#"
            SELECT address
            FROM user_chain_addresses
            WHERE chain_slug = $1
            "#,
            chain_slug
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(addresses)
    }
}

use super::chain_adapter::ChainClient;
use crate::account::{AssetManager, Database};
use rust_decimal::Decimal;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DepositError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Asset not found: {0}")]
    AssetNotFound(String),
    #[error("Deposit already processed (Idempotent check)")]
    AlreadyProcessed,
    #[error("Invalid amount")]
    InvalidAmount,
}

#[derive(Debug, serde::Serialize)]
pub struct DepositRecord {
    pub tx_hash: String,
    pub user_id: i64,
    pub asset: String,
    pub amount: Decimal,
    pub status: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub block_height: Option<i64>,
}

pub struct DepositService {
    db: Arc<Database>,
    // In strict mode, we might verify via ChainClient, but for now we trust the "Mock Scanner" input
    // chain_client: Arc<dyn ChainClient>,
}

impl DepositService {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Process a new deposit from the Mock Chain Scanner
    /// Idempotent: checks tx_hash
    pub async fn process_deposit(
        &self,
        tx_hash: &str,
        user_id: i64,
        asset_name: &str,
        amount: Decimal,
    ) -> Result<String, DepositError> {
        if amount <= Decimal::ZERO {
            return Err(DepositError::InvalidAmount);
        }

        let mut tx = self.db.pool().begin().await?;

        // 1. Idempotency Check
        let exists =
            sqlx::query_scalar!("SELECT 1 FROM deposit_history WHERE tx_hash = $1", tx_hash)
                .fetch_optional(&mut *tx)
                .await?;

        if exists.is_some() {
            // Rollback not needed for read, but good practice to just return
            return Err(DepositError::AlreadyProcessed);
        }

        // 2. Get Asset info
        let asset = AssetManager::get_by_asset(self.db.pool(), asset_name)
            .await
            .map_err(DepositError::Database)?
            .ok_or_else(|| DepositError::AssetNotFound(asset_name.to_string()))?;

        // 3. Insert Deposit Record (CONFIRMING -> SUCCESS immediately for Mock)
        // In real system, scanner might set CONFIRMING, then update later.
        // Here we do atomic Instant Deposit for MVP.
        sqlx::query!(
            r#"
            INSERT INTO deposit_history (tx_hash, user_id, asset, amount, status, block_height)
            VALUES ($1, $2, $3, $4, 'SUCCESS', 100)
            "#,
            tx_hash,
            user_id,
            asset_name,
            amount
        )
        .execute(&mut *tx)
        .await?;

        // 4. Credit User Balance (Spot Account)
        // We insert or update balance (+amount)
        // account_type = 1 (Spot)
        sqlx::query!(
            r#"
            INSERT INTO balances_tb (user_id, asset_id, account_type, available, frozen, version)
            VALUES ($1, $2, 1, $3, 0, 1)
            ON CONFLICT (user_id, asset_id, account_type) 
            DO UPDATE SET available = balances_tb.available + EXCLUDED.available, version = balances_tb.version + 1
            "#,
            user_id,
            asset.asset_id,
            amount
        )
        .execute(&mut *tx)
        .await?;

        // 5. Commit
        tx.commit().await?;

        Ok("Deposit Processed".to_string())
    }

    /// Get Deposit Address (Mock)
    pub async fn get_address(
        &self,
        chain_adapter: &dyn ChainClient,
        user_id: i64,
        asset: &str,
        network: &str,
    ) -> Result<String, DepositError> {
        // Check DB first
        let row = sqlx::query!(
            "SELECT address FROM user_addresses WHERE user_id = $1 AND asset = $2 AND network = $3",
            user_id,
            asset,
            network
        )
        .fetch_optional(self.db.pool())
        .await?;

        if let Some(r) = row {
            return Ok(r.address);
        }

        // Generate New
        // Note: For real concurrency, we might want a lock or upsert logic
        // But for MVP generate -> insert is fine
        // Note 2: ChainAdapter generate_address is async but Mock is fast
        let address = chain_adapter.generate_address(user_id).await.map_err(|e| {
            DepositError::Database(sqlx::Error::Protocol(format!("Chain Error: {:?}", e)))
        })?; // Wrap error

        sqlx::query!(
            "INSERT INTO user_addresses (user_id, asset, network, address) VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
            user_id, asset, network, address
        )
        .execute(self.db.pool())
        .await?;

        // Re-fetch to ensure we return what's in DB (handle race condition on unique constraint)
        let final_addr = sqlx::query!(
            "SELECT address FROM user_addresses WHERE user_id = $1 AND asset = $2 AND network = $3",
            user_id,
            asset,
            network
        )
        .fetch_one(self.db.pool())
        .await?
        .address;

        Ok(final_addr)
    }

    pub async fn get_history(&self, user_id: i64) -> Result<Vec<DepositRecord>, DepositError> {
        let records = sqlx::query_as!(
            DepositRecord,
            r#"
            SELECT tx_hash, user_id, asset, amount, status, created_at, block_height
            FROM deposit_history
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT 50
            "#,
            user_id
        )
        .fetch_all(self.db.pool())
        .await?;

        Ok(records)
    }
}

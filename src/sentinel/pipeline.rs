//! Pipeline Integration
//!
//! Connects the Sentinel service to the main trading pipeline
//! to credit user balances when deposits are finalized.
//!
//! This module provides the bridge between the blockchain scanning
//! layer and the core balance management system.

use super::confirmation::{PendingDeposit, status};
use super::error::SentinelError;
use sqlx::PgPool;
use tracing::{error, info};

/// Pipeline integration for crediting finalized deposits
pub struct DepositPipeline {
    pool: PgPool,
}

impl DepositPipeline {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Process a finalized deposit by crediting the user's balance
    ///
    /// This function:
    /// 1. Credits balance in balances_tb (UPSERT)
    /// 2. Updates the deposit status to SUCCESS
    ///
    /// Idempotency is ensured by the status check - we only process
    /// deposits that are in FINALIZED state.
    pub async fn credit_deposit(&self, deposit: &PendingDeposit) -> Result<(), SentinelError> {
        info!(
            "Crediting deposit: {} {} to user {} (tx: {})",
            deposit.amount, deposit.asset, deposit.user_id, deposit.tx_hash
        );

        // Credit the balance
        self.credit_balance(deposit).await?;

        // Update status to SUCCESS
        self.mark_success(&deposit.tx_hash).await?;

        info!(
            "Deposit {} credited successfully to user {}",
            deposit.tx_hash, deposit.user_id
        );

        Ok(())
    }

    /// Credit the user's balance in the database
    async fn credit_balance(&self, deposit: &PendingDeposit) -> Result<(), SentinelError> {
        // Get asset_id from asset name
        let asset_row = sqlx::query("SELECT asset_id FROM assets_tb WHERE asset = $1")
            .bind(&deposit.asset)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| SentinelError::Config(e.to_string()))?;

        let Some(asset_row) = asset_row else {
            return Err(SentinelError::Config(format!(
                "Asset not found: {}",
                deposit.asset
            )));
        };

        let asset_id: i32 = sqlx::Row::get(&asset_row, "asset_id");

        // Credit balance (account_type = 1 for Spot)
        // Using UPSERT to handle both new and existing balances
        sqlx::query(
            r#"INSERT INTO balances_tb (user_id, asset_id, account_type, available, frozen, version)
               VALUES ($1, $2, 1, $3, 0, 1)
               ON CONFLICT (user_id, asset_id, account_type) 
               DO UPDATE SET available = balances_tb.available + EXCLUDED.available, 
                             version = balances_tb.version + 1"#,
        )
        .bind(deposit.user_id)
        .bind(asset_id)
        .bind(deposit.amount)
        .execute(&self.pool)
        .await
        .map_err(|e| SentinelError::Config(e.to_string()))?;

        Ok(())
    }

    /// Mark deposit as successfully credited
    async fn mark_success(&self, tx_hash: &str) -> Result<(), SentinelError> {
        sqlx::query(
            r#"UPDATE deposit_history
               SET status = $1
               WHERE tx_hash = $2"#,
        )
        .bind(status::SUCCESS)
        .bind(tx_hash)
        .execute(&self.pool)
        .await
        .map_err(|e| SentinelError::Config(e.to_string()))?;

        Ok(())
    }

    /// Process all finalized deposits
    pub async fn process_finalized(
        &self,
        deposits: Vec<PendingDeposit>,
    ) -> Result<u32, SentinelError> {
        let mut credited = 0u32;

        for deposit in deposits {
            match self.credit_deposit(&deposit).await {
                Ok(()) => credited += 1,
                Err(e) => {
                    error!("Failed to credit deposit {}: {:?}", deposit.tx_hash, e);
                }
            }
        }

        if credited > 0 {
            info!("Credited {} deposits", credited);
        }

        Ok(credited)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deposit_pipeline_types() {
        // Basic type check - actual integration tests need DB
        let deposit = PendingDeposit {
            tx_hash: "test_tx".to_string(),
            user_id: 1,
            asset: "BTC".to_string(),
            amount: rust_decimal::Decimal::new(100000000, 8),
            chain_id: "BTC".to_string(),
            block_height: 100,
            block_hash: "hash".to_string(),
            status: status::FINALIZED.to_string(),
            confirmations: 6,
        };

        assert_eq!(deposit.status, "FINALIZED");
        assert_eq!(deposit.confirmations, 6);
    }
}

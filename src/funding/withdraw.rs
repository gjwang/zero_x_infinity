use super::chain_adapter::ChainClient;
use crate::account::{AssetManager, Database};
use rust_decimal::Decimal;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum WithdrawError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Asset not found: {0}")]
    AssetNotFound(String),
    #[error("Insufficient funds")]
    InsufficientFunds,
    #[error("Invalid address")]
    InvalidAddress,
    #[error("Invalid amount")]
    InvalidAmount,
    #[error("Chain error: {0}")]
    Chain(String),
}

#[derive(Debug, serde::Serialize)]
pub struct WithdrawRecord {
    pub request_id: String,
    pub user_id: i64,
    pub asset: String,
    pub amount: Decimal,
    pub fee: Decimal,
    pub to_address: String,
    pub status: String,
    pub tx_hash: Option<String>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub updated_at: Option<chrono::NaiveDateTime>,
}

pub struct WithdrawService {
    db: Arc<Database>,
    // For MVP, we use dynamic dispatch or generic for ChainClient.
    // Ideally we have a map of clients per network.
    // For now assuming we pass the client for the specific network involved.
    // Or we store a map in FundingService wrapper?
    // Let's stick to trait object or similar.
}

impl WithdrawService {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Apply for withdrawal
    /// 1. Lock & Deduct Balance
    /// 2. Create Record (PENDING)
    /// 3. Broadcast (Simulate) -> Update Record (SUCCESS)
    pub async fn apply_withdraw(
        &self,
        chain_adapter: &dyn ChainClient,
        user_id: i64,
        asset_name: &str,
        to_address: &str,
        amount: Decimal,
        fee: Decimal, // fee deducted from request amount or external? Spec says "User Balance Delta = Request Amount. Network Receive = Request - Fee"
    ) -> Result<String, WithdrawError> {
        if amount <= Decimal::ZERO {
            return Err(WithdrawError::InvalidAmount);
        }

        // Validate Address First
        if !chain_adapter.validate_address(to_address) {
            return Err(WithdrawError::InvalidAddress);
        }

        let mut tx = self.db.pool().begin().await?;

        // 1. Get Asset
        let asset = AssetManager::get_by_asset(self.db.pool(), asset_name)
            .await
            .map_err(WithdrawError::Database)?
            .ok_or_else(|| WithdrawError::AssetNotFound(asset_name.to_string()))?;

        // 2. Lock & Check Balance
        // We act on Spot Account (type=1)
        let balance_row = sqlx::query!(
            "SELECT available, version FROM balances_tb WHERE user_id = $1 AND asset_id = $2 AND account_type = 1 FOR UPDATE",
            user_id, asset.asset_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        let available = balance_row
            .as_ref()
            .map(|r| r.available)
            .unwrap_or(Decimal::ZERO);

        if available < amount {
            return Err(WithdrawError::InsufficientFunds);
        }

        // 3. Deduct Balance
        // "Immediate Deduction" per spec
        sqlx::query!(
            "UPDATE balances_tb SET available = available - $1, version = version + 1 WHERE user_id = $2 AND asset_id = $3 AND account_type = 1",
            amount, user_id, asset.asset_id
        )
        .execute(&mut *tx)
        .await?;

        // 4. Create Withdrawal Record
        let request_id = Uuid::new_v4().to_string();
        sqlx::query!(
            r#"
            INSERT INTO withdraw_history (request_id, user_id, asset, amount, fee, to_address, status)
            VALUES ($1, $2, $3, $4, $5, $6, 'PROCESSING')
            "#,
            request_id, user_id, asset_name, amount, fee, to_address
        )
        .execute(&mut *tx)
        .await?;

        // Commit Deduction Logic
        tx.commit().await?;

        // 5. Broadcast (Async/Simulated)
        // In a real system, this would be a separate async job poller.
        // For MVP/Mock, we call it directly here.
        // If broadcast fails, we technically stuck in PROCESSING (requires manual refund or retry).
        // Spec: "User can see Processing".

        let receive_amount = amount - fee;
        let receive_str = receive_amount.to_string();

        match chain_adapter
            .broadcast_withdraw(to_address, &receive_str)
            .await
        {
            Ok(tx_hash) => {
                // Update to SUCCESS
                sqlx::query!(
                    "UPDATE withdraw_history SET status = 'SUCCESS', tx_hash = $1, updated_at = CURRENT_TIMESTAMP WHERE request_id = $2",
                    tx_hash, request_id
                )
                .execute(self.db.pool())
                .await?;

                Ok(request_id)
            }
            Err(e) => {
                // Update to FAILED?
                // If we fail here, funds are already deducted.
                // Ideally we rollback or set FAILED and refund.
                // For MVP Mock, let's mark FAILED.
                // NOTE: Refund logic omitted for MVP brevity unless required.
                // Spec doesn't explicitly demand auto-refund, just "Failed status".
                // But deducting money without refund is bad.
                // Let's implement simple refund for robustness.
                eprintln!("Broadcast failed: {:?}. Refunding...", e);

                let mut tx_refund = self.db.pool().begin().await?;
                sqlx::query!(
                    "UPDATE balances_tb SET available = available + $1, version = version + 1 WHERE user_id = $2 AND asset_id = $3 AND account_type = 1",
                    amount, user_id, asset.asset_id
                )
                .execute(&mut *tx_refund)
                .await?;

                sqlx::query!(
                    "UPDATE withdraw_history SET status = 'FAILED', updated_at = CURRENT_TIMESTAMP WHERE request_id = $1",
                    request_id
                )
                .execute(&mut *tx_refund)
                .await?;

                tx_refund.commit().await?;

                Err(WithdrawError::Chain(format!(
                    "Broadcast failed and refunded: {:?}",
                    e
                )))
            }
        }
    }

    pub async fn get_history(&self, user_id: i64) -> Result<Vec<WithdrawRecord>, WithdrawError> {
        let records = sqlx::query_as!(
            WithdrawRecord,
            r#"
            SELECT request_id, user_id, asset, amount, fee, to_address, status, tx_hash, created_at, updated_at
            FROM withdraw_history
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

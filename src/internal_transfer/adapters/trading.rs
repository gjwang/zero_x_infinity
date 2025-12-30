//! Trading Account Adapter
//!
//! Adapter for UBSCore RAM-based Trading account balance operations.
//! Sends Deposit/Withdraw orders through the pipeline.

use async_trait::async_trait;
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, warn};

use super::ServiceAdapter;
use crate::internal_transfer::channel::{TransferOp, TransferResponse, TransferSender};
use crate::internal_transfer::types::{InternalTransferId, OpResult, ScaledAmount};

/// Trading account adapter
///
/// Interfaces with UBSCore for balance operations.
///
/// # Current Implementation
/// This is a placeholder that will be integrated with the actual pipeline.
/// For Phase 1, we use a simple in-memory implementation.
///
/// # Production Integration
/// In production, this will:
/// 1. Send `OrderType::Deposit` or `OrderType::Withdraw` via pipeline channel
/// 2. Wait for acknowledgment from UBSCore
/// 3. Track processed transfer_ids in RAM (rebuilt from WAL on restart)
pub struct TradingAdapter {
    /// Processed request IDs (for idempotency in RAM)
    /// In production, this is rebuilt from WAL on startup
    processed: Arc<Mutex<HashSet<InternalTransferId>>>,

    /// Optional channel to UBSCore (None for tests, Some for production)
    channel: Option<TransferSender>,

    /// PostgreSQL pool for querying transfer details during rollback
    pool: Option<PgPool>,

    /// Simulated balances for testing (only used when channel is None)
    #[cfg(test)]
    test_balances: Arc<Mutex<std::collections::HashMap<(u64, u32), u64>>>,
}

impl TradingAdapter {
    /// Create a new TradingAdapter (test mode - no UBSCore connection)
    pub fn new() -> Self {
        Self {
            processed: Arc::new(Mutex::new(HashSet::new())),
            channel: None,
            pool: None,
            #[cfg(test)]
            test_balances: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Create adapter connected to UBSCore via channel (production mode)
    pub fn with_channel(channel: TransferSender) -> Self {
        Self {
            processed: Arc::new(Mutex::new(HashSet::new())),
            channel: Some(channel),
            pool: None,
            #[cfg(test)]
            test_balances: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Create adapter with channel and DB pool (full production mode)
    pub fn with_channel_and_pool(channel: TransferSender, pool: PgPool) -> Self {
        Self {
            processed: Arc::new(Mutex::new(HashSet::new())),
            channel: Some(channel),
            pool: Some(pool),
            #[cfg(test)]
            test_balances: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Create adapter with existing processed set (for recovery)
    pub fn with_processed(processed: HashSet<InternalTransferId>) -> Self {
        Self {
            processed: Arc::new(Mutex::new(processed)),
            channel: None,
            pool: None,
            #[cfg(test)]
            test_balances: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Check if connected to UBSCore
    pub fn is_connected(&self) -> bool {
        self.channel.is_some()
    }

    /// Check if a request was already processed
    fn is_processed(&self, transfer_id: InternalTransferId) -> bool {
        self.processed.lock().unwrap().contains(&transfer_id)
    }

    /// Mark a request as processed
    fn mark_processed(&self, transfer_id: InternalTransferId) {
        self.processed.lock().unwrap().insert(transfer_id);
    }

    #[cfg(test)]
    pub fn set_test_balance(&self, user_id: u64, asset_id: u32, amount: u64) {
        self.test_balances
            .lock()
            .unwrap()
            .insert((user_id, asset_id), amount);
    }

    #[cfg(test)]
    pub fn get_test_balance(&self, user_id: u64, asset_id: u32) -> u64 {
        *self
            .test_balances
            .lock()
            .unwrap()
            .get(&(user_id, asset_id))
            .unwrap_or(&0)
    }
}

impl Default for TradingAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ServiceAdapter for TradingAdapter {
    fn name(&self) -> &'static str {
        "Trading"
    }

    async fn withdraw(
        &self,
        transfer_id: InternalTransferId,
        user_id: u64,
        asset_id: u32,
        amount: ScaledAmount,
    ) -> OpResult {
        debug!(
            transfer_id = %transfer_id,
            user_id = user_id,
            asset_id = asset_id,
            amount = *amount,
            "Trading withdraw"
        );

        // Check idempotency (in RAM)
        if self.is_processed(transfer_id) {
            debug!(transfer_id = %transfer_id, "Trading withdraw already processed");
            return OpResult::Success;
        }

        // Production mode: Use channel to UBSCore
        if let Some(ref channel) = self.channel {
            match channel
                .send_request(
                    transfer_id,
                    TransferOp::Withdraw,
                    user_id,
                    asset_id,
                    *amount,
                )
                .await
            {
                Ok(TransferResponse::Success { .. }) => {
                    self.mark_processed(transfer_id);
                    debug!(transfer_id = %transfer_id, "Trading withdraw successful via channel");
                    return OpResult::Success;
                }
                Ok(TransferResponse::Failed(e)) => {
                    debug!(transfer_id = %transfer_id, error = %e, "Trading withdraw failed via channel");
                    return OpResult::Failed(e);
                }
                Err(e) => {
                    warn!(transfer_id = %transfer_id, error = %e, "Trading channel error");
                    return OpResult::Pending;
                }
            }
        }

        // Test mode: Use simulated balances
        #[cfg(test)]
        {
            let mut balances = self.test_balances.lock().unwrap();
            let key = (user_id, asset_id);
            let current = *balances.get(&key).unwrap_or(&0);

            if current < *amount {
                warn!(
                    transfer_id = %transfer_id,
                    current = current,
                    requested = *amount,
                    "Insufficient trading balance"
                );
                return OpResult::Failed("Insufficient balance".to_string());
            }

            balances.insert(key, current - *amount);
        }

        #[cfg(not(test))]
        {
            // No channel configured - warn and assume success for testing
            warn!(
                transfer_id = %transfer_id,
                "Trading adapter not connected to UBSCore (no channel)"
            );
        }

        self.mark_processed(transfer_id);
        debug!(transfer_id = %transfer_id, "Trading withdraw successful");
        OpResult::Success
    }

    async fn deposit(
        &self,
        transfer_id: InternalTransferId,
        user_id: u64,
        asset_id: u32,
        amount: ScaledAmount,
    ) -> OpResult {
        debug!(
            transfer_id = %transfer_id,
            user_id = user_id,
            asset_id = asset_id,
            amount = *amount,
            "Trading deposit"
        );

        // Check idempotency
        if self.is_processed(transfer_id) {
            debug!(transfer_id = %transfer_id, "Trading deposit already processed");
            return OpResult::Success;
        }

        // Production mode: Use channel to UBSCore
        if let Some(ref channel) = self.channel {
            match channel
                .send_request(transfer_id, TransferOp::Deposit, user_id, asset_id, *amount)
                .await
            {
                Ok(TransferResponse::Success { .. }) => {
                    self.mark_processed(transfer_id);
                    debug!(transfer_id = %transfer_id, "Trading deposit successful via channel");
                    return OpResult::Success;
                }
                Ok(TransferResponse::Failed(e)) => {
                    debug!(transfer_id = %transfer_id, error = %e, "Trading deposit failed via channel");
                    return OpResult::Failed(e);
                }
                Err(e) => {
                    warn!(transfer_id = %transfer_id, error = %e, "Trading channel error");
                    return OpResult::Pending;
                }
            }
        }

        // Test mode: Use simulated balances
        #[cfg(test)]
        {
            let mut balances = self.test_balances.lock().unwrap();
            let key = (user_id, asset_id);
            let current = *balances.get(&key).unwrap_or(&0);
            balances.insert(key, current + *amount);
        }

        #[cfg(not(test))]
        {
            // No channel configured - warn and assume success
            warn!(
                transfer_id = %transfer_id,
                "Trading adapter not connected to UBSCore (no channel)"
            );
        }

        self.mark_processed(transfer_id);
        debug!(transfer_id = %transfer_id, "Trading deposit successful");
        OpResult::Success
    }

    async fn rollback(&self, transfer_id: InternalTransferId) -> OpResult {
        debug!(transfer_id = %transfer_id, "Trading rollback");

        // CRITICAL: Trading rollback means re-crediting a previously withdrawn amount
        // This is only called when source was Trading and target deposit failed

        // Check if withdraw was even processed
        if !self.is_processed(transfer_id) {
            debug!(transfer_id = %transfer_id, "No withdraw to rollback");
            return OpResult::Success;
        }

        // Query transfer details from DB
        let pool = match &self.pool {
            Some(p) => p,
            None => {
                error!(transfer_id = %transfer_id, "Trading rollback failed: no DB pool configured");
                return OpResult::Failed("No DB pool for rollback query".to_string());
            }
        };

        // Get transfer record to know user_id, asset_id, amount
        let record = match sqlx::query_as::<_, (i64, i32, rust_decimal::Decimal)>(
            r#"
            SELECT user_id, asset_id, amount
            FROM fsm_transfers_tb
            WHERE transfer_id = $1
            "#,
        )
        .bind(transfer_id.to_string())
        .fetch_optional(pool)
        .await
        {
            Ok(Some(row)) => row,
            Ok(None) => {
                error!(transfer_id = %transfer_id, "Trading rollback: transfer record not found");
                return OpResult::Failed("Transfer record not found".to_string());
            }
            Err(e) => {
                error!(transfer_id = %transfer_id, error = %e, "Trading rollback: DB query failed");
                return OpResult::Failed(format!("DB error: {}", e));
            }
        };

        let user_id = record.0 as u64;
        let asset_id = record.1 as u32;
        use rust_decimal::prelude::ToPrimitive;
        let amount = record.2.trunc().to_i64().unwrap_or(0) as u64;

        // Send Deposit to restore the funds
        if let Some(channel) = &self.channel {
            match channel
                .send_request(transfer_id, TransferOp::Deposit, user_id, asset_id, amount)
                .await
            {
                Ok(TransferResponse::Success { .. }) => {
                    debug!(transfer_id = %transfer_id, user_id, asset_id, amount, "Trading rollback deposit success");
                    OpResult::Success
                }
                Ok(TransferResponse::Failed(msg)) => {
                    error!(transfer_id = %transfer_id, error = %msg, "Trading rollback deposit failed");
                    OpResult::Failed(msg)
                }
                Err(e) => {
                    error!(transfer_id = %transfer_id, error = %e, "Trading rollback channel error");
                    OpResult::Pending
                }
            }
        } else {
            // Test mode: simulate success
            warn!(transfer_id = %transfer_id, "Trading rollback in test mode (no channel)");
            OpResult::Success
        }
    }

    async fn commit(&self, transfer_id: InternalTransferId) -> OpResult {
        debug!(transfer_id = %transfer_id, "Trading commit");

        // For Trading, commit is typically a no-op
        // The withdraw is already final when processed

        OpResult::Success
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_trading_withdraw_success() {
        let adapter = TradingAdapter::new();
        adapter.set_test_balance(1001, 1, 10000);

        let result = adapter
            .withdraw(InternalTransferId::new(), 1001, 1, 5000.into())
            .await;
        assert!(result.is_success());
        assert_eq!(adapter.get_test_balance(1001, 1), 5000);
    }

    #[tokio::test]
    async fn test_trading_withdraw_insufficient() {
        let adapter = TradingAdapter::new();
        adapter.set_test_balance(1001, 1, 1000);

        let result = adapter
            .withdraw(InternalTransferId::new(), 1001, 1, 5000.into())
            .await;
        assert!(result.is_explicit_fail());
        // Balance unchanged
        assert_eq!(adapter.get_test_balance(1001, 1), 1000);
    }

    #[tokio::test]
    async fn test_trading_deposit() {
        let adapter = TradingAdapter::new();
        adapter.set_test_balance(1001, 1, 5000);

        let result = adapter
            .deposit(InternalTransferId::new(), 1001, 1, 3000.into())
            .await;
        assert!(result.is_success());
        assert_eq!(adapter.get_test_balance(1001, 1), 8000);
    }

    #[tokio::test]
    async fn test_trading_idempotency() {
        let adapter = TradingAdapter::new();
        adapter.set_test_balance(1001, 1, 10000);
        let transfer_id = InternalTransferId::new();

        // First call
        let result1 = adapter.withdraw(transfer_id, 1001, 1, 5000.into()).await;
        assert!(result1.is_success());
        assert_eq!(adapter.get_test_balance(1001, 1), 5000);

        // Second call with same transfer_id - should be idempotent
        let result2 = adapter.withdraw(transfer_id, 1001, 1, 5000.into()).await;
        assert!(result2.is_success());
        // Balance should NOT change again
        assert_eq!(adapter.get_test_balance(1001, 1), 5000);
    }
}

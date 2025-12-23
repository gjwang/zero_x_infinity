//! Trading Account Adapter
//!
//! Adapter for UBSCore RAM-based Trading account balance operations.
//! Sends Deposit/Withdraw orders through the pipeline.

use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tracing::{debug, warn};

use super::ServiceAdapter;
use crate::transfer::types::{OpResult, RequestId};

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
/// 3. Track processed req_ids in RAM (rebuilt from WAL on restart)
pub struct TradingAdapter {
    /// Processed request IDs (for idempotency in RAM)
    /// In production, this is rebuilt from WAL on startup
    processed: Arc<Mutex<HashSet<RequestId>>>,

    /// Simulated balances for testing
    /// In production, this would be the UBSCore connection
    #[cfg(test)]
    test_balances: Arc<Mutex<std::collections::HashMap<(u64, u32), u64>>>,
}

impl TradingAdapter {
    /// Create a new TradingAdapter
    pub fn new() -> Self {
        Self {
            processed: Arc::new(Mutex::new(HashSet::new())),
            #[cfg(test)]
            test_balances: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Create adapter with existing processed set (for recovery)
    pub fn with_processed(processed: HashSet<RequestId>) -> Self {
        Self {
            processed: Arc::new(Mutex::new(processed)),
            #[cfg(test)]
            test_balances: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Check if a request was already processed
    fn is_processed(&self, req_id: RequestId) -> bool {
        self.processed.lock().unwrap().contains(&req_id)
    }

    /// Mark a request as processed
    fn mark_processed(&self, req_id: RequestId) {
        self.processed.lock().unwrap().insert(req_id);
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
        req_id: RequestId,
        user_id: u64,
        asset_id: u32,
        amount: u64,
    ) -> OpResult {
        debug!(
            req_id = req_id,
            user_id = user_id,
            asset_id = asset_id,
            amount = amount,
            "Trading withdraw"
        );

        // Check idempotency (in RAM)
        if self.is_processed(req_id) {
            debug!(req_id = req_id, "Trading withdraw already processed");
            return OpResult::Success;
        }

        // TODO: In production, send Withdraw order to UBSCore via pipeline
        // For now, we simulate with in-memory balance check

        #[cfg(test)]
        {
            let mut balances = self.test_balances.lock().unwrap();
            let key = (user_id, asset_id);
            let current = *balances.get(&key).unwrap_or(&0);

            if current < amount {
                warn!(
                    req_id = req_id,
                    current = current,
                    requested = amount,
                    "Insufficient trading balance"
                );
                return OpResult::Failed("Insufficient balance".to_string());
            }

            balances.insert(key, current - amount);
        }

        #[cfg(not(test))]
        {
            // Production: Send to UBSCore via pipeline
            // This is a placeholder - actual implementation will use the order queue
            warn!(
                req_id = req_id,
                "Trading adapter not connected to UBSCore pipeline"
            );
            // For now, assume success to allow FSM to proceed
        }

        self.mark_processed(req_id);
        debug!(req_id = req_id, "Trading withdraw successful");
        OpResult::Success
    }

    async fn deposit(
        &self,
        req_id: RequestId,
        user_id: u64,
        asset_id: u32,
        amount: u64,
    ) -> OpResult {
        debug!(
            req_id = req_id,
            user_id = user_id,
            asset_id = asset_id,
            amount = amount,
            "Trading deposit"
        );

        // Check idempotency
        if self.is_processed(req_id) {
            debug!(req_id = req_id, "Trading deposit already processed");
            return OpResult::Success;
        }

        // TODO: In production, send Deposit order to UBSCore via pipeline

        #[cfg(test)]
        {
            let mut balances = self.test_balances.lock().unwrap();
            let key = (user_id, asset_id);
            let current = *balances.get(&key).unwrap_or(&0);
            balances.insert(key, current + amount);
        }

        #[cfg(not(test))]
        {
            // Production: Send to UBSCore via pipeline
            warn!(
                req_id = req_id,
                "Trading adapter not connected to UBSCore pipeline"
            );
        }

        self.mark_processed(req_id);
        debug!(req_id = req_id, "Trading deposit successful");
        OpResult::Success
    }

    async fn rollback(&self, req_id: RequestId) -> OpResult {
        debug!(req_id = req_id, "Trading rollback");

        // CRITICAL: Trading rollback means re-crediting a previously withdrawn amount
        // This is only called when source was Trading and target deposit failed

        // Check if withdraw was even processed
        if !self.is_processed(req_id) {
            debug!(req_id = req_id, "No withdraw to rollback");
            return OpResult::Success;
        }

        // TODO: In production, send refund Deposit order to UBSCore
        // This needs the original transfer details (amount, user_id, asset_id)

        warn!(
            req_id = req_id,
            "Trading rollback - would need transfer details from DB"
        );

        // For now, return success (actual implementation will query DB)
        OpResult::Success
    }

    async fn commit(&self, req_id: RequestId) -> OpResult {
        debug!(req_id = req_id, "Trading commit");

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

        let result = adapter.withdraw(123, 1001, 1, 5000).await;
        assert!(result.is_success());
        assert_eq!(adapter.get_test_balance(1001, 1), 5000);
    }

    #[tokio::test]
    async fn test_trading_withdraw_insufficient() {
        let adapter = TradingAdapter::new();
        adapter.set_test_balance(1001, 1, 1000);

        let result = adapter.withdraw(123, 1001, 1, 5000).await;
        assert!(result.is_explicit_fail());
        // Balance unchanged
        assert_eq!(adapter.get_test_balance(1001, 1), 1000);
    }

    #[tokio::test]
    async fn test_trading_deposit() {
        let adapter = TradingAdapter::new();
        adapter.set_test_balance(1001, 1, 5000);

        let result = adapter.deposit(123, 1001, 1, 3000).await;
        assert!(result.is_success());
        assert_eq!(adapter.get_test_balance(1001, 1), 8000);
    }

    #[tokio::test]
    async fn test_trading_idempotency() {
        let adapter = TradingAdapter::new();
        adapter.set_test_balance(1001, 1, 10000);

        // First call
        let result1 = adapter.withdraw(123, 1001, 1, 5000).await;
        assert!(result1.is_success());
        assert_eq!(adapter.get_test_balance(1001, 1), 5000);

        // Second call with same req_id - should be idempotent
        let result2 = adapter.withdraw(123, 1001, 1, 5000).await;
        assert!(result2.is_success());
        // Balance should NOT change again
        assert_eq!(adapter.get_test_balance(1001, 1), 5000);
    }
}

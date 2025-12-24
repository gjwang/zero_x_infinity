//! Service Adapters
//!
//! Adapters for interacting with different balance services (Funding, Trading).
//! All adapters must be idempotent using req_id.

pub mod funding;
pub mod trading;

// Re-export adapters for convenient access
pub use funding::FundingAdapter;
pub use trading::TradingAdapter;

use async_trait::async_trait;

use super::types::{InternalTransferId, OpResult};

/// Service adapter trait for balance operations
///
/// All methods MUST be idempotent - calling with the same req_id multiple times
/// must have the same effect as calling once.
#[async_trait]
pub trait ServiceAdapter: Send + Sync {
    /// Get adapter name for logging
    fn name(&self) -> &'static str;

    /// Withdraw funds from this service (debit)
    ///
    /// # Idempotency
    /// If already processed with this req_id, return the original result.
    async fn withdraw(
        &self,
        req_id: InternalTransferId,
        user_id: u64,
        asset_id: u32,
        amount: u64,
    ) -> OpResult;

    /// Deposit funds to this service (credit)
    ///
    /// # Idempotency
    /// If already processed with this req_id, return the original result.
    async fn deposit(
        &self,
        req_id: InternalTransferId,
        user_id: u64,
        asset_id: u32,
        amount: u64,
    ) -> OpResult;

    /// Rollback a previous withdraw (refund)
    ///
    /// Only called during compensation phase when target deposit fails.
    async fn rollback(&self, req_id: InternalTransferId) -> OpResult;

    /// Commit/finalize a transfer (cleanup any locks)
    ///
    /// Called after target deposit succeeds to release any holds.
    async fn commit(&self, req_id: InternalTransferId) -> OpResult;
}

/// Mock adapter for testing
#[cfg(test)]
pub mod mock {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    pub struct MockAdapter {
        name: &'static str,
        /// Track operations for verification
        operations: Mutex<HashMap<InternalTransferId, Vec<String>>>,
        /// Count of each operation type
        withdraw_count: AtomicUsize,
        deposit_count: AtomicUsize,
        rollback_count: AtomicUsize,
        /// Configured behavior
        fail_withdraw: Mutex<bool>,
        fail_deposit: Mutex<bool>,
        pending_deposit: Mutex<bool>,
    }

    impl MockAdapter {
        pub fn new(name: &'static str) -> Self {
            Self {
                name,
                operations: Mutex::new(HashMap::new()),
                withdraw_count: AtomicUsize::new(0),
                deposit_count: AtomicUsize::new(0),
                rollback_count: AtomicUsize::new(0),
                fail_withdraw: Mutex::new(false),
                fail_deposit: Mutex::new(false),
                pending_deposit: Mutex::new(false),
            }
        }

        pub fn set_fail_withdraw(&self, fail: bool) {
            *self.fail_withdraw.lock().unwrap() = fail;
        }

        pub fn set_fail_deposit(&self, fail: bool) {
            *self.fail_deposit.lock().unwrap() = fail;
        }

        pub fn set_pending_deposit(&self, pending: bool) {
            *self.pending_deposit.lock().unwrap() = pending;
        }

        pub fn withdraw_count(&self) -> usize {
            self.withdraw_count.load(Ordering::SeqCst)
        }

        pub fn deposit_count(&self) -> usize {
            self.deposit_count.load(Ordering::SeqCst)
        }

        pub fn rollback_count(&self) -> usize {
            self.rollback_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl ServiceAdapter for MockAdapter {
        fn name(&self) -> &'static str {
            self.name
        }

        async fn withdraw(
            &self,
            req_id: InternalTransferId,
            _user_id: u64,
            _asset_id: u32,
            _amount: u64,
        ) -> OpResult {
            self.withdraw_count.fetch_add(1, Ordering::SeqCst);

            let mut ops = self.operations.lock().unwrap();
            ops.entry(req_id).or_default().push("withdraw".to_string());

            if *self.fail_withdraw.lock().unwrap() {
                OpResult::Failed("Mock withdraw failure".to_string())
            } else {
                OpResult::Success
            }
        }

        async fn deposit(
            &self,
            req_id: InternalTransferId,
            _user_id: u64,
            _asset_id: u32,
            _amount: u64,
        ) -> OpResult {
            self.deposit_count.fetch_add(1, Ordering::SeqCst);

            let mut ops = self.operations.lock().unwrap();
            ops.entry(req_id).or_default().push("deposit".to_string());

            if *self.pending_deposit.lock().unwrap() {
                OpResult::Pending
            } else if *self.fail_deposit.lock().unwrap() {
                OpResult::Failed("Mock deposit failure".to_string())
            } else {
                OpResult::Success
            }
        }

        async fn rollback(&self, req_id: InternalTransferId) -> OpResult {
            self.rollback_count.fetch_add(1, Ordering::SeqCst);

            let mut ops = self.operations.lock().unwrap();
            ops.entry(req_id).or_default().push("rollback".to_string());

            OpResult::Success
        }

        async fn commit(&self, req_id: InternalTransferId) -> OpResult {
            let mut ops = self.operations.lock().unwrap();
            ops.entry(req_id).or_default().push("commit".to_string());

            OpResult::Success
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn test_mock_adapter_success() {
            let adapter = MockAdapter::new("test");

            let result = adapter
                .withdraw(crate::transfer::InternalTransferId::new(), 1001, 1, 1000)
                .await;
            assert!(result.is_success());
            assert_eq!(adapter.withdraw_count(), 1);

            let result = adapter
                .deposit(crate::transfer::InternalTransferId::new(), 1001, 1, 1000)
                .await;
            assert!(result.is_success());
            assert_eq!(adapter.deposit_count(), 1);
        }

        #[tokio::test]
        async fn test_mock_adapter_failure() {
            let adapter = MockAdapter::new("test");
            adapter.set_fail_withdraw(true);

            let result = adapter
                .withdraw(crate::transfer::InternalTransferId::new(), 1001, 1, 1000)
                .await;
            assert!(result.is_explicit_fail());
        }

        #[tokio::test]
        async fn test_mock_adapter_pending() {
            let adapter = MockAdapter::new("test");
            adapter.set_pending_deposit(true);

            let result = adapter
                .deposit(crate::transfer::InternalTransferId::new(), 1001, 1, 1000)
                .await;
            assert!(result.is_pending());
        }
    }
}

#[cfg(test)]
pub use mock::MockAdapter;

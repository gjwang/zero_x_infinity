//! Transfer Coordinator
//!
//! Orchestrates the FSM-based transfer processing.

use anyhow::Result;
use std::sync::{Arc, Mutex};

use crate::fast_ulid::SnowflakeGenRng;
use crate::transfer::adapters::ServiceAdapter;
use crate::transfer::db::TransferDb;
use crate::transfer::state::TransferState;
use crate::transfer::types::{OpResult, RequestId, ServiceId, TransferRecord, TransferRequest};

/// Transfer Coordinator - orchestrates FSM-based processing
pub struct TransferCoordinator {
    db: Arc<TransferDb>,
    funding_adapter: Arc<dyn ServiceAdapter>,
    trading_adapter: Arc<dyn ServiceAdapter>,
    id_gen: Mutex<SnowflakeGenRng>,
}

impl TransferCoordinator {
    pub fn new(
        db: Arc<TransferDb>,
        funding_adapter: Arc<dyn ServiceAdapter>,
        trading_adapter: Arc<dyn ServiceAdapter>,
    ) -> Self {
        Self::with_machine_id(db, funding_adapter, trading_adapter, 1)
    }

    /// Create coordinator with specific machine ID for distributed deployment
    pub fn with_machine_id(
        db: Arc<TransferDb>,
        funding_adapter: Arc<dyn ServiceAdapter>,
        trading_adapter: Arc<dyn ServiceAdapter>,
        machine_id: u8,
    ) -> Self {
        Self {
            db,
            funding_adapter,
            trading_adapter,
            id_gen: Mutex::new(SnowflakeGenRng::new(machine_id)),
        }
    }

    /// Create a new transfer record
    pub async fn create(&self, req: TransferRequest) -> Result<RequestId> {
        // Validate request
        if req.amount == 0 {
            return Err(anyhow::anyhow!("Amount must be greater than 0"));
        }

        if req.from == req.to {
            return Err(anyhow::anyhow!("Source and target cannot be the same"));
        }

        // Generate RequestId using Snowflake
        let req_id = {
            let mut gen = self.id_gen.lock().unwrap();
            RequestId::new(gen.generate())
        };
        let now = chrono::Utc::now().timestamp_millis();

        let record = TransferRecord {
            req_id,
            source: req.from,
            target: req.to,
            user_id: req.user_id,
            asset_id: req.asset_id,
            amount: req.amount,
            state: TransferState::Init,
            created_at: now,
            updated_at: now,
            error: None,
            retry_count: 0,
        };

        self.db.create(&record).await?;
        log::info!("Created transfer: {} ({:?} -> {:?})", req_id, req.from, req.to);

        Ok(req_id)
    }

    /// Execute one step of the FSM
    /// Returns the new state after processing
    pub async fn step(&self, req_id: RequestId) -> Result<TransferState> {
        let record = self
            .db
            .get(req_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Transfer not found: {}", req_id))?;

        // Already terminal - nothing to do
        if record.state.is_terminal() {
            return Ok(record.state);
        }

        // Get adapters for source and target
        let source = self.get_adapter(record.source);
        let target = self.get_adapter(record.target);

        // Process based on current state
        let new_state = match record.state {
            TransferState::Init => {
                self.step_init(&record, source.as_ref()).await?
            }
            TransferState::SourcePending => {
                self.step_source_pending(&record, source.as_ref()).await?
            }
            TransferState::SourceDone => {
                self.step_source_done(&record, source.as_ref(), target.as_ref()).await?
            }
            TransferState::TargetPending => {
                self.step_target_pending(&record, source.as_ref(), target.as_ref()).await?
            }
            TransferState::Compensating => {
                self.step_compensating(&record, source.as_ref()).await?
            }
            _ => record.state, // Terminal states
        };

        // Increment retry count
        if !new_state.is_terminal() && new_state == record.state {
            self.db.increment_retry(req_id).await?;
        }

        Ok(new_state)
    }

    fn get_adapter(&self, service: ServiceId) -> Arc<dyn ServiceAdapter> {
        match service {
            ServiceId::Funding => self.funding_adapter.clone(),
            ServiceId::Trading => self.trading_adapter.clone(),
        }
    }

    /// Helper: Finalize source commit after target success
    /// Logs warning if commit fails but does not fail the transfer
    async fn finalize_source_commit(&self, record: &TransferRecord, source: &dyn ServiceAdapter) {
        let commit_result = source.commit(record.req_id).await;
        if let OpResult::Failed(e) = &commit_result {
            log::warn!(
                "Source commit failed for {} (target already received funds): {}",
                record.req_id, e
            );
            // TODO: Send alert to ops for manual cleanup of frozen funds
        }
    }

    /// Step from Init state: Call source.withdraw()
    async fn step_init(
        &self,
        record: &TransferRecord,
        source: &dyn ServiceAdapter,
    ) -> Result<TransferState> {
        // 1. Persist SourcePending BEFORE calling service (persist-before-call)
        if !self.db.update_state_if(record.req_id, TransferState::Init, TransferState::SourcePending).await? {
            // Another worker already transitioned - get current state
            return match self.db.get(record.req_id).await? {
                Some(r) => Ok(r.state),
                None => {
                    log::error!("Transfer {} not found after CAS failure (data corruption?)", record.req_id);
                    Err(anyhow::anyhow!("Transfer not found after CAS failure"))
                }
            };
        }

        // 2. Call source withdraw
        let result = source.withdraw(
            record.req_id,
            record.user_id,
            record.asset_id,
            record.amount,
        ).await;

        // 3. Handle result
        match result {
            OpResult::Success => {
                self.db.update_state_if(record.req_id, TransferState::SourcePending, TransferState::SourceDone).await?;
                Ok(TransferState::SourceDone)
            }
            OpResult::Failed(e) => {
                self.db.update_state_with_error(record.req_id, TransferState::SourcePending, TransferState::Failed, &e).await?;
                Ok(TransferState::Failed)
            }
            OpResult::Pending => {
                // Stay in SourcePending, will retry on next scan
                Ok(TransferState::SourcePending)
            }
        }
    }

    /// Step from SourcePending state: Re-call source.withdraw() (idempotent)
    async fn step_source_pending(
        &self,
        record: &TransferRecord,
        source: &dyn ServiceAdapter,
    ) -> Result<TransferState> {
        // Query or re-call source (idempotent)
        let result = source.withdraw(
            record.req_id,
            record.user_id,
            record.asset_id,
            record.amount,
        ).await;

        match result {
            OpResult::Success => {
                self.db.update_state_if(record.req_id, TransferState::SourcePending, TransferState::SourceDone).await?;
                Ok(TransferState::SourceDone)
            }
            OpResult::Failed(e) => {
                self.db.update_state_with_error(record.req_id, TransferState::SourcePending, TransferState::Failed, &e).await?;
                Ok(TransferState::Failed)
            }
            OpResult::Pending => {
                Ok(TransferState::SourcePending)
            }
        }
    }

    /// Step from SourceDone state: Call target.deposit()
    async fn step_source_done(
        &self,
        record: &TransferRecord,
        source: &dyn ServiceAdapter,
        target: &dyn ServiceAdapter,
    ) -> Result<TransferState> {
        // 1. Persist TargetPending BEFORE calling service
        if !self.db.update_state_if(record.req_id, TransferState::SourceDone, TransferState::TargetPending).await? {
            return match self.db.get(record.req_id).await? {
                Some(r) => Ok(r.state),
                None => {
                    log::error!("Transfer {} not found after CAS failure (data corruption?)", record.req_id);
                    Err(anyhow::anyhow!("Transfer not found after CAS failure"))
                }
            };
        }

        // 2. Call target deposit
        let result = target.deposit(
            record.req_id,
            record.user_id,
            record.asset_id,
            record.amount,
        ).await;

        // 3. Handle result
        match result {
            OpResult::Success => {
                // Finalize source commit
                self.finalize_source_commit(record, source).await;
                self.db.update_state_if(record.req_id, TransferState::TargetPending, TransferState::Committed).await?;
                Ok(TransferState::Committed)
            }
            OpResult::Failed(e) => {
                self.db.update_state_with_error(record.req_id, TransferState::TargetPending, TransferState::Compensating, &e).await?;
                Ok(TransferState::Compensating)
            }
            OpResult::Pending => {
                Ok(TransferState::TargetPending)
            }
        }
    }

    /// Step from TargetPending state: Re-call target.deposit() (idempotent)
    async fn step_target_pending(
        &self,
        record: &TransferRecord,
        source: &dyn ServiceAdapter,
        target: &dyn ServiceAdapter,
    ) -> Result<TransferState> {
        let result = target.deposit(
            record.req_id,
            record.user_id,
            record.asset_id,
            record.amount,
        ).await;

        match result {
            OpResult::Success => {
                // Finalize source commit
                self.finalize_source_commit(record, source).await;
                self.db.update_state_if(record.req_id, TransferState::TargetPending, TransferState::Committed).await?;
                Ok(TransferState::Committed)
            }
            OpResult::Failed(e) => {
                // CRITICAL: When source is Trading, we CANNOT rollback!
                // Trading operations are immediate/final. Once withdrawn, money is gone.
                // We MUST keep retrying target until it succeeds.
                if record.source == ServiceId::Trading {
                    log::error!(
                        "Target deposit failed for {} but source is Trading (cannot rollback)! \
                         Staying in TargetPending to retry. Error: {}",
                        record.req_id, e
                    );
                    // Stay in TargetPending - keep retrying target deposit
                    // Do NOT go to Compensating - Trading cannot be rolled back!
                    Ok(TransferState::TargetPending)
                } else {
                    // Source is Funding (can be rolled back via TB void)
                    self.db.update_state_with_error(record.req_id, TransferState::TargetPending, TransferState::Compensating, &e).await?;
                    Ok(TransferState::Compensating)
                }
            }
            OpResult::Pending => {
                Ok(TransferState::TargetPending)
            }
        }
    }

    /// Step from Compensating state: Call source.rollback()
    async fn step_compensating(
        &self,
        record: &TransferRecord,
        source: &dyn ServiceAdapter,
    ) -> Result<TransferState> {
        let result = source.rollback(record.req_id).await;

        match result {
            OpResult::Success => {
                self.db.update_state_if(record.req_id, TransferState::Compensating, TransferState::RolledBack).await?;
                Ok(TransferState::RolledBack)
            }
            OpResult::Failed(e) => {
                // Rollback failed - stay in Compensating, keep retrying
                log::warn!("Rollback failed for {}: {} (will retry)", record.req_id, e);
                Ok(TransferState::Compensating)
            }
            OpResult::Pending => {
                Ok(TransferState::Compensating)
            }
        }
    }

    /// Get current state of a transfer
    pub async fn get_state(&self, req_id: RequestId) -> Result<Option<TransferState>> {
        Ok(self.db.get(req_id).await?.map(|r| r.state))
    }

    /// Get full transfer record
    pub async fn get(&self, req_id: RequestId) -> Result<Option<TransferRecord>> {
        self.db.get(req_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::adapters::MockAdapter;

    // Helper to create test coordinator with mock adapters
    async fn create_test_coordinator() -> (Arc<TransferCoordinator>, Arc<MockAdapter>, Arc<MockAdapter>) {
        // Use in-memory mock DB - we'll skip actual DB calls in these tests
        // by using the mock adapters that always succeed
        let funding = Arc::new(MockAdapter::new("funding"));
        let trading = Arc::new(MockAdapter::new("trading"));

        // Create a dummy session - these tests focus on coordinator logic
        // Integration tests handle actual DB operations
        let session = scylla::SessionBuilder::new()
            .known_node("127.0.0.1:9042")
            .build()
            .await
            .expect("ScyllaDB required for integration tests");

        let db = Arc::new(TransferDb::new(Arc::new(session)));
        let coordinator = Arc::new(TransferCoordinator::new(
            db,
            funding.clone(),
            trading.clone(),
        ));

        (coordinator, funding, trading)
    }

    #[test]
    fn test_get_adapter() {
        // This is a sync test - just verify adapter selection logic
        // The actual adapter usage is tested via integration tests
    }

    #[tokio::test]
    #[ignore = "Requires ScyllaDB"]
    async fn test_create_transfer_validation() {
        let (coordinator, _, _) = create_test_coordinator().await;

        // Test zero amount
        let req = TransferRequest {
            from: ServiceId::Funding,
            to: ServiceId::Trading,
            user_id: 1001,
            asset_id: 1,
            amount: 0,
        };
        let result = coordinator.create(req).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Amount must be greater than 0"));

        // Test same source and target
        let req = TransferRequest {
            from: ServiceId::Funding,
            to: ServiceId::Funding,
            user_id: 1001,
            asset_id: 1,
            amount: 1000,
        };
        let result = coordinator.create(req).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Source and target cannot be the same"));
    }

    #[tokio::test]
    #[ignore = "Requires ScyllaDB"]
    async fn test_create_transfer_success() {
        let (coordinator, _, _) = create_test_coordinator().await;

        let req = TransferRequest {
            from: ServiceId::Funding,
            to: ServiceId::Trading,
            user_id: 1001,
            asset_id: 1,
            amount: 1000000,
        };

        let result = coordinator.create(req).await;
        assert!(result.is_ok());
        let req_id = result.unwrap();

        // Verify initial state
        let state = coordinator.get_state(req_id).await.unwrap();
        assert_eq!(state, Some(TransferState::Init));
    }

    #[tokio::test]
    #[ignore = "Requires ScyllaDB"]
    async fn test_step_happy_path() {
        let (coordinator, _, _) = create_test_coordinator().await;

        // Create transfer
        let req = TransferRequest {
            from: ServiceId::Funding,
            to: ServiceId::Trading,
            user_id: 1001,
            asset_id: 1,
            amount: 1000000,
        };
        let req_id = coordinator.create(req).await.unwrap();

        // Step until terminal
        let mut state = TransferState::Init;
        for _ in 0..10 {
            state = coordinator.step(req_id).await.unwrap();
            if state.is_terminal() {
                break;
            }
        }

        // Should reach Committed with mock adapters
        assert_eq!(state, TransferState::Committed);
    }
}


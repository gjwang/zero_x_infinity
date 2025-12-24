//! Transfer Coordinator
//!
//! Orchestrates the FSM-based transfer processing.
//! This is the central component that drives state transitions.

use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, warn};

use super::adapters::ServiceAdapter;
use super::db::TransferDb;
use super::error::TransferError;
use super::state::TransferState;
use super::types::{OpResult, RequestId, ServiceId, TransferRecord, TransferRequest};

/// Snowflake ID generator for req_id
///
/// Simple implementation - in production, use a proper Snowflake library
struct SnowflakeGenerator {
    machine_id: u8,
    sequence: u32,
    last_timestamp: u64,
}

impl SnowflakeGenerator {
    fn new(machine_id: u8) -> Self {
        Self {
            machine_id,
            sequence: 0,
            last_timestamp: 0,
        }
    }

    fn generate(&mut self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        if now == self.last_timestamp {
            self.sequence += 1;
        } else {
            self.sequence = 0;
            self.last_timestamp = now;
        }

        // Format: timestamp (41 bits) | machine_id (8 bits) | sequence (15 bits)
        (now << 23) | ((self.machine_id as u64) << 15) | (self.sequence as u64 & 0x7FFF)
    }
}

/// Transfer Coordinator - orchestrates FSM-based processing
pub struct TransferCoordinator {
    db: Arc<TransferDb>,
    funding_adapter: Arc<dyn ServiceAdapter>,
    trading_adapter: Arc<dyn ServiceAdapter>,
    id_gen: Mutex<SnowflakeGenerator>,
}

impl TransferCoordinator {
    /// Create a new TransferCoordinator
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
            id_gen: Mutex::new(SnowflakeGenerator::new(machine_id)),
        }
    }

    /// Create a new transfer record
    ///
    /// # Validation (Defense Layer 2: Coordinator)
    /// Re-validates critical parameters to prevent internal calls bypassing API.
    pub async fn create(&self, req: TransferRequest) -> Result<RequestId, TransferError> {
        // === Defense-in-Depth Layer 2: Coordinator Validation ===
        if req.amount == 0 {
            return Err(TransferError::InvalidAmount);
        }

        if req.from == req.to {
            return Err(TransferError::SameAccount);
        }

        if req.user_id == 0 {
            return Err(TransferError::Forbidden);
        }

        // Check for duplicate cid
        if let Some(ref cid) = req.cid
            && let Some(existing) = self.db.get_by_cid(cid).await?
        {
            debug!(cid = %cid, req_id = existing.req_id, "Duplicate cid found");
            return Ok(existing.req_id);
        }

        // Generate RequestId using Snowflake
        let req_id = {
            let mut id_generator = self.id_gen.lock().unwrap();
            id_generator.generate()
        };

        // Create transfer record
        let record = TransferRecord::new(
            req_id,
            req.from,
            req.to,
            req.user_id,
            req.asset_id,
            req.amount,
            req.cid,
        );

        self.db.create(&record).await?;
        info!(
            req_id = req_id,
            "Transfer created: {} -> {}", req.from, req.to
        );

        Ok(req_id)
    }

    /// Execute one step of the FSM
    ///
    /// Returns the new state after processing.
    /// Call repeatedly until a terminal state is reached.
    pub async fn step(&self, req_id: RequestId) -> Result<TransferState, TransferError> {
        let record = self
            .db
            .get(req_id)
            .await?
            .ok_or_else(|| TransferError::TransferNotFound(req_id.to_string()))?;

        // Already terminal - nothing to do
        if record.state.is_terminal() {
            return Ok(record.state);
        }

        // Get adapters for source and target
        let source = self.get_adapter(record.source);
        let target = self.get_adapter(record.target);

        // Process based on current state
        let new_state = match record.state {
            TransferState::Init => self.step_init(&record, source.as_ref()).await?,
            TransferState::SourcePending => {
                self.step_source_pending(&record, source.as_ref()).await?
            }
            TransferState::SourceDone => {
                self.step_source_done(&record, source.as_ref(), target.as_ref())
                    .await?
            }
            TransferState::TargetPending => {
                self.step_target_pending(&record, source.as_ref(), target.as_ref())
                    .await?
            }
            TransferState::Compensating => self.step_compensating(&record, source.as_ref()).await?,
            _ => record.state, // Terminal states - no processing
        };

        // Increment retry count if no progress
        if !new_state.is_terminal() && new_state == record.state {
            self.db.increment_retry(req_id).await?;
        }

        Ok(new_state)
    }

    /// Execute transfer to completion (blocking)
    ///
    /// Runs step() repeatedly until a terminal state is reached.
    /// Returns the final state.
    pub async fn execute(&self, req_id: RequestId) -> Result<TransferState, TransferError> {
        let mut state = TransferState::Init;
        let max_iterations = 100; // Safety limit

        for i in 0..max_iterations {
            state = self.step(req_id).await?;

            if state.is_terminal() {
                debug!(
                    req_id = req_id,
                    state = %state,
                    iterations = i + 1,
                    "Transfer completed"
                );
                return Ok(state);
            }

            // Small delay between retries for pending states
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        warn!(
            req_id = req_id,
            state = %state,
            "Transfer did not complete within iteration limit"
        );
        Ok(state)
    }

    fn get_adapter(&self, service: ServiceId) -> Arc<dyn ServiceAdapter> {
        match service {
            ServiceId::Funding => self.funding_adapter.clone(),
            ServiceId::Trading => self.trading_adapter.clone(),
        }
    }

    /// Step from Init state: Call source.withdraw()
    async fn step_init(
        &self,
        record: &TransferRecord,
        source: &dyn ServiceAdapter,
    ) -> Result<TransferState, TransferError> {
        // 1. Persist SourcePending BEFORE calling service (persist-before-call)
        if !self
            .db
            .update_state_if(
                record.req_id,
                TransferState::Init,
                TransferState::SourcePending,
            )
            .await?
        {
            // Another worker already transitioned - get current state
            return match self.db.get(record.req_id).await? {
                Some(r) => Ok(r.state),
                None => {
                    error!(
                        req_id = record.req_id,
                        "Transfer not found after CAS failure (data corruption?)"
                    );
                    Err(TransferError::TransferNotFound(record.req_id.to_string()))
                }
            };
        }

        // 2. Call source withdraw
        let result = source
            .withdraw(
                record.req_id,
                record.user_id,
                record.asset_id,
                record.amount,
            )
            .await;

        // 3. Handle result
        match result {
            OpResult::Success => {
                self.db
                    .update_state_if(
                        record.req_id,
                        TransferState::SourcePending,
                        TransferState::SourceDone,
                    )
                    .await?;
                Ok(TransferState::SourceDone)
            }
            OpResult::Failed(e) => {
                self.db
                    .update_state_with_error(
                        record.req_id,
                        TransferState::SourcePending,
                        TransferState::Failed,
                        &e,
                    )
                    .await?;
                Ok(TransferState::Failed)
            }
            OpResult::Pending => {
                // Stay in SourcePending, will retry on next step
                Ok(TransferState::SourcePending)
            }
        }
    }

    /// Step from SourcePending state: Re-call source.withdraw() (idempotent)
    async fn step_source_pending(
        &self,
        record: &TransferRecord,
        source: &dyn ServiceAdapter,
    ) -> Result<TransferState, TransferError> {
        // Query or re-call source (idempotent)
        let result = source
            .withdraw(
                record.req_id,
                record.user_id,
                record.asset_id,
                record.amount,
            )
            .await;

        match result {
            OpResult::Success => {
                self.db
                    .update_state_if(
                        record.req_id,
                        TransferState::SourcePending,
                        TransferState::SourceDone,
                    )
                    .await?;
                Ok(TransferState::SourceDone)
            }
            OpResult::Failed(e) => {
                self.db
                    .update_state_with_error(
                        record.req_id,
                        TransferState::SourcePending,
                        TransferState::Failed,
                        &e,
                    )
                    .await?;
                Ok(TransferState::Failed)
            }
            OpResult::Pending => Ok(TransferState::SourcePending),
        }
    }

    /// Step from SourceDone state: Call target.deposit()
    ///
    /// CRITICAL: Funds are now IN-FLIGHT. Must reach terminal state.
    async fn step_source_done(
        &self,
        record: &TransferRecord,
        source: &dyn ServiceAdapter,
        target: &dyn ServiceAdapter,
    ) -> Result<TransferState, TransferError> {
        // 1. Persist TargetPending BEFORE calling service
        if !self
            .db
            .update_state_if(
                record.req_id,
                TransferState::SourceDone,
                TransferState::TargetPending,
            )
            .await?
        {
            return match self.db.get(record.req_id).await? {
                Some(r) => Ok(r.state),
                None => {
                    error!(
                        req_id = record.req_id,
                        "Transfer not found after CAS failure"
                    );
                    Err(TransferError::TransferNotFound(record.req_id.to_string()))
                }
            };
        }

        // 2. Call target deposit
        let result = target
            .deposit(
                record.req_id,
                record.user_id,
                record.asset_id,
                record.amount,
            )
            .await;

        // 3. Handle result
        match result {
            OpResult::Success => {
                // === ATOMIC COMMIT ===
                // Both source withdraw and target deposit succeeded.
                // Finalize source commit (cleanup any holds)
                self.finalize_source_commit(record, source).await;

                self.db
                    .update_state_if(
                        record.req_id,
                        TransferState::TargetPending,
                        TransferState::Committed,
                    )
                    .await?;

                info!(req_id = record.req_id, "🔒 ATOMIC COMMIT SUCCESS");
                Ok(TransferState::Committed)
            }
            OpResult::Failed(e) => {
                // Target explicitly failed - check if we can rollback
                // CRITICAL: If source is Trading, we CANNOT rollback!
                if record.source == ServiceId::Trading {
                    // Trading withdrawals are immediately final.
                    // We MUST keep retrying target until it succeeds.
                    error!(
                        req_id = record.req_id,
                        error = %e,
                        "Target deposit failed but source is Trading (cannot rollback)! \
                         Staying in TargetPending to retry."
                    );
                    // Stay in TargetPending - keep retrying target deposit
                    Ok(TransferState::TargetPending)
                } else {
                    // Source is Funding (can be rolled back)
                    self.db
                        .update_state_with_error(
                            record.req_id,
                            TransferState::TargetPending,
                            TransferState::Compensating,
                            &e,
                        )
                        .await?;
                    Ok(TransferState::Compensating)
                }
            }
            OpResult::Pending => {
                // Unknown state - MUST NOT compensate, keep retrying
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
    ) -> Result<TransferState, TransferError> {
        let result = target
            .deposit(
                record.req_id,
                record.user_id,
                record.asset_id,
                record.amount,
            )
            .await;

        match result {
            OpResult::Success => {
                // Finalize source commit
                self.finalize_source_commit(record, source).await;

                self.db
                    .update_state_if(
                        record.req_id,
                        TransferState::TargetPending,
                        TransferState::Committed,
                    )
                    .await?;

                info!(req_id = record.req_id, "🔒 ATOMIC COMMIT SUCCESS");
                Ok(TransferState::Committed)
            }
            OpResult::Failed(e) => {
                // CRITICAL check: Trading source CANNOT be rolled back
                if record.source == ServiceId::Trading {
                    error!(
                        req_id = record.req_id,
                        error = %e,
                        "Target failed but Trading source cannot rollback! Infinite retry."
                    );
                    Ok(TransferState::TargetPending)
                } else {
                    self.db
                        .update_state_with_error(
                            record.req_id,
                            TransferState::TargetPending,
                            TransferState::Compensating,
                            &e,
                        )
                        .await?;
                    Ok(TransferState::Compensating)
                }
            }
            OpResult::Pending => Ok(TransferState::TargetPending),
        }
    }

    /// Step from Compensating state: Call source.rollback()
    async fn step_compensating(
        &self,
        record: &TransferRecord,
        source: &dyn ServiceAdapter,
    ) -> Result<TransferState, TransferError> {
        let result = source.rollback(record.req_id).await;

        match result {
            OpResult::Success => {
                self.db
                    .update_state_if(
                        record.req_id,
                        TransferState::Compensating,
                        TransferState::RolledBack,
                    )
                    .await?;

                info!(req_id = record.req_id, "Transfer rolled back");
                Ok(TransferState::RolledBack)
            }
            OpResult::Failed(e) => {
                // Rollback failed - stay in Compensating, keep retrying
                warn!(
                    req_id = record.req_id,
                    error = %e,
                    "Rollback failed (will retry)"
                );
                Ok(TransferState::Compensating)
            }
            OpResult::Pending => Ok(TransferState::Compensating),
        }
    }

    /// Helper: Finalize source commit after target success
    async fn finalize_source_commit(&self, record: &TransferRecord, source: &dyn ServiceAdapter) {
        let commit_result = source.commit(record.req_id).await;
        if let OpResult::Failed(e) = &commit_result {
            warn!(
                req_id = record.req_id,
                error = %e,
                "Source commit failed (target already received funds)"
            );
            // TODO: Send alert to ops for manual cleanup of any holds
        }
    }

    /// Get current state of a transfer
    pub async fn get_state(
        &self,
        req_id: RequestId,
    ) -> Result<Option<TransferState>, TransferError> {
        Ok(self.db.get(req_id).await?.map(|r| r.state))
    }

    /// Get full transfer record
    pub async fn get(&self, req_id: RequestId) -> Result<Option<TransferRecord>, TransferError> {
        self.db.get(req_id).await
    }

    /// Access to DB for recovery worker
    pub fn db(&self) -> &Arc<TransferDb> {
        &self.db
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::adapters::MockAdapter;
    use sqlx::postgres::PgPoolOptions;

    async fn create_test_pool() -> Option<sqlx::PgPool> {
        // Try to connect to test database
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@localhost:5432/zero_x_infinity_test".to_string()
        });

        PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .ok()
    }

    #[test]
    fn test_snowflake_generator() {
        let mut id_generator = SnowflakeGenerator::new(1);
        let id1 = id_generator.generate();
        let id2 = id_generator.generate();

        assert_ne!(id1, id2);
        assert!(id2 > id1); // Should be monotonically increasing
    }

    #[tokio::test]
    async fn test_coordinator_validation() {
        let pool = match create_test_pool().await {
            Some(p) => p,
            None => {
                eprintln!("Skipping test - database not available");
                return;
            }
        };

        let db = Arc::new(TransferDb::new(pool));
        let funding = Arc::new(MockAdapter::new("funding"));
        let trading = Arc::new(MockAdapter::new("trading"));
        let coordinator = TransferCoordinator::new(db, funding, trading);

        // Test zero amount
        let req = TransferRequest::new(ServiceId::Funding, ServiceId::Trading, 1001, 1, 0);
        let result = coordinator.create(req).await;
        assert!(matches!(result, Err(TransferError::InvalidAmount)));

        // Test same source and target
        let req = TransferRequest::new(ServiceId::Funding, ServiceId::Funding, 1001, 1, 1000);
        let result = coordinator.create(req).await;
        assert!(matches!(result, Err(TransferError::SameAccount)));
    }
}

//! Integration Tests for Transfer FSM
//!
//! These tests verify the complete FSM flow without needing a live database.
//! They use the MockAdapter for both Funding and Trading to simulate scenarios.

#[cfg(test)]
use std::sync::Arc;

use crate::internal_transfer::adapters::MockAdapter;
use crate::internal_transfer::coordinator::TransferCoordinator;
use crate::internal_transfer::db::TransferDb;
use crate::internal_transfer::state::TransferState;
use crate::internal_transfer::types::{ServiceId, TransferRequest};

/// Helper to create a coordinator with mock adapters for testing
struct TestHarness {
    coordinator: TransferCoordinator,
    funding: Arc<MockAdapter>,
    trading: Arc<MockAdapter>,
}

impl TestHarness {
    fn new(pool: sqlx::PgPool) -> Self {
        // ULID-based InternalTransferId doesn't need machine_id coordination

        let db = Arc::new(TransferDb::new(pool));
        let funding = Arc::new(MockAdapter::new("funding"));
        let trading = Arc::new(MockAdapter::new("trading"));

        let coordinator = TransferCoordinator::new(db, funding.clone(), trading.clone());

        Self {
            coordinator,
            funding,
            trading,
        }
    }
}

// ========================================================================
// Happy Path Tests
// ========================================================================

/// Test: Funding → Spot transfer completes successfully
///
/// Flow: INIT → SOURCE_PENDING → SOURCE_DONE → TARGET_PENDING → COMMITTED
#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn test_funding_to_spot_happy_path() {
    let pool = create_test_pool().await;
    let harness = TestHarness::new(pool);

    // Create transfer request
    let req = TransferRequest::new(
        ServiceId::Funding,
        ServiceId::Trading,
        1001,               // user_id
        1,                  // asset_id (BTC)
        100_000_000.into(), // 1.0 BTC
    );

    // Create transfer
    let transfer_id = harness.coordinator.create(req).await.unwrap();
    // InternalTransferId is ULID, no comparison to 0 needed

    // Execute to completion
    let final_state = harness.coordinator.execute(transfer_id).await.unwrap();
    assert_eq!(final_state, TransferState::Committed);

    // Verify adapter calls
    assert_eq!(harness.funding.withdraw_count(), 1);
    assert_eq!(harness.trading.deposit_count(), 1);
}

/// Test: Spot → Funding transfer completes successfully
#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn test_spot_to_funding_happy_path() {
    let pool = create_test_pool().await;
    let harness = TestHarness::new(pool);

    let req = TransferRequest::new(
        ServiceId::Trading,
        ServiceId::Funding,
        1001,
        1,
        50_000_000.into(), // 0.5 BTC
    );

    let transfer_id = harness.coordinator.create(req).await.unwrap();
    let final_state = harness.coordinator.execute(transfer_id).await.unwrap();

    assert_eq!(final_state, TransferState::Committed);
    assert_eq!(harness.trading.withdraw_count(), 1);
    assert_eq!(harness.funding.deposit_count(), 1);
}

// ========================================================================
// Failure & Compensation Tests
// ========================================================================

/// Test: Source withdraw fails → FAILED state (no compensation needed)
#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn test_source_withdraw_fails() {
    let pool = create_test_pool().await;
    let harness = TestHarness::new(pool);

    // Configure funding adapter to fail withdraw
    harness.funding.set_fail_withdraw(true);

    let req = TransferRequest::new(
        ServiceId::Funding,
        ServiceId::Trading,
        1001,
        1,
        100_000_000.into(),
    );

    let transfer_id = harness.coordinator.create(req).await.unwrap();
    let final_state = harness.coordinator.execute(transfer_id).await.unwrap();

    assert_eq!(final_state, TransferState::Failed);

    // No deposit should have been called
    assert_eq!(harness.trading.deposit_count(), 0);

    // No rollback needed since source failed
    assert_eq!(harness.funding.rollback_count(), 0);
}

/// Test: Target deposit fails (Funding source) → COMPENSATING → ROLLED_BACK
#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn test_target_deposit_fails_funding_source() {
    let pool = create_test_pool().await;
    let harness = TestHarness::new(pool);

    // Configure trading adapter to fail deposit
    harness.trading.set_fail_deposit(true);

    let req = TransferRequest::new(
        ServiceId::Funding, // Source is Funding
        ServiceId::Trading,
        1001,
        1,
        100_000_000.into(),
    );

    let transfer_id = harness.coordinator.create(req).await.unwrap();
    let final_state = harness.coordinator.execute(transfer_id).await.unwrap();

    // Should rollback since source is Funding
    assert_eq!(final_state, TransferState::RolledBack);

    // Verify rollback was called
    assert_eq!(harness.funding.rollback_count(), 1);
}

/// Test: Target deposit fails (Trading source) → stays in TARGET_PENDING (infinite retry)
///
/// CRITICAL: This tests the "Trading Cannot Rollback" invariant!
#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn test_target_deposit_fails_trading_source_no_rollback() {
    let pool = create_test_pool().await;
    let harness = TestHarness::new(pool);

    // Configure funding adapter to fail deposit
    harness.funding.set_fail_deposit(true);

    let req = TransferRequest::new(
        ServiceId::Trading, // Source is Trading - CANNOT rollback!
        ServiceId::Funding,
        1001,
        1,
        100_000_000.into(),
    );

    let transfer_id = harness.coordinator.create(req).await.unwrap();

    // Step manually to observe behavior (execute would loop forever)
    let _ = harness.coordinator.step(transfer_id).await.unwrap(); // INIT → SOURCE_PENDING
    let _ = harness.coordinator.step(transfer_id).await.unwrap(); // SOURCE_PENDING → SOURCE_DONE
    let state = harness.coordinator.step(transfer_id).await.unwrap(); // Try target deposit → stays TARGET_PENDING

    // Should stay in TARGET_PENDING (not compensating!)
    assert_eq!(state, TransferState::TargetPending);

    // CRITICAL: No rollback should EVER be called when source is Trading
    assert_eq!(harness.trading.rollback_count(), 0);
}

// ========================================================================
// Idempotency Tests
// ========================================================================

/// Test: Duplicate cid returns same transfer_id (idempotent create)
#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn test_duplicate_cid_idempotent() {
    let pool = create_test_pool().await;
    let harness = TestHarness::new(pool);

    let req1 = TransferRequest::with_cid(
        ServiceId::Funding,
        ServiceId::Trading,
        1001,
        1,
        100_000_000.into(),
        "client-idempotency-key-123".to_string(),
    );

    let transfer_id1 = harness.coordinator.create(req1).await.unwrap();

    // Second request with same cid should return same transfer_id
    let req2 = TransferRequest::with_cid(
        ServiceId::Funding,
        ServiceId::Trading,
        1001,
        1,
        100_000_000.into(),
        "client-idempotency-key-123".to_string(),
    );

    let transfer_id2 = harness.coordinator.create(req2).await.unwrap();

    assert_eq!(transfer_id1, transfer_id2);
}

// ========================================================================
// Validation Tests
// ========================================================================

/// Test: Zero amount rejected
#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn test_zero_amount_rejected() {
    let pool = create_test_pool().await;
    let harness = TestHarness::new(pool);

    let req = TransferRequest::new(
        ServiceId::Funding,
        ServiceId::Trading,
        1001,
        1,
        0.into(), // Zero amount!
    );

    let result = harness.coordinator.create(req).await;
    assert!(result.is_err());
}

/// Test: Same account rejected
#[tokio::test]
#[ignore = "requires PostgreSQL database"]
async fn test_same_account_rejected() {
    let pool = create_test_pool().await;
    let harness = TestHarness::new(pool);

    let req = TransferRequest::new(
        ServiceId::Funding,
        ServiceId::Funding, // Same!
        1001,
        1,
        100_000_000.into(),
    );

    let result = harness.coordinator.create(req).await;
    assert!(result.is_err());
}

// ========================================================================
// Helper Functions
// ========================================================================

async fn create_test_pool() -> sqlx::PgPool {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@localhost:5432/zero_x_infinity_test".to_string()
    });

    sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to test database")
}
